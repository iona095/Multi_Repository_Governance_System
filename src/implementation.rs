use crate::contract::Contract;
use crate::error::Error;
use crate::git::GitRunner;
use crate::path;
use crate::rules::{self, PathRuleSet};
use crate::state::{self, ImplementationAuthority};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[cfg(debug_assertions)]
fn test_only_failpoint_enabled(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value == "1")
}

#[cfg(not(debug_assertions))]
fn test_only_failpoint_enabled(_name: &str) -> bool {
    false
}

#[cfg(debug_assertions)]
fn test_only_atomic_before_publish() -> Result<(), Error> {
    let Some(signal) = std::env::var_os("MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_SIGNAL")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    else {
        return Ok(());
    };
    let Some(release) = std::env::var_os("MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_RELEASE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    else {
        return Ok(());
    };

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(signal)
        .map_err(|_| Error::PersistenceFailed)?;
    use std::io::Write;
    file.write_all(b"reached")
        .map_err(|_| Error::PersistenceFailed)?;
    file.sync_all().map_err(|_| Error::PersistenceFailed)?;
    drop(file);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !release.exists() {
        if std::time::Instant::now() >= deadline {
            return Err(Error::PersistenceFailed);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    Ok(())
}

#[cfg(not(debug_assertions))]
fn test_only_atomic_before_publish() -> Result<(), Error> {
    Ok(())
}

fn test_only_publish_no_clobber(src: &Path, dst: &Path) -> Result<(), Error> {
    if test_only_failpoint_enabled("MRGS_TEST_ONLY_FORCE_NO_CLOBBER_UNSUPPORTED") {
        return Err(Error::PersistenceFailed);
    }
    rename_noclobber(src, dst)
}

/// Strictly accept exactly `expected` bytes, optionally followed by exactly one
/// `\n` or one `\r\n`. No other leading or trailing byte, no repeated line
/// terminator, no extra line, strict UTF-8 only.
fn expect_exact_token(output: &str, expected: &str) -> Result<(), Error> {
    if output == expected {
        return Ok(());
    }
    if output == format!("{}\n", expected) {
        return Ok(());
    }
    if output == format!("{}\r\n", expected) {
        return Ok(());
    }
    Err(Error::GitHeadInvalid)
}

/// Strictly accept exactly one line of text, optionally followed by exactly one
/// `\n` or one `\r\n`. No empty output, no repeated or mixed terminators, no
/// extra line, strict UTF-8 only.
fn expect_exact_single_line(output: &str) -> Result<String, Error> {
    if output.is_empty() {
        return Err(Error::GitHeadInvalid);
    }
    let stripped = if let Some(rest) = output.strip_suffix("\r\n") {
        if rest.contains('\n') || rest.contains('\r') {
            return Err(Error::GitHeadInvalid);
        }
        rest
    } else if let Some(rest) = output.strip_suffix('\n') {
        if rest.contains('\n') || rest.contains('\r') {
            return Err(Error::GitHeadInvalid);
        }
        rest
    } else {
        if output.contains('\n') || output.contains('\r') {
            return Err(Error::GitHeadInvalid);
        }
        output
    };
    if stripped.is_empty() || stripped.contains('\n') || stripped.contains('\r') {
        return Err(Error::GitHeadInvalid);
    }
    Ok(stripped.to_string())
}

pub struct ValidatedAuthority {
    pub repo: PathBuf,
    pub gov_dir: PathBuf,
    pub accepted_plan_sha256: String,
    pub active_phase: String,
    pub contract_id: String,
    pub final_revision: u32,
    pub final_source_path: String,
    pub final_sha256: String,
    pub final_content: String,
    pub rule_set: PathRuleSet,
    pub lifecycle: &'static str,
}

pub fn validate_phase4_authority(repo_arg: &str) -> Result<ValidatedAuthority, Error> {
    let repo_path = Path::new(repo_arg);
    path::assert_existing_dir(repo_path).map_err(|_| Error::RepositoryInvalid)?;
    let repo = std::fs::canonicalize(repo_path).map_err(|_| Error::RepositoryInvalid)?;

    let gov_dir = path::validate_gov_dir_exists(&repo)?;

    // Check all governance files are regular files (not symlinks)
    for fname in &[
        "accepted-plan.json",
        "state.json",
        "contract-draft.json",
        "accepted-contract.json",
    ] {
        let fpath = gov_dir.join(fname);
        if fpath.exists() {
            let meta =
                std::fs::symlink_metadata(&fpath).map_err(|_| Error::GovernanceAuthorityInvalid)?;
            if !meta.file_type().is_file() {
                return Err(Error::GovernanceAuthorityInvalid);
            }
        }
    }

    let accepted = state::read_accepted_plan(&repo)?;
    let gov_state = state::read_state(&repo)?;

    let plan_file = path::resolve_safe_plan_path(&repo, &accepted.plan_path)?;
    let plan_bytes = std::fs::read(&plan_file)?;
    let plan_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(&plan_bytes);
        format!("{:x}", hasher.finalize())
    };
    let plan_str = String::from_utf8(plan_bytes)?;
    let parsed_plan: crate::plan::Plan = toml::from_str(&plan_str)?;

    state::validate_accepted_plan_record(&accepted)?;
    state::validate_state_record(&gov_state, &accepted, &parsed_plan)?;
    state::validate_plan_consistency(&accepted, &parsed_plan, &plan_sha256)?;
    parsed_plan.validate()?;

    let active_phase = gov_state.active_phase.ok_or(Error::NoActivePhase)?;

    // Check for orphaned accepted ledger
    let ledger_path = gov_dir.join("accepted-contract.json");
    let draft_path = gov_dir.join("contract-draft.json");
    if ledger_path.exists() && !draft_path.exists() {
        return Err(Error::GovernanceAuthorityInvalid);
    }

    // Validate draft if present
    if draft_path.exists() {
        let draft: state::ContractDraft = serde_json::from_slice(&std::fs::read(&draft_path)?)?;
        state::validate_contract_draft_record(
            &draft,
            &accepted.sha256,
            &active_phase,
            &draft.contract_id,
        )?;
    }

    // Validate ledger if present
    if !ledger_path.exists() {
        return Err(Error::ContractNotAccepted);
    }

    let ledger: state::AcceptedContractLedger =
        serde_json::from_slice(&std::fs::read(&ledger_path)?)?;
    let draft = if draft_path.exists() {
        Some(state::read_contract_draft(&gov_dir)?)
    } else {
        None
    };
    state::validate_accepted_contract_ledger(
        &ledger,
        &accepted.sha256,
        &active_phase,
        draft.as_ref(),
    )?;

    // Determine lifecycle from validated authority
    let final_rev = ledger.revisions.last().unwrap();
    let lifecycle = match draft {
        Some(ref d)
            if d.revision == final_rev.revision
                && d.sha256 == final_rev.sha256
                && d.source_path == final_rev.source_path
                && d.content == final_rev.content =>
        {
            "ACCEPTED"
        }
        Some(_) => "REVISION_DRAFT",
        None => "ACCEPTED",
    };

    // Parse and validate the final accepted contract content
    let contract: Contract =
        toml::from_str(&final_rev.content).map_err(|_| Error::GovernanceAuthorityInvalid)?;
    contract.validate()?;
    if contract.phase_id != ledger.phase_id {
        return Err(Error::GovernanceAuthorityInvalid);
    }
    if contract.contract_id != ledger.contract_id {
        return Err(Error::GovernanceAuthorityInvalid);
    }
    // Verify content SHA
    {
        let mut hasher = Sha256::new();
        hasher.update(final_rev.content.as_bytes());
        let computed = format!("{:x}", hasher.finalize());
        if computed != final_rev.sha256 {
            return Err(Error::GovernanceAuthorityInvalid);
        }
    }

    // Validate path rules
    let rule_set = PathRuleSet::from_contract(&contract)?;

    // Validate contract_id for Phase 4 interface
    if !contract.contract_id.is_empty() {
        let first = contract.contract_id.chars().next().unwrap();
        if !first.is_ascii_alphanumeric() {
            return Err(Error::GovernanceAuthorityInvalid);
        }
        if !contract
            .contract_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
        {
            return Err(Error::GovernanceAuthorityInvalid);
        }
    }

    Ok(ValidatedAuthority {
        repo,
        gov_dir,
        accepted_plan_sha256: accepted.sha256,
        active_phase,
        contract_id: ledger.contract_id.clone(),
        final_revision: final_rev.revision,
        final_source_path: final_rev.source_path.clone(),
        final_sha256: final_rev.sha256.clone(),
        final_content: final_rev.content.clone(),
        rule_set,
        lifecycle,
    })
}

fn validate_impl_record_against_auth(
    record: &ImplementationAuthority,
    auth: &ValidatedAuthority,
) -> Result<(), Error> {
    // Contextual accepted-authority tuple comparison. A structurally valid
    // record whose accepted tuple no longer matches the current final accepted
    // ledger entry is STALE (BLOCKER 6).
    if record.accepted_plan_sha256 != auth.accepted_plan_sha256 {
        return Err(Error::ImplementationAuthorityStale);
    }
    if record.phase_id != auth.active_phase {
        return Err(Error::ImplementationAuthorityStale);
    }
    if record.contract_id != auth.contract_id {
        return Err(Error::ImplementationAuthorityStale);
    }
    if record.contract_revision != auth.final_revision {
        return Err(Error::ImplementationAuthorityStale);
    }
    if record.contract_source_path != auth.final_source_path {
        return Err(Error::ImplementationAuthorityStale);
    }
    if record.contract_sha256 != auth.final_sha256 {
        return Err(Error::ImplementationAuthorityStale);
    }
    if record.contract_content != auth.final_content {
        return Err(Error::ImplementationAuthorityStale);
    }
    // Recomputed contract-content SHA must equal the stored SHA.
    {
        let mut hasher = Sha256::new();
        hasher.update(record.contract_content.as_bytes());
        let computed = format!("{:x}", hasher.finalize());
        if computed != record.contract_sha256 {
            return Err(Error::ImplementationAuthorityStale);
        }
    }
    Ok(())
}

/// The fixed implementation-authority filename. No externally controlled value
/// may select or alter this destination.
const IMPL_AUTHORITY_FILENAME: &str = "implementation-authority.json";

/// Detect whether a path component is a Windows reparse point that is not a
/// symlink (junction or other reparse point). On non-Windows this is always
/// false because the symlink/junction distinction is not expressible.
#[cfg(windows)]
fn is_reparse_point_not_symlink(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let attrs = meta.file_attributes();
    if attrs & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
        return false;
    }
    // A true symlink also carries the reparse-point attribute; only non-symlink
    // reparse points (junctions, etc.) are rejected here. We cannot read the
    // reparse tag from Metadata alone, so we treat any reparse point that is
    // not reported as a symlink by file_type as unsafe.
    !meta.file_type().is_symlink()
}

#[cfg(not(windows))]
fn is_reparse_point_not_symlink(_meta: &std::fs::Metadata) -> bool {
    false
}

/// Reject every unsafe existing ancestor in the live changed-path topology.
/// Genuine symlinks are unsafe on every platform; Windows junctions and other
/// non-symlink reparse points are additionally rejected by the platform helper.
fn reject_unsafe_ancestor_metadata(meta: &std::fs::Metadata) -> Result<(), Error> {
    if meta.file_type().is_symlink() || is_reparse_point_not_symlink(meta) {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    Ok(())
}

/// Centralized validator for the implementation-authority file. Proves, before
/// reading, that the path is exactly the fixed filename, a direct child of the
/// validated `.mrgs` directory, a regular file, not a directory, symlink,
/// junction, or other Windows reparse point, and that its canonical path
/// remains inside the canonical `.mrgs` directory with no redirected parent
/// component. Returns `Ok(Some(path))` when the file exists and is safe,
/// `Ok(None)` when it is absent, or an error category otherwise.
pub fn validate_impl_authority_file(gov_dir: &Path) -> Result<Option<PathBuf>, Error> {
    let canonical_gov =
        std::fs::canonicalize(gov_dir).map_err(|_| Error::GovernanceAuthorityInvalid)?;
    let candidate = gov_dir.join(IMPL_AUTHORITY_FILENAME);

    // Prove the candidate is a direct child with the exact fixed filename.
    match candidate.parent() {
        Some(parent) => {
            let canonical_parent =
                std::fs::canonicalize(parent).map_err(|_| Error::GovernanceAuthorityInvalid)?;
            if canonical_parent != canonical_gov {
                return Err(Error::ImplementationAuthorityInvalid);
            }
        }
        None => return Err(Error::ImplementationAuthorityInvalid),
    }

    // Use symlink_metadata so a symlink is not followed.
    let meta = match std::fs::symlink_metadata(&candidate) {
        Ok(m) => m,
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::ImplementationAuthorityInvalid),
    };

    // A governance authority file that is a directory, symlink, junction,
    // reparse point, or other non-regular object is invalid (contract §4/§5).
    let ft = meta.file_type();
    if ft.is_dir() {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if ft.is_symlink() {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if is_reparse_point_not_symlink(&meta) {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if !ft.is_file() {
        return Err(Error::ImplementationAuthorityInvalid);
    }

    // Canonical containment after the no-redirection proof above.
    let canonical =
        std::fs::canonicalize(&candidate).map_err(|_| Error::GovernanceAuthorityInvalid)?;
    if !canonical.starts_with(&canonical_gov) {
        return Err(Error::ImplementationAuthorityInvalid);
    }

    Ok(Some(candidate))
}

/// Centralized structural validation of an implementation-authority record.
/// Validates strict JSON (via serde deny_unknown_fields on the type), schema
/// version, hex fields, grammar, object format, baseline length, branch
/// grammar, and recomputed contract-content SHA. Contextual comparison against
/// the accepted authority is performed separately.
pub fn validate_impl_record_structure(
    record: &ImplementationAuthority,
    _objfmt: &str,
) -> Result<(), Error> {
    if record.schema_version != 1 {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if !is_lowercase_hex(&record.accepted_plan_sha256, 64) {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if record.phase_id.is_empty() {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if record.contract_id.is_empty() {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if record.contract_revision == 0 {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if !is_valid_source_path(&record.contract_source_path) {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if !is_lowercase_hex(&record.contract_sha256, 64) {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    // Recompute contract-content SHA and require exact equality.
    {
        let mut hasher = Sha256::new();
        hasher.update(record.contract_content.as_bytes());
        let computed = format!("{:x}", hasher.finalize());
        if computed != record.contract_sha256 {
            return Err(Error::ImplementationAuthorityInvalid);
        }
    }
    // Embedded contract content must parse under the Phase 4 contract parser.
    let contract: Contract = toml::from_str(&record.contract_content)
        .map_err(|_| Error::ImplementationAuthorityInvalid)?;
    contract
        .validate()
        .map_err(|_| Error::ImplementationAuthorityInvalid)?;
    if contract.phase_id != record.phase_id {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if contract.contract_id != record.contract_id {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if record.git_object_format != "sha1" && record.git_object_format != "sha256" {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    let expected_baseline_len = if record.git_object_format == "sha256" {
        64
    } else {
        40
    };
    if !is_lowercase_hex(&record.baseline_head, expected_baseline_len) {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    if record.baseline_branch.is_empty()
        || record.baseline_branch.contains('\r')
        || record.baseline_branch.contains('\n')
    {
        return Err(Error::ImplementationAuthorityInvalid);
    }
    Ok(())
}

fn is_lowercase_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Strict Phase 3 source-path grammar: non-empty, repository-relative, `/`
/// separators only, no backslash, no empty/`.`/`..` segment, no leading `./`,
/// no doubled slash, no control characters, no glob metacharacters.
fn is_valid_source_path(p: &str) -> bool {
    if p.is_empty() {
        return false;
    }
    if p.starts_with('/') || p.starts_with("//") {
        return false;
    }
    if p.contains('\\') {
        return false;
    }
    if p.starts_with("./") || p.contains("//") {
        return false;
    }
    if p.chars()
        .any(|c| c as u32 == 0 || (c as u32 > 0 && (c as u32) < 32) || c as u32 == 127)
    {
        return false;
    }
    if p.contains('*') || p.contains('?') || p.contains('[') || p.contains(']') {
        return false;
    }
    for seg in p.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return false;
        }
    }
    true
}

/// Resolve the current repository object format via the isolated Git child.
pub fn git_object_format_of(git: &GitRunner) -> Result<String, Error> {
    let out = git.run(["rev-parse", "--show-object-format"])?;
    if !out.status.success() {
        return Err(Error::GitCommandFailed(
            "rev-parse --show-object-format failed".into(),
        ));
    }
    let s = std::str::from_utf8(&out.stdout)
        .map_err(|_| Error::GitCommandFailed("non-UTF-8 object-format output".into()))?;
    let fmt = strict_single_line(s)
        .map_err(|_| Error::GitCommandFailed("malformed object-format output".into()))?;
    if fmt != "sha1" && fmt != "sha256" {
        return Err(Error::GitCommandFailed("unsupported object format".into()));
    }
    Ok(fmt)
}

pub fn validate_git_root(git: &GitRunner) -> Result<(String, String, String), Error> {
    let wt_out = git.run_stdout_string(["rev-parse", "--is-inside-work-tree"])?;
    expect_exact_token(&wt_out, "true").map_err(|_| Error::GitRootMismatch)?;

    let toplevel_raw = git.run_stdout_string(["rev-parse", "--show-toplevel"])?;
    let toplevel = expect_exact_single_line(&toplevel_raw).map_err(|_| Error::GitRootMismatch)?;
    let canonical_toplevel =
        std::fs::canonicalize(&toplevel).map_err(|_| Error::GitRootMismatch)?;
    if canonical_toplevel != git.repo_path() {
        return Err(Error::GitRootMismatch);
    }

    let branch_out = git.run(["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    if !branch_out.status.success() {
        // Detached HEAD: symbolic-ref cannot resolve a branch name.
        return Err(Error::GitDetachedHead);
    }
    let branch_raw = std::str::from_utf8(&branch_out.stdout).map_err(|_| Error::GitHeadInvalid)?;
    let branch = expect_exact_single_line(branch_raw).map_err(|_| Error::GitDetachedHead)?;
    if branch.is_empty() {
        return Err(Error::GitDetachedHead);
    }

    let objfmt_raw = git.run_stdout_string(["rev-parse", "--show-object-format"])?;
    let objfmt = expect_exact_single_line(&objfmt_raw).map_err(|_| Error::GitHeadInvalid)?;
    if objfmt != "sha1" && objfmt != "sha256" {
        return Err(Error::GitHeadInvalid);
    }

    let head_raw = git.run_stdout_string(["rev-parse", "--verify", "HEAD^{commit}"])?;
    let head = expect_exact_single_line(&head_raw).map_err(|_| Error::GitHeadInvalid)?;
    let expected_len = if objfmt == "sha1" { 40 } else { 64 };
    if head.len() != expected_len
        || !head
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(Error::GitHeadInvalid);
    }

    Ok((head, branch, objfmt))
}

pub fn validate_operation_state(git: &GitRunner) -> Result<(), Error> {
    let markers = [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "BISECT_LOG",
        "BISECT_START",
        "rebase-apply",
        "rebase-merge",
        "sequencer",
    ];
    for marker in &markers {
        let result = git.run(["rev-parse", "--git-path", marker]);
        match result {
            Ok(out) => {
                if !out.status.success() {
                    return Err(Error::GitCommandFailed(
                        "rev-parse --git-path failed".into(),
                    ));
                }
                let path_str = std::str::from_utf8(&out.stdout)
                    .map_err(|_| Error::GitCommandFailed("non-UTF-8 git-path output".into()))?;
                // Exactly one trailing line terminator is permitted; reject
                // repeated, mixed, or embedded terminators and empty output.
                let path_str = if let Some(rest) = path_str.strip_suffix("\r\n") {
                    if rest.contains('\n') || rest.contains('\r') || rest.is_empty() {
                        return Err(Error::GitCommandFailed("malformed git-path output".into()));
                    }
                    rest
                } else if let Some(rest) = path_str.strip_suffix('\n') {
                    if rest.contains('\n') || rest.contains('\r') || rest.is_empty() {
                        return Err(Error::GitCommandFailed("malformed git-path output".into()));
                    }
                    rest
                } else if path_str.contains('\n') || path_str.contains('\r') || path_str.is_empty()
                {
                    return Err(Error::GitCommandFailed("malformed git-path output".into()));
                } else {
                    path_str
                };
                if git.repo_path().join(path_str).exists() {
                    return Err(Error::GitOperationInProgress);
                }
            }
            Err(e) => {
                return Err(Error::GitCommandFailed(format!(
                    "rev-parse --git-path failed: {}",
                    e
                )))
            }
        }
    }
    Ok(())
}

/// Strictly parse the complete `--sparse --stage -z` index and return the set
/// of repository-relative paths whose first segment is exactly `.mrgs`. The
/// caller uses this set to gate the begin-time governance-path exemption on the
/// Section 6.4 proof that no tracked index entry exists for each fixed
/// governance path (contract §6.5). All stage/mode/classification errors are
/// surfaced before any path is collected.
pub fn tracked_governance_paths(git: &GitRunner, objfmt: &str) -> Result<Vec<String>, Error> {
    let out = git.run(["ls-files", "--sparse", "--stage", "-z"])?;
    // Any execution failure (spawn/runner failure, signal, non-zero exit) is
    // GIT_COMMAND_FAILED; only exit-zero malformed records are
    // GIT_INVENTORY_INVALID (BLOCKER 8).
    if !out.status.success() {
        return Err(Error::GitCommandFailed(
            "ls-files --sparse --stage failed".into(),
        ));
    }

    let mut tracked: Vec<String> = Vec::new();
    let stdout = out.stdout;
    let mut i = 0;
    while i < stdout.len() {
        if stdout[i..].is_empty() {
            break;
        }
        let nul = stdout[i..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::GitInventoryInvalid)?;
        let record = &stdout[i..i + nul];
        i += nul + 1;

        let parsed = parse_index_stage_record(record, objfmt)?;
        if is_governance_path(&parsed.path) {
            tracked.push(parsed.path);
        }
    }
    Ok(tracked)
}

pub fn validate_index_structure(git: &GitRunner, objfmt: &str) -> Result<(), Error> {
    // Classification errors (conflict, gitlink, sparse-directory, malformed
    // mode/object/path) are invariant whether or not the tracked-governance set
    // is retained, so run the full structural pass first.
    let out = git.run(["ls-files", "--sparse", "--stage", "-z"])?;
    if !out.status.success() {
        return Err(Error::GitCommandFailed(
            "ls-files --sparse --stage failed".into(),
        ));
    }

    let stdout = out.stdout;
    let mut i = 0;
    while i < stdout.len() {
        if stdout[i..].is_empty() {
            break;
        }
        let nul = stdout[i..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::GitInventoryInvalid)?;
        let record = &stdout[i..i + nul];
        i += nul + 1;

        parse_index_record(record, objfmt)?;
    }
    Ok(())
}

fn parse_index_record(record: &[u8], objfmt: &str) -> Result<(), Error> {
    parse_index_stage_record(record, objfmt).map(|_| ())
}

fn parse_index_stage_record(record: &[u8], objfmt: &str) -> Result<IndexStageRecord, Error> {
    let record_str = std::str::from_utf8(record).map_err(|_| Error::GitInventoryInvalid)?;
    let mode_end = record_str.find(' ').ok_or(Error::GitInventoryInvalid)?;
    let mode_str = &record_str[..mode_end];
    if mode_str.len() != 6
        || !mode_str
            .chars()
            .all(|c| c.is_ascii_digit() && ('0'..='7').contains(&c))
    {
        return Err(Error::GitInventoryInvalid);
    }

    let rest = &record_str[mode_end + 1..];
    let oid_end = rest.find(' ').ok_or(Error::GitInventoryInvalid)?;
    let oid_str = &rest[..oid_end];
    let expected_oid_len = match objfmt {
        "sha1" => 40,
        "sha256" => 64,
        _ => return Err(Error::GitInventoryInvalid),
    };
    if oid_str.len() != expected_oid_len
        || !oid_str
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(Error::GitInventoryInvalid);
    }

    let rest2 = &rest[oid_end + 1..];
    let tab_pos = rest2.find('\t').ok_or(Error::GitInventoryInvalid)?;
    if rest2[tab_pos + 1..].contains('\t') {
        return Err(Error::GitInventoryInvalid);
    }
    let stage_str = &rest2[..tab_pos];
    let path = &rest2[tab_pos + 1..];
    let stage = match stage_str {
        "0" => 0,
        "1" | "2" | "3" => return Err(Error::GitConflict),
        _ => return Err(Error::GitInventoryInvalid),
    };
    let mode = u32::from_str_radix(mode_str, 8).map_err(|_| Error::GitInventoryInvalid)?;

    match mode {
        0o40000 => {
            if !path.ends_with('/') || path.ends_with("//") {
                return Err(Error::GitInventoryInvalid);
            }
            let inner = &path[..path.len() - 1];
            validate_index_path(inner)?;
            // A structurally valid sparse-directory record is itself
            // prohibited evidence, including when it is a child record.
            return Err(Error::GitInventoryInvalid);
        }
        0o160000 => {
            validate_index_path(path)?;
            return Err(Error::GitSubmoduleUnsupported);
        }
        0o100644 | 0o100755 | 0o120000 => {
            validate_index_path(path)?;
        }
        _ => return Err(Error::GitInventoryInvalid),
    }

    if is_governance_path(path) {
        return Err(Error::GitInventoryInvalid);
    }
    Ok(IndexStageRecord {
        mode: mode_str.to_string(),
        oid: oid_str.to_string(),
        stage,
        path: path.to_string(),
    })
}

/// Complete index-path grammar (BLOCKER 9): strict UTF-8 (already enforced by
/// the caller), non-empty, repository-relative, no leading `/`, no drive
/// prefix, no UNC prefix, no device prefix, no backslash, no empty component,
/// no doubled `/`, no `.` component, no `..` component, no ASCII control
/// character, no ordinary trailing `/`.
fn validate_index_path(path: &str) -> Result<(), Error> {
    if path.is_empty() {
        return Err(Error::GitInventoryInvalid);
    }
    if path.starts_with('/') || path.starts_with("//") {
        return Err(Error::GitInventoryInvalid);
    }
    if path.len() >= 2 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':' {
        return Err(Error::GitInventoryInvalid);
    }
    if path.starts_with("\\\\") {
        return Err(Error::GitInventoryInvalid);
    }
    if path.contains('\\') {
        return Err(Error::GitInventoryInvalid);
    }
    if path.contains("//") {
        return Err(Error::GitInventoryInvalid);
    }
    if path.ends_with('/') {
        return Err(Error::GitInventoryInvalid);
    }
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(Error::GitInventoryInvalid);
        }
    }
    if path
        .chars()
        .any(|c| c as u32 == 0 || (c as u32 > 0 && (c as u32) < 32) || c as u32 == 127)
    {
        return Err(Error::GitInventoryInvalid);
    }
    Ok(())
}

pub fn validate_index_flags(git: &GitRunner) -> Result<(), Error> {
    let out = git.run(["ls-files", "-v", "-z"])?;
    if !out.status.success() {
        return Err(Error::GitCommandFailed("ls-files -v failed".into()));
    }
    let stdout = out.stdout;
    let mut i = 0;
    while i < stdout.len() {
        if stdout[i..].is_empty() {
            break;
        }
        let nul = stdout[i..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::GitInventoryInvalid)?;
        let record = &stdout[i..i + nul];
        i += nul + 1;

        if record.len() < 2 {
            return Err(Error::GitInventoryInvalid);
        }
        let flag = record[0] as char;
        if record[1] as char != ' ' {
            return Err(Error::GitInventoryInvalid);
        }
        if flag.is_ascii_lowercase() {
            return Err(Error::GitInventoryInvalid);
        }
        if flag == 'S' {
            return Err(Error::GitInventoryInvalid);
        }
        if flag != 'H' {
            return Err(Error::GitInventoryInvalid);
        }
    }
    Ok(())
}

pub fn validate_sparse_config(git: &GitRunner) -> Result<(), Error> {
    // core.sparseCheckout --get
    let sc_get = git.run(["config", "--type=bool", "--get", "core.sparseCheckout"])?;
    let sc_get_val = parse_sparse_bool(&sc_get, "core.sparseCheckout")?;

    // core.sparseCheckout --get-all
    let sc_get_all = git.run(["config", "--type=bool", "--get-all", "core.sparseCheckout"])?;
    check_sparse_getall(&sc_get_all, sc_get_val, "core.sparseCheckout")?;

    // index.sparse --get
    let is_get = git.run(["config", "--type=bool", "--get", "index.sparse"])?;
    let is_get_val = parse_sparse_bool(&is_get, "index.sparse")?;

    // index.sparse --get-all
    let is_get_all = git.run(["config", "--type=bool", "--get-all", "index.sparse"])?;
    check_sparse_getall(&is_get_all, is_get_val, "index.sparse")?;

    // Effective active sparse configuration rejects the repository.
    if sc_get_val == Some(true) || is_get_val == Some(true) {
        return Err(Error::GitInventoryInvalid);
    }

    Ok(())
}

/// Strictly parse one `--get` boolean result from raw bytes. The only accepted
/// successful outputs are exactly `true`, `true\n`, `true\r\n`, `false`,
/// `false\n`, `false\r\n`. Any other successful output (empty, blank line,
/// repeated terminator, multiple lines, leading/trailing whitespace, mixed
/// terminators, bare CR, malformed UTF-8, non-boolean token, or unexpected
/// stderr) is rejected. Unset means exit 1 with empty stdout and stderr.
fn parse_sparse_bool(output: &crate::git::GitOutput, key: &str) -> Result<Option<bool>, Error> {
    if !output.status.success() {
        if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
            return Ok(None);
        }
        return Err(Error::GitCommandFailed(format!("{} config failed", key)));
    }
    if !output.stderr.is_empty() {
        return Err(Error::GitInventoryInvalid);
    }
    let value = parse_exact_bool_token(&output.stdout)?;
    Ok(Some(value))
}

/// Parse exactly one boolean token from raw bytes: `true`, `true\n`,
/// `true\r\n`, `false`, `false\n`, `false\r\n`. Rejects everything else.
fn parse_exact_bool_token(bytes: &[u8]) -> Result<bool, Error> {
    // Reject any embedded NUL or control byte other than a single trailing
    // line terminator.
    let token = match bytes {
        b"true" => true,
        b"true\n" => true,
        b"true\r\n" => true,
        b"false" => false,
        b"false\n" => false,
        b"false\r\n" => false,
        _ => {
            // Strictly reject: empty, blank line, repeated terminator, multiple
            // lines, duplicate values, leading/trailing whitespace, mixed
            // terminators, bare CR, malformed UTF-8, non-boolean token.
            return Err(Error::GitInventoryInvalid);
        }
    };
    Ok(token)
}

/// Strictly parse `--get-all` boolean results. The only accepted successful
/// output is exactly one boolean token (see `parse_exact_bool_token`). The
/// normalized token bytes must exactly equal the `--get` result.
fn check_sparse_getall(
    output: &crate::git::GitOutput,
    get_result: Option<bool>,
    key: &str,
) -> Result<(), Error> {
    let all_value = parse_sparse_bool(output, key)?;
    match (get_result, all_value) {
        (Some(g), Some(v)) if g == v => Ok(()),
        (None, None) => Ok(()),
        (None, Some(_)) => Err(Error::GitInventoryInvalid),
        (Some(_), None) => Err(Error::GitInventoryInvalid),
        _ => Err(Error::GitInventoryInvalid),
    }
}

/// Strictly accept exactly one line of text, optionally followed by one line
/// terminator. Rejects repeated, mixed, or embedded terminators.
fn strict_single_line(s: &str) -> Result<String, Error> {
    if s.is_empty() {
        return Err(Error::GitInventoryInvalid);
    }
    let stripped = if let Some(rest) = s.strip_suffix("\r\n") {
        if rest.contains('\n') || rest.contains('\r') {
            return Err(Error::GitInventoryInvalid);
        }
        rest
    } else if let Some(rest) = s.strip_suffix('\n') {
        if rest.contains('\n') || rest.contains('\r') {
            return Err(Error::GitInventoryInvalid);
        }
        rest
    } else if s.contains('\n') || s.contains('\r') {
        return Err(Error::GitInventoryInvalid);
    } else {
        s
    };
    if stripped.is_empty() || stripped.contains('\n') || stripped.contains('\r') {
        return Err(Error::GitInventoryInvalid);
    }
    Ok(stripped.to_string())
}

fn parse_revision_token(token: &str) -> Result<u32, Error> {
    if token.is_empty() {
        return Err(Error::InvalidArgument);
    }
    if token.len() > 1 && token.starts_with('0') {
        return Err(Error::InvalidArgument);
    }
    if token.starts_with('+') || token.starts_with('-') {
        return Err(Error::InvalidArgument);
    }
    if !token.chars().all(|c| c.is_ascii_digit()) {
        return Err(Error::InvalidArgument);
    }
    let val: u32 = token.parse().map_err(|_| Error::InvalidArgument)?;
    if val < 1 {
        return Err(Error::InvalidArgument);
    }
    Ok(val)
}

fn parse_sha256_token(token: &str) -> Result<(), Error> {
    if token.len() != 64 {
        return Err(Error::InvalidArgument);
    }
    if !token
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}

pub fn cmd_implementation_begin(
    repo_arg: &str,
    revision_token: &str,
    sha256_arg: &str,
) -> Result<String, Error> {
    let revision = parse_revision_token(revision_token)?;
    parse_sha256_token(sha256_arg)?;

    let auth = validate_phase4_authority(repo_arg)?;

    if auth.lifecycle == "DRAFT" {
        return Err(Error::ContractNotAccepted);
    }

    // Validate revision against final accepted
    if revision != auth.final_revision {
        return Err(Error::RequestedRevisionStale);
    }
    if sha256_arg != auth.final_sha256 {
        return Err(Error::RequestedShaStale);
    }

    // Validate ID grammar for Phase 4
    if !auth.contract_id.is_empty() {
        let first = auth.contract_id.chars().next().unwrap();
        if !first.is_ascii_alphanumeric() {
            return Err(Error::GovernanceAuthorityInvalid);
        }
    }

    // Setup git
    let git = GitRunner::new(&auth.repo);

    // Validate git root

    let (current_head, current_branch, objfmt) = validate_git_root(&git)?;

    // Validate operation state
    validate_operation_state(&git)?;

    // Validate index structure
    validate_index_structure(&git, &objfmt)?;

    // Validate index flags
    validate_index_flags(&git)?;

    // Validate sparse config
    validate_sparse_config(&git)?;

    // Check existing implementation record
    let impl_path = match validate_impl_authority_file(&auth.gov_dir)? {
        Some(p) => p,
        None => {
            // No existing record: proceed to first publication below.
            return begin_first_publication(
                &auth,
                &git,
                revision,
                sha256_arg,
                &current_head,
                &current_branch,
            );
        }
    };
    handle_existing_record(
        &auth,
        &git,
        &impl_path,
        revision,
        sha256_arg,
        &current_head,
        &current_branch,
    )
}

fn begin_first_publication(
    auth: &ValidatedAuthority,
    git: &GitRunner,
    _revision: u32,
    _sha256_arg: &str,
    current_head: &str,
    current_branch: &str,
) -> Result<String, Error> {
    // Cleanliness check for begin

    let tracked_gov = tracked_governance_paths(git, &git_object_format_of(git)?)?;
    validate_begin_cleanliness(git, auth, &tracked_gov)?;

    // Capture HEAD and branch
    let baseline_head = current_head.to_string();
    let baseline_branch = current_branch.to_string();
    let objfmt = git_object_format_of(git)?;

    // Build the implementation record
    let record = ImplementationAuthority {
        schema_version: 1,
        accepted_plan_sha256: auth.accepted_plan_sha256.clone(),
        phase_id: auth.active_phase.clone(),
        contract_id: auth.contract_id.clone(),
        contract_revision: auth.final_revision,
        contract_source_path: auth.final_source_path.clone(),
        contract_sha256: auth.final_sha256.clone(),
        contract_content: auth.final_content.clone(),
        git_object_format: objfmt,
        baseline_head: baseline_head.clone(),
        baseline_branch: baseline_branch.clone(),
    };

    // Atomic no-clobber publication
    atomic_first_publish(&auth.gov_dir, &record)?;

    Ok(format!(
        "IMPLEMENTATION_BOUND {} {} {} {}",
        auth.contract_id, auth.final_revision, auth.final_sha256, baseline_head
    ))
}

fn handle_existing_record(
    auth: &ValidatedAuthority,
    git: &GitRunner,
    impl_path: &Path,
    _revision: u32,
    _sha256_arg: &str,
    current_head: &str,
    current_branch: &str,
) -> Result<String, Error> {
    // Parse and validate existing record
    let existing: ImplementationAuthority = serde_json::from_slice(
        &std::fs::read(impl_path).map_err(|_| Error::ImplementationAuthorityInvalid)?,
    )
    .map_err(|_| Error::ImplementationAuthorityInvalid)?;

    // Centralized structural validation first (unknown/missing fields, schema
    // version, hex grammar, object format, baseline length, branch grammar,
    // recomputed contract-content SHA, embedded contract parse).
    validate_impl_record_structure(&existing, &git_object_format_of(git)?)?;

    // Contextual comparison against the accepted authority tuple.
    validate_impl_record_against_auth(&existing, auth)?;

    // Validate cleanliness
    let tracked_gov = tracked_governance_paths(git, &git_object_format_of(git)?)?;
    validate_begin_cleanliness(git, auth, &tracked_gov)?;

    // Now validate current branch/HEAD vs record
    if existing.baseline_branch != current_branch {
        return Err(Error::ImplementationAuthorityConflict);
    }
    if existing.baseline_head != current_head {
        return Err(Error::ImplementationAuthorityConflict);
    }

    // Idempotent success: reconstruct the deterministic expected record and
    // compare every field; perform no write, preserve existing bytes exactly.
    let expected = ImplementationAuthority {
        schema_version: 1,
        accepted_plan_sha256: auth.accepted_plan_sha256.clone(),
        phase_id: auth.active_phase.clone(),
        contract_id: auth.contract_id.clone(),
        contract_revision: auth.final_revision,
        contract_source_path: auth.final_source_path.clone(),
        contract_sha256: auth.final_sha256.clone(),
        contract_content: auth.final_content.clone(),
        git_object_format: git_object_format_of(git)?,
        baseline_head: current_head.to_string(),
        baseline_branch: current_branch.to_string(),
    };
    if !records_identical(&existing, &expected) {
        return Err(Error::ImplementationAuthorityConflict);
    }

    // Record is valid for idempotent success
    // No write - preserve existing record byte-for-byte
    Ok(format!(
        "IMPLEMENTATION_BOUND {} {} {} {}",
        auth.contract_id, auth.final_revision, auth.final_sha256, existing.baseline_head
    ))
}

/// Exact field-by-field comparison of two implementation-authority records.
fn records_identical(a: &ImplementationAuthority, b: &ImplementationAuthority) -> bool {
    a.schema_version == b.schema_version
        && a.accepted_plan_sha256 == b.accepted_plan_sha256
        && a.phase_id == b.phase_id
        && a.contract_id == b.contract_id
        && a.contract_revision == b.contract_revision
        && a.contract_source_path == b.contract_source_path
        && a.contract_sha256 == b.contract_sha256
        && a.contract_content == b.contract_content
        && a.git_object_format == b.git_object_format
        && a.baseline_head == b.baseline_head
        && a.baseline_branch == b.baseline_branch
}

/// `tracked_governance` is the exact set of repository-relative paths whose
/// first segment is `.mrgs` as proved by Section 6.4 index inspection. The
/// fixed-governance-path exemption for `??` and ignored-untracked output is
/// applied only when the path is NOT in this set, proving no tracked index
/// entry exists for it (contract §6.5).
fn validate_begin_cleanliness(
    git: &GitRunner,
    auth: &ValidatedAuthority,
    tracked_governance: &[String],
) -> Result<(), Error> {
    let out = git.run([
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--ignore-submodules=none",
        "--renames",
    ])?;
    if !out.status.success() {
        return Err(Error::GitCommandFailed("status failed".into()));
    }
    for record in parse_porcelain_output(&out.stdout)? {
        let xy_0 = record.xy.as_bytes()[0] as char;
        if xy_0 == '?' && record.xy == "??" {
            if !is_exempt_governance_path(&record.path, auth, tracked_governance) {
                return Err(Error::GitDirty);
            }
            continue;
        }

        if is_governance_path(&record.path) {
            return Err(Error::GitInventoryInvalid);
        }
        if let Some(source) = record.source.as_deref() {
            if is_governance_path(source) {
                return Err(Error::GitInventoryInvalid);
            }
        }

        return Err(Error::GitDirty);
    }

    // Check ignored-untracked files
    let ignored_out = git.run([
        "ls-files",
        "--others",
        "--ignored",
        "--exclude-standard",
        "-z",
        "--",
    ])?;
    if !ignored_out.status.success() {
        return Err(Error::GitCommandFailed("ls-files ignored failed".into()));
    }
    for path in parse_ignored_output(&ignored_out.stdout)? {
        if !is_exempt_governance_path(&path, auth, tracked_governance) {
            return Err(Error::GitDirty);
        }
    }

    Ok(())
}

fn is_exempt_governance_path(
    path: &str,
    _auth: &ValidatedAuthority,
    tracked_governance: &[String],
) -> bool {
    let gov_paths = [
        ".mrgs/accepted-plan.json",
        ".mrgs/state.json",
        ".mrgs/contract-draft.json",
        ".mrgs/accepted-contract.json",
        ".mrgs/implementation-authority.json",
    ];
    // Only exempt one of the exact fixed paths AND only when Section 6.4
    // proved that no tracked index entry exists for it (contract §6.5).
    gov_paths.contains(&path) && !tracked_governance.iter().any(|p| p == path)
}

fn is_governance_path(path: &str) -> bool {
    path.split('/')
        .next()
        .is_some_and(|segment| segment.eq_ignore_ascii_case(".mrgs"))
}

fn atomic_first_publish(gov_dir: &Path, record: &ImplementationAuthority) -> Result<(), Error> {
    let dst = gov_dir.join("implementation-authority.json");

    // Serialize fully before opening any file.
    let json = serde_json::to_string_pretty(record).map_err(|_| Error::PersistenceFailed)?;
    let json_bytes = json.as_bytes();

    // Create a unique same-directory temporary file with create-new semantics.
    // Retry on name collision.
    let mut tmp_path = None;
    for attempt in 0..16u64 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!(
            ".mrgs_impl_tmp_{}_{}_{}.tmp",
            std::process::id(),
            attempt,
            nanos
        );
        let candidate = gov_dir.join(&name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                use std::io::Write;
                if file.write_all(json_bytes).is_err() {
                    let _ = std::fs::remove_file(&candidate);
                    return Err(Error::PersistenceFailed);
                }
                if file.sync_all().is_err() {
                    let _ = std::fs::remove_file(&candidate);
                    return Err(Error::PersistenceFailed);
                }
                drop(file);
                tmp_path = Some(candidate);
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                continue;
            }
            Err(_) => {
                return Err(Error::PersistenceFailed);
            }
        }
    }

    let tmp_path = tmp_path.ok_or(Error::PersistenceFailed)?;

    if let Err(error) = test_only_atomic_before_publish() {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(error);
    }

    // Atomic no-clobber rename.
    match test_only_publish_no_clobber(&tmp_path, &dst) {
        Ok(()) => Ok(()),
        Err(Error::ImplementationAuthorityConflict) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(Error::ImplementationAuthorityConflict)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

#[cfg(windows)]
fn rename_noclobber(src: &Path, dst: &Path) -> Result<(), Error> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(
            lpExistingFileName: *const u16,
            lpNewFileName: *const u16,
            dwFlags: u32,
        ) -> i32;
    }

    let src_wide: Vec<u16> = OsStr::new(src)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let dst_wide: Vec<u16> = OsStr::new(dst)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let result = unsafe { MoveFileExW(src_wide.as_ptr(), dst_wide.as_ptr(), 0) };
    if result == 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(Error::ImplementationAuthorityConflict);
        }
        return Err(Error::PersistenceFailed);
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn rename_noclobber(src: &Path, dst: &Path) -> Result<(), Error> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    extern "C" {
        fn link(src: *const i8, dst: *const i8) -> i32;
        fn unlink(path: *const i8) -> i32;
    }

    let src_bytes = src.as_os_str().as_bytes();
    let dst_bytes = dst.as_os_str().as_bytes();
    let src_c = CString::new(src_bytes).map_err(|_| Error::PersistenceFailed)?;
    let dst_c = CString::new(dst_bytes).map_err(|_| Error::PersistenceFailed)?;
    let ret = unsafe { link(src_c.as_ptr(), dst_c.as_ptr()) };
    if ret == 0 {
        unsafe { unlink(src_c.as_ptr()) };
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.kind() == std::io::ErrorKind::AlreadyExists {
        return Err(Error::ImplementationAuthorityConflict);
    }
    Err(Error::PersistenceFailed)
}

#[cfg(target_os = "macos")]
fn rename_noclobber(src: &Path, dst: &Path) -> Result<(), Error> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    extern "C" {
        fn link(src: *const i8, dst: *const i8) -> i32;
        fn unlink(path: *const i8) -> i32;
    }

    let src_bytes = src.as_os_str().as_bytes();
    let dst_bytes = dst.as_os_str().as_bytes();
    let src_c = CString::new(src_bytes).map_err(|_| Error::PersistenceFailed)?;
    let dst_c = CString::new(dst_bytes).map_err(|_| Error::PersistenceFailed)?;
    let ret = unsafe { link(src_c.as_ptr(), dst_c.as_ptr()) };
    if ret == 0 {
        unsafe { unlink(src_c.as_ptr()) };
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.kind() == std::io::ErrorKind::AlreadyExists {
        return Err(Error::ImplementationAuthorityConflict);
    }
    Err(Error::PersistenceFailed)
}

/// Classify the result of a leaf `symlink_metadata` call for Section 12.2
/// topology inspection. Pure and deterministic: `Ok` yields `Some(metadata)`,
/// `NotFound` yields `None` (absent), and every other error yields
/// `FilesystemBoundaryUnsafe`. Permission-denied, invalid topology, I/O, and
/// race errors are never reinterpreted as absence (Blocker 1).
fn classify_metadata_result(
    result: std::io::Result<std::fs::Metadata>,
) -> Result<Option<std::fs::Metadata>, Error> {
    match result {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::FilesystemBoundaryUnsafe),
    }
}

/// Classify a changed live leaf for Section 12.2 topology inspection.
fn classify_live_leaf_metadata(full: &Path) -> Result<Option<std::fs::Metadata>, Error> {
    classify_metadata_result(std::fs::symlink_metadata(full))
}

pub fn cmd_implementation_check(repo_arg: &str) -> Result<String, Error> {
    let auth = validate_phase4_authority(repo_arg)?;

    let git = GitRunner::new(&auth.repo);

    // Validate git root

    let (_current_head, _current_branch, _objfmt) = validate_git_root(&git)?;

    // Validate operation state
    validate_operation_state(&git)?;

    // Validate index structure
    validate_index_structure(&git, &_objfmt)?;

    // Validate index flags
    validate_index_flags(&git)?;

    // Validate sparse config
    validate_sparse_config(&git)?;

    // Require implementation authority exists and is a safe regular file.
    let impl_path = match validate_impl_authority_file(&auth.gov_dir)? {
        Some(p) => p,
        None => return Err(Error::ImplementationAuthorityMissing),
    };

    let record: ImplementationAuthority = serde_json::from_slice(
        &std::fs::read(&impl_path).map_err(|_| Error::ImplementationAuthorityInvalid)?,
    )
    .map_err(|_| Error::ImplementationAuthorityInvalid)?;

    // Centralized structural validation first.
    let objfmt = git_object_format_of(&git)?;
    validate_impl_record_structure(&record, &objfmt)?;

    // Contextual comparison against the accepted authority tuple.
    validate_impl_record_against_auth(&record, &auth)?;

    if record.git_object_format != objfmt {
        return Err(Error::ImplementationAuthorityStale);
    }

    // Validate baseline branch
    if !_current_branch.is_empty() {
        let branch = _current_branch;
        if branch != record.baseline_branch {
            return Err(Error::BaselineBranchChanged);
        }
    } else {
        return Err(Error::GitDetachedHead);
    }

    // Promisor marker check must run before resolving baseline_head.
    // Section 6.1.1 / 11.1: --get and --get-all must agree; unset means no
    // marker; one non-empty consistent line means promisor-enabled; any
    // empty, malformed, multi-line, multi-valued, disagreeing, or failed
    // evidence is fatal under the ordinary configuration-evidence rules.
    let promisor_get = git.run(["config", "--get", "extensions.partialClone"])?;
    let promisor_getall = git.run(["config", "--get-all", "extensions.partialClone"])?;

    // Parse and retain the exact normalized token bytes. Unset means no
    // promisor marker; one non-empty strict UTF-8 token (with optional single
    // LF or CRLF) means promisor-enabled. Different valid non-empty tokens
    // between --get and --get-all must reject (BLOCKER 12).
    fn parse_promisor_value(out: &crate::git::GitOutput) -> Result<Option<String>, Error> {
        if !out.status.success() {
            if out.status.code() == Some(1) && out.stdout.is_empty() && out.stderr.is_empty() {
                return Ok(None);
            }
            return Err(Error::GitCommandFailed("config command failed".into()));
        }
        if !out.stderr.is_empty() {
            return Err(Error::GitInventoryInvalid);
        }
        let s = std::str::from_utf8(&out.stdout).map_err(|_| Error::GitInventoryInvalid)?;
        // Successful output must be exactly one non-empty strict UTF-8 token
        // with optional single LF or CRLF only; no leading/trailing
        // whitespace, no duplicate values, no second line, no repeated
        // terminator, no unexpected stderr.
        let val = if let Some(rest) = s.strip_suffix("\r\n") {
            if rest.is_empty() || rest.contains('\n') || rest.contains('\r') {
                return Err(Error::GitInventoryInvalid);
            }
            rest
        } else if let Some(rest) = s.strip_suffix('\n') {
            if rest.is_empty() || rest.contains('\n') || rest.contains('\r') {
                return Err(Error::GitInventoryInvalid);
            }
            rest
        } else {
            if s.is_empty() || s.contains('\n') || s.contains('\r') {
                return Err(Error::GitInventoryInvalid);
            }
            s
        };
        if val.is_empty() {
            return Err(Error::GitInventoryInvalid);
        }
        Ok(Some(val.to_string()))
    }

    let promisor_get_val = parse_promisor_value(&promisor_get)?;
    let promisor_getall_val = parse_promisor_value(&promisor_getall)?;
    // --get and --get-all normalized token bytes must be exactly equal.
    let is_promisor = match (&promisor_get_val, &promisor_getall_val) {
        (None, None) => false,
        (Some(g), Some(a)) if g == a => true,
        _ => return Err(Error::GitInventoryInvalid),
    };

    // Validate baseline head exists
    let baselines_out = git.run([
        "rev-parse",
        "--verify",
        &format!("{}^{{commit}}", record.baseline_head),
    ])?;
    if !baselines_out.status.success() {
        if is_promisor {
            // A promised commit that cannot be resolved locally under
            // no-lazy-fetch must not be retried or fetched.
            return Err(Error::GitCommandFailed(
                "baseline commit unavailable in promisor repository".into(),
            ));
        }
        return Err(Error::BaselineCommitMissing);
    }

    // Verify baseline is ancestor. Distinguish exit codes exactly (BLOCKER 7):
    // exit 0 -> ancestor (success); exit 1 -> valid semantic negative
    // (BASELINE_HISTORY_CHANGED); any other exit, signal, or spawn failure ->
    // GIT_COMMAND_FAILED. Non-zero results are never all mapped to
    // BASELINE_HISTORY_CHANGED.
    let ancestor = git.run(["merge-base", "--is-ancestor", &record.baseline_head, "HEAD"])?;
    match ancestor.status.code() {
        Some(0) => {}
        Some(1) => return Err(Error::BaselineHistoryChanged),
        _ => {
            return Err(Error::GitCommandFailed(
                "merge-base --is-ancestor failed".into(),
            ))
        }
    }

    // Conflict precheck. A failed command (spawn/signal/non-zero exit) maps to
    // GIT_COMMAND_FAILED; a successfully executed but malformed result maps to
    // GIT_INVENTORY_INVALID; actual conflict entries map to GIT_CONFLICT. A
    // failed command with empty stdout must never be treated as "no conflicts"
    // (BLOCKER 8).
    let unmerged = git.run(["ls-files", "--unmerged", "-z"])?;
    if !unmerged.status.success() {
        return Err(Error::GitCommandFailed("ls-files --unmerged failed".into()));
    }
    if !unmerged.stdout.is_empty() {
        // Strictly parse; malformed output is GIT_INVENTORY_INVALID rather than
        // a silent conflict. A valid unmerged record rejects the check.
        let mut i = 0usize;
        let stdout = &unmerged.stdout;
        let mut found_conflict = false;
        while i < stdout.len() {
            if stdout[i..].is_empty() {
                break;
            }
            let nul = stdout[i..]
                .iter()
                .position(|&b| b == 0)
                .ok_or(Error::GitInventoryInvalid)?;
            let record = &stdout[i..i + nul];
            i += nul + 1;
            // Parse as `<mode> SP <oid> SP <stage> TAB <path>`; reject malformed.
            let entry_str = std::str::from_utf8(record).map_err(|_| Error::GitInventoryInvalid)?;
            let tab = entry_str.find('\t').ok_or(Error::GitInventoryInvalid)?;
            let meta = &entry_str[..tab];
            let mut parts = meta.splitn(3, ' ');
            let _m = parts.next().ok_or(Error::GitInventoryInvalid)?;
            let _o = parts.next().ok_or(Error::GitInventoryInvalid)?;
            let stage = parts.next().ok_or(Error::GitInventoryInvalid)?;
            if stage != "1" && stage != "2" && stage != "3" {
                return Err(Error::GitInventoryInvalid);
            }
            found_conflict = true;
        }
        if found_conflict {
            return Err(Error::GitConflict);
        }
    }

    // Build change inventory
    let tracked_gov = tracked_governance_paths(&git, &objfmt)?;
    let (inventory, raw_entries) =
        build_change_inventory(&git, &record, &auth, &_objfmt, &tracked_gov)?;

    // Validate paths with symlink and filesystem checks
    let mut validated_count = 0u32;
    for path in &inventory {
        rules::validate_changed_path(path)?;
        if test_only_failpoint_enabled("MRGS_TEST_ONLY_FORCE_CHANGE_PATH_INVALID") {
            return Err(Error::ChangePathInvalid);
        }

        // Section 12.3: inspect HEAD and index symlink layers for this path
        // before live-layer inspection. The raw-diff entries carry the exact
        // new-side OID used for HEAD symlink cat-file (BLOCKER 2).
        inspect_symlink_git_layers(
            &git,
            &auth.repo,
            path,
            &raw_entries,
            &auth.rule_set,
            &_objfmt,
        )?;

        // Section 12.2: live path topology. Classify the live leaf exactly.
        let full = auth.repo.join(path);
        let meta = match classify_live_leaf_metadata(&full)? {
            Some(m) => m,
            None => {
                // Absent/deleted live leaf: perform no live-layer inspection.
                // An absent changed path is still validated by the git layers
                // above and the rule set below.
                auth.rule_set.evaluate(path)?;
                validated_count += 1;
                continue;
            }
        };

        // Blockers 2/3: inspect every ancestor component. Any metadata error,
        // including NotFound, is fatal because an existing leaf cannot have a
        // missing ancestor under consistent evidence. Reject Unix symlink
        // ancestors and any Windows reparse-point ancestor.
        let mut prefix = String::new();
        let components: Vec<&str> = path.split('/').collect();
        for (idx, component) in components.iter().enumerate() {
            if idx > 0 {
                prefix.push('/');
            }
            prefix.push_str(component);
            let comp_full = auth.repo.join(&prefix);
            // Skip the leaf; it is handled below, not an ancestor.
            if comp_full == full {
                continue;
            }
            let cm = std::fs::symlink_metadata(&comp_full)
                .map_err(|_| Error::FilesystemBoundaryUnsafe)?;
            reject_unsafe_ancestor_metadata(&cm)?;
        }

        // Blockers 3: a non-symlink Windows reparse-point leaf (junction, etc.)
        // is rejected before ordinary or allowed-symlink handling. A genuine
        // symlink leaf is preserved for its required target proof below.
        if is_reparse_point_not_symlink(&meta) {
            return Err(Error::FilesystemBoundaryUnsafe);
        }

        if meta.file_type().is_symlink() {
            // 1. Read the link without following it; failure is fatal.
            let target_bytes =
                std::fs::read_link(&full).map_err(|_| Error::FilesystemBoundaryUnsafe)?;
            // 2. Require strict UTF-8 and a non-empty target.
            let target_str = target_bytes
                .to_str()
                .ok_or(Error::FilesystemBoundaryUnsafe)?;
            if target_str.is_empty() {
                return Err(Error::FilesystemBoundaryUnsafe);
            }
            // 3. Lexical target validation (contract §12.1 rules 1-4).
            validate_symlink_target(&auth.repo, path, target_str)?;
            // Relative to the symlink's repository-relative parent.
            let parent = match path.rfind('/') {
                Some(pos) => &path[..pos],
                None => "",
            };
            // 4. Lexically resolve the target relative to the parent and
            // reject any lexical escape.
            let resolved = resolve_lexical_symlink_target(parent, target_str)?;
            // 5. Separate lexical-target rule evaluation (contract §12.1 rule 8
            // / §12.3): the lexical resolved path is matched against the rule
            // set independently of the canonical proof below.
            evaluate_symlink_target(&auth.rule_set, &resolved)?;

            // 6. Live-layer chain proof (contract §12.1 rule 5): inspect every
            // existing target prefix and the target leaf via symlink_metadata.
            // Metadata inspection errors are fatal; a missing component marks
            // the target broken and stops inspection (broken-target lexical
            // rules already applied above). Any symlink, junction, or
            // non-symlink reparse point in the live-layer chain is rejected.
            let link_parent = auth.repo.join(parent);
            prove_live_target_no_chain(&link_parent, target_str)?;

            // 7-8. If the resolved live target exists, require a canonical
            // proof. Canonicalization is performed only after the no-chain
            // proof; the canonical target must remain inside the canonical
            // repository and convert without loss to a normalized
            // repository-relative `/` path, which is then matched
            // independently against the rule set (contract §12.1 rules 6-8).
            let target_path = link_parent.join(target_str);
            match std::fs::symlink_metadata(&target_path) {
                Ok(_) => {
                    let canonical = std::fs::canonicalize(&target_path)
                        .map_err(|_| Error::FilesystemBoundaryUnsafe)?;
                    let repo_relative = canonical_target_to_repo_relative(&auth.repo, &canonical)?;
                    auth.rule_set
                        .evaluate(&repo_relative)
                        .map_err(|e| match e {
                            Error::ChangeForbidden | Error::ChangeNotAllowed => {
                                Error::FilesystemBoundaryUnsafe
                            }
                            _ => e,
                        })?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Broken target: lexical rules already applied. No
                    // canonical proof is possible (contract §12.2).
                }
                Err(_) => return Err(Error::FilesystemBoundaryUnsafe),
            }
        }

        auth.rule_set.evaluate(path)?;
        validated_count += 1;
    }

    Ok(format!(
        "IMPLEMENTATION_OK {} {} {} {}",
        auth.contract_id, auth.final_revision, auth.final_sha256, validated_count
    ))
}

/// Section 12: resolve a symlink target for one layer and validate it.
/// `raw_target` is the reported target bytes (already strict UTF-8). The layer
/// is identified only for diagnostics; all layers apply identical lexical rules.
/// Rejects empty targets, absolute paths, UNC paths, device-prefixed paths,
/// Windows-drive-prefixed paths, backslashes, control characters, malformed
/// components, escapes, and symlink-chain continuation (the caller ensures each
/// layer's ancestors are free of symlinks before calling).
fn validate_symlink_target(_repo: &Path, path: &str, raw_target: &str) -> Result<(), Error> {
    if raw_target.is_empty() {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    // Absolute paths (BLOCKER 3).
    if raw_target.starts_with('/') || raw_target.starts_with("//") {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    // Windows drive-prefixed paths (BLOCKER 3).
    if raw_target.len() >= 2
        && raw_target.as_bytes()[0].is_ascii_alphabetic()
        && raw_target.as_bytes()[1] == b':'
    {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    // UNC or device-prefixed paths such as `\\server\share` or `\\.\` (BLOCKER 3).
    if raw_target.starts_with("\\\\") {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    // Backslashes are forbidden where path separators are required (BLOCKER 3).
    if raw_target.contains('\\') {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    // Control characters and DEL are forbidden (BLOCKER 3).
    if raw_target
        .chars()
        .any(|c| c as u32 == 0 || (c as u32 > 0 && (c as u32) < 32) || c as u32 == 127)
    {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    let parent = match path.rfind('/') {
        Some(pos) => &path[..pos],
        None => "",
    };
    let mut resolved_components: Vec<&str> = if parent.is_empty() {
        Vec::new()
    } else {
        parent.split('/').collect()
    };
    for seg in raw_target.split('/') {
        match seg {
            // Empty segment (doubled slash) is a malformed component (BLOCKER 3).
            "" => return Err(Error::FilesystemBoundaryUnsafe),
            "." => {}
            ".." => {
                if resolved_components.pop().is_none() {
                    return Err(Error::FilesystemBoundaryUnsafe);
                }
            }
            other => resolved_components.push(other),
        }
    }
    let resolved = resolved_components.join("/");
    if resolved.starts_with("..") || resolved.starts_with('/') {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    Ok(())
}

/// Evaluate a resolved symlink target against the path-rule set. A resolved
/// target that matches a forbidden rule, matches no allowed rule, or otherwise
/// fails the contract scope check must produce `FILESYSTEM_BOUNDARY_UNSAFE`,
/// not the ordinary changed-path categories `CHANGE_FORBIDDEN` or
/// `CHANGE_NOT_ALLOWED` (BLOCKER 1).
fn evaluate_symlink_target(ruleset: &PathRuleSet, target: &str) -> Result<(), Error> {
    ruleset.evaluate(target).map_err(|e| match e {
        Error::ChangeForbidden | Error::ChangeNotAllowed => Error::FilesystemBoundaryUnsafe,
        _ => e,
    })
}

/// Lexically resolve a symlink target relative to the symlink's parent
/// repository-relative path, rejecting any escape (contract §12.1 rules 3-4).
/// Pure: no filesystem access. The caller must have already validated the raw
/// target lexically via `validate_symlink_target`.
fn resolve_lexical_symlink_target(parent: &str, raw_target: &str) -> Result<String, Error> {
    let mut components: Vec<&str> = if parent.is_empty() {
        Vec::new()
    } else {
        parent.split('/').collect()
    };
    for seg in raw_target.split('/') {
        match seg {
            "" => return Err(Error::FilesystemBoundaryUnsafe),
            "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(Error::FilesystemBoundaryUnsafe);
                }
            }
            other => components.push(other),
        }
    }
    let resolved = components.join("/");
    if resolved.starts_with("..") || resolved.starts_with('/') {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    Ok(resolved)
}

/// Inspect the live-layer chain of a resolved symlink target. Every component
/// prefix that exists is inspected with `symlink_metadata` (errors are fatal),
/// and any symlink, junction, or non-symlink reparse point rejects the change.
/// A component that does not exist marks the target broken and stops
/// inspection (broken-target lexical rules are applied separately by the
/// caller). Used for an existing live symlink target; never for HEAD or index
/// targets (those are proven via git object inspection).
fn prove_live_target_no_chain(link_parent: &Path, target_str: &str) -> Result<(), Error> {
    let comps: Vec<&str> = target_str.split('/').collect();
    let mut prefix = String::new();
    for comp in comps {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(comp);
        let comp_full = link_parent.join(&prefix);
        match std::fs::symlink_metadata(&comp_full) {
            Ok(meta) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err(Error::FilesystemBoundaryUnsafe);
                    }
                }
                #[cfg(not(windows))]
                {
                    if meta.file_type().is_symlink() {
                        return Err(Error::FilesystemBoundaryUnsafe);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Broken target: stop inspecting the chain.
                break;
            }
            Err(_) => return Err(Error::FilesystemBoundaryUnsafe),
        }
    }
    Ok(())
}

/// Convert a canonical absolute target path to a normalized repository-relative
/// `/`-separated path, requiring the canonical target to remain inside the
/// canonical repository (contract §12.1 rule 7). Any containment or conversion
/// failure returns `FILESYSTEM_BOUNDARY_UNSAFE`.
fn canonical_target_to_repo_relative(
    repo_canonical: &Path,
    target_canonical: &Path,
) -> Result<String, Error> {
    let repo_str = repo_canonical
        .to_str()
        .ok_or(Error::FilesystemBoundaryUnsafe)?
        .replace('\\', "/");
    let target_str = target_canonical
        .to_str()
        .ok_or(Error::FilesystemBoundaryUnsafe)?
        .replace('\\', "/");
    if !target_str.starts_with(&repo_str) {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    let suffix = &target_str[repo_str.len()..];
    let rel = if let Some(stripped) = suffix.strip_prefix('/') {
        stripped
    } else if suffix.is_empty() {
        return Err(Error::FilesystemBoundaryUnsafe);
    } else {
        // `repo_str` was not a directory boundary; reject.
        return Err(Error::FilesystemBoundaryUnsafe);
    };
    if rel.is_empty() {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    let normalized: String = rel.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains("//") {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    Ok(normalized)
}

/// Section 12.3: inspect HEAD and index symlink layers for a changed path.
/// For each layer the reported symlink target is validated lexically against
/// repository containment. HEAD targets come from `ls-tree -z HEAD`; index
/// targets come from `ls-files --stage -z` filtered to stage 0.
/// Inspect the HEAD symlink layer for a changed path. The exact raw-diff
/// new-side OID drives `git cat-file blob` (BLOCKER 2); `ls-tree` is used only
/// to prove same-layer HEAD topology of the target prefix and leaf (BLOCKER 3).
fn inspect_head_symlink_layer(
    git: &GitRunner,
    repo: &Path,
    path: &str,
    new_oid: &str,
    ruleset: &PathRuleSet,
    objfmt: &str,
) -> Result<(), Error> {
    ruleset.evaluate(path)?;
    // Use the exact raw-diff new-side OID; never an OID from ls-tree.
    let target = git.run(["cat-file", "blob", new_oid])?;
    if !target.status.success() {
        return Err(Error::GitCommandFailed(
            "head symlink blob unavailable".into(),
        ));
    }
    // Preserve blob bytes exactly; require strict UTF-8; do not trim/normalize.
    let target_str =
        std::str::from_utf8(&target.stdout).map_err(|_| Error::FilesystemBoundaryUnsafe)?;
    validate_symlink_target(repo, path, target_str)?;

    // Resolve the target relative to the link's repository-relative parent and
    // prove every target prefix and leaf is free of a same-layer symlink chain.
    let resolved = lexical_resolve(path, target_str);
    evaluate_symlink_target(ruleset, &resolved)?;
    inspect_head_topology(git, &resolved, objfmt)?;
    Ok(())
}

/// Strictly parse `git ls-tree -z HEAD -- <path>` as zero or one exact record
/// and reject any same-layer symlink (mode 120000) in the resolved target
/// prefix or leaf (BLOCKER 3).
fn inspect_head_topology(git: &GitRunner, target_path: &str, objfmt: &str) -> Result<(), Error> {
    // Inspect every existing target prefix and the target leaf (BLOCKER 4): for
    // a target `a/b/c`, inspect `a`, `a/b`, and `a/b/c` each as a single exact
    // record proving no same-layer symlink chain.
    for prefix in path_prefixes(target_path) {
        let out = git.run(["ls-tree", "-z", "HEAD", "--", &prefix])?;
        if !out.status.success() {
            return Err(Error::GitCommandFailed("ls-tree HEAD failed".into()));
        }
        if let Some(rec) = parse_ls_tree_z(&out.stdout, &prefix, objfmt)? {
            if rec.mode == "120000" {
                // Same-layer symlink chain in the HEAD layer.
                return Err(Error::FilesystemBoundaryUnsafe);
            }
        }
    }
    Ok(())
}

/// Yield each path prefix and the full path, in increasing order. For
/// `a/b/c` this yields `a`, `a/b`, `a/b/c`. A single-segment path yields only
/// itself.
fn path_prefixes(target: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut acc = String::new();
    let segs: Vec<&str> = target.split('/').collect();
    for (idx, seg) in segs.iter().enumerate() {
        if idx > 0 {
            acc.push('/');
        }
        acc.push_str(seg);
        out.push(acc.clone());
    }
    out
}

/// Strictly parse `git ls-files --sparse --stage -z -- <path>` for each
/// resolved index target prefix and the target leaf. For a target `a/b/c`,
/// perform separate exact lookups for `a`, `a/b`, and `a/b/c`. Reject any
/// same-layer symlink (mode 120000) in the index layer (BLOCKER 4 / BLOCKER 5).
///
/// A directory prefix lookup naturally returns the child entries under that
/// directory, none of which have the requested path. Those entries are parsed
/// for structural validity but otherwise ignored; only a record whose path
/// exactly equals the requested prefix is considered evidence for that prefix.
fn inspect_index_topology(git: &GitRunner, target_path: &str, objfmt: &str) -> Result<(), Error> {
    let prefixes = path_prefixes(target_path);
    let last_idx = prefixes.len().saturating_sub(1);
    for (idx, prefix) in prefixes.iter().enumerate() {
        let out = git.run(["ls-files", "--sparse", "--stage", "-z", "--", prefix])?;
        if !out.status.success() {
            return Err(Error::GitCommandFailed(
                "ls-files stage topology failed".into(),
            ));
        }
        let rec = if idx == last_idx {
            // The leaf lookup must match exactly; reject wrong paths and
            // multiple records strictly.
            parse_index_stage_z(&out.stdout, prefix, objfmt)?
        } else {
            // Directory prefixes have no index entry; Git returns the child
            // entries under them. Parse them for validity, but only a record
            // whose path equals the requested prefix is evidence.
            parse_index_topology_z(&out.stdout, prefix, objfmt)?
        };
        if let Some(rec) = rec {
            if rec.mode == "120000" {
                // Same-layer symlink chain in the index layer.
                return Err(Error::FilesystemBoundaryUnsafe);
            }
        }
    }
    Ok(())
}

/// Parse a `git ls-files --sparse --stage -z -- <path>` output for topology
/// inspection. Child records whose path differs from the requested prefix are
/// parsed for structural validity but ignored. Only a record whose path equals
/// the requested prefix is evidence. Zero matching records is safe (directory or
/// absent); more than one matching record is malformed.
fn parse_index_topology_z(
    stdout: &[u8],
    expected_path: &str,
    objfmt: &str,
) -> Result<Option<IndexStageRecord>, Error> {
    let mut found: Option<IndexStageRecord> = None;
    let mut start = 0usize;
    while start < stdout.len() {
        let relative_end = stdout[start..]
            .iter()
            .position(|&byte| byte == 0)
            .ok_or(Error::GitInventoryInvalid)?;
        let end = start + relative_end;
        let record = &stdout[start..end];
        start = end + 1;

        let parsed = parse_index_stage_record(record, objfmt)?;
        if parsed.path == expected_path {
            if found.is_some() {
                return Err(Error::GitInventoryInvalid);
            }
            found = Some(parsed);
        }
    }
    Ok(found)
}

/// Strictly parse one `git ls-tree -z HEAD -- <path>` output into zero or one
/// exact record. Format: `<mode> SP <type> SP <object-id> TAB <path> NUL`.
fn parse_ls_tree_z(
    stdout: &[u8],
    expected_path: &str,
    objfmt: &str,
) -> Result<Option<LsTreeRecord>, Error> {
    if stdout.is_empty() {
        return Ok(None);
    }
    let nul = stdout
        .iter()
        .position(|&b| b == 0)
        .ok_or(Error::GitInventoryInvalid)?;
    let entry = &stdout[..nul];
    if nul + 1 != stdout.len() {
        return Err(Error::GitInventoryInvalid);
    }
    let tab = entry
        .iter()
        .position(|&b| b == b'\t')
        .ok_or(Error::GitInventoryInvalid)?;
    let meta_part = &entry[..tab];
    let file_path = &entry[tab + 1..];
    let path_str = std::str::from_utf8(file_path).map_err(|_| Error::GitInventoryInvalid)?;
    if path_str != expected_path {
        return Err(Error::GitInventoryInvalid);
    }
    let mut it = meta_part.split(|&b| b == b' ');
    let mode = it.next().ok_or(Error::GitInventoryInvalid)?;
    let type_tok = it.next().ok_or(Error::GitInventoryInvalid)?;
    let oid = it.next().ok_or(Error::GitInventoryInvalid)?;
    if it.next().is_some() {
        return Err(Error::GitInventoryInvalid);
    }
    let mode_str = std::str::from_utf8(mode).map_err(|_| Error::GitInventoryInvalid)?;
    let type_str = std::str::from_utf8(type_tok).map_err(|_| Error::GitInventoryInvalid)?;
    let oid_str = std::str::from_utf8(oid).map_err(|_| Error::GitInventoryInvalid)?;
    // Exact mode grammar: six octal digits.
    if mode_str.len() != 6
        || !mode_str
            .chars()
            .all(|c| c.is_ascii_digit() && ('0'..='7').contains(&c))
    {
        return Err(Error::GitInventoryInvalid);
    }
    let valid_relationship = matches!(
        (mode_str, type_str),
        ("040000", "tree")
            | ("100644", "blob")
            | ("100755", "blob")
            | ("120000", "blob")
            | ("160000", "commit")
    );
    if !valid_relationship {
        return Err(Error::GitInventoryInvalid);
    }
    // Lowercase OID of expected length.
    let expected_len = if objfmt == "sha1" {
        40
    } else if objfmt == "sha256" {
        64
    } else {
        return Err(Error::GitInventoryInvalid);
    };
    if oid_str.len() != expected_len
        || !oid_str
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(Error::GitInventoryInvalid);
    }
    Ok(Some(LsTreeRecord {
        mode: mode_str.to_string(),
        oid: oid_str.to_string(),
    }))
}

#[allow(dead_code)]
struct LsTreeRecord {
    mode: String,
    oid: String,
}

/// Inspect the index symlink layer for a changed path. Uses the exact index
/// OID from `ls-files --sparse --stage -z` for `cat-file blob` and proves
/// same-layer index topology of the target prefix/leaf (BLOCKER 4).
fn inspect_index_symlink_layer(
    git: &GitRunner,
    repo: &Path,
    path: &str,
    ruleset: &PathRuleSet,
    objfmt: &str,
) -> Result<(), Error> {
    ruleset.evaluate(path)?;
    let out = git.run(["ls-files", "--sparse", "--stage", "-z", "--", path])?;
    if !out.status.success() {
        return Err(Error::GitCommandFailed("ls-files stage failed".into()));
    }
    let rec = match parse_index_stage_z(&out.stdout, path, objfmt)? {
        Some(r) => r,
        None => return Ok(()),
    };
    if rec.mode == "120000" {
        let target = git.run(["cat-file", "blob", &rec.oid])?;
        if !target.status.success() {
            return Err(Error::GitCommandFailed(
                "index symlink blob unavailable".into(),
            ));
        }
        let target_str =
            std::str::from_utf8(&target.stdout).map_err(|_| Error::FilesystemBoundaryUnsafe)?;
        validate_symlink_target(repo, path, target_str)?;
        let resolved = lexical_resolve(path, target_str);
        evaluate_symlink_target(ruleset, &resolved)?;
        inspect_index_topology(git, &resolved, objfmt)?;
    }
    Ok(())
}

/// Strictly parse `git ls-files --sparse --stage -z -- <path>` into zero or one
/// exact record. Format: `<mode> SP <oid> SP <stage> TAB <path> NUL`.
fn parse_index_stage_z(
    stdout: &[u8],
    expected_path: &str,
    objfmt: &str,
) -> Result<Option<IndexStageRecord>, Error> {
    if stdout.is_empty() {
        return Ok(None);
    }
    let nul = stdout
        .iter()
        .position(|&b| b == 0)
        .ok_or(Error::GitInventoryInvalid)?;
    let entry = &stdout[..nul];
    if nul + 1 != stdout.len() {
        return Err(Error::GitInventoryInvalid);
    }
    let parsed = parse_index_stage_record(entry, objfmt)?;
    if parsed.path != expected_path {
        return Err(Error::GitInventoryInvalid);
    }
    Ok(Some(parsed))
}

struct IndexStageRecord {
    mode: String,
    oid: String,
    #[allow(dead_code)]
    stage: u8,
    path: String,
}

/// Lexically resolve a symlink target relative to the link's repository-
/// relative parent, returning the normalized repository-relative target path
/// used for same-layer topology inspection.
fn lexical_resolve(link_path: &str, target: &str) -> String {
    let parent = match link_path.rfind('/') {
        Some(pos) => &link_path[..pos],
        None => "",
    };
    let mut comps: Vec<&str> = if parent.is_empty() {
        Vec::new()
    } else {
        parent.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                comps.pop();
            }
            other => comps.push(other),
        }
    }
    comps.join("/")
}

/// Inspect both the HEAD and index symlink layers for a changed path. The HEAD
/// layer uses the exact raw-diff new-side OID (BLOCKER 2); the index layer uses
/// the exact index OID (BLOCKER 4). Both reject same-layer symlink chains.
fn inspect_symlink_git_layers(
    git: &GitRunner,
    repo: &Path,
    path: &str,
    raw_diff_entries: &[RawDiffEntry],
    ruleset: &PathRuleSet,
    objfmt: &str,
) -> Result<(), Error> {
    // HEAD layer: find the raw-diff entry whose destination is this path with a
    // new-side symlink mode (status not D).
    for entry in raw_diff_entries {
        if entry.dst == path && entry.status != 'D' && entry.new_mode == "120000" {
            inspect_head_symlink_layer(git, repo, path, &entry.new_oid, ruleset, objfmt)?;
        }
    }

    // Index layer: inspect the exact index OID for this path.
    inspect_index_symlink_layer(git, repo, path, ruleset, objfmt)?;

    Ok(())
}

/// Validate a raw diff mode field: exactly 6 octal digits.
/// A structured raw-diff entry retained for symlink inspection. The contract
/// (BLOCKER 2) requires retaining old mode, new mode, old OID, new OID, status,
/// score, source path, and destination path as structured fields.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct RawDiffEntry {
    old_mode: String,
    new_mode: String,
    old_oid: String,
    new_oid: String,
    status: char,
    score: Option<u32>,
    /// Destination path (for R/C: the destination; otherwise the single path).
    dst: String,
    /// Source path (only for R/C).
    src: Option<String>,
}

/// Strictly parse the baseline-to-HEAD raw diff into structured entries,
/// retaining old mode, new mode, old OID, new OID, status, score, and paths.
fn parse_raw_diff_entries(
    git: &GitRunner,
    baseline_head: &str,
    objfmt: &str,
) -> Result<Vec<RawDiffEntry>, Error> {
    let diff_out = git.run([
        "diff",
        "--no-ext-diff",
        "--raw",
        "-z",
        "--no-abbrev",
        "--find-renames=50%",
        "--find-copies=50%",
        "--find-copies-harder",
        baseline_head,
        "HEAD",
        "--",
    ])?;
    if !diff_out.status.success() {
        return Err(Error::GitCommandFailed("diff inventory failed".into()));
    }
    parse_raw_diff_output(&diff_out.stdout, objfmt)
}

fn parse_raw_diff_output(stdout: &[u8], objfmt: &str) -> Result<Vec<RawDiffEntry>, Error> {
    let mut entries = Vec::new();

    let mut i = 0;
    while i < stdout.len() {
        if stdout[i..].is_empty() {
            break;
        }
        let nul = stdout[i..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::GitInventoryInvalid)?;
        let record_bytes = &stdout[i..i + nul];
        i += nul + 1;

        if !record_bytes.starts_with(b":") {
            return Err(Error::GitInventoryInvalid);
        }
        let after_colon = &record_bytes[1..];
        let parts: Vec<&[u8]> = after_colon.splitn(5, |&b| b == b' ').collect();
        if parts.len() != 5 {
            return Err(Error::GitInventoryInvalid);
        }
        // Strict raw-mode classification (BLOCKER 2). Present-side modes must
        // be exactly one of 000000, 100644, 100755, 120000, or 160000; any
        // other mode is unsupported and rejected.
        let old_mode = classify_raw_mode(parts[0])?;
        let new_mode = classify_raw_mode(parts[1])?;
        validate_raw_oid(parts[2], objfmt)?;
        validate_raw_oid(parts[3], objfmt)?;
        let status_part = parts[4];
        if status_part.is_empty() {
            return Err(Error::GitInventoryInvalid);
        }
        let status_byte = status_part[0] as char;
        let score_bytes = &status_part[1..];

        let score = match status_byte {
            'C' | 'R' => {
                let score_str =
                    std::str::from_utf8(score_bytes).map_err(|_| Error::GitInventoryInvalid)?;
                Some(validate_raw_change_score(score_str)?)
            }
            'A' | 'D' | 'M' | 'T' => {
                if !score_bytes.is_empty() {
                    return Err(Error::GitInventoryInvalid);
                }
                None
            }
            _ => return Err(Error::GitInventoryInvalid),
        };

        let old_mode = old_mode.to_string();
        let new_mode = new_mode.clone();
        let old_oid = std::str::from_utf8(parts[2]).map_err(|_| Error::GitInventoryInvalid)?;
        let new_oid = std::str::from_utf8(parts[3]).map_err(|_| Error::GitInventoryInvalid)?;
        let zero_oid = "0000000000000000000000000000000000000000";
        let zero_oid_64 = "0000000000000000000000000000000000000000000000000000000000000000";
        let expected_zero = if objfmt == "sha256" {
            zero_oid_64
        } else {
            zero_oid
        };
        // A present-side 160000 (gitlink) new mode is separately categorized as
        // an unsupported submodule (BLOCKER 2), not an ordinary invalid mode.
        if new_mode == "160000" {
            return Err(Error::GitSubmoduleUnsupported);
        }
        match status_byte {
            'A' => {
                if old_mode != "000000" || old_oid != expected_zero {
                    return Err(Error::GitInventoryInvalid);
                }
                if new_mode == "000000" || new_oid == expected_zero {
                    return Err(Error::GitInventoryInvalid);
                }
            }
            'D' => {
                if new_mode != "000000" || new_oid != expected_zero {
                    return Err(Error::GitInventoryInvalid);
                }
                if old_mode == "000000" || old_oid == expected_zero {
                    return Err(Error::GitInventoryInvalid);
                }
            }
            _ => {
                if old_mode == "000000" || old_oid == expected_zero {
                    return Err(Error::GitInventoryInvalid);
                }
                if new_mode == "000000" || new_oid == expected_zero {
                    return Err(Error::GitInventoryInvalid);
                }
            }
        }

        if i >= stdout.len() {
            return Err(Error::GitInventoryInvalid);
        }
        let nul2 = stdout[i..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::GitInventoryInvalid)?;
        // For R/C the strict `--raw -z` ordering is <source>NUL<destination>NUL
        // (BLOCKER 1): the first path is the source, the second is the
        // destination. The earlier code incorrectly read the first path as the
        // destination.
        let src_path = std::str::from_utf8(&stdout[i..i + nul2])
            .map_err(|_| Error::GitInventoryInvalid)?
            .to_string();
        i += nul2 + 1;
        validate_inventory_path(&src_path)?;

        let dst_path = match status_byte {
            'R' | 'C' => {
                if i >= stdout.len() {
                    return Err(Error::GitInventoryInvalid);
                }
                let nul3 = stdout[i..]
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or(Error::GitInventoryInvalid)?;
                let d = std::str::from_utf8(&stdout[i..i + nul3])
                    .map_err(|_| Error::GitInventoryInvalid)?
                    .to_string();
                i += nul3 + 1;
                validate_inventory_path(&d)?;
                d
            }
            _ => src_path.clone(),
        };

        // Raw-diff paths are inventory evidence, so reserved first segments
        // must be rejected before they enter the ordinary change set. This
        // also covers rename/copy sources and destinations.
        if is_governance_path(&src_path) || is_governance_path(&dst_path) {
            return Err(Error::GitInventoryInvalid);
        }

        entries.push(RawDiffEntry {
            old_mode: old_mode.clone(),
            new_mode: new_mode.clone(),
            old_oid: old_oid.to_string(),
            new_oid: new_oid.to_string(),
            status: status_byte,
            score,
            // Destination path: for R/C this is the second path; otherwise the
            // single path. Used for new-side HEAD symlink inspection.
            dst: dst_path,
            // Source path: only for R/C, the first path.
            src: if status_byte == 'R' || status_byte == 'C' {
                Some(src_path)
            } else {
                None
            },
        });
    }
    Ok(entries)
}

fn validate_raw_change_score(score: &str) -> Result<u32, Error> {
    if score.is_empty() || !score.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::GitInventoryInvalid);
    }
    let value: u32 = score.parse().map_err(|_| Error::GitInventoryInvalid)?;
    if value > 100 {
        return Err(Error::GitInventoryInvalid);
    }
    Ok(value)
}

/// Strictly classify a raw-diff mode field (BLOCKER 2). The accepted modes are
/// exactly `000000` (absent side), `100644` (ordinary file), `100755`
/// (executable ordinary file), `120000` (symlink), and `160000`
/// (gitlink/submodule). Any other mode is unsupported and rejected.
fn classify_raw_mode(mode: &[u8]) -> Result<String, Error> {
    if mode.len() != 6 || !mode.iter().all(|&b| b.is_ascii_digit() && (b - b'0') <= 7) {
        return Err(Error::GitInventoryInvalid);
    }
    let s = std::str::from_utf8(mode).map_err(|_| Error::GitInventoryInvalid)?;
    match s {
        "000000" | "100644" | "100755" | "120000" | "160000" => Ok(s.to_string()),
        _ => Err(Error::GitInventoryInvalid),
    }
}

/// Validate a raw diff OID field: lowercase hex of expected length, or all zeros.
fn validate_raw_oid(oid: &[u8], objfmt: &str) -> Result<(), Error> {
    let expected_len = if objfmt == "sha256" { 64 } else { 40 };
    if oid.len() != expected_len {
        return Err(Error::GitInventoryInvalid);
    }
    for &b in oid {
        if !b.is_ascii_hexdigit() || (b.is_ascii_uppercase()) {
            return Err(Error::GitInventoryInvalid);
        }
    }
    Ok(())
}

/// Strict porcelain `-z` `XY` grammar (BLOCKER 6 / §11.5). Returns `Ok` only
/// for the exactly documented accepted codes; `Err(GitConflict)` for any
/// unmerged combination; `Err(GitInventoryInvalid)` for any unsupported or
/// malformed status byte. The exact supported set is normative and the blank
/// inside each code is significant.
fn classify_porcelain_xy(xy: &str) -> Result<(), Error> {
    let accepted = [
        " M", " T", " D", "M ", "MM", "MT", "MD", "T ", "TM", "TT", "TD", "A ", "AM", "AT", "AD",
        "D ", "R ", "RM", "RT", "RD", "C ", "CM", "CT", "CD", "??",
    ];
    if accepted.contains(&xy) {
        return Ok(());
    }
    // Any unmerged combination rejects as conflict before ordinary dirty.
    if xy.contains('U')
        || xy == "DD"
        || xy == "AA"
        || xy == "AU"
        || xy == "UA"
        || xy == "UD"
        || xy == "DU"
    {
        return Err(Error::GitConflict);
    }
    Err(Error::GitInventoryInvalid)
}

/// Validate a raw inventory path: strict UTF-8, repository-relative,
/// component-safe, no control characters, no doubled separators, no
/// invalid trailing separator, and no tracked `.mrgs` first segment.
fn validate_inventory_path(path: &str) -> Result<(), Error> {
    if path.is_empty() {
        return Err(Error::GitInventoryInvalid);
    }
    if path.starts_with('/') || path.starts_with("//") {
        return Err(Error::GitInventoryInvalid);
    }
    // Drive prefix (e.g. C:)
    if path.len() >= 2 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':' {
        return Err(Error::GitInventoryInvalid);
    }
    // UNC or device prefix (e.g. \\server, \\.\)
    if path.starts_with("\\\\") {
        return Err(Error::GitInventoryInvalid);
    }
    if path.contains('\\') {
        return Err(Error::GitInventoryInvalid);
    }
    if path.contains("//") {
        return Err(Error::GitInventoryInvalid);
    }
    if path.ends_with('/') {
        return Err(Error::GitInventoryInvalid);
    }
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(Error::GitInventoryInvalid);
        }
    }
    // Control characters
    if path
        .chars()
        .any(|c| c as u32 == 0 || (c as u32 > 0 && (c as u32) < 32) || c as u32 == 127)
    {
        return Err(Error::GitInventoryInvalid);
    }
    Ok(())
}

#[derive(Debug)]
struct PorcelainRecord {
    xy: String,
    path: String,
    source: Option<String>,
}

fn parse_porcelain_output(stdout: &[u8]) -> Result<Vec<PorcelainRecord>, Error> {
    let mut records = Vec::new();
    let mut i = 0;
    while i < stdout.len() {
        let nul = stdout[i..]
            .iter()
            .position(|&byte| byte == 0)
            .ok_or(Error::GitInventoryInvalid)?;
        if nul < 4 || stdout[i + 2] != b' ' {
            return Err(Error::GitInventoryInvalid);
        }
        let xy = std::str::from_utf8(&stdout[i..i + 2])
            .map_err(|_| Error::GitInventoryInvalid)?
            .to_string();
        let path_data = &stdout[i + 3..i + nul];
        if path_data.is_empty() {
            return Err(Error::GitInventoryInvalid);
        }
        let path = std::str::from_utf8(path_data)
            .map_err(|_| Error::GitInventoryInvalid)?
            .to_string();

        // `!!` is unexpected (ignored paths were not requested) and fatal.
        if xy == "!!" {
            return Err(Error::GitInventoryInvalid);
        }
        classify_porcelain_xy(&xy)?;
        validate_inventory_path(&path)?;
        i += nul + 1;

        let source = if xy.starts_with('R') || xy.starts_with('C') {
            if i >= stdout.len() {
                return Err(Error::GitInventoryInvalid);
            }
            let nul2 = stdout[i..]
                .iter()
                .position(|&byte| byte == 0)
                .ok_or(Error::GitInventoryInvalid)?;
            let source = std::str::from_utf8(&stdout[i..i + nul2])
                .map_err(|_| Error::GitInventoryInvalid)?
                .to_string();
            validate_inventory_path(&source)?;
            i += nul2 + 1;
            Some(source)
        } else {
            None
        };

        records.push(PorcelainRecord { xy, path, source });
    }
    Ok(records)
}

fn parse_ignored_output(stdout: &[u8]) -> Result<Vec<String>, Error> {
    let mut paths = Vec::new();
    let mut i = 0;
    while i < stdout.len() {
        let nul = stdout[i..]
            .iter()
            .position(|&byte| byte == 0)
            .ok_or(Error::GitInventoryInvalid)?;
        let path = std::str::from_utf8(&stdout[i..i + nul])
            .map_err(|_| Error::GitInventoryInvalid)?
            .to_string();
        validate_inventory_path(&path)?;
        paths.push(path);
        i += nul + 1;
    }
    Ok(paths)
}

fn build_change_inventory(
    git: &GitRunner,
    record: &ImplementationAuthority,
    auth: &ValidatedAuthority,
    objfmt: &str,
    tracked_governance: &[String],
) -> Result<(BTreeSet<String>, Vec<RawDiffEntry>), Error> {
    let mut paths = BTreeSet::new();

    // Raw diff from baseline to HEAD, parsed into structured entries that
    // retain the exact new-side OID for HEAD symlink inspection (BLOCKER 2).
    let raw_entries = parse_raw_diff_entries(git, &record.baseline_head, objfmt)?;
    for entry in &raw_entries {
        paths.insert(entry.dst.clone());
        if let Some(ref src) = entry.src {
            paths.insert(src.clone());
        }
    }

    // Porcelain status
    let status_out = git.run([
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--ignore-submodules=none",
        "--renames",
    ])?;
    if !status_out.status.success() {
        return Err(Error::GitCommandFailed("status inventory failed".into()));
    }
    {
        for record in parse_porcelain_output(&status_out.stdout)? {
            if record.xy == "??" {
                if is_exempt_governance_path(&record.path, auth, tracked_governance) {
                    continue;
                }
                paths.insert(record.path);
                continue;
            }

            if is_governance_path(&record.path) {
                return Err(Error::GitInventoryInvalid);
            }
            paths.insert(record.path);
            if let Some(source) = record.source {
                if is_governance_path(&source) {
                    return Err(Error::GitInventoryInvalid);
                }
                paths.insert(source);
            }
        }
    }

    // Ignored untracked files
    let ignored_out = git.run([
        "ls-files",
        "--others",
        "--ignored",
        "--exclude-standard",
        "-z",
        "--",
    ])?;
    if !ignored_out.status.success() {
        return Err(Error::GitCommandFailed(
            "ls-files ignored inventory failed".into(),
        ));
    }
    {
        for path in parse_ignored_output(&ignored_out.stdout)? {
            if !is_exempt_governance_path(&path, auth, tracked_governance) {
                paths.insert(path);
            }
        }
    }

    Ok((paths, raw_entries))
}

#[cfg(test)]
mod part1_tests {
    use super::*;

    // ---- BLOCKER 2: raw mode classification ----
    // PHASE4_PART1_TEST
    #[test]
    fn raw_mode_classifies_allowed_modes() {
        assert_eq!(classify_raw_mode(b"000000").unwrap(), "000000");
        assert_eq!(classify_raw_mode(b"100644").unwrap(), "100644");
        assert_eq!(classify_raw_mode(b"100755").unwrap(), "100755");
        assert_eq!(classify_raw_mode(b"120000").unwrap(), "120000");
        assert_eq!(classify_raw_mode(b"160000").unwrap(), "160000");
    }

    // PHASE4_PART1_TEST
    #[test]
    fn raw_mode_rejects_unsupported_modes() {
        assert!(classify_raw_mode(b"040000").is_err());
        assert!(classify_raw_mode(b"160700").is_err());
        assert!(classify_raw_mode(b"100664").is_err());
        assert!(classify_raw_mode(b"123456").is_err());
        assert!(classify_raw_mode(b"10064").is_err()); // wrong length
        assert!(classify_raw_mode(b"1006444").is_err()); // wrong length
    }

    // PHASE4_PART1_TEST
    #[test]
    fn raw_change_score_requires_decimal_zero_to_hundred() {
        for (input, expected) in [("R100", 100), ("R0", 0), ("C100", 100)] {
            assert_eq!(validate_raw_change_score(&input[1..]).unwrap(), expected);
        }
        for input in ["", "101", "999", "+50", " 50", "abc"] {
            assert!(matches!(
                validate_raw_change_score(input),
                Err(Error::GitInventoryInvalid)
            ));
        }
    }

    #[test]
    fn raw_diff_output_accepts_consistent_records_and_rejects_invalid_combinations() {
        let oid = "a".repeat(40);
        let zero = "0".repeat(40);
        let valid = [
            format!(":000000 100644 {zero} {oid} A\0new\0"),
            format!(":100644 000000 {oid} {zero} D\0old\0"),
            format!(":100755 120000 {oid} {oid} M\0link\0"),
            format!(":100644 100644 {oid} {oid} R50\0old\0new\0"),
            format!(":100644 100644 {oid} {oid} C100\0source\0copy\0"),
        ];
        for record in valid {
            let entries = parse_raw_diff_output(record.as_bytes(), "sha1").unwrap();
            assert_eq!(entries.len(), 1);
        }

        for record in [
            format!(":100644 100644 {oid} {oid} M50\0path\0"),
            format!(":100644 100644 {oid} {oid} R\0old\0new\0"),
            format!(":100644 100644 {oid} {oid} R101\0old\0new\0"),
            format!(":000000 100644 {oid} {oid} A\0new\0"),
            format!(":000000 100644 {zero} {zero} A\0new\0"),
            format!(":100644 000000 {oid} {oid} D\0old\0"),
            format!(":100644 100644 {zero} {oid} M\0path\0"),
            format!(":100644 160000 {oid} {oid} M\0submodule\0"),
            format!(":100644 100644 {oid} {oid} M\0.MRGS/path\0"),
        ] {
            assert!(matches!(
                parse_raw_diff_output(record.as_bytes(), "sha1"),
                Err(Error::GitInventoryInvalid) | Err(Error::GitSubmoduleUnsupported)
            ));
        }

        let sha256_oid = "b".repeat(64);
        let sha256_record = format!(":100644 100644 {sha256_oid} {sha256_oid} M\0path\0");
        assert!(parse_raw_diff_output(sha256_record.as_bytes(), "sha256").is_ok());
    }

    #[test]
    fn raw_diff_output_rejects_malformed_records_and_requires_complete_stream() {
        let oid = "a".repeat(40);
        let malformed = [
            format!("100644 100644 {oid} {oid} M\0path\0"),
            format!(":100644 100644 {oid} M\0path\0"),
            format!(":100644 100644 {oid} {oid} Z\0path\0"),
            format!(":100644 100644 {oid} {oid} R50\0only-source\0"),
            format!(":100644 100644 {oid} {oid} M\0path\0orphan"),
            format!(":100644 100644 {oid} {oid} M\0path"),
        ];
        for record in malformed {
            assert!(matches!(
                parse_raw_diff_output(record.as_bytes(), "sha1"),
                Err(Error::GitInventoryInvalid)
            ));
        }

        let stream = format!(
            ":000000 100644 {} {} A\0one\0:100644 000000 {} {} D\0two\0",
            "0".repeat(40),
            oid,
            oid,
            "0".repeat(40)
        );
        assert_eq!(
            parse_raw_diff_output(stream.as_bytes(), "sha1")
                .unwrap()
                .len(),
            2
        );
    }

    // ---- BLOCKER 6: porcelain XY classification ----
    // PHASE4_PART1_TEST
    #[test]
    fn porcelain_xy_accepts_exact_codes() {
        for xy in [
            " M", " T", " D", "M ", "MM", "MT", "MD", "T ", "TM", "TT", "TD", "A ", "AM", "AT",
            "AD", "D ", "R ", "RM", "RT", "RD", "C ", "CM", "CT", "CD",
        ] {
            assert!(classify_porcelain_xy(xy).is_ok(), "expected ok for {}", xy);
        }
    }

    // PHASE4_PART1_TEST
    #[test]
    fn porcelain_xy_rejects_missing_separator_form() {
        // An unknown status code (e.g. a single character without the
        // significant separator) is unsupported evidence.
        assert!(matches!(
            classify_porcelain_xy("ZQ"),
            Err(Error::GitInventoryInvalid)
        ));
        assert!(matches!(
            classify_porcelain_xy("XY"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn porcelain_xy_conflict_precedence() {
        assert!(matches!(
            classify_porcelain_xy("DD"),
            Err(Error::GitConflict)
        ));
        assert!(matches!(
            classify_porcelain_xy("AA"),
            Err(Error::GitConflict)
        ));
        assert!(matches!(
            classify_porcelain_xy("UU"),
            Err(Error::GitConflict)
        ));
        assert!(matches!(
            classify_porcelain_xy("DU"),
            Err(Error::GitConflict)
        ));
        // `??` is a valid untracked code: it proceeds to the exempt/dirty
        // branch rather than being malformed evidence.
        assert!(matches!(classify_porcelain_xy("??"), Ok(())));
        assert!(matches!(
            classify_porcelain_xy("!!"),
            Err(Error::GitInventoryInvalid)
        ));
        // Unknown non-conflict code is malformed evidence.
        assert!(matches!(
            classify_porcelain_xy("Z "),
            Err(Error::GitInventoryInvalid)
        ));
    }

    #[test]
    fn porcelain_output_rejects_malformed_records_and_consumes_complete_stream() {
        let valid = b"?? .mrgs/accepted-plan.json\0R  new.txt\0old.txt\0 M src/file\0";
        let records = parse_porcelain_output(valid).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[1].source.as_deref(), Some("old.txt"));

        for output in [
            b"??path\0".as_slice(),
            b"Z  path\0".as_slice(),
            b"R  new.txt\0".as_slice(),
            b"R  new.txt\0old.txt".as_slice(),
            b" M path\0trailing".as_slice(),
            b" M \0".as_slice(),
            b" M \xff\0".as_slice(),
        ] {
            assert!(matches!(
                parse_porcelain_output(output),
                Err(Error::GitInventoryInvalid)
            ));
        }
    }

    #[test]
    fn ignored_output_rejects_malformed_records_and_non_utf8() {
        assert_eq!(
            parse_ignored_output(b"ignored/a\0ignored/b\0").unwrap(),
            vec!["ignored/a", "ignored/b"]
        );
        for output in [
            b"ignored/a".as_slice(),
            b"ignored/a\0trailing".as_slice(),
            b"\0".as_slice(),
            b"ignored/\xff\0".as_slice(),
        ] {
            assert!(matches!(
                parse_ignored_output(output),
                Err(Error::GitInventoryInvalid)
            ));
        }
    }

    // ---- BLOCKER 3/4/5: symlink target lexical validation ----
    // PHASE4_PART1_TEST
    #[test]
    fn symlink_target_accepts_contained_relative() {
        assert!(validate_symlink_target(Path::new("a/b"), "a/b", "c/d").is_ok());
        assert!(validate_symlink_target(Path::new("link"), "link", "sub/target").is_ok());
    }

    // PHASE4_PART1_TEST
    #[test]
    fn symlink_target_rejects_absolute() {
        assert!(validate_symlink_target(Path::new("l"), "l", "/etc/passwd").is_err());
        assert!(validate_symlink_target(Path::new("l"), "l", "//server/x").is_err());
    }

    // PHASE4_PART1_TEST
    #[test]
    fn symlink_target_rejects_drive_unc_device() {
        assert!(validate_symlink_target(Path::new("l"), "l", "C:foo").is_err());
        assert!(validate_symlink_target(Path::new("l"), "l", "\\\\server\\share").is_err());
        assert!(validate_symlink_target(Path::new("l"), "l", "\\\\.\\x").is_err());
    }

    // PHASE4_PART1_TEST
    #[test]
    fn symlink_target_rejects_backslash_control_malformed() {
        assert!(validate_symlink_target(Path::new("l"), "l", "a\\b").is_err());
        assert!(validate_symlink_target(Path::new("l"), "l", "a\x01b").is_err());
        assert!(validate_symlink_target(Path::new("l"), "l", "a//b").is_err()); // empty segment
        assert!(validate_symlink_target(Path::new("l"), "l", "..").is_err()); // escape
        assert!(validate_symlink_target(Path::new("l"), "l", "../x").is_err());
    }

    // ---- BLOCKER 3/4: ls-tree -z strict parsing ----
    // PHASE4_PART1_TEST
    #[test]
    fn ls_tree_z_parses_exact_record() {
        let oid = "a".repeat(40);
        let out = format!("120000 blob {oid}\tlink/target\0");
        let rec = parse_ls_tree_z(out.as_bytes(), "link/target", "sha1")
            .unwrap()
            .unwrap();
        assert_eq!(rec.mode, "120000");
        assert_eq!(rec.oid, oid);
    }

    // PHASE4_PART1_TEST
    #[test]
    fn ls_tree_z_rejects_wrong_path() {
        let out = b"100644 blob abc123\tx/y\x00";
        assert!(parse_ls_tree_z(out, "a/b", "sha1").is_err());
    }

    // PHASE4_PART1_TEST
    #[test]
    fn ls_tree_z_rejects_bad_type() {
        let out = b"100644 weird abc123\ta/b\x00";
        assert!(parse_ls_tree_z(out, "a/b", "sha1").is_err());
    }

    // PHASE4_PART1_TEST
    #[test]
    fn ls_tree_z_rejects_bad_mode_grammar() {
        let out = b"10064 blob abc123\ta/b\x00";
        assert!(parse_ls_tree_z(out, "a/b", "sha1").is_err());
    }

    // PHASE4_PART1_TEST
    #[test]
    fn ls_tree_z_rejects_extra_record() {
        // A single `ls-tree -z HEAD -- <path>` lookup must return exactly zero
        // or one record; a second record for a *different* path proves the
        // lookup returned more than expected and is rejected.
        let out = b"100644 blob a\tp\x00100644 blob b\tq\x00";
        assert!(parse_ls_tree_z(out, "p", "sha1").is_err());
    }

    // ---- BLOCKER 5: index stage -z strict parsing ----
    // PHASE4_PART1_TEST
    #[test]
    fn index_stage_z_parses_stage0() {
        let oid = "a".repeat(40);
        let out = format!("100644 {oid} 0\ta/b\0");
        let rec = parse_index_stage_z(out.as_bytes(), "a/b", "sha1")
            .unwrap()
            .unwrap();
        assert_eq!(rec.mode, "100644");
        assert_eq!(rec.stage, 0);
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_stage_z_rejects_nonzero_stage() {
        let oid = "a".repeat(40);
        let out = format!("100644 {oid} 1\ta/b\0");
        assert!(matches!(
            parse_index_stage_z(out.as_bytes(), "a/b", "sha1"),
            Err(Error::GitConflict)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_stage_z_rejects_wrong_path() {
        let out = b"100644 abc123 0\tx/y\x00";
        assert!(parse_index_stage_z(out, "a/b", "sha1").is_err());
    }

    #[test]
    fn index_stage_parser_covers_modes_object_lengths_and_malformed_shapes() {
        let sha1 = "a".repeat(40);
        for mode in ["100644", "100755", "120000"] {
            let out = format!("{mode} {sha1} 0\ta/b\0");
            assert!(parse_index_stage_z(out.as_bytes(), "a/b", "sha1").is_ok());
        }
        let sha256 = "b".repeat(64);
        let out = format!("100644 {sha256} 0\ta/b\0");
        assert!(parse_index_stage_z(out.as_bytes(), "a/b", "sha256").is_ok());

        for out in [
            b"10064 a 0\ta/b\0".as_slice(),
            b"100999 a 0\ta/b\0".as_slice(),
            b"100644 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA 0\ta/b\0".as_slice(),
            b"100644 a 0\ta/b extra\0".as_slice(),
            b"100644 a 0\ta/b".as_slice(),
            b"100644 a\ta/b\0".as_slice(),
        ] {
            assert!(matches!(
                parse_index_stage_z(out, "a/b", "sha1"),
                Err(Error::GitInventoryInvalid)
            ));
        }
    }

    // ---- BLOCKER 5: index topology -z sparse-directory parsing ----
    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_sparse_child_records() {
        // Sparse-directory records are structural evidence even when returned
        // beneath a requested directory prefix.
        let oid = "a".repeat(40);
        let out = format!("040000 {oid} 0\ta/b/\0");
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_exact_sparse_directory_record() {
        let oid = "a".repeat(40);
        let out = format!("040000 {oid} 0\ta/b/\0");
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a/b", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_sparse_directory_extra_record() {
        let oid = "a".repeat(40);
        let out = format!("040000 {oid} 0\ta/b/\0040000 {oid} 0\ta/b/\0");
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a/b", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_sparse_directory_missing_trailing_slash() {
        let out = format!("040000 {} 0\ta/b\0", "a".repeat(40));
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_sparse_directory_double_trailing_slash() {
        let out = format!("040000 {} 0\ta/b//\0", "a".repeat(40));
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_sparse_directory_root_path() {
        let out = format!("040000 {} 0\t/\0", "a".repeat(40));
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_sparse_directory_unsafe_component() {
        let out = format!("040000 {} 0\ta/../b/\0", "a".repeat(40));
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_sparse_directory_wrong_oid_length() {
        let out = b"040000 abc 0\ta/b/\0";
        assert!(matches!(
            parse_index_topology_z(out, "a", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_sparse_directory_uppercase_oid() {
        let out = format!("040000 {} 0\ta/b/\0", "A".repeat(40));
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_sparse_directory_invalid_stage() {
        let out = format!("040000 {} x\ta/b/\0", "a".repeat(40));
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_accepts_ordinary_child_record() {
        let out = format!("100644 {} 0\ta/b/c\0", "a".repeat(40));
        assert!(parse_index_topology_z(out.as_bytes(), "a/b", "sha1")
            .unwrap()
            .is_none());
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_accepts_ordinary_exact_records() {
        for mode in ["100644", "100755"] {
            let out = format!("{mode} {} 0\ta/b\0", "a".repeat(40));
            let record = parse_index_topology_z(out.as_bytes(), "a/b", "sha1")
                .unwrap()
                .unwrap();
            assert_eq!(record.mode, mode);
        }
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_accepts_symlink_exact_record() {
        let out = format!("120000 {} 0\ta/b\0", "a".repeat(40));
        let record = parse_index_topology_z(out.as_bytes(), "a/b", "sha1")
            .unwrap()
            .unwrap();
        assert_eq!(record.mode, "120000");
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_gitlink_record() {
        let out = format!("160000 {} 0\ta/b\0", "a".repeat(40));
        assert!(matches!(
            parse_index_topology_z(out.as_bytes(), "a/b", "sha1"),
            Err(Error::GitSubmoduleUnsupported)
        ));
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_topology_z_rejects_malformed_child_record() {
        let out = b"10064 abc123 0\ta/b/c\0";
        assert!(matches!(
            parse_index_topology_z(out, "a/b", "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
    }

    // ---- BLOCKER 5: full-index path safety on leaf lookups ----
    // PHASE4_PART1_TEST
    #[test]
    fn index_stage_z_rejects_child_path() {
        // Requested `a/b`, got `a/b/c` (a child entry under the leaf path).
        let out = b"100644 abc123 0\ta/b/c\0";
        assert!(parse_index_stage_z(out, "a/b", "sha1").is_err());
    }

    // PHASE4_PART1_TEST
    #[test]
    fn index_stage_z_rejects_parent_path() {
        // Requested `a/b/c`, got `a/b` (a parent entry, not the exact leaf).
        let out = b"100644 abc123 0\ta/b\0";
        assert!(parse_index_stage_z(out, "a/b/c", "sha1").is_err());
    }

    // ---- BLOCKER 4/5: prefix decomposition ----
    // PHASE4_PART1_TEST
    #[test]
    fn path_prefixes_yields_increasing() {
        assert_eq!(path_prefixes("a/b/c"), vec!["a", "a/b", "a/b/c"]);
        assert_eq!(path_prefixes("a"), vec!["a"]);
    }
}

#[cfg(test)]
mod p2a_tests {
    use super::*;

    // P2-A: the begin-cleanliness governance exemption is gated on the Section
    // 6.4 proof that no tracked index entry exists for the path. A path that IS
    // tracked must never be exempted from `??`/ignored-untracked even when it is
    // one of the exact fixed governance filenames.
    #[test]
    fn exemption_rejected_when_path_is_tracked() {
        let auth = ValidatedAuthority {
            repo: PathBuf::from("/repo"),
            gov_dir: PathBuf::from("/repo/.mrgs"),
            accepted_plan_sha256: String::new(),
            active_phase: String::new(),
            contract_id: String::new(),
            final_revision: 1,
            final_source_path: String::new(),
            final_sha256: String::new(),
            final_content: String::new(),
            rule_set: PathRuleSet {
                allowed: vec![],
                forbidden: vec![],
            },
            lifecycle: "ACCEPTED",
        };
        // `.mrgs/state.json` reported as an untracked `??` path, but Section 6.4
        // proved a tracked index entry exists for it -> not exempt.
        let tracked = vec![".mrgs/state.json".to_string()];
        assert!(!is_exempt_governance_path(
            ".mrgs/state.json",
            &auth,
            &tracked
        ));
        // A different fixed governance path with no tracked entry IS exempt.
        let none_tracked: Vec<String> = vec![];
        assert!(is_exempt_governance_path(
            ".mrgs/state.json",
            &auth,
            &none_tracked
        ));
        // A non-governance path is never exempt regardless of tracking.
        assert!(!is_exempt_governance_path(
            "src/main.rs",
            &auth,
            &none_tracked
        ));
        // A tracked `.MRGS/state.json` (case alias) is gated using the exact
        // path git reports: the exemption for `.mrgs/state.json` is unaffected
        // only because git reports the alias under its own bytes; the alias
        // path itself would be gated if it were the one being exempted.
        let tracked_alias = vec![".MRGS/state.json".to_string()];
        assert!(!is_exempt_governance_path(
            ".MRGS/state.json",
            &auth,
            &tracked_alias
        ));
        // When git reports `.mrgs/state.json` exactly and it is tracked, it is
        // gated precisely (exact bytes, no normalization).
        let tracked_exact = vec![".mrgs/state.json".to_string()];
        assert!(!is_exempt_governance_path(
            ".mrgs/state.json",
            &auth,
            &tracked_exact
        ));
    }

    // P2-A: exact fixed-governance exemption set is exactly the five contract
    // paths and no other `.mrgs` path (unknown/temporary/child paths are not
    // exempt even when untracked).
    #[test]
    fn exemption_only_exact_fixed_paths() {
        let auth = ValidatedAuthority {
            repo: PathBuf::from("/repo"),
            gov_dir: PathBuf::from("/repo/.mrgs"),
            accepted_plan_sha256: String::new(),
            active_phase: String::new(),
            contract_id: String::new(),
            final_revision: 1,
            final_source_path: String::new(),
            final_sha256: String::new(),
            final_content: String::new(),
            rule_set: PathRuleSet {
                allowed: vec![],
                forbidden: vec![],
            },
            lifecycle: "ACCEPTED",
        };
        let none: Vec<String> = vec![];
        for p in [
            ".mrgs/accepted-plan.json",
            ".mrgs/state.json",
            ".mrgs/contract-draft.json",
            ".mrgs/accepted-contract.json",
            ".mrgs/implementation-authority.json",
        ] {
            assert!(is_exempt_governance_path(p, &auth, &none));
        }
        // Unknown / temporary / child `.mrgs` paths are never exempt.
        for p in [
            ".mrgs/extra.json",
            ".mrgs/.tmp_x.tmp",
            ".mrgs/sub/draft.json",
            "src/main.rs",
        ] {
            assert!(!is_exempt_governance_path(p, &auth, &none));
        }
    }

    // P2-A: a tracked `.mrgs` index entry is rejected by the same stage parser
    // that `tracked_governance_paths` consumes, proving Section 6.4 catches a
    // tracked governance path before any exemption is even considered. We
    // exercise the parser directly rather than spawning git.
    #[test]
    fn tracked_governance_index_entry_rejected_by_parser() {
        let oid = "a".repeat(40);
        // A tracked `.mrgs/state.json` (stage 0, ordinary file) must be rejected
        // as GIT_INVENTORY_INVALID by the stage parser itself.
        let rec = format!("100644 {oid} 0\t.mrgs/state.json\0");
        assert!(matches!(
            parse_index_stage_record(rec.as_bytes(), "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
        // A tracked `.MRGS/state.json` (case alias) is likewise rejected.
        let rec2 = format!("100644 {oid} 0\t.MRGS/state.json\0");
        assert!(matches!(
            parse_index_stage_record(rec2.as_bytes(), "sha1"),
            Err(Error::GitInventoryInvalid)
        ));
        // A tracked ordinary `src/main.rs` parses fine (not a `.mrgs` segment).
        // (No trailing NUL: the production parser receives a single record
        // slice already split on the `-z` separator.)
        let rec3 = format!("100644 {oid} 0\tsrc/main.rs");
        assert!(parse_index_stage_record(rec3.as_bytes(), "sha1").is_ok());
    }

    // P2-A: `evaluate_symlink_target` maps ordinary changed-path categories
    // (CHANGE_FORBIDDEN / CHANGE_NOT_ALLOWED) to FILESYSTEM_BOUNDARY_UNSAFE for a
    // symlink target, never leaking the ordinary changed-path category.
    #[test]
    fn symlink_target_scope_maps_to_boundary_unsafe() {
        let ruleset = PathRuleSet {
            allowed: vec!["allowed/file".to_string()],
            forbidden: vec!["secret/file".to_string()],
        };
        // Forbidden target -> FILESYSTEM_BOUNDARY_UNSAFE.
        assert!(matches!(
            evaluate_symlink_target(&ruleset, "secret/file"),
            Err(Error::FilesystemBoundaryUnsafe)
        ));
        // No-allowed target -> FILESYSTEM_BOUNDARY_UNSAFE.
        assert!(matches!(
            evaluate_symlink_target(&ruleset, "other/file"),
            Err(Error::FilesystemBoundaryUnsafe)
        ));
        // Allowed target -> Ok.
        assert!(evaluate_symlink_target(&ruleset, "allowed/file").is_ok());
    }

    // P2-A: `validate_symlink_target` rejects a target that lexically escapes
    // the repository and accepts a contained relative target.
    #[test]
    fn symlink_target_lexical_escape_rejected() {
        assert!(validate_symlink_target(Path::new("link"), "link", "../escape").is_err());
        assert!(validate_symlink_target(Path::new("a/link"), "a/link", "../../x").is_err());
        assert!(validate_symlink_target(Path::new("link"), "link", "sub/ok").is_ok());
    }

    // P2-A (Blocker 1): `resolve_lexical_symlink_target` produces the exact
    // repository-relative path the live canonical proof relies on, and rejects
    // any lexical escape.
    #[test]
    fn resolve_lexical_symlink_target_ok_and_escape() {
        // Root-level link -> relative target stays at top.
        assert_eq!(
            resolve_lexical_symlink_target("", "sub/target").unwrap(),
            "sub/target"
        );
        // Nested link resolves relative to its parent.
        assert_eq!(
            resolve_lexical_symlink_target("a/b", "c/d").unwrap(),
            "a/b/c/d"
        );
        // "." segments collapse.
        assert_eq!(
            resolve_lexical_symlink_target("a/b", "./c").unwrap(),
            "a/b/c"
        );
        // ".." can pop within the repository but not escape it.
        assert_eq!(
            resolve_lexical_symlink_target("a/b", "../c").unwrap(),
            "a/c"
        );
        assert_eq!(
            resolve_lexical_symlink_target("a/b", "../../c").unwrap(),
            "c"
        );
        // Escape above the repository root is rejected.
        assert!(resolve_lexical_symlink_target("a", "../../c").is_err());
        // Resolving exactly to the repository root is lexically allowed (the
        // canonical proof rejects an empty repository-relative target).
        assert_eq!(resolve_lexical_symlink_target("a/b", "../..").unwrap(), "");
        // Empty and absolute-ish segments rejected.
        assert!(resolve_lexical_symlink_target("a", "b//c").is_err());
        assert!(resolve_lexical_symlink_target("a", "/abs").is_err());
        // Absolute targets (leading slash) are rejected.
        assert!(resolve_lexical_symlink_target("a/b", "/etc/passwd").is_err());
    }

    // P2-A (Blocker 1): `canonical_target_to_repo_relative` requires the
    // canonical target to remain inside the canonical repository and converts
    // without loss to a normalized `/`-separated repository-relative path.
    #[test]
    fn canonical_target_to_repo_relative_normalizes() {
        let repo = Path::new("/repo");
        // Inside the repository.
        let t = Path::new("/repo/sub/dir/file");
        assert_eq!(
            canonical_target_to_repo_relative(repo, t).unwrap(),
            "sub/dir/file"
        );
        // Direct child.
        let t2 = Path::new("/repo/file");
        assert_eq!(canonical_target_to_repo_relative(repo, t2).unwrap(), "file");
        // Outside the repository -> rejected.
        assert!(canonical_target_to_repo_relative(repo, Path::new("/other/x")).is_err());
        // Sibling prefix (not a directory boundary) -> rejected.
        assert!(canonical_target_to_repo_relative(repo, Path::new("/repoX/x")).is_err());
        // Equal to the repository root -> rejected (empty relative).
        assert!(canonical_target_to_repo_relative(repo, Path::new("/repo")).is_err());
        // Non-UTF-8 / invalid -> rejected.
        let bad = Path::new("/repo");
        // Construct a path whose components are valid but suffix is a stray
        // absolute root marker.
        assert!(canonical_target_to_repo_relative(bad, Path::new("/repo/")).is_err());
    }

    // P2-A (Blocker 1): the live canonical proof requires the canonical target
    // to match the rule set independently. Here we exercise the helper chain on
    // a real temporary tree: a live symlink whose existing target resolves
    // inside the repo is accepted when allowed and rejected when forbidden.
    #[test]
    fn live_symlink_canonical_proof_real_fs() {
        let tmp = std::env::temp_dir().join(format!("mrgs_p2a_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("allowed")).unwrap();
        std::fs::create_dir_all(tmp.join("secret")).unwrap();
        std::fs::write(tmp.join("allowed/target.txt"), b"ok").unwrap();
        std::fs::write(tmp.join("secret/target.txt"), b"no").unwrap();

        // Allowed target. The symlink points at allowed/target.txt (exists),
        // so the canonical proof path is taken.
        let allowed_rules = PathRuleSet {
            allowed: vec!["allowed/target.txt".to_string()],
            forbidden: vec![], // explicit allow covers it; forbidden blank
        };
        // Use a relaxed ruleset so lexical eval passes; containment is what we
        // verify. Build a symlink and run the production live inspection helpers.
        let link = tmp.join("link_a");
        #[cfg(unix)]
        std::os::unix::fs::symlink("allowed/target.txt", &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file("allowed/target.txt", &link).unwrap();

        let repo = std::fs::canonicalize(&tmp).unwrap();
        let link_parent = repo.join("");
        // `prove_live_target_no_chain` must accept a plain-file chain.
        prove_live_target_no_chain(&link_parent, "allowed/target.txt").unwrap();
        // The canonical target converts to the expected repository-relative path.
        let target_path = link_parent.join("allowed/target.txt");
        let canon = std::fs::canonicalize(&target_path).unwrap();
        let rel = canonical_target_to_repo_relative(&repo, &canon).unwrap();
        // Rule-set evaluation against the canonical repo-relative path.
        assert!(allowed_rules.evaluate(&rel).is_ok());

        // Forbidden target: lexical match alone must also reject at the separate
        // lexical evaluation stage.
        let forbidden_rules = PathRuleSet {
            allowed: vec!["allowed/".to_string()],
            forbidden: vec!["secret/target.txt".to_string()],
        };
        // A lexical target under `secret/` is forbidden-first rejected.
        assert!(matches!(
            forbidden_rules.evaluate("secret/target.txt"),
            Err(Error::ChangeForbidden)
        ));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // P2-A (Blocker 1): `classify_metadata_result` classifies a leaf exactly.
    // Only `NotFound` yields absence; every other error kind yields
    // FILESYSTEM_BOUNDARY_UNSAFE (never reinterpreted as absence).
    #[test]
    fn classify_metadata_result_absent_only_for_notfound() {
        use std::io::{Error as IoError, ErrorKind};
        // Existing metadata -> Some.
        let tmp = std::env::temp_dir().join(format!("mrgs_p2a_cls_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let meta = std::fs::symlink_metadata(&tmp).unwrap();
        assert!(classify_metadata_result(Ok(meta)).unwrap().is_some());
        // Missing leaf -> None (absent), exactly NotFound.
        let missing = tmp.join("does_not_exist_xyz");
        let r = classify_metadata_result(std::fs::symlink_metadata(&missing));
        assert!(r.unwrap().is_none());
        // Other error kinds -> FILESYSTEM_BOUNDARY_UNSAFE.
        for kind in [
            ErrorKind::PermissionDenied,
            ErrorKind::Other,
            ErrorKind::Interrupted,
            ErrorKind::UnexpectedEof,
        ] {
            let err = classify_metadata_result(Err(IoError::new(kind, "simulated")));
            assert!(matches!(err, Err(Error::FilesystemBoundaryUnsafe)));
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // P2-A (Blocker 1/2): `classify_live_leaf_metadata` on a real tree yields
    // Some for an existing ordinary leaf and None for an absent leaf.
    #[test]
    fn classify_live_leaf_metadata_real_fs() {
        let tmp = std::env::temp_dir().join(format!("mrgs_p2a_leaf_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("sub")).unwrap();
        std::fs::write(tmp.join("sub/file.txt"), b"x").unwrap();
        // Existing ordinary leaf -> Some.
        assert!(classify_live_leaf_metadata(&tmp.join("sub/file.txt"))
            .unwrap()
            .is_some());
        // Absent leaf -> None.
        assert!(classify_live_leaf_metadata(&tmp.join("sub/missing.txt"))
            .unwrap()
            .is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // P2-A (Blocker 3): `is_reparse_point_not_symlink` rejects a Windows
    // junction leaf via a real `mklink /J` fixture when safely available, and
    // never rejects a genuine symlink. Non-Windows: the helper is always false.
    #[cfg(windows)]
    #[test]
    fn windows_junction_leaf_rejected_real_fixture() {
        let tmp = std::env::temp_dir().join(format!("mrgs_p2a_junc_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("target")).unwrap();
        std::fs::create_dir_all(tmp.join("link_parent")).unwrap();
        // Create a real junction `link_parent/j` -> `target`.
        let junc = tmp.join("link_parent/j");
        let mk = std::process::Command::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                &junc.to_string_lossy(),
                &tmp.join("target").to_string_lossy(),
            ])
            .output();
        if mk.is_err() || !mk.unwrap().status.success() {
            // Fixture creation not safely available in this environment.
            let _ = std::fs::remove_dir_all(&tmp);
            return;
        }
        let meta = std::fs::symlink_metadata(&junc).unwrap();
        assert!(is_reparse_point_not_symlink(&meta));
        assert!(matches!(
            reject_unsafe_ancestor_metadata(&meta),
            Err(Error::FilesystemBoundaryUnsafe)
        ));
        // The reparse-only helper does not flag a genuine symlink; the
        // cross-platform ancestor helper still rejects it.
        let sl = tmp.join("link_parent/s");
        if let Err(err) = std::os::windows::fs::symlink_file("target", &sl) {
            eprintln!("WINDOWS_FIXTURE_LIMITATION: symlink creation failed: {err}");
            let _ = std::fs::remove_dir_all(&tmp);
            return;
        }
        let sl_meta = std::fs::symlink_metadata(&sl).unwrap();
        assert!(!is_reparse_point_not_symlink(&sl_meta));
        assert!(matches!(
            reject_unsafe_ancestor_metadata(&sl_meta),
            Err(Error::FilesystemBoundaryUnsafe)
        ));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn ancestor_helper_rejects_symlink_and_accepts_ordinary_directory() {
        let tmp = std::env::temp_dir().join(format!("mrgs_p2a_ancestor_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("target")).unwrap();
        let ordinary = std::fs::symlink_metadata(&tmp).unwrap();
        assert!(reject_unsafe_ancestor_metadata(&ordinary).is_ok());

        let link = tmp.join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink("target", &link).unwrap();
        #[cfg(windows)]
        if let Err(err) = std::os::windows::fs::symlink_dir("target", &link) {
            eprintln!("WINDOWS_FIXTURE_LIMITATION: symlink creation failed: {err}");
            let _ = std::fs::remove_dir_all(&tmp);
            return;
        }
        let link_meta = std::fs::symlink_metadata(&link).unwrap();
        assert!(link_meta.file_type().is_symlink());
        assert!(matches!(
            reject_unsafe_ancestor_metadata(&link_meta),
            Err(Error::FilesystemBoundaryUnsafe)
        ));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
