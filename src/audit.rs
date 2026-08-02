use crate::error::Error;
use crate::git::GitRunner;
use crate::implementation::{
    self, git_object_format_of, validate_git_root, validate_index_flags, validate_index_structure,
    validate_operation_state, validate_phase4_authority, validate_sparse_config,
    ValidatedAuthority,
};
use crate::state::ImplementationAuthority;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ============================================================================
// Reparse-point safety check (moved from path.rs to stay within authorized
// Phase 5 path boundary)
// ============================================================================

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

// ============================================================================
// Section 7: Audit Subject
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditSubject {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub phase_id: String,
    pub contract_id: String,
    pub contract_revision: u32,
    pub contract_source_path: String,
    pub contract_sha256: String,
    pub implementation_baseline_head: String,
    pub implementation_baseline_branch: String,
    pub git_object_format: String,
    pub current_head: String,
    pub current_branch: String,
    pub entries: Vec<AuditSubjectEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditSubjectEntry {
    pub path: String,
    pub baseline: Option<LayerRecord>,
    pub head: Option<LayerRecord>,
    pub index: Option<LayerRecord>,
    pub worktree: WorktreeRecord,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerRecord {
    pub mode: String,
    pub oid: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorktreeRecord {
    pub kind: String,
    pub sha256: Option<String>,
}

// ============================================================================
// Section 14: Audit Ledger
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditLedger {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub phase_id: String,
    pub contract_id: String,
    pub contract_revision: u32,
    pub contract_source_path: String,
    pub contract_sha256: String,
    pub implementation_baseline_head: String,
    pub implementation_baseline_branch: String,
    pub git_object_format: String,
    pub max_repair_attempts: u32,
    pub rounds: Vec<AuditRound>,
}

// ============================================================================
// Section 15: Audit Round Record
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditRound {
    pub round: u32,
    pub audit_id: String,
    pub auditor_id: String,
    pub subject_sha256: String,
    pub subject: AuditSubject,
    pub status: String,
    pub report_source_path: Option<String>,
    pub report_sha256: Option<String>,
    pub report_content: Option<String>,
    pub repair: Option<RepairRoute>,
}

// ============================================================================
// Section 16: Repair Route Record
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairRoute {
    pub attempt: u32,
    pub status: String,
    pub finding_ids: Vec<String>,
    pub allowed_paths: Vec<String>,
    pub pre_subject_sha256: String,
    pub post_subject_sha256: Option<String>,
    pub post_subject: Option<AuditSubject>,
    pub changed_paths: Vec<String>,
}

// ============================================================================
// Section 12: Audit Report
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditReport {
    pub schema_version: u32,
    pub audit_id: String,
    pub subject_sha256: String,
    pub auditor_id: String,
    pub independence_declaration: String,
    pub verdict: String,
    pub summary: String,
    pub requirement_results: Vec<RequirementResult>,
    pub verification_results: Vec<VerificationResult>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementResult {
    pub requirement: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationResult {
    pub command: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    pub severity: String,
    pub claim_kind: String,
    pub claim_index: u32,
    pub summary: String,
    pub evidence: String,
    pub repair_paths: Vec<String>,
}

// ============================================================================
// Lifecycle inference
// ============================================================================

pub fn infer_lifecycle(ledger: Option<&AuditLedger>) -> &'static str {
    match ledger {
        None => "NOT_STARTED",
        Some(l) => {
            let last = match l.rounds.last() {
                None => "NOT_STARTED",
                Some(r) => match r.status.as_str() {
                    "PASS" => "PASSED",
                    "FAIL" => {
                        if let Some(ref repair) = r.repair {
                            match repair.status.as_str() {
                                "ROUTED" => "REPAIR_ROUTED",
                                "CHECKED" => "REPAIR_CHECKED",
                                _ => "REPAIR_CHECKED",
                            }
                        } else {
                            // repair: null on a FAIL round means terminal
                            "FAILED_FINAL"
                        }
                    }
                    "PENDING" => "PENDING",
                    _ => "PENDING",
                },
            };
            // Cannot be not-started after first round publication
            if last == "NOT_STARTED" {
                "PENDING"
            } else {
                last
            }
        }
    }
}

fn count_checked_repairs(ledger: &AuditLedger) -> u32 {
    let mut count = 0u32;
    for round in &ledger.rounds {
        if let Some(ref repair) = round.repair {
            if repair.status == "CHECKED" {
                count += 1;
            }
        }
    }
    count
}

// ============================================================================
// Section 6: Auditor Identity Validation
// ============================================================================

pub fn validate_auditor_id(auditor_id: &str) -> Result<(), Error> {
    // Must be strict UTF-8 (already satisfied by &str)
    let bytes = auditor_id.as_bytes();
    if bytes.is_empty() || bytes.len() > 128 {
        return Err(Error::AuditorIdInvalid);
    }
    if auditor_id.trim() != auditor_id {
        return Err(Error::AuditorIdInvalid);
    }
    // Must begin with ASCII alphanumeric
    let first = bytes[0];
    if !first.is_ascii_alphanumeric() {
        return Err(Error::AuditorIdInvalid);
    }
    // Must contain only ASCII alphanumeric, '.', '_', '-', '@', ':'
    for &b in bytes {
        if !b.is_ascii_alphanumeric()
            && b != b'.'
            && b != b'_'
            && b != b'-'
            && b != b'@'
            && b != b':'
        {
            return Err(Error::AuditorIdInvalid);
        }
    }
    // No whitespace or control character
    for &b in bytes {
        if b.is_ascii_control() || b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            return Err(Error::AuditorIdInvalid);
        }
    }
    Ok(())
}

// ============================================================================
// Section 9: Subject Hash
// ============================================================================

pub fn compute_subject_sha256(subject: &AuditSubject) -> Result<String, Error> {
    // Serialize to compact JSON with exact field order, no trailing newline
    let json = serde_json::to_string(subject).map_err(|_| Error::AuditReportInvalid)?;
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

// ============================================================================
// Section 10: Audit ID
// ============================================================================

#[allow(clippy::too_many_arguments)]
pub fn compute_audit_id(
    accepted_plan_sha256: &str,
    phase_id: &str,
    contract_id: &str,
    contract_revision: u32,
    contract_sha256: &str,
    round: u32,
    subject_sha256: &str,
    auditor_id: &str,
) -> Result<String, Error> {
    // Build identity seed as compact JSON with exact field order.
    // Manual construction avoids serde_json map key ordering and ensures
    // contract_revision is included in the seed.
    let mut json = String::from('{');
    json.push_str("\"schema_version\":1,");
    json.push_str("\"accepted_plan_sha256\":");
    push_json_string(&mut json, accepted_plan_sha256);
    json.push(',');
    json.push_str("\"phase_id\":");
    push_json_string(&mut json, phase_id);
    json.push(',');
    json.push_str("\"contract_id\":");
    push_json_string(&mut json, contract_id);
    json.push(',');
    json.push_str("\"contract_revision\":");
    json.push_str(&contract_revision.to_string());
    json.push(',');
    json.push_str("\"contract_sha256\":");
    push_json_string(&mut json, contract_sha256);
    json.push(',');
    json.push_str("\"round\":");
    json.push_str(&round.to_string());
    json.push(',');
    json.push_str("\"subject_sha256\":");
    push_json_string(&mut json, subject_sha256);
    json.push(',');
    json.push_str("\"auditor_id\":");
    push_json_string(&mut json, auditor_id);
    json.push('}');
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

/// Push a JSON-escaped string value (with surrounding quotes) into `out`.
fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
}

// ============================================================================
// Audit Ledger Path
// ============================================================================

const AUDIT_LEDGER_FILENAME: &str = "audit-ledger.json";

fn audit_ledger_path(gov_dir: &Path) -> PathBuf {
    gov_dir.join(AUDIT_LEDGER_FILENAME)
}

pub(crate) fn read_audit_ledger(gov_dir: &Path) -> Result<Option<AuditLedger>, Error> {
    let path = audit_ledger_path(gov_dir);
    if !path.exists() {
        return Ok(None);
    }
    let meta = std::fs::symlink_metadata(&path).map_err(|_| Error::AuditLedgerInvalid)?;
    if !meta.file_type().is_file() {
        return Err(Error::AuditLedgerInvalid);
    }
    let bytes = std::fs::read(&path).map_err(|_| Error::AuditLedgerInvalid)?;
    let ledger: AuditLedger =
        serde_json::from_slice(&bytes).map_err(|_| Error::AuditLedgerInvalid)?;
    Ok(Some(ledger))
}

// ============================================================================
// Subject Construction (Section 7 + 8)
// ============================================================================

pub(crate) fn build_audit_subject(
    auth: &ValidatedAuthority,
    git: &GitRunner,
    impl_record: &ImplementationAuthority,
    change_paths: &BTreeSet<String>,
    _tracked_governance: &[String],
) -> Result<AuditSubject, Error> {
    let (current_head, current_branch, _objfmt) = validate_git_root(git)?;
    let objfmt = git_object_format_of(git)?;

    let mut entries = Vec::new();
    for path in change_paths {
        let entry = build_subject_entry(git, path, impl_record, &objfmt)?;
        entries.push(entry);
    }

    // Sort by path
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    // Deduplicate
    let mut seen = BTreeSet::new();
    entries.retain(|e| seen.insert(e.path.clone()));

    Ok(AuditSubject {
        schema_version: 1,
        accepted_plan_sha256: auth.accepted_plan_sha256.clone(),
        phase_id: auth.active_phase.clone(),
        contract_id: auth.contract_id.clone(),
        contract_revision: auth.final_revision,
        contract_source_path: auth.final_source_path.clone(),
        contract_sha256: auth.final_sha256.clone(),
        implementation_baseline_head: impl_record.baseline_head.clone(),
        implementation_baseline_branch: impl_record.baseline_branch.clone(),
        git_object_format: objfmt,
        current_head,
        current_branch,
        entries,
    })
}

fn build_subject_entry(
    git: &GitRunner,
    path: &str,
    impl_record: &ImplementationAuthority,
    objfmt: &str,
) -> Result<AuditSubjectEntry, Error> {
    // Baseline layer: tree entry at baseline commit
    let baseline = get_baseline_layer(git, &impl_record.baseline_head, path, objfmt)?;

    // HEAD layer: tree entry at current HEAD
    let head = get_head_layer(git, path, objfmt)?;

    // Index layer: stage-zero index entry
    let index = get_index_layer(git, path, objfmt)?;

    // Worktree layer: live file state
    let worktree = get_worktree_layer(git.repo_path(), path)?;

    Ok(AuditSubjectEntry {
        path: path.to_string(),
        baseline,
        head,
        index,
        worktree,
    })
}

fn parse_ls_tree_entry(entry: &[u8]) -> Result<Option<LayerRecord>, Error> {
    let tab = entry
        .iter()
        .position(|&b| b == b'\t')
        .ok_or(Error::AuditReportInvalid)?;
    let meta = &entry[..tab];
    let meta_str = std::str::from_utf8(meta).map_err(|_| Error::AuditReportInvalid)?;
    let mut parts = meta_str.splitn(3, ' ');
    let mode = parts.next().ok_or(Error::AuditReportInvalid)?;
    let _type = parts.next().ok_or(Error::AuditReportInvalid)?;
    let oid = parts.next().ok_or(Error::AuditReportInvalid)?;
    Ok(Some(LayerRecord {
        mode: mode.to_string(),
        oid: oid.to_string(),
    }))
}

fn get_baseline_layer(
    git: &GitRunner,
    baseline_head: &str,
    path: &str,
    _objfmt: &str,
) -> Result<Option<LayerRecord>, Error> {
    let out = git.run(["ls-tree", "-z", baseline_head, "--", path])?;
    if !out.status.success() {
        return Err(Error::GitCommandFailed("ls-tree baseline failed".into()));
    }
    if out.stdout.is_empty() {
        return Ok(None);
    }
    let nul = out
        .stdout
        .iter()
        .position(|&b| b == 0)
        .ok_or(Error::AuditReportInvalid)?;
    let entry = &out.stdout[..nul];
    parse_ls_tree_entry(entry)
}

fn get_head_layer(
    git: &GitRunner,
    path: &str,
    _objfmt: &str,
) -> Result<Option<LayerRecord>, Error> {
    let out = git.run(["ls-tree", "-z", "HEAD", "--", path])?;
    if !out.status.success() {
        return Err(Error::GitCommandFailed("ls-tree HEAD failed".into()));
    }
    if out.stdout.is_empty() {
        return Ok(None);
    }
    let nul = out
        .stdout
        .iter()
        .position(|&b| b == 0)
        .ok_or(Error::AuditReportInvalid)?;
    let entry = &out.stdout[..nul];
    parse_ls_tree_entry(entry)
}

fn get_index_layer(
    git: &GitRunner,
    path: &str,
    _objfmt: &str,
) -> Result<Option<LayerRecord>, Error> {
    let out = git.run(["ls-files", "--sparse", "--stage", "-z", "--", path])?;
    if !out.status.success() {
        return Err(Error::GitCommandFailed("ls-files stage failed".into()));
    }
    if out.stdout.is_empty() {
        return Ok(None);
    }
    let nul = out
        .stdout
        .iter()
        .position(|&b| b == 0)
        .ok_or(Error::AuditReportInvalid)?;
    let record = &out.stdout[..nul];
    let record_str = std::str::from_utf8(record).map_err(|_| Error::AuditReportInvalid)?;
    let space = record_str.find(' ').ok_or(Error::AuditReportInvalid)?;
    let mode = &record_str[..space];
    let rest = &record_str[space + 1..];
    let space2 = rest.find(' ').ok_or(Error::AuditReportInvalid)?;
    let oid = &rest[..space2];
    Ok(Some(LayerRecord {
        mode: mode.to_string(),
        oid: oid.to_string(),
    }))
}

fn get_worktree_layer(repo: &Path, path: &str) -> Result<WorktreeRecord, Error> {
    let full = repo.join(path);
    // Use symlink_metadata so symlinks are not followed
    match std::fs::symlink_metadata(&full) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                let target = std::fs::read_link(&full).map_err(|_| Error::AuditReportInvalid)?;
                let target_bytes = target
                    .to_str()
                    .ok_or(Error::AuditReportInvalid)?
                    .as_bytes()
                    .to_vec();
                let mut hasher = Sha256::new();
                hasher.update(&target_bytes);
                let sha = format!("{:x}", hasher.finalize());
                Ok(WorktreeRecord {
                    kind: "SYMLINK".to_string(),
                    sha256: Some(sha),
                })
            } else if meta.is_file() {
                let bytes = std::fs::read(&full).map_err(|_| Error::AuditReportInvalid)?;
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let sha = format!("{:x}", hasher.finalize());
                Ok(WorktreeRecord {
                    kind: "REGULAR".to_string(),
                    sha256: Some(sha),
                })
            } else {
                // Non-file, non-symlink existing filesystem object (directory,
                // junction, device, etc.) is an error, not ABSENT.
                Err(Error::AuditReportInvalid)
            }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(WorktreeRecord {
            kind: "ABSENT".to_string(),
            sha256: None,
        }),
        Err(_) => Err(Error::AuditReportInvalid),
    }
}

// ============================================================================
// Section 17: `audit begin`
// ============================================================================

pub fn cmd_audit_begin(repo_arg: &str, auditor_id: &str) -> Result<String, Error> {
    // 1. Validate auditor ID
    validate_auditor_id(auditor_id)?;

    // 2. Validate Phase 4 authority (includes common validation order)
    let auth = validate_phase4_authority(repo_arg)?;

    // 3. Require current Phase 4 implementation check success
    let git = GitRunner::new(&auth.repo);

    // Full Phase 4 check: validate git root, operations, index, sparse, impl authority
    let (_head, _branch, objfmt) = validate_git_root(&git)?;
    validate_operation_state(&git)?;
    validate_index_structure(&git, &objfmt)?;
    validate_index_flags(&git)?;
    validate_sparse_config(&git)?;

    // Require implementation authority exists
    let impl_path = match implementation::validate_impl_authority_file(&auth.gov_dir)? {
        Some(p) => p,
        None => return Err(Error::ImplementationAuthorityMissing),
    };
    let record: ImplementationAuthority = serde_json::from_slice(
        &std::fs::read(&impl_path).map_err(|_| Error::ImplementationAuthorityInvalid)?,
    )
    .map_err(|_| Error::ImplementationAuthorityInvalid)?;
    implementation::validate_impl_record_structure(&record, &objfmt)?;
    implementation::validate_impl_record_against_auth(&record, &auth)?;

    // Run check (baseline, ancestor, inventory)
    if record.git_object_format != objfmt {
        return Err(Error::ImplementationAuthorityStale);
    }
    let (_, current_branch, _) = validate_git_root(&git)?;
    if current_branch != record.baseline_branch {
        return Err(Error::BaselineBranchChanged);
    }
    let ancestor = git.run(["merge-base", "--is-ancestor", &record.baseline_head, "HEAD"])?;
    match ancestor.status.code() {
        Some(0) => {}
        Some(1) => return Err(Error::BaselineHistoryChanged),
        _ => return Err(Error::GitCommandFailed("merge-base failed".into())),
    }

    // Build change inventory
    let tracked_gov = implementation::tracked_governance_paths(&git, &objfmt)?;
    let change_paths =
        implementation::build_change_inventory(&git, &record, &auth, &objfmt, &tracked_gov)?;

    // 4. Build the exact current audit subject
    let subject = build_audit_subject(&auth, &git, &record, &change_paths, &tracked_gov)?;

    // 5. Compute subject hash
    let subject_sha256 = compute_subject_sha256(&subject)?;

    // 6. Validate existing audit ledger if present
    let mut ledger = match read_audit_ledger(&auth.gov_dir)? {
        Some(l) => {
            validate_ledger_authority(&l, &auth, &record)?;
            validate_ledger_history(&l, &auth)?;
            l
        }
        None => create_new_ledger(&auth, &record, &objfmt)?,
    };

    // 7. Enforce lifecycle preconditions
    let lifecycle = infer_lifecycle(Some(&ledger));
    match lifecycle {
        "PASSED" | "FAILED_FINAL" => return Err(Error::AuditTerminal),
        _ => {}
    }

    // 8. Idempotent begin
    if let Some(last_round) = ledger.rounds.last() {
        if last_round.status == "PENDING" {
            if last_round.auditor_id == auditor_id && last_round.subject_sha256 == subject_sha256 {
                // Idempotent: return same output without writing
                return Ok(format!(
                    "AUDIT_OPEN {} {} {}",
                    last_round.audit_id, last_round.round, subject_sha256
                ));
            } else {
                return Err(Error::AuditPendingConflict);
            }
        }
    }

    // 9. Compute next round and audit ID
    let next_round = ledger.rounds.len() as u32 + 1;
    let audit_id = compute_audit_id(
        &ledger.accepted_plan_sha256,
        &ledger.phase_id,
        &ledger.contract_id,
        ledger.contract_revision,
        &ledger.contract_sha256,
        next_round,
        &subject_sha256,
        auditor_id,
    )?;

    // 10. Create PENDING round
    let round = AuditRound {
        round: next_round,
        audit_id: audit_id.clone(),
        auditor_id: auditor_id.to_string(),
        subject_sha256: subject_sha256.clone(),
        subject,
        status: "PENDING".to_string(),
        report_source_path: None,
        report_sha256: None,
        report_content: None,
        repair: None,
    };
    ledger.rounds.push(round);

    // 11. Atomic write
    atomic_write_ledger(&auth.gov_dir, &ledger)?;

    Ok(format!(
        "AUDIT_OPEN {} {} {}",
        audit_id, next_round, subject_sha256
    ))
}

// ============================================================================
// Phase 9 revision 3: per-repository `audit record` coordination
//
// Concurrent `audit record` callers must not commit conflicting payloads
// for the same pending round from the same stale ledger preimage. A kernel
// coordination primitive serializes callers per canonical repository across
// the whole ledger read/validate/transition/publish interval. The primitive
// creates no durable filesystem artifact (no lock file, no coordination
// file, nothing in the repository, `.git`, another repository, or an
// external temporary directory) and is released automatically by the
// operating system when a participating process exits or crashes, so no
// stale permanent lock state can accumulate. Git remains read-only.
// ============================================================================

/// RAII guard held for the duration of the coordinated audit-record
/// interval (ledger read through durable publication).
struct AuditRecordCoordinationGuard {
    #[cfg(windows)]
    handle: *mut std::ffi::c_void,
    #[cfg(unix)]
    _file: std::fs::File,
}

#[cfg(windows)]
mod audit_coordination_ffi {
    extern "system" {
        pub fn CreateMutexW(
            lpMutexAttributes: *mut std::ffi::c_void,
            bInitialOwner: i32,
            lpName: *const u16,
        ) -> *mut std::ffi::c_void;
        pub fn WaitForSingleObject(hHandle: *mut std::ffi::c_void, dwMilliseconds: u32) -> u32;
        pub fn ReleaseMutex(hMutex: *mut std::ffi::c_void) -> i32;
        pub fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
    }
}

/// Acquire the per-repository coordination for `audit record`.
///
/// Windows: OS-wide named mutex derived from the canonical repository path
/// (case-folded so equivalent canonical references resolve to the same
/// name). The kernel object leaves no filesystem artifact and is destroyed
/// when the last handle closes; a crashed holder makes the mutex abandoned
/// and the next waiter acquires it (WAIT_ABANDONED).
///
/// Unix: `flock(2)`-style advisory lock (`std::fs::File::lock`) on the
/// canonical repository directory itself. No file is created or written;
/// the kernel releases the lock when the process exits or crashes.
///
/// Every acquisition failure maps to the existing publication-safety
/// category PERSISTENCE_FAILED (never an authority-read category).
fn acquire_audit_record_coordination(repo: &Path) -> Result<AuditRecordCoordinationGuard, Error> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        const WAIT_OBJECT_0: u32 = 0;
        const WAIT_ABANDONED: u32 = 0x0000_0080;
        const INFINITE: u32 = 0xFFFF_FFFF;

        let mut hasher = Sha256::new();
        // Windows paths are case-insensitive: fold the canonical identity so
        // case-alias references to the same repository coordinate together.
        let identity: Vec<u8> = repo
            .as_os_str()
            .as_encoded_bytes()
            .iter()
            .map(|b| b.to_ascii_lowercase())
            .collect();
        hasher.update(&identity);
        let digest = format!("{:x}", hasher.finalize());
        // OS-wide namespace: coordinates processes accessing the same
        // repository from any session. No filesystem artifact. The distinct
        // name keeps this inert outside the audit-record mutation.
        let name_str = format!("Global\\MRGS-AUDIT-RECORD-{}", digest);
        let name: Vec<u16> = std::ffi::OsStr::new(&name_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle =
            unsafe { audit_coordination_ffi::CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(Error::PersistenceFailed);
        }
        let wait = unsafe { audit_coordination_ffi::WaitForSingleObject(handle, INFINITE) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            unsafe {
                audit_coordination_ffi::CloseHandle(handle);
            }
            return Err(Error::PersistenceFailed);
        }
        Ok(AuditRecordCoordinationGuard { handle })
    }
    #[cfg(unix)]
    {
        // Advisory lock on the repository root directory itself; no file is
        // created and nothing is written (Git remains read-only).
        let file = std::fs::File::open(repo).map_err(|_| Error::PersistenceFailed)?;
        file.lock().map_err(|_| Error::PersistenceFailed)?;
        Ok(AuditRecordCoordinationGuard { _file: file })
    }
}

#[cfg(windows)]
impl Drop for AuditRecordCoordinationGuard {
    fn drop(&mut self) {
        unsafe {
            audit_coordination_ffi::ReleaseMutex(self.handle);
            audit_coordination_ffi::CloseHandle(self.handle);
        }
    }
}

#[cfg(unix)]
impl Drop for AuditRecordCoordinationGuard {
    fn drop(&mut self) {
        let _ = self._file.unlock();
    }
}

// ============================================================================
// Section 18: `audit record`
// ============================================================================

pub fn cmd_audit_record(repo_arg: &str, report_arg: &str) -> Result<String, Error> {
    // 1. Validate Phase 4 authority
    let auth = validate_phase4_authority(repo_arg)?;

    // 2. Load implementation authority record for ledger validation
    let impl_path = match implementation::validate_impl_authority_file(&auth.gov_dir)? {
        Some(p) => p,
        None => return Err(Error::ImplementationAuthorityMissing),
    };
    let record: ImplementationAuthority = serde_json::from_slice(
        &std::fs::read(&impl_path).map_err(|_| Error::ImplementationAuthorityInvalid)?,
    )
    .map_err(|_| Error::ImplementationAuthorityInvalid)?;

    // Phase 9 revision 3: acquire the per-repository coordination guard
    // BEFORE the first ledger read. The guard is held through validation,
    // replay/conflict/stale classification, ledger construction, and durable
    // publication, so no conflicting writer can commit from the same stale
    // preimage. It is released on every return path (including errors) and
    // by the operating system if this process exits or crashes.
    let _audit_record_coordination = acquire_audit_record_coordination(&auth.repo)?;

    // 3. Require existing final PENDING round
    let mut ledger = match read_audit_ledger(&auth.gov_dir)? {
        Some(l) => l,
        None => return Err(Error::AuditLedgerMissing),
    };

    validate_ledger_authority(&ledger, &auth, &record)?;
    validate_ledger_history(&ledger, &auth)?;

    let lifecycle = infer_lifecycle(Some(&ledger));

    // Section 18.4: Idempotent record
    // If the lifecycle is already terminal (PASSED or FAILED_FINAL), check
    // if the same report was already recorded for the final round. If so,
    // return the same output without writing.
    if lifecycle == "PASSED" || lifecycle == "FAILED_FINAL" {
        let last_round = ledger.rounds.last().ok_or(Error::AuditNotPending)?;
        if let (Some(_stored_path), Some(ref stored_sha), Some(ref stored_content)) = (
            &last_round.report_source_path,
            &last_round.report_sha256,
            &last_round.report_content,
        ) {
            // Read and hash the new report
            let report_path_check = Path::new(report_arg);
            if !report_path_check.exists() {
                return Err(Error::AuditReportInvalid);
            }
            let meta_check = std::fs::symlink_metadata(report_path_check)
                .map_err(|_| Error::AuditReportInvalid)?;
            if !meta_check.file_type().is_file() {
                return Err(Error::AuditReportInvalid);
            }
            let report_bytes_check =
                std::fs::read(report_path_check).map_err(|_| Error::AuditReportInvalid)?;
            let mut hasher_check = Sha256::new();
            hasher_check.update(&report_bytes_check);
            let report_sha256_check = format!("{:x}", hasher_check.finalize());

            if report_sha256_check == *stored_sha
                && report_bytes_check.to_vec() == stored_content.as_bytes()
            {
                // Idempotent: same report, same transition
                if last_round.status == "PASS" {
                    return Ok(format!(
                        "AUDIT_PASS {} {} {}",
                        last_round.audit_id, last_round.round, last_round.subject_sha256
                    ));
                } else {
                    // FAIL terminal
                    return Ok(format!(
                        "AUDIT_FAIL_FINAL {} {} {}",
                        last_round.audit_id, last_round.round, last_round.subject_sha256
                    ));
                }
            } else {
                // Different report after terminal
                return Err(Error::AuditReportConflict);
            }
        }
        return Err(Error::AuditNotPending);
    }

    // Phase 9 revision 3: idempotent replay of a FAIL record whose repair
    // is already ROUTED. When a conflicting FAIL payload won the round, an
    // exactly identical caller must return the exact REPAIR_ROUTED result
    // the winner returned (byte-identical stdout, no write); a different
    // report stays rejected with the existing stale category exactly as
    // before. This is replay classification inside the coordinated
    // interval, required for obligation 38's identical-winner-payload
    // assertion.
    if lifecycle == "REPAIR_ROUTED" {
        let last_round = ledger.rounds.last().ok_or(Error::AuditNotPending)?;
        if let (Some(_stored_path), Some(stored_sha), Some(stored_content)) = (
            &last_round.report_source_path,
            &last_round.report_sha256,
            &last_round.report_content,
        ) {
            // Read and hash the new report
            let report_path_check = Path::new(report_arg);
            if !report_path_check.exists() {
                return Err(Error::AuditNotPending);
            }
            let meta_check =
                std::fs::symlink_metadata(report_path_check).map_err(|_| Error::AuditNotPending)?;
            if !meta_check.file_type().is_file() {
                return Err(Error::AuditNotPending);
            }
            let report_bytes_check =
                std::fs::read(report_path_check).map_err(|_| Error::AuditNotPending)?;
            let mut hasher_check = Sha256::new();
            hasher_check.update(&report_bytes_check);
            let report_sha256_check = format!("{:x}", hasher_check.finalize());

            if report_sha256_check == stored_sha.as_str()
                && report_bytes_check.to_vec() == stored_content.as_bytes()
            {
                let repair = last_round.repair.as_ref().ok_or(Error::AuditNotPending)?;
                if repair.status == "ROUTED" {
                    // Idempotent: same report, same repair route
                    return Ok(format!(
                        "REPAIR_ROUTED {} {} {} {}",
                        last_round.audit_id,
                        last_round.round,
                        repair.attempt,
                        repair.allowed_paths.len()
                    ));
                }
            }
        }
        return Err(Error::AuditNotPending);
    }

    if lifecycle != "PENDING" {
        return Err(Error::AuditNotPending);
    }

    // 3. Recompute current audit subject
    let git = GitRunner::new(&auth.repo);
    let (_head, _branch, objfmt) = validate_git_root(&git)?;
    validate_operation_state(&git)?;
    validate_index_structure(&git, &objfmt)?;
    validate_index_flags(&git)?;
    validate_sparse_config(&git)?;

    implementation::validate_impl_record_structure(&record, &objfmt)?;
    implementation::validate_impl_record_against_auth(&record, &auth)?;

    let tracked_gov = implementation::tracked_governance_paths(&git, &objfmt)?;
    let change_paths =
        implementation::build_change_inventory(&git, &record, &auth, &objfmt, &tracked_gov)?;

    let subject = build_audit_subject(&auth, &git, &record, &change_paths, &tracked_gov)?;
    let subject_sha256 = compute_subject_sha256(&subject)?;

    // 4. Require exact equality with pending subject and hash
    let pending_round = ledger.rounds.last().ok_or(Error::AuditNotPending)?;
    if subject_sha256 != pending_round.subject_sha256 {
        return Err(Error::AuditSubjectStale);
    }

    // 5. Read and hash report source
    let report_path = Path::new(report_arg);
    if !report_path.exists() {
        return Err(Error::AuditReportInvalid);
    }
    let meta = std::fs::symlink_metadata(report_path).map_err(|_| Error::AuditReportInvalid)?;
    if !meta.file_type().is_file() {
        return Err(Error::AuditReportInvalid);
    }
    // Reject junction/unsafe reparse point (Phase 4 boundary)
    if is_reparse_point(&meta) {
        return Err(Error::AuditReportInvalid);
    }
    let report_bytes = std::fs::read(report_path).map_err(|_| Error::AuditReportInvalid)?;
    let mut hasher = Sha256::new();
    hasher.update(&report_bytes);
    let report_sha256 = format!("{:x}", hasher.finalize());

    // 6. Parse and validate report
    let report: AuditReport =
        serde_json::from_slice(&report_bytes).map_err(|_| Error::AuditReportInvalid)?;

    // 7. Validate matching audit ID, subject hash, auditor ID
    let pending_round = ledger.rounds.last().ok_or(Error::AuditNotPending)?;
    if report.audit_id != pending_round.audit_id {
        return Err(Error::AuditReportMismatch);
    }
    if report.subject_sha256 != pending_round.subject_sha256 {
        return Err(Error::AuditReportMismatch);
    }
    if report.auditor_id != pending_round.auditor_id {
        return Err(Error::AuditReportMismatch);
    }

    // 8. Validate independence declaration
    if report.independence_declaration != "INDEPENDENT" {
        return Err(Error::AuditReportInvalid);
    }

    // 9. Validate verdict consistency
    validate_verdict_consistency(&report)?;

    // 10. Validate completeness of requirement/verification coverage
    validate_report_coverage(&report, &auth)?;

    // 11. Validate report schema
    validate_report_schema(&report)?;

    // 11a. Validate repair_paths are permitted by accepted authority rule set
    for finding in &report.findings {
        for rp in &finding.repair_paths {
            auth.rule_set
                .evaluate(rp)
                .map_err(|_| Error::AuditReportInvalid)?;
        }
    }

    // 12. Transition the final pending round
    let checked_count = count_checked_repairs(&ledger);
    let last_idx = ledger.rounds.len() - 1;
    match report.verdict.as_str() {
        "PASS" => {
            ledger.rounds[last_idx].status = "PASS".to_string();
            ledger.rounds[last_idx].report_source_path = Some(
                std::fs::canonicalize(report_path)
                    .map_err(|_| Error::AuditReportInvalid)?
                    .to_str()
                    .ok_or(Error::AuditReportInvalid)?
                    .replace('\\', "/"),
            );
            ledger.rounds[last_idx].report_sha256 = Some(report_sha256);
            ledger.rounds[last_idx].report_content =
                Some(String::from_utf8(report_bytes).map_err(|_| Error::AuditReportInvalid)?);
            ledger.rounds[last_idx].repair = None;

            // 13. Atomic replace
            atomic_write_ledger(&auth.gov_dir, &ledger)?;

            Ok(format!(
                "AUDIT_PASS {} {} {}",
                ledger.rounds[last_idx].audit_id, ledger.rounds[last_idx].round, subject_sha256
            ))
        }
        "FAIL" => {
            ledger.rounds[last_idx].status = "FAIL".to_string();
            ledger.rounds[last_idx].report_source_path = Some(
                std::fs::canonicalize(report_path)
                    .map_err(|_| Error::AuditReportInvalid)?
                    .to_str()
                    .ok_or(Error::AuditReportInvalid)?
                    .replace('\\', "/"),
            );
            ledger.rounds[last_idx].report_sha256 = Some(report_sha256.clone());
            ledger.rounds[last_idx].report_content =
                Some(String::from_utf8(report_bytes).map_err(|_| Error::AuditReportInvalid)?);

            if checked_count < 2 {
                // Create repair route
                let attempt = checked_count + 1;
                let finding_ids: Vec<String> =
                    report.findings.iter().map(|f| f.id.clone()).collect();
                let mut allowed_paths: Vec<String> = report
                    .findings
                    .iter()
                    .flat_map(|f| f.repair_paths.iter().cloned())
                    .collect();
                allowed_paths.sort();
                allowed_paths.dedup();

                ledger.rounds[last_idx].repair = Some(RepairRoute {
                    attempt,
                    status: "ROUTED".to_string(),
                    finding_ids,
                    allowed_paths: allowed_paths.clone(),
                    pre_subject_sha256: subject_sha256.clone(),
                    post_subject_sha256: None,
                    post_subject: None,
                    changed_paths: vec![],
                });

                atomic_write_ledger(&auth.gov_dir, &ledger)?;

                Ok(format!(
                    "REPAIR_ROUTED {} {} {} {}",
                    ledger.rounds[last_idx].audit_id,
                    ledger.rounds[last_idx].round,
                    attempt,
                    allowed_paths.len()
                ))
            } else {
                // Terminal FAIL
                ledger.rounds[last_idx].repair = None;

                atomic_write_ledger(&auth.gov_dir, &ledger)?;

                Ok(format!(
                    "AUDIT_FAIL_FINAL {} {} {}",
                    ledger.rounds[last_idx].audit_id, ledger.rounds[last_idx].round, subject_sha256
                ))
            }
        }
        _ => Err(Error::AuditReportInvalid),
    }
}

// ============================================================================
// Section 20: `repair check`
// ============================================================================

pub fn cmd_repair_check(repo_arg: &str) -> Result<String, Error> {
    // 1. Validate Phase 4 authority
    let auth = validate_phase4_authority(repo_arg)?;

    // 1a. Load implementation authority record for ledger validation
    let impl_path_early = match implementation::validate_impl_authority_file(&auth.gov_dir)? {
        Some(p) => p,
        None => return Err(Error::ImplementationAuthorityMissing),
    };
    let record_early: ImplementationAuthority = serde_json::from_slice(
        &std::fs::read(&impl_path_early).map_err(|_| Error::ImplementationAuthorityInvalid)?,
    )
    .map_err(|_| Error::ImplementationAuthorityInvalid)?;

    // 2. Require final round FAIL with ROUTED repair
    let mut ledger = match read_audit_ledger(&auth.gov_dir)? {
        Some(l) => l,
        None => return Err(Error::AuditLedgerMissing),
    };

    validate_ledger_authority(&ledger, &auth, &record_early)?;
    validate_ledger_history(&ledger, &auth)?;

    let lifecycle = infer_lifecycle(Some(&ledger));

    // Section 20.1: Idempotent repair check
    // When the final repair is already CHECKED, check for idempotency or drift.
    if lifecycle == "REPAIR_CHECKED" {
        let last_idx = ledger.rounds.len() - 1;
        let r = &ledger.rounds[last_idx];
        let repair = r.repair.as_ref().ok_or(Error::RepairNotRouted)?;
        let post_sha = repair
            .post_subject_sha256
            .clone()
            .ok_or(Error::RepairSubjectStale)?;
        let attempt = repair.attempt;

        // Recompute subject
        let git = GitRunner::new(&auth.repo);
        let (_head, _branch, objfmt) = validate_git_root(&git)?;
        validate_operation_state(&git)?;
        validate_index_structure(&git, &objfmt)?;
        validate_index_flags(&git)?;
        validate_sparse_config(&git)?;

        let impl_path = match implementation::validate_impl_authority_file(&auth.gov_dir)? {
            Some(p) => p,
            None => return Err(Error::ImplementationAuthorityMissing),
        };
        let record: ImplementationAuthority = serde_json::from_slice(
            &std::fs::read(&impl_path).map_err(|_| Error::ImplementationAuthorityInvalid)?,
        )
        .map_err(|_| Error::ImplementationAuthorityInvalid)?;
        implementation::validate_impl_record_structure(&record, &objfmt)?;
        implementation::validate_impl_record_against_auth(&record, &auth)?;

        let tracked_gov = implementation::tracked_governance_paths(&git, &objfmt)?;
        let change_paths =
            implementation::build_change_inventory(&git, &record, &auth, &objfmt, &tracked_gov)?;
        let subject = build_audit_subject(&auth, &git, &record, &change_paths, &tracked_gov)?;
        let current_sha = compute_subject_sha256(&subject)?;

        if current_sha == post_sha {
            // Idempotent: same post subject
            return Ok(format!(
                "REPAIR_OK {} {} {} {} {}",
                r.audit_id,
                r.round,
                attempt,
                post_sha,
                repair.changed_paths.len()
            ));
        } else {
            // Drift after checked repair
            return Err(Error::RepairSubjectStale);
        }
    }

    if lifecycle != "REPAIR_ROUTED" {
        return Err(Error::RepairNotRouted);
    }

    // Extract what we need from the mutable round before releasing the borrow
    let (pre_subject_sha256, allowed_paths, round_idx, repair_attempt, pre_subject_entries) = {
        let last_idx = ledger.rounds.len() - 1;
        let r = &ledger.rounds[last_idx];
        let repair = r.repair.as_ref().ok_or(Error::RepairNotRouted)?;
        if repair.status != "ROUTED" {
            return Err(Error::RepairNotRouted);
        }
        (
            repair.pre_subject_sha256.clone(),
            repair.allowed_paths.clone(),
            last_idx,
            repair.attempt,
            r.subject.entries.clone(),
        )
    };

    // 3. Require unchanged authority, baseline, branch, HEAD
    let git = GitRunner::new(&auth.repo);
    let (_head, _branch, objfmt) = validate_git_root(&git)?;
    validate_operation_state(&git)?;
    validate_index_structure(&git, &objfmt)?;
    validate_index_flags(&git)?;
    validate_sparse_config(&git)?;

    let impl_path = match implementation::validate_impl_authority_file(&auth.gov_dir)? {
        Some(p) => p,
        None => return Err(Error::ImplementationAuthorityMissing),
    };
    let record: ImplementationAuthority = serde_json::from_slice(
        &std::fs::read(&impl_path).map_err(|_| Error::ImplementationAuthorityInvalid)?,
    )
    .map_err(|_| Error::ImplementationAuthorityInvalid)?;
    implementation::validate_impl_record_structure(&record, &objfmt)?;
    implementation::validate_impl_record_against_auth(&record, &auth)?;

    if record.git_object_format != objfmt {
        return Err(Error::ImplementationAuthorityStale);
    }
    let (_, current_branch, _) = validate_git_root(&git)?;
    if current_branch != record.baseline_branch {
        return Err(Error::BaselineBranchChanged);
    }
    let baselines_out = git.run([
        "rev-parse",
        "--verify",
        &format!("{}^{{commit}}", record.baseline_head),
    ])?;
    if !baselines_out.status.success() {
        return Err(Error::BaselineCommitMissing);
    }
    let ancestor = git.run(["merge-base", "--is-ancestor", &record.baseline_head, "HEAD"])?;
    match ancestor.status.code() {
        Some(0) => {}
        Some(1) => return Err(Error::BaselineHistoryChanged),
        _ => return Err(Error::GitCommandFailed("merge-base failed".into())),
    }

    // 4. Recompute current audit subject
    let tracked_gov = implementation::tracked_governance_paths(&git, &objfmt)?;
    let change_paths =
        implementation::build_change_inventory(&git, &record, &auth, &objfmt, &tracked_gov)?;
    let subject = build_audit_subject(&auth, &git, &record, &change_paths, &tracked_gov)?;
    let post_subject_sha256 = compute_subject_sha256(&subject)?;

    // 5. Compare with pre-repair subject
    if post_subject_sha256 == pre_subject_sha256 {
        return Err(Error::RepairNoChange);
    }

    // 6. Derive sorted unique subject-entry delta
    let changed_paths = compute_delta(&pre_subject_entries, &subject.entries);

    if changed_paths.is_empty() {
        return Err(Error::RepairNoChange);
    }

    // 7. Every changed path in allowed_paths
    for cp in &changed_paths {
        if !allowed_paths.contains(cp) {
            return Err(Error::RepairScopeViolation);
        }
    }

    // 8. Every finding has at least one changed path
    // Load the report to check finding repair_paths coverage
    if let Some(ref report_content) = ledger.rounds[round_idx].report_content {
        let report: AuditReport =
            serde_json::from_str(report_content).map_err(|_| Error::AuditReportInvalid)?;
        for finding in &report.findings {
            let has_intersection = finding
                .repair_paths
                .iter()
                .any(|rp| changed_paths.contains(rp));
            if !has_intersection {
                return Err(Error::RepairScopeViolation);
            }
        }
    }

    // 9. Require Phase 4 implementation check success
    let ancestor2 = git.run(["merge-base", "--is-ancestor", &record.baseline_head, "HEAD"])?;
    match ancestor2.status.code() {
        Some(0) => {}
        _ => return Err(Error::BaselineHistoryChanged),
    }

    // 10. Set repair to CHECKED
    {
        let repair = ledger.rounds[round_idx].repair.as_mut().unwrap();
        repair.status = "CHECKED".to_string();
        repair.post_subject_sha256 = Some(post_subject_sha256.clone());
        repair.post_subject = Some(subject.clone());
        repair.changed_paths = changed_paths.clone();
    }

    // 11. Atomic replace
    atomic_write_ledger(&auth.gov_dir, &ledger)?;

    Ok(format!(
        "REPAIR_OK {} {} {} {} {}",
        ledger.rounds[round_idx].audit_id,
        ledger.rounds[round_idx].round,
        repair_attempt,
        post_subject_sha256,
        changed_paths.len()
    ))
}

// ============================================================================
// Subject Delta (Section 21)
// ============================================================================

fn compute_delta(
    pre_entries: &[AuditSubjectEntry],
    post_entries: &[AuditSubjectEntry],
) -> Vec<String> {
    let mut all_paths = BTreeSet::new();
    for e in pre_entries {
        all_paths.insert(e.path.clone());
    }
    for e in post_entries {
        all_paths.insert(e.path.clone());
    }

    let mut changed = Vec::new();
    for path in &all_paths {
        let pre = pre_entries.iter().find(|e| &e.path == path);
        let post = post_entries.iter().find(|e| &e.path == path);
        match (pre, post) {
            (None, Some(_)) | (Some(_), None) => changed.push(path.clone()),
            (Some(a), Some(b)) => {
                if entry_changed(a, b) {
                    changed.push(path.clone());
                }
            }
            (None, None) => unreachable!(),
        }
    }
    changed
}

fn entry_changed(a: &AuditSubjectEntry, b: &AuditSubjectEntry) -> bool {
    a.baseline != b.baseline
        || a.head != b.head
        || a.index != b.index
        || a.worktree.kind != b.worktree.kind
        || a.worktree.sha256 != b.worktree.sha256
}

// ============================================================================
// Ledger Validation
// ============================================================================

fn validate_ledger_authority(
    ledger: &AuditLedger,
    auth: &ValidatedAuthority,
    record: &ImplementationAuthority,
) -> Result<(), Error> {
    if ledger.schema_version != 1 {
        return Err(Error::AuditLedgerInvalid);
    }
    if ledger.accepted_plan_sha256 != auth.accepted_plan_sha256 {
        return Err(Error::AuditLedgerStale);
    }
    if ledger.phase_id != auth.active_phase {
        return Err(Error::AuditLedgerStale);
    }
    if ledger.contract_id != auth.contract_id {
        return Err(Error::AuditLedgerStale);
    }
    if ledger.contract_revision != auth.final_revision {
        return Err(Error::AuditLedgerStale);
    }
    if ledger.contract_source_path != auth.final_source_path {
        return Err(Error::AuditLedgerStale);
    }
    if ledger.contract_sha256 != auth.final_sha256 {
        return Err(Error::AuditLedgerStale);
    }
    if ledger.implementation_baseline_head != record.baseline_head {
        return Err(Error::AuditLedgerStale);
    }
    if ledger.implementation_baseline_branch != record.baseline_branch {
        return Err(Error::AuditLedgerStale);
    }
    if ledger.git_object_format != record.git_object_format {
        return Err(Error::AuditLedgerStale);
    }
    if ledger.max_repair_attempts != 2 {
        return Err(Error::AuditLedgerInvalid);
    }
    Ok(())
}

pub(crate) fn validate_ledger_history(
    ledger: &AuditLedger,
    auth: &ValidatedAuthority,
) -> Result<(), Error> {
    for (idx, round) in ledger.rounds.iter().enumerate() {
        let expected_round = (idx + 1) as u32;
        if round.round != expected_round {
            return Err(Error::AuditLedgerInvalid);
        }
        // Validate subject hash recomputation
        let computed_hash = compute_subject_sha256(&round.subject)?;
        if computed_hash != round.subject_sha256 {
            return Err(Error::AuditLedgerInvalid);
        }
        // Validate audit ID recomputation
        let computed_audit_id = compute_audit_id(
            &ledger.accepted_plan_sha256,
            &ledger.phase_id,
            &ledger.contract_id,
            ledger.contract_revision,
            &ledger.contract_sha256,
            round.round,
            &round.subject_sha256,
            &round.auditor_id,
        )?;
        if computed_audit_id != round.audit_id {
            return Err(Error::AuditLedgerInvalid);
        }
        // Validate no round follows PASS
        if idx > 0 {
            let prev = &ledger.rounds[idx - 1];
            if prev.status == "PASS" {
                return Err(Error::AuditTerminal);
            }
            if prev.status == "FAIL" {
                if let Some(ref repair) = prev.repair {
                    if repair.status != "CHECKED" {
                        return Err(Error::AuditLedgerInvalid);
                    }
                } else {
                    // Terminal FAIL without repair
                    let checked = count_checked_repairs_before(ledger, idx as u32 - 1);
                    if checked >= ledger.max_repair_attempts {
                        return Err(Error::AuditTerminal);
                    }
                }
            }
        }
        // Validate later subject equals prior checked post subject
        if idx > 0 {
            let prev = &ledger.rounds[idx - 1];
            if prev.status == "FAIL" {
                if let Some(ref repair) = prev.repair {
                    if repair.status == "CHECKED" {
                        if let Some(ref post_sha) = repair.post_subject_sha256 {
                            if *post_sha != round.subject_sha256 {
                                return Err(Error::AuditLedgerInvalid);
                            }
                        }
                    }
                }
            }
        }
        // Validate report SHA if present
        if let Some(ref report_sha) = round.report_sha256 {
            if let Some(ref report_content) = round.report_content {
                let mut hasher = Sha256::new();
                hasher.update(report_content.as_bytes());
                let computed = format!("{:x}", hasher.finalize());
                if computed != *report_sha {
                    return Err(Error::AuditLedgerInvalid);
                }
            }
        }
        // Revalidate stored report against round/contract
        if let Some(ref report_content) = round.report_content {
            let report: AuditReport =
                serde_json::from_str(report_content).map_err(|_| Error::AuditLedgerInvalid)?;
            if report.audit_id != round.audit_id {
                return Err(Error::AuditLedgerInvalid);
            }
            if report.subject_sha256 != round.subject_sha256 {
                return Err(Error::AuditLedgerInvalid);
            }
            if report.auditor_id != round.auditor_id {
                return Err(Error::AuditLedgerInvalid);
            }
            validate_report_schema(&report)?;
            validate_verdict_consistency(&report)?;
            validate_report_coverage(&report, auth)?;
        }
    }

    // Only the final round may be PENDING or contain a ROUTED unchecked repair
    if ledger.rounds.len() > 1 {
        for round in &ledger.rounds[..ledger.rounds.len() - 1] {
            if round.status == "PENDING" {
                return Err(Error::AuditLedgerInvalid);
            }
            if let Some(ref repair) = round.repair {
                if repair.status == "ROUTED" {
                    return Err(Error::AuditLedgerInvalid);
                }
            }
        }
    }

    // Repair attempts are contiguous 1, then 2, with no duplicates
    let mut repair_attempts = Vec::new();
    for round in &ledger.rounds {
        if let Some(ref repair) = round.repair {
            repair_attempts.push(repair.attempt);
        }
    }
    for (idx, &attempt) in repair_attempts.iter().enumerate() {
        if attempt != (idx as u32 + 1) {
            return Err(Error::AuditLedgerInvalid);
        }
    }
    if repair_attempts.len() > 2 {
        return Err(Error::AuditLedgerInvalid);
    }

    Ok(())
}

fn count_checked_repairs_before(ledger: &AuditLedger, before_idx: u32) -> u32 {
    let mut count = 0u32;
    for i in 0..(before_idx as usize).min(ledger.rounds.len()) {
        if let Some(ref repair) = ledger.rounds[i].repair {
            if repair.status == "CHECKED" {
                count += 1;
            }
        }
    }
    count
}

// ============================================================================
// Report Validation
// ============================================================================

fn validate_report_schema(report: &AuditReport) -> Result<(), Error> {
    if report.schema_version != 1 {
        return Err(Error::AuditReportInvalid);
    }
    if report.summary.trim().is_empty()
        || report.summary.contains('\0')
        || report.summary != report.summary.trim()
    {
        return Err(Error::AuditReportInvalid);
    }
    // Validate requirement results
    for rr in &report.requirement_results {
        if rr.status != "PASS" && rr.status != "FAIL" && rr.status != "BLOCKED" {
            return Err(Error::AuditReportInvalid);
        }
        if rr.evidence.trim().is_empty()
            || rr.evidence.contains('\0')
            || rr.evidence != rr.evidence.trim()
        {
            return Err(Error::AuditReportInvalid);
        }
    }
    // Validate verification results
    for vr in &report.verification_results {
        if vr.status != "PASS" && vr.status != "FAIL" && vr.status != "BLOCKED" {
            return Err(Error::AuditReportInvalid);
        }
        if vr.evidence.trim().is_empty()
            || vr.evidence.contains('\0')
            || vr.evidence != vr.evidence.trim()
        {
            return Err(Error::AuditReportInvalid);
        }
    }
    // Validate findings
    let mut finding_ids = BTreeSet::new();
    for finding in &report.findings {
        validate_finding_id(&finding.id)?;
        if finding.severity != "BLOCKER"
            && finding.severity != "MAJOR"
            && finding.severity != "MINOR"
        {
            return Err(Error::AuditReportInvalid);
        }
        if finding.claim_kind != "REQUIREMENT" && finding.claim_kind != "VERIFICATION" {
            return Err(Error::AuditReportInvalid);
        }
        if finding.claim_index < 1 {
            return Err(Error::AuditReportInvalid);
        }
        if finding.summary.trim().is_empty()
            || finding.summary.contains('\0')
            || finding.summary != finding.summary.trim()
        {
            return Err(Error::AuditReportInvalid);
        }
        if finding.evidence.trim().is_empty()
            || finding.evidence.contains('\0')
            || finding.evidence != finding.evidence.trim()
        {
            return Err(Error::AuditReportInvalid);
        }
        if finding.repair_paths.is_empty() {
            return Err(Error::AuditReportInvalid);
        }
        // Validate repair paths: unique, valid, and strictly sorted by raw UTF-8 bytes
        let mut seen_paths = BTreeSet::new();
        for rp in &finding.repair_paths {
            validate_repair_path(rp)?;
            if !seen_paths.insert(rp.clone()) {
                return Err(Error::AuditReportInvalid);
            }
        }
        // Strict sorted ascending check: each element must be > its predecessor by raw bytes
        for i in 1..finding.repair_paths.len() {
            if finding.repair_paths[i].as_bytes() <= finding.repair_paths[i - 1].as_bytes() {
                return Err(Error::AuditReportInvalid);
            }
        }
        if !finding_ids.insert(finding.id.clone()) {
            return Err(Error::AuditReportInvalid);
        }
    }
    Ok(())
}

fn validate_finding_id(id: &str) -> Result<(), Error> {
    if id.is_empty() || id.len() > 64 {
        return Err(Error::AuditReportInvalid);
    }
    if id.trim() != id {
        return Err(Error::AuditReportInvalid);
    }
    let first = id.as_bytes()[0];
    if !first.is_ascii_alphanumeric() {
        return Err(Error::AuditReportInvalid);
    }
    for &b in id.as_bytes() {
        if !b.is_ascii_alphanumeric() && b != b'.' && b != b'_' && b != b'-' {
            return Err(Error::AuditReportInvalid);
        }
    }
    Ok(())
}

fn validate_repair_path(path: &str) -> Result<(), Error> {
    if path.is_empty() {
        return Err(Error::AuditReportInvalid);
    }
    if path.starts_with('/') || path.starts_with("//") {
        return Err(Error::AuditReportInvalid);
    }
    if path.contains('\\') {
        return Err(Error::AuditReportInvalid);
    }
    if path.contains("//") {
        return Err(Error::AuditReportInvalid);
    }
    if path.ends_with('/') {
        return Err(Error::AuditReportInvalid);
    }
    // Reject Windows drive-prefix paths directly (C:/foo, C:\foo, D:relative)
    if path.len() >= 2 {
        let bytes = path.as_bytes();
        if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return Err(Error::AuditReportInvalid);
        }
    }
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(Error::AuditReportInvalid);
        }
    }
    if path.chars().any(|c| c.is_ascii_control()) {
        return Err(Error::AuditReportInvalid);
    }
    // Must not be .git or .mrgs
    let first_seg = path.split('/').next().unwrap_or("");
    if first_seg.eq_ignore_ascii_case(".git") || first_seg.eq_ignore_ascii_case(".mrgs") {
        return Err(Error::AuditReportInvalid);
    }
    // No wildcard/glob metacharacters
    if path.contains('*') || path.contains('?') || path.contains('[') || path.contains(']') {
        return Err(Error::AuditReportInvalid);
    }
    Ok(())
}

fn validate_verdict_consistency(report: &AuditReport) -> Result<(), Error> {
    match report.verdict.as_str() {
        "PASS" => {
            // Every requirement and verification must be PASS
            for rr in &report.requirement_results {
                if rr.status != "PASS" {
                    return Err(Error::AuditReportInvalid);
                }
            }
            for vr in &report.verification_results {
                if vr.status != "PASS" {
                    return Err(Error::AuditReportInvalid);
                }
            }
            // Findings must be empty
            if !report.findings.is_empty() {
                return Err(Error::AuditReportInvalid);
            }
        }
        "FAIL" => {
            // At least one non-PASS claim
            let has_non_pass = report
                .requirement_results
                .iter()
                .any(|r| r.status != "PASS")
                || report
                    .verification_results
                    .iter()
                    .any(|r| r.status != "PASS");
            if !has_non_pass {
                return Err(Error::AuditReportInvalid);
            }
            // Findings must be nonempty
            if report.findings.is_empty() {
                return Err(Error::AuditReportInvalid);
            }
            // Every finding references a non-PASS claim
            for finding in &report.findings {
                let idx = (finding.claim_index - 1) as usize;
                match finding.claim_kind.as_str() {
                    "REQUIREMENT" => {
                        let rr = report
                            .requirement_results
                            .get(idx)
                            .ok_or(Error::AuditReportInvalid)?;
                        if rr.status == "PASS" {
                            return Err(Error::AuditReportInvalid);
                        }
                    }
                    "VERIFICATION" => {
                        let vr = report
                            .verification_results
                            .get(idx)
                            .ok_or(Error::AuditReportInvalid)?;
                        if vr.status == "PASS" {
                            return Err(Error::AuditReportInvalid);
                        }
                    }
                    _ => return Err(Error::AuditReportInvalid),
                }
            }
            // Every non-PASS claim is referenced by at least one finding
            for (idx, rr) in report.requirement_results.iter().enumerate() {
                if rr.status != "PASS" {
                    let claim_idx = (idx + 1) as u32;
                    if !report
                        .findings
                        .iter()
                        .any(|f| f.claim_kind == "REQUIREMENT" && f.claim_index == claim_idx)
                    {
                        return Err(Error::AuditReportInvalid);
                    }
                }
            }
            for (idx, vr) in report.verification_results.iter().enumerate() {
                if vr.status != "PASS" {
                    let claim_idx = (idx + 1) as u32;
                    if !report
                        .findings
                        .iter()
                        .any(|f| f.claim_kind == "VERIFICATION" && f.claim_index == claim_idx)
                    {
                        return Err(Error::AuditReportInvalid);
                    }
                }
            }
        }
        _ => return Err(Error::AuditReportInvalid),
    }
    Ok(())
}

fn validate_report_coverage(report: &AuditReport, auth: &ValidatedAuthority) -> Result<(), Error> {
    // Parse accepted contract content to get requirements and verification commands
    let contract: crate::contract::Contract =
        toml::from_str(&auth.final_content).map_err(|_| Error::AuditReportInvalid)?;

    // Exactly one result per requirement, in order
    if report.requirement_results.len() != contract.requirements.len() {
        return Err(Error::AuditReportInvalid);
    }
    for (expected, actual) in contract
        .requirements
        .iter()
        .zip(report.requirement_results.iter())
    {
        if &actual.requirement != expected {
            return Err(Error::AuditReportInvalid);
        }
    }

    // Exactly one result per verification command, in order
    if report.verification_results.len() != contract.verification_commands.len() {
        return Err(Error::AuditReportInvalid);
    }
    for (expected, actual) in contract
        .verification_commands
        .iter()
        .zip(report.verification_results.iter())
    {
        if &actual.command != expected {
            return Err(Error::AuditReportInvalid);
        }
    }

    Ok(())
}

// ============================================================================
// Ledger Creation
// ============================================================================

fn create_new_ledger(
    auth: &ValidatedAuthority,
    record: &ImplementationAuthority,
    objfmt: &str,
) -> Result<AuditLedger, Error> {
    Ok(AuditLedger {
        schema_version: 1,
        accepted_plan_sha256: auth.accepted_plan_sha256.clone(),
        phase_id: auth.active_phase.clone(),
        contract_id: auth.contract_id.clone(),
        contract_revision: auth.final_revision,
        contract_source_path: auth.final_source_path.clone(),
        contract_sha256: auth.final_sha256.clone(),
        implementation_baseline_head: record.baseline_head.clone(),
        implementation_baseline_branch: record.baseline_branch.clone(),
        git_object_format: objfmt.to_string(),
        max_repair_attempts: 2,
        rounds: vec![],
    })
}

// ============================================================================
// Atomic Persistence
// ============================================================================

fn atomic_write_ledger(gov_dir: &Path, ledger: &AuditLedger) -> Result<(), Error> {
    // Serialize before opening any file
    let json = serde_json::to_string_pretty(ledger).map_err(|_| Error::PersistenceFailed)?;
    let json_bytes = json.as_bytes();

    let final_path = gov_dir.join(AUDIT_LEDGER_FILENAME);

    // Create unique temp file with no-clobber
    let mut tmp_path = None;
    for attempt in 0..16u64 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!(
            ".mrgs_audit_tmp_{}_{}_{}.tmp",
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
            Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(Error::PersistenceFailed),
        }
    }

    let tmp_path = tmp_path.ok_or(Error::PersistenceFailed)?;

    // Atomic replace
    #[cfg(windows)]
    {
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
        const MOVEFILE_REPLACE_EXISTING: u32 = 0x00000001;
        let src_wide: Vec<u16> = OsStr::new(&tmp_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let dst_wide: Vec<u16> = OsStr::new(&final_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            MoveFileExW(
                src_wide.as_ptr(),
                dst_wide.as_ptr(),
                MOVEFILE_REPLACE_EXISTING,
            )
        };
        if result == 0 {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(Error::PersistenceFailed);
        }
    }
    #[cfg(not(windows))]
    {
        if std::fs::rename(&tmp_path, &final_path).is_err() {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(Error::PersistenceFailed);
        }
    }

    Ok(())
}
