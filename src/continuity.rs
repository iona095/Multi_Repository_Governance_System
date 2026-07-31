//! Phase 7 — Model, Host, and Cross-Repository Continuity Metadata.
//!
//! Implements exactly one command: `mrgs continuity record`. It records
//! deterministic, privacy-minimal continuity metadata for completed phases,
//! with optional locally verified cross-repository predecessor links, in one
//! append-only `.mrgs/continuity-ledger.json`.
//!
//! Continuity metadata is descriptive evidence. It never replaces plan,
//! contract, implementation, audit, or completion authority, and it never
//! executes models, discovers providers, reads the network, or mutates Git.

use crate::closeout::{self, CompletionEntry, CompletionLedger, CompletionReceipt};
use crate::error::Error;
use crate::plan::Plan;
use crate::state::{self, GovernanceState};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// ============================================================================
// Public entry point
// ============================================================================

/// `continuity record --repo <REPO> --metadata <METADATA> [--source-repo <SRC>]...`
pub fn cmd_continuity_record(
    repo_arg: &str,
    metadata_arg: &str,
    source_repos: &[String],
) -> Result<String, Error> {
    // 2. Canonical target repository path.
    let repo_path = Path::new(repo_arg);
    crate::path::assert_existing_dir(repo_path)?;
    let repo = std::fs::canonicalize(repo_path).map_err(|_| Error::RepositoryInvalid)?;

    // 3. Safe target `.mrgs` directory.
    let gov_dir = crate::path::validate_gov_dir_exists(&repo)?;

    // 4. Accepted plan record, exact plan source bytes, plan SHA-256, and
    //    plan structure.
    let accepted = state::read_accepted_plan(&repo)?;
    state::validate_accepted_plan_record(&accepted)?;
    let plan_file = crate::path::resolve_safe_plan_path(&repo, &accepted.plan_path)?;
    let plan_bytes = std::fs::read(&plan_file)?;
    let plan_sha256 = sha256_hex(&plan_bytes);
    let plan_str = String::from_utf8(plan_bytes)?;
    let plan: Plan = toml::from_str(&plan_str)?;
    plan.validate()?;
    state::validate_plan_consistency(&accepted, &plan, &plan_sha256)?;

    // 5. State structure and accepted-plan relation.
    let gov_state = state::read_state(&repo)?;
    state::validate_state_record(&gov_state, &accepted, &plan)?;

    // 6. Completion-ledger topology, structure, hashes, chain, and state
    //    relation. The target must have a valid Phase 6 completion ledger;
    //    structure/hash/chain validation precedes the state relation so a
    //    malformed ledger fails with the completion-ledger category.
    let completion_ledger = closeout::read_completion_ledger(&gov_dir)?;
    let completion_ledger = match &completion_ledger {
        Some(ledger) => {
            closeout::validate_completion_ledger(ledger, &accepted.sha256, &plan.plan_id)?;
            ledger
        }
        None => {
            closeout::validate_state_ledger_relation(&gov_state, None)?;
            return Err(Error::CloseoutNotReady);
        }
    };
    closeout::validate_state_ledger_relation(&gov_state, Some(completion_ledger))?;

    // 7. Metadata source path and safe regular-file topology.
    let (metadata_bytes, normalized_path) =
        validate_metadata_source(&repo, &gov_dir, metadata_arg)?;

    // 8. Exact metadata bytes, UTF-8, TOML structure, strict fields, and
    //    scalar grammar.
    let metadata_sha256 = sha256_hex(&metadata_bytes);
    let metadata = parse_metadata(&metadata_bytes)?;

    // 10. Existing continuity-ledger topology and complete validation when
    //     present. A tracked ledger path is never a legal governance file.
    let git = crate::git::GitRunner::new(&repo);
    let tracked_out = git.run(["ls-files", "--", ".mrgs/continuity-ledger.json"])?;
    if !tracked_out.status.success() {
        return Err(Error::GitCommandFailed(
            "ls-files continuity-ledger failed".into(),
        ));
    }
    if !tracked_out.stdout.is_empty() {
        return Err(Error::GitInventoryInvalid);
    }
    let existing = read_continuity_ledger(&gov_dir)?;
    if let Some(ledger) = &existing {
        validate_continuity_ledger(ledger, &accepted.sha256, &plan.plan_id, completion_ledger)?;
    }

    // 11. Exact replay or conflict detection (contract section 22). When an
    //     existing entry carries the same continuity ID, every replay field
    //     must match the durable entry — including the target completion
    //     binding — or the attempt is a conflict. Ledger validation above
    //     guarantees the durable entry is intact before this comparison.
    if let Some(ledger) = &existing {
        if let Some(entry) = ledger
            .entries
            .iter()
            .find(|e| e.continuity_receipt.continuity_id == metadata.continuity_id)
        {
            if is_exact_replay(
                entry,
                &metadata,
                &normalized_path,
                &metadata_bytes,
                &metadata_sha256,
            ) {
                if !source_repos.is_empty() {
                    verify_replay_sources(source_repos, &repo, entry)?;
                }
                return Ok(output_for(ledger, entry));
            }
            return Err(Error::ContinuityConflict);
        }
    }

    // 9. Target completed-phase and completion-receipt binding for new
    //    records (contract section 11).
    let target_entry = bind_target_phase(&plan, &gov_state, completion_ledger, &metadata)?;

    // Ordering and identity conflicts for new entries (contract section 12).
    if let Some(ledger) = &existing {
        if ledger.repository_id != metadata.repository_id {
            return Err(Error::ContinuityConflict);
        }
        if ledger
            .entries
            .iter()
            .any(|e| e.continuity_receipt.phase_id == metadata.phase_id)
        {
            return Err(Error::ContinuityConflict);
        }
        if let Some(last) = ledger.entries.last() {
            if target_entry.completion_receipt.completion_sequence
                <= last.continuity_manifest.target_completion_sequence
            {
                return Err(Error::ContinuityConflict);
            }
        }
    }

    // 12. Source-repository argument set.
    let resolved_links = if metadata.links.is_empty() {
        if !source_repos.is_empty() {
            return Err(Error::ContinuitySourceMismatch);
        }
        Vec::new()
    } else {
        // 13. First-publication cross-repository proof resolution.
        resolve_link_sources(source_repos, &repo, &metadata.links)?
    };

    // 14. Deterministic continuity-manifest construction.
    let manifest = build_manifest(
        &accepted.sha256,
        &plan.plan_id,
        &metadata,
        target_entry,
        &normalized_path,
        &metadata_sha256,
        &metadata_bytes,
        resolved_links,
    );
    let manifest_hash = hash_compact(&manifest)?;

    // 15. Deterministic continuity-receipt construction.
    let sequence = existing
        .as_ref()
        .map(|l| l.entries.len() as u32 + 1)
        .unwrap_or(1);
    let previous_hash = existing
        .as_ref()
        .and_then(|l| l.entries.last())
        .map(|e| e.continuity_receipt_sha256.clone());
    let receipt = build_receipt(
        &accepted.sha256,
        &plan.plan_id,
        &metadata,
        target_entry,
        sequence,
        &manifest_hash,
        previous_hash,
    );
    let receipt_hash = hash_compact(&receipt)?;

    // Build the complete replacement ledger in memory and validate it before
    // any file is touched.
    let mut entries: Vec<ContinuityLedgerEntry> = existing
        .as_ref()
        .map(|l| l.entries.clone())
        .unwrap_or_default();
    entries.push(ContinuityLedgerEntry {
        continuity_manifest: manifest,
        continuity_manifest_sha256: manifest_hash,
        continuity_receipt: receipt,
        continuity_receipt_sha256: receipt_hash,
    });
    let new_ledger = ContinuityLedgerFile {
        schema_version: 1,
        accepted_plan_sha256: accepted.sha256.clone(),
        plan_id: plan.plan_id.clone(),
        repository_id: metadata.repository_id.clone(),
        entries,
    };
    validate_continuity_ledger(
        &new_ledger,
        &accepted.sha256,
        &plan.plan_id,
        completion_ledger,
    )?;

    // 16. Atomic continuity-ledger publication.
    atomic_publish_continuity_ledger(&gov_dir, &new_ledger)?;

    // 17. Complete post-publication validation.
    let reloaded = read_continuity_ledger(&gov_dir)?.ok_or(Error::ContinuityLedgerInvalid)?;
    validate_continuity_ledger(
        &reloaded,
        &accepted.sha256,
        &plan.plan_id,
        completion_ledger,
    )?;
    let entry = reloaded
        .entries
        .last()
        .ok_or(Error::ContinuityLedgerInvalid)?;

    // 18. Exact output.
    Ok(output_for(&reloaded, entry))
}

// ============================================================================
// Hashing helpers
// ============================================================================

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn hash_compact<T: serde::Serialize>(value: &T) -> Result<String, Error> {
    let json = serde_json::to_string(value).map_err(|_| Error::ContinuityLedgerInvalid)?;
    Ok(sha256_hex(json.as_bytes()))
}

// ============================================================================
// Scalar grammar (contract section 7)
// ============================================================================

fn invalid_metadata() -> Error {
    Error::ContinuityMetadataInvalid
}

fn is_valid_sha64(s: &str) -> bool {
    s.len() == 64
        && s.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn validate_scalar(s: &str, max_bytes: usize) -> Result<(), Error> {
    if s.is_empty() || s.len() > max_bytes {
        return Err(invalid_metadata());
    }
    if s != s.trim() {
        return Err(invalid_metadata());
    }
    if s.chars().any(|c| c.is_control()) {
        return Err(invalid_metadata());
    }
    Ok(())
}

/// Token fields: ASCII alphanumeric start; only ASCII alphanumeric, `.`, `_`,
/// `-`; no slash, backslash, colon, whitespace, shell metacharacter, or path
/// syntax.
fn validate_token(s: &str, max_bytes: usize) -> Result<(), Error> {
    validate_scalar(s, max_bytes)?;
    let first = s.chars().next().ok_or_else(invalid_metadata)?;
    if !first.is_ascii_alphanumeric() {
        return Err(invalid_metadata());
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(invalid_metadata());
    }
    Ok(())
}

/// Extended fields (`provider`, `model_id`, `execution_surface`): token base
/// plus `/`, `:`, `@`, and single internal ASCII spaces; no leading/trailing
/// whitespace and no repeated whitespace.
fn validate_extended(s: &str, max_bytes: usize) -> Result<(), Error> {
    validate_scalar(s, max_bytes)?;
    let first = s.chars().next().ok_or_else(invalid_metadata)?;
    if !first.is_ascii_alphanumeric() {
        return Err(invalid_metadata());
    }
    let mut prev_space = false;
    for c in s.chars() {
        let ok = c.is_ascii_alphanumeric()
            || c == '.'
            || c == '_'
            || c == '-'
            || c == '/'
            || c == ':'
            || c == '@'
            || c == ' ';
        if !ok {
            return Err(invalid_metadata());
        }
        if c == ' ' {
            if prev_space {
                return Err(invalid_metadata());
            }
            prev_space = true;
        } else {
            prev_space = false;
        }
    }
    Ok(())
}

// ============================================================================
// Metadata model (contract sections 6-10)
// ============================================================================

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelEntry {
    role: String,
    provider: String,
    model_id: String,
    execution_mode: String,
    session_label: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HostEntry {
    host_id: String,
    platform: String,
    architecture: String,
    execution_surface: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkEntry {
    relation: String,
    repository_id: String,
    accepted_plan_sha256: String,
    phase_id: String,
    completion_receipt_sha256: String,
    #[serde(default)]
    source_continuity_receipt_sha256: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataToml {
    schema_version: u32,
    repository_id: String,
    continuity_id: String,
    phase_id: String,
    completion_receipt_sha256: String,
    note: String,
    models: Vec<ModelEntry>,
    hosts: Vec<HostEntry>,
    links: Vec<LinkEntry>,
}

struct Metadata {
    repository_id: String,
    continuity_id: String,
    phase_id: String,
    completion_receipt_sha256: String,
    note: String,
    models: Vec<ModelEntry>,
    hosts: Vec<HostEntry>,
    links: Vec<LinkEntry>,
}

fn parse_metadata(bytes: &[u8]) -> Result<Metadata, Error> {
    let content = String::from_utf8(bytes.to_vec()).map_err(|_| invalid_metadata())?;
    let raw: MetadataToml = toml::from_str(&content).map_err(|_| invalid_metadata())?;

    if raw.schema_version != 1 {
        return Err(invalid_metadata());
    }
    validate_token(&raw.repository_id, 128)?;
    validate_token(&raw.continuity_id, 128)?;
    validate_scalar(&raw.phase_id, 128)?;
    if !is_valid_sha64(&raw.completion_receipt_sha256) {
        return Err(invalid_metadata());
    }
    validate_scalar(&raw.note, 1024)?;
    if raw.models.is_empty() {
        return Err(invalid_metadata());
    }
    if raw.hosts.is_empty() {
        return Err(invalid_metadata());
    }

    for m in &raw.models {
        validate_token(&m.role, 256)?;
        validate_extended(&m.provider, 256)?;
        validate_extended(&m.model_id, 256)?;
        validate_token(&m.execution_mode, 256)?;
        validate_token(&m.session_label, 256)?;
    }
    // Strictly sorted and unique by (role, provider, model_id,
    // execution_mode, session_label).
    for pair in raw.models.windows(2) {
        let key_a = (
            pair[0].role.as_str(),
            pair[0].provider.as_str(),
            pair[0].model_id.as_str(),
            pair[0].execution_mode.as_str(),
            pair[0].session_label.as_str(),
        );
        let key_b = (
            pair[1].role.as_str(),
            pair[1].provider.as_str(),
            pair[1].model_id.as_str(),
            pair[1].execution_mode.as_str(),
            pair[1].session_label.as_str(),
        );
        if key_a >= key_b {
            return Err(invalid_metadata());
        }
    }

    for h in &raw.hosts {
        validate_token(&h.host_id, 256)?;
        validate_token(&h.platform, 256)?;
        validate_token(&h.architecture, 256)?;
        validate_extended(&h.execution_surface, 256)?;
    }
    // Strictly sorted by host_id; host_id unique.
    for pair in raw.hosts.windows(2) {
        if pair[0].host_id >= pair[1].host_id {
            return Err(invalid_metadata());
        }
    }

    for l in &raw.links {
        if l.relation != "continues_from" {
            return Err(invalid_metadata());
        }
        validate_token(&l.repository_id, 128)?;
        if !is_valid_sha64(&l.accepted_plan_sha256) {
            return Err(invalid_metadata());
        }
        validate_scalar(&l.phase_id, 128)?;
        if !is_valid_sha64(&l.completion_receipt_sha256) {
            return Err(invalid_metadata());
        }
        if let Some(h) = &l.source_continuity_receipt_sha256 {
            if !is_valid_sha64(h) {
                return Err(invalid_metadata());
            }
        }
        // A link may not name the target repository_id.
        if l.repository_id == raw.repository_id {
            return Err(invalid_metadata());
        }
    }
    // Strictly sorted and unique by (repository_id, phase_id,
    // completion_receipt_sha256).
    for pair in raw.links.windows(2) {
        let key_a = (
            pair[0].repository_id.as_str(),
            pair[0].phase_id.as_str(),
            pair[0].completion_receipt_sha256.as_str(),
        );
        let key_b = (
            pair[1].repository_id.as_str(),
            pair[1].phase_id.as_str(),
            pair[1].completion_receipt_sha256.as_str(),
        );
        if key_a >= key_b {
            return Err(invalid_metadata());
        }
    }

    Ok(Metadata {
        repository_id: raw.repository_id,
        continuity_id: raw.continuity_id,
        phase_id: raw.phase_id,
        completion_receipt_sha256: raw.completion_receipt_sha256,
        note: raw.note,
        models: raw.models,
        hosts: raw.hosts,
        links: raw.links,
    })
}

// ============================================================================
// Metadata source path validation (contract section 5)
// ============================================================================

#[cfg(windows)]
fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &std::fs::Metadata) -> bool {
    false
}

fn validate_metadata_source(
    repo: &Path,
    gov_dir: &Path,
    metadata_arg: &str,
) -> Result<(Vec<u8>, String), Error> {
    let arg = Path::new(metadata_arg);

    // Leaf topology via a no-follow proof.
    let meta = std::fs::symlink_metadata(arg).map_err(|_| invalid_metadata())?;
    if meta.file_type().is_symlink() || is_reparse_point(&meta) {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    if !meta.file_type().is_file() {
        return Err(Error::FilesystemBoundaryUnsafe);
    }

    // Ordinary-directory ancestors only: every existing lexical ancestor up to
    // the repository root must be a real directory (no symlink, junction,
    // reparse point, device, socket, or FIFO). Lexical `..` components that do
    // not exist are skipped; the canonical containment check below remains
    // authoritative for escape.
    let mut cur = arg.parent();
    while let Some(dir) = cur {
        if dir == repo {
            break;
        }
        match std::fs::symlink_metadata(dir) {
            Ok(m) => {
                if m.file_type().is_symlink() || is_reparse_point(&m) {
                    return Err(Error::FilesystemBoundaryUnsafe);
                }
                if !m.is_dir() {
                    return Err(Error::FilesystemBoundaryUnsafe);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(invalid_metadata()),
        }
        cur = dir.parent();
    }

    // Canonical containment: the resolved file must be inside the canonical
    // target repository and outside `.mrgs` and `.git`.
    let canonical = std::fs::canonicalize(arg).map_err(|_| invalid_metadata())?;
    if !crate::path::plan_is_inside_repo(&canonical, repo) {
        return Err(invalid_metadata());
    }
    if canonical.starts_with(gov_dir) {
        return Err(invalid_metadata());
    }

    let rel = crate::path::relative_plan_path(&canonical, repo);
    let rel_str = rel
        .to_str()
        .ok_or_else(invalid_metadata)?
        .replace('\\', "/");
    crate::path::validate_strict_normalized_path(&rel_str).map_err(|_| invalid_metadata())?;
    // Case-insensitive `.git`/`.mrgs` governance exclusion.
    let first = rel_str.split('/').next().unwrap_or("");
    if first.eq_ignore_ascii_case(".git") || first.eq_ignore_ascii_case(".mrgs") {
        return Err(invalid_metadata());
    }
    if rel_str.chars().any(|c| c.is_control()) {
        return Err(invalid_metadata());
    }

    let bytes = std::fs::read(&canonical).map_err(|_| invalid_metadata())?;
    Ok((bytes, rel_str))
}

// ============================================================================
// Target completion binding (contract section 11)
// ============================================================================

fn bind_target_phase<'a>(
    plan: &Plan,
    gov_state: &GovernanceState,
    completion_ledger: &'a CompletionLedger,
    metadata: &Metadata,
) -> Result<&'a CompletionEntry, Error> {
    // The phase must exist in the accepted plan.
    if !plan.phases.iter().any(|p| p.id == metadata.phase_id) {
        return Err(Error::UnknownPhase(metadata.phase_id.clone()));
    }
    // It must identify exactly one completion entry and be closed in state.
    let matching: Vec<&CompletionEntry> = completion_ledger
        .completions
        .iter()
        .filter(|e| e.completion_receipt.phase_id == metadata.phase_id)
        .collect();
    if matching.is_empty() {
        return Err(Error::CloseoutConflict);
    }
    if !gov_state.closed_phases.contains(&metadata.phase_id) {
        return Err(Error::CloseoutConflict);
    }
    let entry = matching[0];
    if entry.completion_receipt_sha256 != metadata.completion_receipt_sha256 {
        return Err(invalid_metadata());
    }
    Ok(entry)
}

// ============================================================================
// Continuity manifest, receipt, and ledger (contract sections 16-20)
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityManifest {
    schema_version: u32,
    accepted_plan_sha256: String,
    plan_id: String,
    repository_id: String,
    continuity_id: String,
    phase_id: String,
    target_completion_sequence: u32,
    target_final_manifest_sha256: String,
    target_completion_receipt: CompletionReceipt,
    target_completion_receipt_sha256: String,
    metadata_source_path: String,
    metadata_sha256: String,
    metadata_content: String,
    note: String,
    models: Vec<ModelEntry>,
    hosts: Vec<HostEntry>,
    resolved_links: Vec<ResolvedLink>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityReceipt {
    schema_version: u32,
    accepted_plan_sha256: String,
    plan_id: String,
    repository_id: String,
    continuity_sequence: u32,
    continuity_id: String,
    phase_id: String,
    target_completion_sequence: u32,
    target_completion_receipt_sha256: String,
    continuity_manifest_sha256: String,
    previous_continuity_receipt_sha256: Option<String>,
}

/// Resolved cross-repository proof (contract section 14). Never stores a
/// source filesystem path.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedLink {
    relation: String,
    source_repository_id: String,
    source_accepted_plan_sha256: String,
    source_plan_id: String,
    source_phase_id: String,
    source_completion_sequence: u32,
    source_final_manifest_sha256: String,
    source_completion_receipt: CompletionReceipt,
    source_completion_receipt_sha256: String,
    source_continuity_manifest_sha256: Option<String>,
    source_continuity_receipt: Option<ContinuityReceipt>,
    source_continuity_receipt_sha256: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityLedgerEntry {
    continuity_manifest: ContinuityManifest,
    continuity_manifest_sha256: String,
    continuity_receipt: ContinuityReceipt,
    continuity_receipt_sha256: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityLedgerFile {
    schema_version: u32,
    accepted_plan_sha256: String,
    plan_id: String,
    repository_id: String,
    entries: Vec<ContinuityLedgerEntry>,
}

fn require_keys(obj: &serde_json::Value, keys: &[&str]) -> Result<(), Error> {
    if let serde_json::Value::Object(map) = obj {
        for key in keys {
            if !map.contains_key(*key) {
                return Err(Error::ContinuityLedgerInvalid);
            }
        }
        Ok(())
    } else {
        Err(Error::ContinuityLedgerInvalid)
    }
}

const MANIFEST_KEYS: [&str; 17] = [
    "schema_version",
    "accepted_plan_sha256",
    "plan_id",
    "repository_id",
    "continuity_id",
    "phase_id",
    "target_completion_sequence",
    "target_final_manifest_sha256",
    "target_completion_receipt",
    "target_completion_receipt_sha256",
    "metadata_source_path",
    "metadata_sha256",
    "metadata_content",
    "note",
    "models",
    "hosts",
    "resolved_links",
];

const RECEIPT_KEYS: [&str; 11] = [
    "schema_version",
    "accepted_plan_sha256",
    "plan_id",
    "repository_id",
    "continuity_sequence",
    "continuity_id",
    "phase_id",
    "target_completion_sequence",
    "target_completion_receipt_sha256",
    "continuity_manifest_sha256",
    "previous_continuity_receipt_sha256",
];

const COMPLETION_RECEIPT_KEYS: [&str; 13] = [
    "schema_version",
    "accepted_plan_sha256",
    "plan_id",
    "phase_id",
    "phase_title",
    "plan_phase_index",
    "completion_sequence",
    "final_manifest_sha256",
    "previous_completion_receipt_sha256",
    "closed_phases_before",
    "closed_phases_after",
    "active_phase_before",
    "active_phase_after",
];

const RESOLVED_LINK_KEYS: [&str; 12] = [
    "relation",
    "source_repository_id",
    "source_accepted_plan_sha256",
    "source_plan_id",
    "source_phase_id",
    "source_completion_sequence",
    "source_final_manifest_sha256",
    "source_completion_receipt",
    "source_completion_receipt_sha256",
    "source_continuity_manifest_sha256",
    "source_continuity_receipt",
    "source_continuity_receipt_sha256",
];

fn read_continuity_ledger(gov_dir: &Path) -> Result<Option<ContinuityLedgerFile>, Error> {
    let path = gov_dir.join("continuity-ledger.json");
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::ContinuityLedgerInvalid),
    };
    // Safe topology: a regular file, not a symlink/junction/device.
    if meta.file_type().is_symlink() || is_reparse_point(&meta) || !meta.file_type().is_file() {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    let bytes = std::fs::read(&path).map_err(|_| Error::ContinuityLedgerInvalid)?;
    let raw: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| Error::ContinuityLedgerInvalid)?;
    require_keys(
        &raw,
        &[
            "schema_version",
            "accepted_plan_sha256",
            "plan_id",
            "repository_id",
            "entries",
        ],
    )?;
    let entries = raw["entries"]
        .as_array()
        .ok_or(Error::ContinuityLedgerInvalid)?;
    if entries.is_empty() {
        return Err(Error::ContinuityLedgerInvalid);
    }
    for entry in entries {
        require_keys(
            entry,
            &[
                "continuity_manifest",
                "continuity_manifest_sha256",
                "continuity_receipt",
                "continuity_receipt_sha256",
            ],
        )?;
        require_keys(&entry["continuity_manifest"], &MANIFEST_KEYS)?;
        require_keys(&entry["continuity_receipt"], &RECEIPT_KEYS)?;
        require_keys(
            &entry["continuity_manifest"]["target_completion_receipt"],
            &COMPLETION_RECEIPT_KEYS,
        )?;
        if let Some(links) = entry["continuity_manifest"]["resolved_links"].as_array() {
            for link in links {
                require_keys(link, &RESOLVED_LINK_KEYS)?;
            }
        }
    }
    let ledger: ContinuityLedgerFile =
        serde_json::from_slice(&bytes).map_err(|_| Error::ContinuityLedgerInvalid)?;
    Ok(Some(ledger))
}

fn validate_continuity_ledger(
    ledger: &ContinuityLedgerFile,
    accepted_plan_sha256: &str,
    plan_id: &str,
    completion_ledger: &CompletionLedger,
) -> Result<(), Error> {
    if ledger.schema_version != 1 {
        return Err(Error::ContinuityLedgerInvalid);
    }
    // Accepted-plan authority: a structurally valid ledger bound to different
    // authority is stale.
    if ledger.accepted_plan_sha256 != accepted_plan_sha256 || ledger.plan_id != plan_id {
        return Err(Error::ContinuityLedgerStale);
    }
    if ledger.repository_id.is_empty() {
        return Err(Error::ContinuityLedgerInvalid);
    }
    if ledger.entries.is_empty() {
        return Err(Error::ContinuityLedgerInvalid);
    }

    let mut seen_phases = std::collections::BTreeSet::new();
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut previous_receipt_hash: Option<String> = None;
    let mut previous_target_sequence: Option<u32> = None;

    for (i, entry) in ledger.entries.iter().enumerate() {
        let expected_sequence = (i as u32) + 1;
        let manifest = &entry.continuity_manifest;
        let receipt = &entry.continuity_receipt;

        // Internal schema and authority consistency.
        if manifest.schema_version != 1 {
            return Err(Error::ContinuityLedgerInvalid);
        }
        if manifest.accepted_plan_sha256 != ledger.accepted_plan_sha256
            || manifest.plan_id != ledger.plan_id
        {
            return Err(Error::ContinuityLedgerInvalid);
        }
        if manifest.repository_id != ledger.repository_id
            || receipt.repository_id != ledger.repository_id
        {
            return Err(Error::ContinuityLedgerInvalid);
        }

        // Contiguous continuity sequence from one.
        if receipt.continuity_sequence != expected_sequence {
            return Err(Error::ContinuityLedgerInvalid);
        }
        // Unique phase IDs and continuity IDs.
        if !seen_phases.insert(manifest.phase_id.clone()) {
            return Err(Error::ContinuityLedgerInvalid);
        }
        if !seen_ids.insert(receipt.continuity_id.clone()) {
            return Err(Error::ContinuityLedgerInvalid);
        }
        // Strict target completion-sequence increase (gaps allowed).
        if let Some(prev) = previous_target_sequence {
            if manifest.target_completion_sequence <= prev {
                return Err(Error::ContinuityLedgerInvalid);
            }
        }
        previous_target_sequence = Some(manifest.target_completion_sequence);

        // Every manifest hash recomputes.
        if hash_compact(manifest)? != entry.continuity_manifest_sha256 {
            return Err(Error::ContinuityLedgerInvalid);
        }
        // Every receipt hash recomputes.
        if hash_compact(receipt)? != entry.continuity_receipt_sha256 {
            return Err(Error::ContinuityLedgerInvalid);
        }
        // Manifest-to-receipt binding.
        if receipt.continuity_manifest_sha256 != entry.continuity_manifest_sha256 {
            return Err(Error::ContinuityLedgerInvalid);
        }
        if receipt.continuity_id != manifest.continuity_id || receipt.phase_id != manifest.phase_id
        {
            return Err(Error::ContinuityLedgerInvalid);
        }
        if receipt.target_completion_sequence != manifest.target_completion_sequence {
            return Err(Error::ContinuityLedgerInvalid);
        }
        if receipt.target_completion_receipt_sha256 != manifest.target_completion_receipt_sha256 {
            return Err(Error::ContinuityLedgerInvalid);
        }
        // Every previous-receipt link.
        if receipt.previous_continuity_receipt_sha256 != previous_receipt_hash {
            return Err(Error::ContinuityLedgerInvalid);
        }
        previous_receipt_hash = Some(entry.continuity_receipt_sha256.clone());

        // Every target completion binding against the current valid
        // completion ledger (contract sections 11 and 20): the archived
        // receipt object must equal the authoritative stored receipt exactly,
        // not merely its hash string, sequence, and final-manifest binding.
        let binding = completion_ledger
            .completions
            .iter()
            .find(|c| c.completion_receipt.phase_id == manifest.phase_id)
            .ok_or(Error::ContinuityLedgerStale)?;
        if binding.completion_receipt.completion_sequence != manifest.target_completion_sequence
            || binding.completion_receipt_sha256 != manifest.target_completion_receipt_sha256
            || binding.final_manifest_sha256 != manifest.target_final_manifest_sha256
        {
            return Err(Error::ContinuityLedgerStale);
        }
        if manifest.target_completion_receipt != binding.completion_receipt {
            return Err(Error::ContinuityLedgerInvalid);
        }
    }
    Ok(())
}

// ============================================================================
// Manifest and receipt construction (contract sections 14-19)
// ============================================================================

// Deterministic manifest construction: the argument list mirrors the frozen
// contract's manifest field sources exactly.
#[allow(clippy::too_many_arguments)]
fn build_manifest(
    accepted_plan_sha256: &str,
    plan_id: &str,
    metadata: &Metadata,
    target_entry: &CompletionEntry,
    metadata_source_path: &str,
    metadata_sha256: &str,
    metadata_bytes: &[u8],
    resolved_links: Vec<ResolvedLink>,
) -> ContinuityManifest {
    ContinuityManifest {
        schema_version: 1,
        accepted_plan_sha256: accepted_plan_sha256.to_string(),
        plan_id: plan_id.to_string(),
        repository_id: metadata.repository_id.clone(),
        continuity_id: metadata.continuity_id.clone(),
        phase_id: metadata.phase_id.clone(),
        target_completion_sequence: target_entry.completion_receipt.completion_sequence,
        target_final_manifest_sha256: target_entry.final_manifest_sha256.clone(),
        target_completion_receipt: target_entry.completion_receipt.clone(),
        target_completion_receipt_sha256: target_entry.completion_receipt_sha256.clone(),
        metadata_source_path: metadata_source_path.to_string(),
        metadata_sha256: metadata_sha256.to_string(),
        metadata_content: String::from_utf8(metadata_bytes.to_vec())
            .expect("metadata bytes validated as UTF-8"),
        note: metadata.note.clone(),
        models: metadata.models.clone(),
        hosts: metadata.hosts.clone(),
        resolved_links,
    }
}

fn build_receipt(
    accepted_plan_sha256: &str,
    plan_id: &str,
    metadata: &Metadata,
    target_entry: &CompletionEntry,
    continuity_sequence: u32,
    manifest_hash: &str,
    previous_receipt_hash: Option<String>,
) -> ContinuityReceipt {
    ContinuityReceipt {
        schema_version: 1,
        accepted_plan_sha256: accepted_plan_sha256.to_string(),
        plan_id: plan_id.to_string(),
        repository_id: metadata.repository_id.clone(),
        continuity_sequence,
        continuity_id: metadata.continuity_id.clone(),
        phase_id: metadata.phase_id.clone(),
        target_completion_sequence: target_entry.completion_receipt.completion_sequence,
        target_completion_receipt_sha256: target_entry.completion_receipt_sha256.clone(),
        continuity_manifest_sha256: manifest_hash.to_string(),
        previous_continuity_receipt_sha256: previous_receipt_hash,
    }
}

fn output_for(ledger: &ContinuityLedgerFile, entry: &ContinuityLedgerEntry) -> String {
    format!(
        "CONTINUITY_RECORDED {} {} {} {} {}",
        ledger.repository_id,
        entry.continuity_receipt.phase_id,
        entry.continuity_receipt.continuity_sequence,
        entry.continuity_manifest_sha256,
        entry.continuity_receipt_sha256,
    )
}

// ============================================================================
// Replay and conflict detection (contract section 22)
// ============================================================================

fn is_exact_replay(
    entry: &ContinuityLedgerEntry,
    metadata: &Metadata,
    metadata_source_path: &str,
    metadata_bytes: &[u8],
    metadata_sha256: &str,
) -> bool {
    let manifest = &entry.continuity_manifest;
    let receipt = &entry.continuity_receipt;
    if manifest.repository_id != metadata.repository_id
        || receipt.repository_id != metadata.repository_id
    {
        return false;
    }
    if manifest.continuity_id != metadata.continuity_id
        || receipt.continuity_id != metadata.continuity_id
    {
        return false;
    }
    if manifest.phase_id != metadata.phase_id || receipt.phase_id != metadata.phase_id {
        return false;
    }
    if manifest.target_completion_receipt_sha256 != metadata.completion_receipt_sha256
        || receipt.target_completion_receipt_sha256 != metadata.completion_receipt_sha256
    {
        return false;
    }
    if manifest.metadata_source_path != metadata_source_path {
        return false;
    }
    if manifest.metadata_sha256 != metadata_sha256 {
        return false;
    }
    if manifest.metadata_content.as_bytes() != metadata_bytes {
        return false;
    }
    if manifest.note != metadata.note {
        return false;
    }
    if manifest.models != metadata.models || manifest.hosts != metadata.hosts {
        return false;
    }
    // Raw link fields (which are fully reflected in the durable resolved
    // proofs) must match; the durable proofs themselves are validated by the
    // ledger validation and re-verified only when sources are supplied.
    if manifest.resolved_links.len() != metadata.links.len() {
        return false;
    }
    for (link, resolved) in metadata.links.iter().zip(manifest.resolved_links.iter()) {
        if link.repository_id != resolved.source_repository_id
            || link.accepted_plan_sha256 != resolved.source_accepted_plan_sha256
            || link.phase_id != resolved.source_phase_id
            || link.completion_receipt_sha256 != resolved.source_completion_receipt_sha256
            || link.source_continuity_receipt_sha256 != resolved.source_continuity_receipt_sha256
        {
            return false;
        }
    }
    true
}

fn verify_replay_sources(
    source_repos: &[String],
    target_repo: &Path,
    entry: &ContinuityLedgerEntry,
) -> Result<(), Error> {
    // Re-resolve each supplied source against the durable proofs.
    let stored: Vec<LinkEntry> = entry
        .continuity_manifest
        .resolved_links
        .iter()
        .map(|r| LinkEntry {
            relation: r.relation.clone(),
            repository_id: r.source_repository_id.clone(),
            accepted_plan_sha256: r.source_accepted_plan_sha256.clone(),
            phase_id: r.source_phase_id.clone(),
            completion_receipt_sha256: r.source_completion_receipt_sha256.clone(),
            source_continuity_receipt_sha256: r.source_continuity_receipt_sha256.clone(),
        })
        .collect();
    let resolved = resolve_link_sources(source_repos, target_repo, &stored)?;
    if resolved != entry.continuity_manifest.resolved_links {
        return Err(Error::ContinuitySourceMismatch);
    }
    Ok(())
}

// ============================================================================
// Cross-repository resolution (contract sections 12-15)
// ============================================================================

struct SourceAuthority {
    repo: PathBuf,
    plan_sha256: String,
    plan_id: String,
    completion_ledger: CompletionLedger,
}

fn validate_source_repo(source_arg: &str, target_repo: &Path) -> Result<SourceAuthority, Error> {
    let source_path = Path::new(source_arg);
    crate::path::assert_existing_dir(source_path).map_err(|_| Error::ContinuitySourceInvalid)?;
    let source = std::fs::canonicalize(source_path).map_err(|_| Error::ContinuitySourceInvalid)?;
    // A source must differ from the canonical target root.
    if source == target_repo {
        return Err(Error::ContinuitySourceMismatch);
    }
    // Safe `.mrgs` directory.
    let gov_dir = crate::path::validate_gov_dir_exists(&source)
        .map_err(|_| Error::ContinuitySourceInvalid)?;
    // Accepted plan and exact source relation.
    let accepted =
        state::read_accepted_plan(&source).map_err(|_| Error::ContinuitySourceInvalid)?;
    state::validate_accepted_plan_record(&accepted).map_err(|_| Error::ContinuitySourceInvalid)?;
    let plan_file = crate::path::resolve_safe_plan_path(&source, &accepted.plan_path)
        .map_err(|_| Error::ContinuitySourceInvalid)?;
    let plan_bytes = std::fs::read(&plan_file).map_err(|_| Error::ContinuitySourceInvalid)?;
    let plan_sha256 = sha256_hex(&plan_bytes);
    let plan_str = String::from_utf8(plan_bytes).map_err(|_| Error::ContinuitySourceInvalid)?;
    let plan: Plan = toml::from_str(&plan_str).map_err(|_| Error::ContinuitySourceInvalid)?;
    plan.validate()
        .map_err(|_| Error::ContinuitySourceInvalid)?;
    state::validate_plan_consistency(&accepted, &plan, &plan_sha256)
        .map_err(|_| Error::ContinuitySourceInvalid)?;
    // State record and relation.
    let gov_state = state::read_state(&source).map_err(|_| Error::ContinuitySourceInvalid)?;
    state::validate_state_record(&gov_state, &accepted, &plan)
        .map_err(|_| Error::ContinuitySourceInvalid)?;
    // Valid Phase 6 completion ledger and state relation.
    let completion_ledger = closeout::read_completion_ledger(&gov_dir)
        .map_err(|_| Error::ContinuitySourceInvalid)?
        .ok_or(Error::ContinuitySourceInvalid)?;
    closeout::validate_completion_ledger(&completion_ledger, &accepted.sha256, &plan.plan_id)
        .map_err(|_| Error::ContinuitySourceInvalid)?;
    closeout::validate_state_ledger_relation(&gov_state, Some(&completion_ledger))
        .map_err(|_| Error::ContinuitySourceInvalid)?;

    Ok(SourceAuthority {
        repo: source,
        plan_sha256,
        plan_id: plan.plan_id.clone(),
        completion_ledger,
    })
}

fn find_source_completion<'a>(
    source: &'a SourceAuthority,
    phase_id: &str,
) -> Option<&'a CompletionEntry> {
    source
        .completion_ledger
        .completions
        .iter()
        .find(|e| e.completion_receipt.phase_id == phase_id)
}

fn resolve_link_sources(
    source_repos: &[String],
    target_repo: &Path,
    links: &[LinkEntry],
) -> Result<Vec<ResolvedLink>, Error> {
    if links.is_empty() {
        if !source_repos.is_empty() {
            return Err(Error::ContinuitySourceMismatch);
        }
        return Ok(Vec::new());
    }
    // Exactly one canonical source repository must be supplied for each link.
    if source_repos.len() != links.len() {
        return Err(Error::ContinuitySourceMismatch);
    }

    // Every source root must be unique and must differ from the target root.
    let mut sources: Vec<SourceAuthority> = Vec::new();
    let mut seen_roots = std::collections::BTreeSet::new();
    for arg in source_repos {
        let source = validate_source_repo(arg, target_repo)?;
        if !seen_roots.insert(source.repo.clone()) {
            return Err(Error::ContinuitySourceMismatch);
        }
        sources.push(source);
    }

    let mut used = vec![false; sources.len()];
    let mut resolved: Vec<ResolvedLink> = Vec::new();
    for link in links {
        // Completion-level candidate matching.
        let mut candidates: Vec<usize> = Vec::new();
        for (i, source) in sources.iter().enumerate() {
            let entry = match find_source_completion(source, &link.phase_id) {
                Some(e) => e,
                None => continue,
            };
            if source.plan_sha256 != link.accepted_plan_sha256 {
                continue;
            }
            if entry.completion_receipt_sha256 != link.completion_receipt_sha256 {
                continue;
            }
            candidates.push(i);
        }
        if candidates.is_empty() {
            return Err(Error::ContinuitySourceMismatch);
        }

        // Continuity-level filtering when the link requires a source
        // continuity receipt.
        if let Some(requested) = &link.source_continuity_receipt_sha256 {
            let mut with_continuity: Vec<(usize, ContinuityLedgerFile)> = Vec::new();
            for i in &candidates {
                let gov = crate::path::validate_gov_dir_exists(&sources[*i].repo)
                    .map_err(|_| Error::ContinuitySourceInvalid)?;
                let ledger = read_source_continuity_ledger(&gov)?;
                validate_continuity_ledger_for_source(&ledger, &sources[*i])?;
                with_continuity.push((*i, ledger));
            }
            candidates = with_continuity
                .iter()
                .filter(|(_, ledger)| {
                    ledger
                        .entries
                        .iter()
                        .any(|e| e.continuity_receipt_sha256 == *requested)
                })
                .map(|(i, _)| *i)
                .collect();
            if candidates.is_empty() {
                return Err(Error::ContinuitySourceMismatch);
            }
        }

        // Exactly one source must resolve exactly one link.
        if candidates.len() != 1 {
            return Err(Error::ContinuitySourceMismatch);
        }
        let idx = candidates[0];
        if used[idx] {
            return Err(Error::ContinuitySourceMismatch);
        }
        used[idx] = true;
        resolved.push(resolve_proof(link, &sources[idx])?);
    }
    // Every supplied source must resolve exactly one link.
    if used.iter().any(|u| !u) {
        return Err(Error::ContinuitySourceMismatch);
    }
    Ok(resolved)
}

/// Validates a source continuity ledger against that source's own authority.
fn validate_continuity_ledger_for_source(
    ledger: &ContinuityLedgerFile,
    source: &SourceAuthority,
) -> Result<(), Error> {
    validate_continuity_ledger(
        ledger,
        &source.plan_sha256,
        &source.plan_id,
        &source.completion_ledger,
    )
    .map_err(|_| Error::ContinuitySourceInvalid)
}

/// Reads a source continuity ledger; every topology, parse, or content
/// failure is a source-repository invalidity.
fn read_source_continuity_ledger(gov_dir: &Path) -> Result<ContinuityLedgerFile, Error> {
    read_continuity_ledger(gov_dir)
        .map_err(|_| Error::ContinuitySourceInvalid)?
        .ok_or(Error::ContinuitySourceInvalid)
}

fn resolve_proof(link: &LinkEntry, source: &SourceAuthority) -> Result<ResolvedLink, Error> {
    let entry =
        find_source_completion(source, &link.phase_id).ok_or(Error::ContinuitySourceMismatch)?;
    // Repository identity when the link requires a source continuity receipt.
    let mut continuity_manifest_sha256: Option<String> = None;
    let mut continuity_receipt: Option<ContinuityReceipt> = None;
    let mut continuity_receipt_sha256: Option<String> = None;
    if let Some(requested) = &link.source_continuity_receipt_sha256 {
        let gov = crate::path::validate_gov_dir_exists(&source.repo)
            .map_err(|_| Error::ContinuitySourceInvalid)?;
        let ledger = read_source_continuity_ledger(&gov)?;
        validate_continuity_ledger_for_source(&ledger, source)?;
        // The source ledger repository ID must equal the link repository ID.
        if ledger.repository_id != link.repository_id {
            return Err(Error::ContinuitySourceMismatch);
        }
        let matched = ledger
            .entries
            .iter()
            .filter(|e| e.continuity_receipt_sha256 == *requested)
            .collect::<Vec<_>>();
        if matched.len() != 1 {
            return Err(Error::ContinuitySourceMismatch);
        }
        let found = matched[0];
        // The entry must bind the same source phase and completion receipt.
        if found.continuity_receipt.phase_id != link.phase_id
            || found.continuity_receipt.target_completion_receipt_sha256
                != link.completion_receipt_sha256
        {
            return Err(Error::ContinuitySourceMismatch);
        }
        continuity_manifest_sha256 = Some(found.continuity_manifest_sha256.clone());
        continuity_receipt = Some(found.continuity_receipt.clone());
        continuity_receipt_sha256 = Some(found.continuity_receipt_sha256.clone());
    }

    Ok(ResolvedLink {
        relation: "continues_from".to_string(),
        source_repository_id: link.repository_id.clone(),
        source_accepted_plan_sha256: source.plan_sha256.clone(),
        source_plan_id: source.plan_id.clone(),
        source_phase_id: entry.completion_receipt.phase_id.clone(),
        source_completion_sequence: entry.completion_receipt.completion_sequence,
        source_final_manifest_sha256: entry.final_manifest_sha256.clone(),
        source_completion_receipt: entry.completion_receipt.clone(),
        source_completion_receipt_sha256: entry.completion_receipt_sha256.clone(),
        source_continuity_manifest_sha256: continuity_manifest_sha256,
        source_continuity_receipt: continuity_receipt,
        source_continuity_receipt_sha256: continuity_receipt_sha256,
    })
}

// ============================================================================
// Atomic publication (contract sections 21 and 23)
// ============================================================================

fn atomic_publish_continuity_ledger(
    gov_dir: &Path,
    ledger: &ContinuityLedgerFile,
) -> Result<(), Error> {
    // 1. Serialize completely before opening any file; pretty JSON with no
    //    trailing newline, consistent with existing governance files.
    let json = serde_json::to_string_pretty(ledger).map_err(|_| Error::ContinuityLedgerInvalid)?;
    let final_path = gov_dir.join("continuity-ledger.json");

    // 2-4. Unique same-directory temporary file with create-new semantics;
    //      never truncate an existing colliding path.
    let mut tmp_path: Option<PathBuf> = None;
    for attempt in 0..16u32 {
        let name = format!(".continuity.{}.tmp", attempt);
        let candidate = gov_dir.join(&name);
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(Error::PersistenceFailed),
        };
        // 5. Write exact bytes and flush.
        use std::io::Write;
        if file.write_all(json.as_bytes()).is_err() {
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
    let tmp_path = tmp_path.ok_or(Error::PersistenceFailed)?;

    // 6-7. Atomically publish or replace the final file; on failure remove
    //      only the temporary file created here and preserve prior bytes.
    if state::rename_replace(&tmp_path, &final_path).is_err() {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(Error::PersistenceFailed);
    }
    Ok(())
}
