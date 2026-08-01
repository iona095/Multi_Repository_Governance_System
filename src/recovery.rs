//! Phase 8 — State Recovery and Corruption Handling.
//!
//! Implements exactly two commands:
//!
//! ```text
//! mrgs recovery inspect --repo <REPOSITORY_PATH>
//! mrgs recovery apply --repo <REPOSITORY_PATH> --recovery-id <RECOVERY_ID>
//!                         --subject-sha256 <SUBJECT_SHA256> --decision <DECISION>
//! ```
//!
//! Recovery derives authority only from surviving records that fully validate
//! under their original Phase 1-7 rules. Only `accepted-plan.json` and
//! `state.json` are reconstructible, under the exact rules of the Phase 8
//! contract. All writes are limited to exact authorized `.mrgs` targets and
//! use create-new temporary files plus atomic replacement.

use crate::closeout::{self, CompletionLedger};
use crate::error::Error;
use crate::git::GitRunner;
use crate::implementation;
use crate::state::{self, AcceptedPlan, GovernanceState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

// ============================================================================
// Constants
// ============================================================================

/// Permanent recognized filenames (recovery-ledger.json is validated
/// separately as the recovery journal and is excluded from the subject).
const PERMANENT_FILES: [&str; 8] = [
    "accepted-plan.json",
    "state.json",
    "contract-draft.json",
    "accepted-contract.json",
    "implementation-authority.json",
    "audit-ledger.json",
    "completion-ledger.json",
    "continuity-ledger.json",
];

/// Phase-scoped files in their fixed Phase 2-5 order.
const PHASE_SCOPED_FILES: [&str; 4] = [
    "contract-draft.json",
    "accepted-contract.json",
    "implementation-authority.json",
    "audit-ledger.json",
];

const RECOVERY_LEDGER_FILENAME: &str = "recovery-ledger.json";

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn is_lowercase_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn is_valid_sha64(s: &str) -> bool {
    is_lowercase_hex(s, 64)
}

fn compact_sha<T: Serialize>(value: &T) -> Result<String, Error> {
    let json = serde_json::to_string(value).map_err(|_| Error::RecoveryLedgerInvalid)?;
    Ok(sha256_hex(json.as_bytes()))
}

fn require_keys(obj: &serde_json::Value, keys: &[&str]) -> Result<(), Error> {
    if let serde_json::Value::Object(map) = obj {
        for key in keys {
            if !map.contains_key(*key) {
                return Err(Error::RecoveryLedgerInvalid);
            }
        }
        Ok(())
    } else {
        Err(Error::RecoveryLedgerInvalid)
    }
}

/// Exact raw-key validation: the object must contain exactly the given keys
/// — no missing and no unknown keys. Used for the reconstructible records
/// whose Phase 1-7 serde structs do not reject unknown fields.
fn exact_keys(obj: &serde_json::Value, keys: &[&str]) -> Result<(), Error> {
    let map = obj.as_object().ok_or(Error::RecoveryUnrecoverable)?;
    if map.len() != keys.len() {
        return Err(Error::RecoveryUnrecoverable);
    }
    for key in keys {
        if !map.contains_key(*key) {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    Ok(())
}

/// Contract-authorized accepted-plan record keys (section 8).
const ACCEPTED_PLAN_KEYS: [&str; 5] = [
    "schema_version",
    "plan_id",
    "plan_path",
    "sha256",
    "phase_count",
];

/// Contract-authorized state record keys (sections 9-12).
const STATE_KEYS: [&str; 4] = [
    "schema_version",
    "accepted_plan_sha256",
    "active_phase",
    "closed_phases",
];

// ============================================================================
// Recovery subject (contract section 5)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySubject {
    pub schema_version: u32,
    pub repository_git_object_format: String,
    pub repository_head: String,
    pub repository_branch: String,
    pub governance_entries: Vec<GovernanceEntry>,
    pub plan_source: Option<PlanSourceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceEntry {
    pub filename: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSourceInfo {
    pub path: String,
    pub topology: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Interior inventory representation carrying exact bytes for regular files.
#[derive(Debug, Clone)]
struct InventoryEntry {
    filename: String,
    kind: &'static str,
    bytes: Option<Vec<u8>>,
}

fn entry_kind_for(meta: &std::fs::Metadata) -> &'static str {
    if meta.file_type().is_file() {
        "REGULAR"
    } else if meta.file_type().is_symlink() {
        "SYMLINK"
    } else if meta.is_dir() {
        "DIRECTORY"
    } else {
        "OTHER"
    }
}

// ============================================================================
// Recovery plan, actions, and receipt (contract sections 14-15, 21)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecoveryActionKind {
    RemoveRedundantTemp,
    RestoreAcceptedPlan,
    RestoreState,
    ResumeCloseout,
}

impl RecoveryActionKind {
    fn as_str(&self) -> &'static str {
        match self {
            RecoveryActionKind::RemoveRedundantTemp => "REMOVE_REDUNDANT_TEMP",
            RecoveryActionKind::RestoreAcceptedPlan => "RESTORE_ACCEPTED_PLAN",
            RecoveryActionKind::RestoreState => "RESTORE_STATE",
            RecoveryActionKind::ResumeCloseout => "RESUME_CLOSEOUT",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecoveryAction {
    pub kind: RecoveryActionKind,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}

impl RecoveryAction {
    fn validate(&self) -> Result<(), Error> {
        if self.target.is_empty() {
            return Err(Error::RecoveryLedgerInvalid);
        }
        // Strict normalized target labels (contract sections 13-14): exact
        // permanent filenames for restores, a recognized producer temp
        // grammar for removals, and the Phase 6 phase-ID grammar for
        // closeout resumption. No traversal, backslash, separator, alias, or
        // unknown label is accepted.
        match self.kind {
            RecoveryActionKind::RestoreAcceptedPlan => {
                if self.target != "accepted-plan.json" {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                let replacement = self
                    .replacement
                    .as_deref()
                    .ok_or(Error::RecoveryLedgerInvalid)?;
                // The replacement must decode as the exact record type with
                // the exact raw keys and canonical pretty serialization;
                // arbitrary parseable JSON is never accepted.
                let raw: serde_json::Value =
                    serde_json::from_str(replacement).map_err(|_| Error::RecoveryLedgerInvalid)?;
                exact_keys(&raw, &ACCEPTED_PLAN_KEYS).map_err(|_| Error::RecoveryLedgerInvalid)?;
                let record: AcceptedPlan =
                    serde_json::from_value(raw).map_err(|_| Error::RecoveryLedgerInvalid)?;
                state::validate_accepted_plan_record(&record)
                    .map_err(|_| Error::RecoveryLedgerInvalid)?;
                // The replacement's plan path must also pass the strict
                // normalized-path rules; the Phase 1 validator alone permits
                // backslashes and noncanonical spellings on Unix.
                safe_plan_path_str(&record.plan_path).ok_or(Error::RecoveryLedgerInvalid)?;
                let canonical = serde_json::to_string_pretty(&record)
                    .map_err(|_| Error::RecoveryLedgerInvalid)?;
                if canonical != replacement {
                    return Err(Error::RecoveryLedgerInvalid);
                }
            }
            RecoveryActionKind::RestoreState => {
                if self.target != "state.json" {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                let replacement = self
                    .replacement
                    .as_deref()
                    .ok_or(Error::RecoveryLedgerInvalid)?;
                let raw: serde_json::Value =
                    serde_json::from_str(replacement).map_err(|_| Error::RecoveryLedgerInvalid)?;
                exact_keys(&raw, &STATE_KEYS).map_err(|_| Error::RecoveryLedgerInvalid)?;
                if !raw["active_phase"].is_null() && !raw["active_phase"].is_string() {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if !raw["closed_phases"]
                    .as_array()
                    .map(|arr| arr.iter().all(|v| v.is_string()))
                    .unwrap_or(false)
                {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                let state: GovernanceState =
                    serde_json::from_value(raw).map_err(|_| Error::RecoveryLedgerInvalid)?;
                let canonical = serde_json::to_string_pretty(&state)
                    .map_err(|_| Error::RecoveryLedgerInvalid)?;
                if canonical != replacement {
                    return Err(Error::RecoveryLedgerInvalid);
                }
            }
            RecoveryActionKind::RemoveRedundantTemp => {
                // Only a recognized producer temp mapping to one exact
                // permanent target is removable; recovery-owned temps are
                // bound to the journal rules and are never REMOVE targets.
                if self.replacement.is_some() {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if !matches!(classify_temp_name(&self.target), Some(TempClass::Target(_))) {
                    return Err(Error::RecoveryLedgerInvalid);
                }
            }
            RecoveryActionKind::ResumeCloseout => {
                if self.replacement.is_some() {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                validate_phase_id_label(&self.target)?;
            }
        }
        Ok(())
    }
}

/// Phase-ID label grammar for the RESUME_CLOSEOUT target. This is exactly
/// the Phase 6 closeout grammar (`closeout::validate_phase_id`): non-empty,
/// at most 128 bytes, no surrounding whitespace, no control characters.
/// Phase IDs that Phase 6 accepted (including separator-bearing IDs such as
/// `a/b`) must remain resumable; the label is never used as a filesystem
/// path. Semantic binding to the final completion receipt phase happens at
/// execution (`resume_closeout`), where any mismatch fails closed with
/// `RECOVERY_ACTION_FAILED` before any mutation.
fn validate_phase_id_label(phase_id: &str) -> Result<(), Error> {
    if phase_id.is_empty() {
        return Err(Error::RecoveryLedgerInvalid);
    }
    if phase_id.len() > 128 {
        return Err(Error::RecoveryLedgerInvalid);
    }
    if phase_id != phase_id.trim() {
        return Err(Error::RecoveryLedgerInvalid);
    }
    if phase_id.chars().any(|c| c.is_control()) {
        return Err(Error::RecoveryLedgerInvalid);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPlanSeed {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub plan_id: String,
    pub pre_subject_sha256: String,
    pub actions: Vec<RecoveryAction>,
    pub prefix_subject_sha256: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecoveryReceipt {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub plan_id: String,
    pub recovery_sequence: u32,
    pub recovery_id: String,
    pub pre_subject_sha256: String,
    pub post_subject_sha256: String,
    pub action_count: usize,
    pub actions_sha256: String,
    pub previous_recovery_receipt_sha256: Option<String>,
}

/// A computed recovery plan plus the state needed to execute it.
struct RecoveryPlan {
    seed: RecoveryPlanSeed,
    recovery_id: String,
}

// ============================================================================
// Recovery journal (contract section 18)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryJournalFile {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub plan_id: String,
    pub recoveries: Vec<RecoveryJournalEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryJournalEntry {
    pub recovery_id: String,
    pub plan: RecoveryPlanSeed,
    pub next_action: usize,
    pub status: String,
    pub post_subject_sha256: Option<String>,
    pub recovery_receipt: Option<RecoveryReceipt>,
    pub recovery_receipt_sha256: Option<String>,
}

struct JournalState {
    file: RecoveryJournalFile,
    pending: Option<usize>,
}

const JOURNAL_ENTRY_KEYS: [&str; 7] = [
    "recovery_id",
    "plan",
    "next_action",
    "status",
    "post_subject_sha256",
    "recovery_receipt",
    "recovery_receipt_sha256",
];

const PLAN_KEYS: [&str; 6] = [
    "schema_version",
    "accepted_plan_sha256",
    "plan_id",
    "pre_subject_sha256",
    "actions",
    "prefix_subject_sha256",
];

const RECEIPT_KEYS: [&str; 10] = [
    "schema_version",
    "accepted_plan_sha256",
    "plan_id",
    "recovery_sequence",
    "recovery_id",
    "pre_subject_sha256",
    "post_subject_sha256",
    "action_count",
    "actions_sha256",
    "previous_recovery_receipt_sha256",
];

// ============================================================================
// Test-only failpoints (debug builds only)
// ============================================================================

#[cfg(debug_assertions)]
fn test_only_recovery_barrier(point: &str) -> Result<(), Error> {
    let Some(current) = std::env::var_os("MRGS_TEST_ONLY_RECOVERY_POINT")
        .filter(|value| !value.is_empty())
        .map(|v| v.to_string_lossy().into_owned())
    else {
        return Ok(());
    };
    if current != point {
        return Ok(());
    }
    let Some(signal) = std::env::var_os("MRGS_TEST_ONLY_RECOVERY_SIGNAL_FILE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    else {
        return Ok(());
    };
    let Some(release) = std::env::var_os("MRGS_TEST_ONLY_RECOVERY_RELEASE_FILE")
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !release.exists() {
        if std::time::Instant::now() >= deadline {
            return Err(Error::PersistenceFailed);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    Ok(())
}

#[cfg(not(debug_assertions))]
fn test_only_recovery_barrier(_point: &str) -> Result<(), Error> {
    Ok(())
}

#[cfg(debug_assertions)]
fn test_only_fail_rename_after_publish() -> bool {
    std::env::var_os("MRGS_TEST_ONLY_RECOVERY_FAIL_RENAME_AFTER_PUBLISH")
        .is_some_and(|value| value == "1")
}

#[cfg(not(debug_assertions))]
fn test_only_fail_rename_after_publish() -> bool {
    false
}

#[cfg(debug_assertions)]
static PENDING_PUBLISHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(not(debug_assertions))]
static PENDING_PUBLISHED: () = ();

fn pending_published_flag() -> bool {
    #[cfg(debug_assertions)]
    {
        PENDING_PUBLISHED.load(std::sync::atomic::Ordering::Relaxed)
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = PENDING_PUBLISHED;
        false
    }
}

// ============================================================================
// Repository identity and topology (contract sections 4, 24, 25)
// ============================================================================

struct Identity {
    repo: PathBuf,
    gov_dir: PathBuf,
    head: String,
    branch: String,
    objfmt: String,
}

fn resolve_identity(repo_arg: &str) -> Result<Identity, Error> {
    let repo_path = Path::new(repo_arg);
    crate::path::assert_existing_dir(repo_path).map_err(|_| Error::RepositoryInvalid)?;
    let repo = std::fs::canonicalize(repo_path).map_err(|_| Error::RepositoryInvalid)?;
    let git = GitRunner::new(&repo);
    // Distinguish an unreadable/non-repository directory from a repository
    // with an unborn HEAD: only inside a recognized work tree does a git
    // child failure mean an invalid HEAD.
    let inside = git.run(["rev-parse", "--is-inside-work-tree"])?;
    if !inside.status.success() {
        return Err(Error::RepositoryInvalid);
    }
    let (head, branch, objfmt) = implementation::validate_git_root(&git).map_err(|e| match e {
        Error::GitDetachedHead | Error::GitHeadInvalid => e,
        Error::GitRootMismatch => Error::RepositoryInvalid,
        // Inside a recognized work tree, a failed HEAD^{commit} resolution
        // means an unborn HEAD.
        Error::GitCommandFailed(_) => Error::GitHeadInvalid,
        other => other,
    })?;
    let gov_dir = crate::path::validate_gov_dir_exists(&repo).map_err(|e| match e {
        Error::GovDirEscape(_) | Error::GovDirNotDirectory(_) => Error::FilesystemBoundaryUnsafe,
        Error::GovDirNotExists(_) => Error::RecoveryUnrecoverable,
        other => other,
    })?;
    Ok(Identity {
        repo,
        gov_dir,
        head,
        branch,
        objfmt,
    })
}

// ============================================================================
// Journal reading and strict validation (contract section 18)
// ============================================================================

fn read_journal(gov_dir: &Path) -> Result<Option<JournalState>, Error> {
    let path = gov_dir.join(RECOVERY_LEDGER_FILENAME);
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::RecoveryLedgerInvalid),
    };
    if meta.file_type().is_symlink()
        || meta.file_type().is_dir()
        || !meta.file_type().is_file()
        || is_reparse_point(&meta)
    {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    let bytes = std::fs::read(&path).map_err(|_| Error::RecoveryLedgerInvalid)?;
    let raw: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryLedgerInvalid)?;
    require_keys(
        &raw,
        &[
            "schema_version",
            "accepted_plan_sha256",
            "plan_id",
            "recoveries",
        ],
    )?;
    let entries = raw["recoveries"]
        .as_array()
        .ok_or(Error::RecoveryLedgerInvalid)?;
    if entries.is_empty() {
        return Err(Error::RecoveryLedgerInvalid);
    }
    for entry in entries {
        require_keys(entry, &JOURNAL_ENTRY_KEYS)?;
        require_keys(&entry["plan"], &PLAN_KEYS)?;
        if let Some(receipt) = entry.get("recovery_receipt") {
            if !receipt.is_null() {
                require_keys(receipt, &RECEIPT_KEYS)?;
            }
        }
    }
    let file: RecoveryJournalFile =
        serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryLedgerInvalid)?;
    validate_journal_structure(&file)?;
    let mut pending = None;
    for (i, entry) in file.recoveries.iter().enumerate() {
        if entry.status == "PENDING" {
            if pending.is_some() {
                return Err(Error::RecoveryLedgerInvalid);
            }
            pending = Some(i);
        }
    }
    Ok(Some(JournalState { file, pending }))
}

fn validate_journal_structure(file: &RecoveryJournalFile) -> Result<(), Error> {
    if file.schema_version != 1 {
        return Err(Error::RecoveryLedgerInvalid);
    }
    if !is_valid_sha64(&file.accepted_plan_sha256) {
        return Err(Error::RecoveryLedgerInvalid);
    }
    if file.plan_id.is_empty() {
        return Err(Error::RecoveryLedgerInvalid);
    }
    if file.recoveries.is_empty() {
        return Err(Error::RecoveryLedgerInvalid);
    }
    let mut pending_index: Option<usize> = None;
    let mut previous_receipt_hash: Option<String> = None;
    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    for (i, entry) in file.recoveries.iter().enumerate() {
        if !is_valid_sha64(&entry.recovery_id) {
            return Err(Error::RecoveryLedgerInvalid);
        }
        if !seen_ids.insert(entry.recovery_id.clone()) {
            // Recovery IDs must be unique across the complete history.
            return Err(Error::RecoveryLedgerInvalid);
        }
        if entry.plan.schema_version != 1 {
            return Err(Error::RecoveryLedgerInvalid);
        }
        if entry.plan.accepted_plan_sha256 != file.accepted_plan_sha256
            || entry.plan.plan_id != file.plan_id
        {
            return Err(Error::RecoveryLedgerInvalid);
        }
        if !is_valid_sha64(&entry.plan.pre_subject_sha256) {
            return Err(Error::RecoveryLedgerInvalid);
        }
        if entry.plan.actions.is_empty() {
            return Err(Error::RecoveryLedgerInvalid);
        }
        for action in &entry.plan.actions {
            action.validate()?;
        }
        // Exact action ordering: redundant-temp removals first (sorted by
        // target, strictly increasing), then at most one accepted-plan
        // restoration, at most one state restoration, at most one closeout
        // resumption. No duplicate or contradictory target combination.
        let mut saw_non_remove = false;
        let mut seen_restore_plan = false;
        let mut seen_restore_state = false;
        let mut seen_resume = false;
        let mut previous_remove_target: Option<&str> = None;
        for action in &entry.plan.actions {
            match action.kind {
                RecoveryActionKind::RemoveRedundantTemp => {
                    if saw_non_remove {
                        return Err(Error::RecoveryLedgerInvalid);
                    }
                    if let Some(prev) = previous_remove_target {
                        if action.target.as_bytes() <= prev.as_bytes() {
                            return Err(Error::RecoveryLedgerInvalid);
                        }
                    }
                    previous_remove_target = Some(&action.target);
                }
                RecoveryActionKind::RestoreAcceptedPlan => {
                    saw_non_remove = true;
                    if seen_restore_plan || seen_restore_state || seen_resume {
                        return Err(Error::RecoveryLedgerInvalid);
                    }
                    seen_restore_plan = true;
                }
                RecoveryActionKind::RestoreState => {
                    saw_non_remove = true;
                    if seen_restore_state || seen_resume {
                        return Err(Error::RecoveryLedgerInvalid);
                    }
                    seen_restore_state = true;
                }
                RecoveryActionKind::ResumeCloseout => {
                    saw_non_remove = true;
                    if seen_resume {
                        return Err(Error::RecoveryLedgerInvalid);
                    }
                    seen_resume = true;
                }
            }
        }
        let expected_prefix_len = entry.plan.actions.len() + 1;
        if entry.plan.prefix_subject_sha256.len() != expected_prefix_len {
            return Err(Error::RecoveryLedgerInvalid);
        }
        if entry.plan.prefix_subject_sha256[0] != entry.plan.pre_subject_sha256 {
            return Err(Error::RecoveryLedgerInvalid);
        }
        for p in &entry.plan.prefix_subject_sha256 {
            if !is_valid_sha64(p) {
                return Err(Error::RecoveryLedgerInvalid);
            }
        }
        // Recovery ID is the SHA-256 of the compact canonical plan seed.
        let seed_json =
            serde_json::to_string(&entry.plan).map_err(|_| Error::RecoveryLedgerInvalid)?;
        if sha256_hex(seed_json.as_bytes()) != entry.recovery_id {
            return Err(Error::RecoveryLedgerInvalid);
        }
        let action_count = entry.plan.actions.len();
        if entry.next_action > action_count {
            return Err(Error::RecoveryLedgerInvalid);
        }
        match entry.status.as_str() {
            "PENDING" => {
                if pending_index.is_some() {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                pending_index = Some(i);
                if entry.post_subject_sha256.is_some()
                    || entry.recovery_receipt.is_some()
                    || entry.recovery_receipt_sha256.is_some()
                {
                    return Err(Error::RecoveryLedgerInvalid);
                }
            }
            "APPLIED" => {
                if entry.next_action != action_count {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                let receipt = entry
                    .recovery_receipt
                    .as_ref()
                    .ok_or(Error::RecoveryLedgerInvalid)?;
                let post = entry
                    .post_subject_sha256
                    .as_deref()
                    .ok_or(Error::RecoveryLedgerInvalid)?;
                let receipt_sha = entry
                    .recovery_receipt_sha256
                    .as_deref()
                    .ok_or(Error::RecoveryLedgerInvalid)?;
                // Every stored subject and receipt hash is lowercase 64-hex.
                if !is_valid_sha64(post)
                    || !is_valid_sha64(receipt.pre_subject_sha256.as_str())
                    || !is_valid_sha64(receipt.post_subject_sha256.as_str())
                    || !is_valid_sha64(receipt_sha)
                {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                // The stored post-subject must be exactly the final prefix.
                let final_prefix = entry
                    .plan
                    .prefix_subject_sha256
                    .last()
                    .ok_or(Error::RecoveryLedgerInvalid)?;
                if post != final_prefix {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if receipt.schema_version != 1 {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if receipt.accepted_plan_sha256 != file.accepted_plan_sha256
                    || receipt.plan_id != file.plan_id
                {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if receipt.recovery_sequence != (i as u32) + 1 {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if receipt.recovery_id != entry.recovery_id {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if receipt.pre_subject_sha256 != entry.plan.pre_subject_sha256 {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if receipt.post_subject_sha256 != post {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if receipt.action_count != action_count {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                let actions_json = serde_json::to_string(&entry.plan.actions)
                    .map_err(|_| Error::RecoveryLedgerInvalid)?;
                if receipt.actions_sha256 != sha256_hex(actions_json.as_bytes()) {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if receipt.previous_recovery_receipt_sha256 != previous_receipt_hash {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                let receipt_json =
                    serde_json::to_string(receipt).map_err(|_| Error::RecoveryLedgerInvalid)?;
                if sha256_hex(receipt_json.as_bytes()) != receipt_sha {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                if entry.recovery_receipt_sha256.as_deref() != Some(receipt_sha) {
                    return Err(Error::RecoveryLedgerInvalid);
                }
                previous_receipt_hash = Some(receipt_sha.to_string());
            }
            _ => return Err(Error::RecoveryLedgerInvalid),
        }
    }
    // At most one final entry may be pending, and no entry may follow it.
    if let Some(idx) = pending_index {
        if idx != file.recoveries.len() - 1 {
            return Err(Error::RecoveryLedgerInvalid);
        }
    }
    Ok(())
}

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

// ============================================================================
// Temporary-file grammar (contract section 13)
// ============================================================================

enum TempClass {
    Target(String),
    RecoveryOwned(String, usize),
}

/// Parse a recognized temporary filename. `None` means the name is not a
/// recognized temporary grammar (unknown or malformed).
fn classify_temp_name(name: &str) -> Option<TempClass> {
    // Phase 8 recovery-owned: .recovery-<rid>-<index>.tmp
    if let Some(rest) = name.strip_prefix(".recovery-") {
        if let Some(idx_str) = rest.strip_suffix(".tmp") {
            if let Some(dash) = idx_str.rfind('-') {
                let rid = &idx_str[..dash];
                let index_str = &idx_str[dash + 1..];
                if is_valid_sha64(rid)
                    && !index_str.is_empty()
                    && index_str.bytes().all(|b| b.is_ascii_digit())
                {
                    if let Ok(index) = index_str.parse::<usize>() {
                        // Canonical index only: aliases with leading zeros or
                        // other spellings are not the deterministic grammar.
                        if index_str == index.to_string() {
                            return Some(TempClass::RecoveryOwned(rid.to_string(), index));
                        }
                    }
                }
            }
        }
        return None;
    }
    // Phase 6 closeout: .closeout.{attempt}.tmp -> completion-ledger.json
    if let Some(rest) = name.strip_prefix(".closeout.") {
        if let Some(attempt) = rest.strip_suffix(".tmp") {
            if !attempt.is_empty() && attempt.bytes().all(|b| b.is_ascii_digit()) {
                return Some(TempClass::Target("completion-ledger.json".to_string()));
            }
        }
        return None;
    }
    // Phase 6 closeout state: .closeout-state.{attempt}.tmp -> state.json
    if let Some(rest) = name.strip_prefix(".closeout-state.") {
        if let Some(attempt) = rest.strip_suffix(".tmp") {
            if !attempt.is_empty() && attempt.bytes().all(|b| b.is_ascii_digit()) {
                if let Ok(index) = attempt.parse::<usize>() {
                    // Canonical attempt only: aliases with leading zeros or
                    // other spellings are not the deterministic grammar.
                    if attempt == index.to_string() {
                        return Some(TempClass::Target("state.json".to_string()));
                    }
                }
            }
        }
        return None;
    }
    // Phase 7 continuity: .continuity.{attempt}.tmp -> continuity-ledger.json
    if let Some(rest) = name.strip_prefix(".continuity.") {
        if let Some(attempt) = rest.strip_suffix(".tmp") {
            if !attempt.is_empty() && attempt.bytes().all(|b| b.is_ascii_digit()) {
                return Some(TempClass::Target("continuity-ledger.json".to_string()));
            }
        }
        return None;
    }
    // Phase 4 implementation: .mrgs_impl_tmp_{pid}_{attempt}_{nanos}.tmp
    if let Some(rest) = name.strip_prefix(".mrgs_impl_tmp_") {
        if let Some(mid) = rest.strip_suffix(".tmp") {
            let fields: Vec<&str> = mid.split('_').collect();
            if fields.len() == 3
                && fields
                    .iter()
                    .all(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
            {
                return Some(TempClass::Target(
                    "implementation-authority.json".to_string(),
                ));
            }
        }
        return None;
    }
    // Phase 5 audit: .mrgs_audit_tmp_{pid}_{attempt}_{nanos}.tmp
    if let Some(rest) = name.strip_prefix(".mrgs_audit_tmp_") {
        if let Some(mid) = rest.strip_suffix(".tmp") {
            let fields: Vec<&str> = mid.split('_').collect();
            if fields.len() == 3
                && fields
                    .iter()
                    .all(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
            {
                return Some(TempClass::Target("audit-ledger.json".to_string()));
            }
        }
        return None;
    }
    // Phase 1 producer: .{pid}.{count}.{ts}.{filename}.tmp
    if let Some(rest) = name.strip_prefix('.') {
        if let Some(mid) = rest.strip_suffix(".tmp") {
            let mut parts = mid.splitn(4, '.');
            let pid = parts.next()?;
            let count = parts.next()?;
            let ts = parts.next()?;
            let filename = parts.next()?;
            let numeric = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
            if numeric(pid) && numeric(count) && numeric(ts) && PERMANENT_FILES.contains(&filename)
            {
                return Some(TempClass::Target(filename.to_string()));
            }
        }
        return None;
    }
    None
}

fn recovery_temp_name(recovery_id: &str, index: usize) -> String {
    format!(".recovery-{}-{}.tmp", recovery_id, index)
}

/// Whether a recognized state.json-targeting temp is the Phase 6 closeout
/// state-write temp (canonical attempt spelling only).
fn is_closeout_state_temp_name(name: &str) -> bool {
    name.strip_prefix(".closeout-state.")
        .and_then(|rest| rest.strip_suffix(".tmp"))
        .map(|attempt| {
            !attempt.is_empty()
                && attempt.bytes().all(|b| b.is_ascii_digit())
                && attempt
                    .parse::<usize>()
                    .map(|index| attempt == index.to_string())
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}

// ============================================================================
// Subject capture (contract section 5)
// ============================================================================

fn safe_plan_path_str(path_str: &str) -> Option<String> {
    crate::path::validate_safe_relative_path(path_str).ok()?;
    crate::path::validate_strict_normalized_path(path_str).ok()?;
    // Platform-independent drive-prefix and control-character rejection so
    // aliases and noncanonical spellings never resolve.
    if path_str.len() >= 2
        && path_str.as_bytes()[0].is_ascii_alphabetic()
        && path_str.as_bytes()[1] == b':'
    {
        return None;
    }
    if path_str.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(path_str.to_string())
}

/// The plan-source path recorded inside the simulated inventory, in the same
/// deterministic order as live capture: the simulated accepted-plan bytes
/// first, then the simulated final completion-manifest bytes. Strict
/// normalized-path rules apply to the recorded path.
fn recorded_plan_path_from_entries(entries: &[InventoryEntry]) -> Option<String> {
    if let Some(entry) = entries.iter().find(|e| e.filename == "accepted-plan.json") {
        if let Some(bytes) = entry.bytes.as_deref() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) {
                if let Some(p) = v.get("plan_path").and_then(|x| x.as_str()) {
                    if let Some(clean) = safe_plan_path_str(p) {
                        return Some(clean);
                    }
                }
            }
        }
    }
    if let Some(entry) = entries
        .iter()
        .find(|e| e.filename == "completion-ledger.json")
    {
        if let Some(bytes) = entry.bytes.as_deref() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) {
                if let Some(arr) = v.get("completions").and_then(|x| x.as_array()) {
                    if let Some(last) = arr.last() {
                        if let Some(p) = last
                            .get("final_manifest")
                            .and_then(|m| m.get("plan_source_path"))
                            .and_then(|x| x.as_str())
                        {
                            if let Some(clean) = safe_plan_path_str(p) {
                                return Some(clean);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Capture the actual plan-source topology and bytes from the repository at
/// a strict normalized recorded path.
fn capture_plan_source_at(
    identity: &Identity,
    path: &str,
) -> Result<Option<PlanSourceInfo>, Error> {
    let full = identity.repo.join(path);
    let meta = match std::fs::symlink_metadata(&full) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::RecoveryUnrecoverable),
    };
    let kind = entry_kind_for(&meta);
    if kind == "REGULAR" {
        let bytes = std::fs::read(&full).map_err(|_| Error::RecoveryUnrecoverable)?;
        Ok(Some(PlanSourceInfo {
            path: path.to_string(),
            topology: "REGULAR".to_string(),
            byte_length: Some(bytes.len() as u64),
            sha256: Some(sha256_hex(&bytes)),
        }))
    } else {
        Ok(Some(PlanSourceInfo {
            path: path.to_string(),
            topology: kind.to_string(),
            byte_length: None,
            sha256: None,
        }))
    }
}

/// The plan-source object derived from the simulated governance inventory
/// plus the repository's actual plan-source topology and bytes. Prefix
/// simulation is self-contained: after a simulated RESTORE_ACCEPTED_PLAN
/// the next prefix uses the reconstructed record's plan path, never the
/// live files. Produces the same result as live capture when the inventory
/// represents the live state.
fn plan_source_from_entries(
    identity: &Identity,
    entries: &[InventoryEntry],
) -> Result<Option<PlanSourceInfo>, Error> {
    match recorded_plan_path_from_entries(entries) {
        Some(path) => capture_plan_source_at(identity, &path),
        None => Ok(None),
    }
}

/// Capture the complete recovery subject. `journal` supplies the pending
/// recovery ID used to authorize recovery-owned temporary files.
fn capture_subject(
    identity: &Identity,
    journal: Option<&JournalState>,
) -> Result<(Vec<InventoryEntry>, String), Error> {
    let mut entries: Vec<InventoryEntry> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let dir_entries =
        std::fs::read_dir(&identity.gov_dir).map_err(|_| Error::RecoveryUnrecoverable)?;
    for entry in dir_entries {
        let entry = entry.map_err(|_| Error::RecoveryUnrecoverable)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::RecoveryUnrecoverable)?;
        if name == RECOVERY_LEDGER_FILENAME {
            continue;
        }
        let meta =
            std::fs::symlink_metadata(entry.path()).map_err(|_| Error::RecoveryUnrecoverable)?;
        let kind = entry_kind_for(&meta);
        let filename = name.clone();
        if seen.contains(&filename) {
            continue;
        }
        seen.insert(filename.clone());
        if PERMANENT_FILES.contains(&filename.as_str()) {
            if kind != "REGULAR" {
                // Symlink, junction, directory, device, or other unsafe
                // object at a permanent governance filename.
                return Err(Error::FilesystemBoundaryUnsafe);
            }
            let bytes = std::fs::read(entry.path()).map_err(|_| Error::RecoveryUnrecoverable)?;
            entries.push(InventoryEntry {
                filename,
                kind,
                bytes: Some(bytes),
            });
            continue;
        }
        // Temporary or unknown child.
        match classify_temp_name(&filename) {
            Some(TempClass::RecoveryOwned(rid, _index)) => {
                // Authorized only when a valid pending journal binds the same
                // recovery ID; otherwise the leftover is unrecoverable.
                let authorized = match journal.as_ref().and_then(|j| j.pending) {
                    Some(i) => journal.as_ref().unwrap().file.recoveries[i].recovery_id == rid,
                    None => false,
                };
                if !authorized {
                    return Err(Error::RecoveryUnrecoverable);
                }
                if kind != "REGULAR" {
                    return Err(Error::FilesystemBoundaryUnsafe);
                }
                let bytes =
                    std::fs::read(entry.path()).map_err(|_| Error::RecoveryUnrecoverable)?;
                entries.push(InventoryEntry {
                    filename,
                    kind,
                    bytes: Some(bytes),
                });
            }
            Some(TempClass::Target(_)) => {
                if kind != "REGULAR" {
                    return Err(Error::FilesystemBoundaryUnsafe);
                }
                let bytes =
                    std::fs::read(entry.path()).map_err(|_| Error::RecoveryUnrecoverable)?;
                entries.push(InventoryEntry {
                    filename,
                    kind,
                    bytes: Some(bytes),
                });
            }
            None => {
                // Unknown direct child: unrecoverable. Unsupported objects
                // (symlink, directory, device, FIFO) are filesystem-boundary
                // violations; unknown regular names are unrecoverable.
                if kind != "REGULAR" {
                    return Err(Error::FilesystemBoundaryUnsafe);
                }
                return Err(Error::RecoveryUnrecoverable);
            }
        }
    }
    // Fixed ABSENT inventory entries so identical missing-state subjects
    // hash identically.
    for name in PERMANENT_FILES {
        if !seen.contains(name) {
            entries.push(InventoryEntry {
                filename: name.to_string(),
                kind: "ABSENT",
                bytes: None,
            });
        }
    }
    entries.sort_by(|a, b| a.filename.as_bytes().cmp(b.filename.as_bytes()));
    let sha = subject_sha_from_entries(identity, &entries)?;
    Ok((entries, sha))
}

fn subject_sha_from_entries(
    identity: &Identity,
    entries: &[InventoryEntry],
) -> Result<String, Error> {
    let governance_entries = entries
        .iter()
        .map(|e| GovernanceEntry {
            filename: e.filename.clone(),
            kind: e.kind.to_string(),
            byte_length: e.bytes.as_ref().map(|b| b.len() as u64),
            sha256: e.bytes.as_ref().map(|b| sha256_hex(b)),
        })
        .collect();
    let plan_source = plan_source_from_entries(identity, entries)?;
    let subject = RecoverySubject {
        schema_version: 1,
        repository_git_object_format: identity.objfmt.clone(),
        repository_head: identity.head.clone(),
        repository_branch: identity.branch.clone(),
        governance_entries,
        plan_source,
    };
    let json = serde_json::to_string(&subject).map_err(|_| Error::RecoveryUnrecoverable)?;
    Ok(sha256_hex(json.as_bytes()))
}

// ============================================================================
// Accepted-plan authority (contract section 8)
// ============================================================================

struct AcceptedAuthority {
    record: AcceptedPlan,
    plan: crate::plan::Plan,
    reconstructed: bool,
}

/// Validate the plan source at the recorded path: safe regular file, exact
/// bytes, exact SHA-256, parse, and plan consistency.
fn validate_plan_source(
    identity: &Identity,
    plan_path_str: &str,
    expected_sha256: &str,
    expected_plan_id: &str,
    expected_phase_count: usize,
) -> Result<(crate::plan::Plan, String, Vec<u8>), Error> {
    // Non-following topology inspection first: a symlink, junction, or other
    // reparse point at the plan source is a filesystem-boundary violation.
    let raw_path = identity.repo.join(plan_path_str);
    let meta = std::fs::symlink_metadata(&raw_path).map_err(|_| Error::RecoveryUnrecoverable)?;
    if !meta.file_type().is_file() || is_reparse_point(&meta) {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    let plan_path = crate::path::resolve_safe_plan_path(&identity.repo, plan_path_str)
        .map_err(|_| Error::RecoveryUnrecoverable)?;
    let bytes = std::fs::read(&plan_path).map_err(|_| Error::RecoveryUnrecoverable)?;
    let sha = sha256_hex(&bytes);
    if sha != expected_sha256 {
        return Err(Error::RecoveryUnrecoverable);
    }
    let text = String::from_utf8(bytes.clone()).map_err(|_| Error::RecoveryUnrecoverable)?;
    let plan: crate::plan::Plan =
        toml::from_str(&text).map_err(|_| Error::RecoveryUnrecoverable)?;
    plan.validate().map_err(|_| Error::RecoveryUnrecoverable)?;
    if plan.plan_id != expected_plan_id {
        return Err(Error::RecoveryUnrecoverable);
    }
    if plan.phases.len() != expected_phase_count {
        return Err(Error::RecoveryUnrecoverable);
    }
    Ok((plan, sha, bytes))
}

/// Raw completion-ledger read (topology, raw keys, parse) without the
/// accepted-plan binding.
fn read_completion_raw(identity: &Identity) -> Result<Option<CompletionLedger>, Error> {
    closeout::read_completion_ledger(&identity.gov_dir).map_err(|_| Error::RecoveryUnrecoverable)
}

/// Existing accepted-plan authority, or exact reconstruction from a valid
/// self-contained completion ledger and the exact plan source.
fn derive_accepted_authority(
    identity: &Identity,
    completion: Option<&CompletionLedger>,
) -> Result<AcceptedAuthority, Error> {
    let path = identity.gov_dir.join("accepted-plan.json");
    match std::fs::symlink_metadata(&path) {
        Ok(meta) => {
            if !meta.file_type().is_file() || is_reparse_point(&meta) {
                return Err(Error::FilesystemBoundaryUnsafe);
            }
            let bytes = std::fs::read(&path).map_err(|_| Error::RecoveryUnrecoverable)?;
            // A malformed regular accepted-plan falls through to exact
            // reconstruction; only unsafe topology is a hard boundary error.
            let existing = (|| -> Result<AcceptedAuthority, Error> {
                let raw: serde_json::Value =
                    serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryUnrecoverable)?;
                // Exact raw-key validation: no missing and no unknown keys.
                exact_keys(&raw, &ACCEPTED_PLAN_KEYS).map_err(|_| Error::RecoveryUnrecoverable)?;
                let plan_path_str = raw["plan_path"]
                    .as_str()
                    .ok_or(Error::RecoveryUnrecoverable)?;
                // Strict normalized plan path, not merely safe resolution:
                // backslashes, dot segments, repeated separators, leading or
                // trailing separators, drive prefixes, control characters,
                // and `.mrgs` paths are all rejected.
                safe_plan_path_str(plan_path_str).ok_or(Error::RecoveryUnrecoverable)?;
                let record: AcceptedPlan =
                    serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryUnrecoverable)?;
                state::validate_accepted_plan_record(&record)
                    .map_err(|_| Error::RecoveryUnrecoverable)?;
                let (plan, sha, _plan_bytes) = validate_plan_source(
                    identity,
                    &record.plan_path,
                    &record.sha256,
                    &record.plan_id,
                    record.phase_count,
                )?;
                state::validate_plan_consistency(&record, &plan, &sha)
                    .map_err(|_| Error::RecoveryUnrecoverable)?;
                Ok(AcceptedAuthority {
                    record,
                    plan,
                    reconstructed: false,
                })
            })();
            match existing {
                Ok(authority) => Ok(authority),
                // Unsafe plan-source topology is unrecoverable regardless of
                // the record; everything else is a reconstruction candidate.
                Err(Error::FilesystemBoundaryUnsafe) => Err(Error::FilesystemBoundaryUnsafe),
                Err(_) => reconstruct_accepted_plan(identity, completion),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            reconstruct_accepted_plan(identity, completion)
        }
        Err(_) => Err(Error::RecoveryUnrecoverable),
    }
}

/// A malformed or missing accepted-plan is recoverable only from a valid
/// self-contained completion ledger and the exact plan source.
fn reconstruct_accepted_plan(
    identity: &Identity,
    completion: Option<&CompletionLedger>,
) -> Result<AcceptedAuthority, Error> {
    let ledger = completion.ok_or(Error::RecoveryUnrecoverable)?;
    let entries = &ledger.completions;
    let first_manifest = &entries[0].final_manifest;
    let plan_path_str =
        safe_plan_path_str(&first_manifest.plan_source_path).ok_or(Error::RecoveryUnrecoverable)?;
    // Every final manifest agrees on plan ID, path, content, accepted-plan
    // SHA-256, and phase inventory.
    for entry in entries {
        let m = &entry.final_manifest;
        if m.plan_id != first_manifest.plan_id
            || m.plan_source_path != first_manifest.plan_source_path
            || m.plan_content != first_manifest.plan_content
            || m.accepted_plan_sha256 != first_manifest.accepted_plan_sha256
        {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    let accepted_sha256 = ledger.accepted_plan_sha256.clone();
    // The plan source currently exists as a safe regular file at the exact
    // stored path; current bytes exactly equal the archived content/SHA.
    let raw_path = identity.repo.join(&plan_path_str);
    let meta = std::fs::symlink_metadata(&raw_path).map_err(|_| Error::RecoveryUnrecoverable)?;
    if !meta.file_type().is_file() || is_reparse_point(&meta) {
        return Err(Error::FilesystemBoundaryUnsafe);
    }
    let plan_path = crate::path::resolve_safe_plan_path(&identity.repo, &plan_path_str)
        .map_err(|_| Error::RecoveryUnrecoverable)?;
    let plan_bytes = std::fs::read(&plan_path).map_err(|_| Error::RecoveryUnrecoverable)?;
    let plan_sha = sha256_hex(&plan_bytes);
    if plan_sha != accepted_sha256 {
        return Err(Error::RecoveryUnrecoverable);
    }
    let plan_text =
        String::from_utf8(plan_bytes.clone()).map_err(|_| Error::RecoveryUnrecoverable)?;
    if plan_text != first_manifest.plan_content {
        return Err(Error::RecoveryUnrecoverable);
    }
    let plan: crate::plan::Plan =
        toml::from_str(&plan_text).map_err(|_| Error::RecoveryUnrecoverable)?;
    plan.validate().map_err(|_| Error::RecoveryUnrecoverable)?;
    if plan.plan_id != ledger.plan_id {
        return Err(Error::RecoveryUnrecoverable);
    }
    // Phase inventory agreement: every manifest's phase binds to the plan.
    for entry in entries {
        let m = &entry.final_manifest;
        let phase = plan
            .phases
            .get(m.plan_phase_index)
            .ok_or(Error::RecoveryUnrecoverable)?;
        if phase.id != m.phase_id
            || phase.title != m.phase_title
            || phase.depends_on != m.phase_dependencies
        {
            return Err(Error::RecoveryUnrecoverable);
        }
        if m.accepted_plan_sha256 != accepted_sha256 {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    let record = AcceptedPlan {
        schema_version: 1,
        plan_id: plan.plan_id.clone(),
        plan_path: plan_path_str.clone(),
        sha256: accepted_sha256.clone(),
        phase_count: plan.phases.len(),
    };
    state::validate_accepted_plan_record(&record).map_err(|_| Error::RecoveryUnrecoverable)?;
    Ok(AcceptedAuthority {
        record,
        plan,
        reconstructed: true,
    })
}

// ============================================================================
// Completion and continuity ledgers (contract section 11, obligations 41-42)
// ============================================================================

fn validate_completion_against(
    ledger: &CompletionLedger,
    accepted: &AcceptedAuthority,
) -> Result<(), Error> {
    closeout::validate_completion_ledger(ledger, &accepted.record.sha256, &accepted.plan.plan_id)
        .map_err(|_| Error::RecoveryUnrecoverable)
}

fn validate_continuity(
    identity: &Identity,
    accepted: &AcceptedAuthority,
    completion: Option<&CompletionLedger>,
) -> Result<(), Error> {
    let ledger = crate::continuity::read_continuity_ledger(&identity.gov_dir)?;
    if let Some(ref ledger) = ledger {
        let completion = completion.ok_or(Error::RecoveryUnrecoverable)?;
        crate::continuity::validate_continuity_ledger(
            ledger,
            &accepted.record.sha256,
            &accepted.plan.plan_id,
            completion,
        )
        .map_err(|_| Error::RecoveryUnrecoverable)?;
    }
    Ok(())
}

// ============================================================================
// Phase-scoped authority validation (contract section 10)
// ============================================================================

struct PhaseScopedState {
    draft: Option<state::ContractDraft>,
    accepted_contract: Option<state::AcceptedContractLedger>,
    implementation_authority: Option<state::ImplementationAuthority>,
    audit_ledger: Option<crate::audit::AuditLedger>,
}

/// Read phase-scoped files with safe-topology checks; enforce the contiguous
/// prefix rule (a later file requires every predecessor).
fn read_phase_scoped(identity: &Identity) -> Result<PhaseScopedState, Error> {
    let mut present: Vec<&str> = Vec::new();
    for name in PHASE_SCOPED_FILES {
        let path = identity.gov_dir.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.file_type().is_file() || is_reparse_point(&meta) {
                    return Err(Error::FilesystemBoundaryUnsafe);
                }
                present.push(name);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::RecoveryUnrecoverable),
        }
    }
    // Contiguity: if any later file exists, every predecessor must exist.
    let mut first_missing: Option<usize> = None;
    for (i, name) in PHASE_SCOPED_FILES.iter().enumerate() {
        if !present.contains(name) {
            if first_missing.is_none() {
                first_missing = Some(i);
            }
        } else if first_missing.is_some() {
            // A later file exists after a missing predecessor.
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    let read = |name: &str| -> Result<Vec<u8>, Error> {
        std::fs::read(identity.gov_dir.join(name)).map_err(|_| Error::RecoveryUnrecoverable)
    };
    let draft = if present.contains(&"contract-draft.json") {
        let bytes = read("contract-draft.json")?;
        Some(serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryUnrecoverable)?)
    } else {
        None
    };
    let accepted_contract = if present.contains(&"accepted-contract.json") {
        let bytes = read("accepted-contract.json")?;
        Some(serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryUnrecoverable)?)
    } else {
        None
    };
    let implementation_authority = if present.contains(&"implementation-authority.json") {
        let bytes = read("implementation-authority.json")?;
        Some(serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryUnrecoverable)?)
    } else {
        None
    };
    let audit_ledger = if present.contains(&"audit-ledger.json") {
        let bytes = read("audit-ledger.json")?;
        Some(serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryUnrecoverable)?)
    } else {
        None
    };
    Ok(PhaseScopedState {
        draft,
        accepted_contract,
        implementation_authority,
        audit_ledger,
    })
}

/// Fully validate and bind every present phase-scoped successor to the
/// candidate phase (contract section 10).
fn validate_phase_scoped_bindings(
    identity: &Identity,
    scoped: &PhaseScopedState,
    accepted: &AcceptedAuthority,
    candidate_phase: &str,
) -> Result<(), Error> {
    if let Some(draft) = &scoped.draft {
        state::validate_contract_draft_record(
            draft,
            &accepted.record.sha256,
            candidate_phase,
            &draft.contract_id,
        )
        .map_err(|_| Error::RecoveryUnrecoverable)?;
    }
    if let Some(ledger) = &scoped.accepted_contract {
        state::validate_accepted_contract_ledger(
            ledger,
            &accepted.record.sha256,
            candidate_phase,
            scoped.draft.as_ref(),
        )
        .map_err(|_| Error::RecoveryUnrecoverable)?;
    }
    if let Some(record) = &scoped.implementation_authority {
        implementation::validate_impl_record_structure(record, &identity.objfmt)
            .map_err(|_| Error::RecoveryUnrecoverable)?;
        // Bind to the inferred accepted contract's final revision.
        let ledger = scoped
            .accepted_contract
            .as_ref()
            .ok_or(Error::RecoveryUnrecoverable)?;
        let final_rev = ledger
            .revisions
            .last()
            .ok_or(Error::RecoveryUnrecoverable)?;
        if record.accepted_plan_sha256 != accepted.record.sha256
            || record.phase_id != candidate_phase
            || record.contract_id != ledger.contract_id
            || record.contract_revision != final_rev.revision
            || record.contract_source_path != final_rev.source_path
            || record.contract_sha256 != final_rev.sha256
            || record.contract_content != final_rev.content
        {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    if let Some(audit) = &scoped.audit_ledger {
        let record = scoped
            .implementation_authority
            .as_ref()
            .ok_or(Error::RecoveryUnrecoverable)?;
        validate_audit_ledger_full(audit, record, accepted, candidate_phase)?;
    }
    Ok(())
}

// ============================================================================
// Audit ledger validation (Phase 5 rules, obligations 28-30)
// ============================================================================

fn validate_audit_ledger_full(
    ledger: &crate::audit::AuditLedger,
    record: &state::ImplementationAuthority,
    accepted: &AcceptedAuthority,
    candidate_phase: &str,
) -> Result<(), Error> {
    if ledger.schema_version != 1 {
        return Err(Error::RecoveryUnrecoverable);
    }
    // Bind to the inferred implementation authority.
    if ledger.accepted_plan_sha256 != accepted.record.sha256
        || ledger.phase_id != candidate_phase
        || ledger.contract_id != record.contract_id
        || ledger.contract_revision != record.contract_revision
        || ledger.contract_source_path != record.contract_source_path
        || ledger.contract_sha256 != record.contract_sha256
        || ledger.implementation_baseline_head != record.baseline_head
        || ledger.implementation_baseline_branch != record.baseline_branch
        || ledger.git_object_format != record.git_object_format
    {
        return Err(Error::RecoveryUnrecoverable);
    }
    if ledger.max_repair_attempts != 2 {
        return Err(Error::RecoveryUnrecoverable);
    }
    // Full history validation.
    for (idx, round) in ledger.rounds.iter().enumerate() {
        let expected_round = (idx + 1) as u32;
        if round.round != expected_round {
            return Err(Error::RecoveryUnrecoverable);
        }
        if round.status != "PENDING" && round.status != "PASS" && round.status != "FAIL" {
            return Err(Error::RecoveryUnrecoverable);
        }
        let computed_hash = crate::audit::compute_subject_sha256(&round.subject)
            .map_err(|_| Error::RecoveryUnrecoverable)?;
        if computed_hash != round.subject_sha256 {
            return Err(Error::RecoveryUnrecoverable);
        }
        let computed_audit_id = crate::audit::compute_audit_id(
            &ledger.accepted_plan_sha256,
            &ledger.phase_id,
            &ledger.contract_id,
            ledger.contract_revision,
            &ledger.contract_sha256,
            round.round,
            &round.subject_sha256,
            &round.auditor_id,
        )
        .map_err(|_| Error::RecoveryUnrecoverable)?;
        if computed_audit_id != round.audit_id {
            return Err(Error::RecoveryUnrecoverable);
        }
        if idx > 0 {
            let prev = &ledger.rounds[idx - 1];
            if prev.status == "PASS" {
                return Err(Error::RecoveryUnrecoverable);
            }
            if prev.status == "FAIL" {
                if let Some(ref repair) = prev.repair {
                    if repair.status != "CHECKED" {
                        return Err(Error::RecoveryUnrecoverable);
                    }
                    if let Some(ref post_sha) = repair.post_subject_sha256 {
                        if *post_sha != round.subject_sha256 {
                            return Err(Error::RecoveryUnrecoverable);
                        }
                    }
                } else {
                    let checked = count_checked_repairs_before(ledger, idx - 1);
                    if checked >= ledger.max_repair_attempts {
                        return Err(Error::RecoveryUnrecoverable);
                    }
                }
            }
        }
        if let (Some(report_sha), Some(report_content)) =
            (&round.report_sha256, &round.report_content)
        {
            if sha256_hex(report_content.as_bytes()) != *report_sha {
                return Err(Error::RecoveryUnrecoverable);
            }
            let report: crate::audit::AuditReport =
                serde_json::from_str(report_content).map_err(|_| Error::RecoveryUnrecoverable)?;
            if report.audit_id != round.audit_id
                || report.subject_sha256 != round.subject_sha256
                || report.auditor_id != round.auditor_id
            {
                return Err(Error::RecoveryUnrecoverable);
            }
            validate_report_schema(&report)?;
            validate_verdict_consistency(&report)?;
            validate_report_coverage(&report, &record.contract_content)?;
        }
    }
    // Only the final round may be PENDING or carry a ROUTED unchecked repair.
    if ledger.rounds.len() > 1 {
        for round in &ledger.rounds[..ledger.rounds.len() - 1] {
            if round.status == "PENDING" {
                return Err(Error::RecoveryUnrecoverable);
            }
            if let Some(ref repair) = round.repair {
                if repair.status == "ROUTED" {
                    return Err(Error::RecoveryUnrecoverable);
                }
            }
        }
    }
    let mut repair_attempts = Vec::new();
    for round in &ledger.rounds {
        if let Some(ref repair) = round.repair {
            repair_attempts.push(repair.attempt);
        }
    }
    for (idx, &attempt) in repair_attempts.iter().enumerate() {
        if attempt != (idx as u32 + 1) {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    if repair_attempts.len() > 2 {
        return Err(Error::RecoveryUnrecoverable);
    }
    Ok(())
}

fn count_checked_repairs_before(ledger: &crate::audit::AuditLedger, before_idx: usize) -> u32 {
    let mut count = 0u32;
    for i in 0..before_idx.min(ledger.rounds.len()) {
        if let Some(ref repair) = ledger.rounds[i].repair {
            if repair.status == "CHECKED" {
                count += 1;
            }
        }
    }
    count
}

fn validate_report_schema(report: &crate::audit::AuditReport) -> Result<(), Error> {
    if report.schema_version != 1 {
        return Err(Error::RecoveryUnrecoverable);
    }
    if report.summary.trim().is_empty()
        || report.summary.contains('\0')
        || report.summary != report.summary.trim()
    {
        return Err(Error::RecoveryUnrecoverable);
    }
    for rr in &report.requirement_results {
        if rr.status != "PASS" && rr.status != "FAIL" && rr.status != "BLOCKED" {
            return Err(Error::RecoveryUnrecoverable);
        }
        if rr.evidence.trim().is_empty()
            || rr.evidence.contains('\0')
            || rr.evidence != rr.evidence.trim()
        {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    for vr in &report.verification_results {
        if vr.status != "PASS" && vr.status != "FAIL" && vr.status != "BLOCKED" {
            return Err(Error::RecoveryUnrecoverable);
        }
        if vr.evidence.trim().is_empty()
            || vr.evidence.contains('\0')
            || vr.evidence != vr.evidence.trim()
        {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    let mut finding_ids = BTreeSet::new();
    for finding in &report.findings {
        validate_finding_id(&finding.id)?;
        if finding.severity != "BLOCKER"
            && finding.severity != "MAJOR"
            && finding.severity != "MINOR"
        {
            return Err(Error::RecoveryUnrecoverable);
        }
        if finding.claim_kind != "REQUIREMENT" && finding.claim_kind != "VERIFICATION" {
            return Err(Error::RecoveryUnrecoverable);
        }
        if finding.claim_index < 1 {
            return Err(Error::RecoveryUnrecoverable);
        }
        if finding.summary.trim().is_empty()
            || finding.summary.contains('\0')
            || finding.summary != finding.summary.trim()
        {
            return Err(Error::RecoveryUnrecoverable);
        }
        if finding.evidence.trim().is_empty()
            || finding.evidence.contains('\0')
            || finding.evidence != finding.evidence.trim()
        {
            return Err(Error::RecoveryUnrecoverable);
        }
        if finding.repair_paths.is_empty() {
            return Err(Error::RecoveryUnrecoverable);
        }
        let mut seen_paths = BTreeSet::new();
        for rp in &finding.repair_paths {
            validate_repair_path(rp)?;
            if !seen_paths.insert(rp.clone()) {
                return Err(Error::RecoveryUnrecoverable);
            }
        }
        for i in 1..finding.repair_paths.len() {
            if finding.repair_paths[i].as_bytes() <= finding.repair_paths[i - 1].as_bytes() {
                return Err(Error::RecoveryUnrecoverable);
            }
        }
        if !finding_ids.insert(finding.id.clone()) {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    Ok(())
}

fn validate_finding_id(id: &str) -> Result<(), Error> {
    if id.is_empty() || id.len() > 64 || id.trim() != id {
        return Err(Error::RecoveryUnrecoverable);
    }
    let first = id.as_bytes()[0];
    if !first.is_ascii_alphanumeric() {
        return Err(Error::RecoveryUnrecoverable);
    }
    for &b in id.as_bytes() {
        if !b.is_ascii_alphanumeric() && b != b'.' && b != b'_' && b != b'-' {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    Ok(())
}

fn validate_repair_path(path: &str) -> Result<(), Error> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with("//")
        || path.contains('\\')
        || path.contains("//")
        || path.ends_with('/')
    {
        return Err(Error::RecoveryUnrecoverable);
    }
    if path.len() >= 2 {
        let bytes = path.as_bytes();
        if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    if path.chars().any(|c| c.is_ascii_control()) {
        return Err(Error::RecoveryUnrecoverable);
    }
    let first_seg = path.split('/').next().unwrap_or("");
    if first_seg.eq_ignore_ascii_case(".git") || first_seg.eq_ignore_ascii_case(".mrgs") {
        return Err(Error::RecoveryUnrecoverable);
    }
    if path.contains('*') || path.contains('?') || path.contains('[') || path.contains(']') {
        return Err(Error::RecoveryUnrecoverable);
    }
    Ok(())
}

fn validate_verdict_consistency(report: &crate::audit::AuditReport) -> Result<(), Error> {
    match report.verdict.as_str() {
        "PASS" => {
            for rr in &report.requirement_results {
                if rr.status != "PASS" {
                    return Err(Error::RecoveryUnrecoverable);
                }
            }
            for vr in &report.verification_results {
                if vr.status != "PASS" {
                    return Err(Error::RecoveryUnrecoverable);
                }
            }
            if !report.findings.is_empty() {
                return Err(Error::RecoveryUnrecoverable);
            }
        }
        "FAIL" => {
            let has_non_pass = report
                .requirement_results
                .iter()
                .any(|r| r.status != "PASS")
                || report
                    .verification_results
                    .iter()
                    .any(|r| r.status != "PASS");
            if !has_non_pass || report.findings.is_empty() {
                return Err(Error::RecoveryUnrecoverable);
            }
            for finding in &report.findings {
                let idx = (finding.claim_index - 1) as usize;
                let claim_ok = match finding.claim_kind.as_str() {
                    "REQUIREMENT" => report
                        .requirement_results
                        .get(idx)
                        .map(|rr| rr.status != "PASS")
                        .unwrap_or(false),
                    "VERIFICATION" => report
                        .verification_results
                        .get(idx)
                        .map(|vr| vr.status != "PASS")
                        .unwrap_or(false),
                    _ => false,
                };
                if !claim_ok {
                    return Err(Error::RecoveryUnrecoverable);
                }
            }
            for (idx, rr) in report.requirement_results.iter().enumerate() {
                if rr.status != "PASS" {
                    let claim_idx = (idx + 1) as u32;
                    if !report
                        .findings
                        .iter()
                        .any(|f| f.claim_kind == "REQUIREMENT" && f.claim_index == claim_idx)
                    {
                        return Err(Error::RecoveryUnrecoverable);
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
                        return Err(Error::RecoveryUnrecoverable);
                    }
                }
            }
        }
        _ => return Err(Error::RecoveryUnrecoverable),
    }
    Ok(())
}

fn validate_report_coverage(
    report: &crate::audit::AuditReport,
    contract_content: &str,
) -> Result<(), Error> {
    let contract: crate::contract::Contract =
        toml::from_str(contract_content).map_err(|_| Error::RecoveryUnrecoverable)?;
    if report.requirement_results.len() != contract.requirements.len() {
        return Err(Error::RecoveryUnrecoverable);
    }
    for (expected, actual) in contract
        .requirements
        .iter()
        .zip(report.requirement_results.iter())
    {
        if &actual.requirement != expected {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    if report.verification_results.len() != contract.verification_commands.len() {
        return Err(Error::RecoveryUnrecoverable);
    }
    for (expected, actual) in contract
        .verification_commands
        .iter()
        .zip(report.verification_results.iter())
    {
        if &actual.command != expected {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    Ok(())
}

// ============================================================================
// State derivation (contract sections 9-12)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum StateRelation {
    /// closed == closed_phases_after && active == null
    After,
    /// closed == closed_phases_after && active == a valid later phase
    LaterSelected,
    /// closed == closed_phases_before && active == active_phase_before
    PreCloseout,
    /// anything else (only recoverable as an incomplete closeout)
    Other,
}

fn relation_to_final(state: &GovernanceState, ledger: &CompletionLedger) -> StateRelation {
    let Some(final_entry) = ledger.completions.last() else {
        return StateRelation::Other;
    };
    let receipt = &final_entry.completion_receipt;
    if state.closed_phases == receipt.closed_phases_after && state.active_phase.is_none() {
        StateRelation::After
    } else if state.closed_phases == receipt.closed_phases_after && state.active_phase.is_some() {
        StateRelation::LaterSelected
    } else if state.closed_phases == receipt.closed_phases_before
        && state.active_phase == receipt.active_phase_before
    {
        StateRelation::PreCloseout
    } else {
        StateRelation::Other
    }
}

/// Check the contract-section-12 resumption preconditions for an incomplete
/// closeout of the final completion phase.
fn check_closeout_preconditions(
    identity: &Identity,
    scoped: &PhaseScopedState,
    ledger: &CompletionLedger,
) -> Result<(), Error> {
    let final_entry = ledger
        .completions
        .last()
        .ok_or(Error::RecoveryUnrecoverable)?;
    let archived = &final_entry.final_manifest.archived_governance;
    // Every remaining phase-scoped file must byte-match the archived copy;
    // the final completion entry must remain the unique applicable entry.
    let checks: [(&str, &str, &str); 4] = [
        (
            "contract-draft.json",
            &archived.contract_draft_content,
            &archived.contract_draft_sha256,
        ),
        (
            "accepted-contract.json",
            &archived.accepted_contract_content,
            &archived.accepted_contract_sha256,
        ),
        (
            "implementation-authority.json",
            &archived.implementation_authority_content,
            &archived.implementation_authority_sha256,
        ),
        (
            "audit-ledger.json",
            &archived.audit_ledger_content,
            &archived.audit_ledger_sha256,
        ),
    ];
    for (name, expected_content, expected_sha) in checks {
        let path = identity.gov_dir.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.file_type().is_file() || is_reparse_point(&meta) {
                    return Err(Error::FilesystemBoundaryUnsafe);
                }
                let bytes = std::fs::read(&path).map_err(|_| Error::RecoveryUnrecoverable)?;
                if bytes.as_slice() != expected_content.as_bytes()
                    || sha256_hex(&bytes) != *expected_sha
                {
                    return Err(Error::RecoveryUnrecoverable);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::RecoveryUnrecoverable),
        }
    }
    // No later-phase authority may coexist: the draft must bind to the final
    // phase (a later phase's files cannot byte-match the final archive).
    if let Some(draft) = &scoped.draft {
        if draft.phase_id != final_entry.completion_receipt.phase_id {
            return Err(Error::RecoveryUnrecoverable);
        }
    }
    Ok(())
}

/// Derive the exact state for a missing or malformed state.json.
/// Returns the derived state and the closeout phase when the derivation is
/// an incomplete closeout resumption.
fn derive_state(
    identity: &Identity,
    scoped: &PhaseScopedState,
    accepted: &AcceptedAuthority,
    completion: Option<&CompletionLedger>,
) -> Result<(GovernanceState, Option<String>), Error> {
    match completion {
        Some(ledger) => {
            let final_entry = ledger
                .completions
                .last()
                .ok_or(Error::RecoveryUnrecoverable)?;
            let receipt = &final_entry.completion_receipt;
            let final_phase = receipt.phase_id.clone();
            if let Some(draft) = &scoped.draft {
                if draft.phase_id == final_phase {
                    // Incomplete closeout: remaining files must byte-match
                    // the archived copies.
                    check_closeout_preconditions(identity, scoped, ledger)?;
                    let state = GovernanceState {
                        schema_version: 1,
                        accepted_plan_sha256: accepted.record.sha256.clone(),
                        active_phase: receipt.active_phase_before.clone(),
                        closed_phases: receipt.closed_phases_before.clone(),
                    };
                    validate_derived_state(&state, accepted)?;
                    return Ok((state, Some(final_phase)));
                }
                // Later selected phase: validate the whole prefix against the
                // draft phase.
                validate_phase_scoped_bindings(identity, scoped, accepted, &draft.phase_id)?;
                let state = GovernanceState {
                    schema_version: 1,
                    accepted_plan_sha256: accepted.record.sha256.clone(),
                    active_phase: Some(draft.phase_id.clone()),
                    closed_phases: receipt.closed_phases_after.clone(),
                };
                validate_derived_state(&state, accepted)?;
                return Ok((state, None));
            }
            if scoped.accepted_contract.is_some()
                || scoped.implementation_authority.is_some()
                || scoped.audit_ledger.is_some()
            {
                return Err(Error::RecoveryUnrecoverable);
            }
            let state = GovernanceState {
                schema_version: 1,
                accepted_plan_sha256: accepted.record.sha256.clone(),
                active_phase: None,
                closed_phases: receipt.closed_phases_after.clone(),
            };
            validate_derived_state(&state, accepted)?;
            Ok((state, None))
        }
        None => {
            if let Some(draft) = &scoped.draft {
                validate_phase_scoped_bindings(identity, scoped, accepted, &draft.phase_id)?;
                let state = GovernanceState {
                    schema_version: 1,
                    accepted_plan_sha256: accepted.record.sha256.clone(),
                    active_phase: Some(draft.phase_id.clone()),
                    closed_phases: vec![],
                };
                validate_derived_state(&state, accepted)?;
                return Ok((state, None));
            }
            if scoped.accepted_contract.is_some()
                || scoped.implementation_authority.is_some()
                || scoped.audit_ledger.is_some()
            {
                return Err(Error::RecoveryUnrecoverable);
            }
            let state = GovernanceState {
                schema_version: 1,
                accepted_plan_sha256: accepted.record.sha256.clone(),
                active_phase: None,
                closed_phases: vec![],
            };
            validate_derived_state(&state, accepted)?;
            Ok((state, None))
        }
    }
}

fn validate_derived_state(
    state: &GovernanceState,
    accepted: &AcceptedAuthority,
) -> Result<(), Error> {
    state::validate_state_record(state, &accepted.record, &accepted.plan)
        .map_err(|_| Error::RecoveryUnrecoverable)
}

/// Strict state decoding shared by analysis and replacement recomputation:
/// exact raw keys, type shapes, and the complete Phase 1 state validator.
/// "JSON deserialized" is never equivalent to "valid state": syntactically
/// invalid, unknown-field, wrong-schema, wrong-binding, invalid-active-phase,
/// and duplicate-closed-phase records all fail here.
fn strict_parse_state(bytes: &[u8], accepted: &AcceptedAuthority) -> Result<GovernanceState, ()> {
    let raw: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| ())?;
    exact_keys(&raw, &STATE_KEYS).map_err(|_| ())?;
    if !raw["active_phase"].is_null() && !raw["active_phase"].is_string() {
        return Err(());
    }
    if !raw["closed_phases"]
        .as_array()
        .map(|arr| arr.iter().all(|v| v.is_string()))
        .unwrap_or(false)
    {
        return Err(());
    }
    let state: GovernanceState = serde_json::from_value(raw).map_err(|_| ())?;
    state::validate_state_record(&state, &accepted.record, &accepted.plan).map_err(|_| ())?;
    Ok(state)
}

fn state_json_bytes(state: &GovernanceState) -> Result<String, Error> {
    serde_json::to_string_pretty(state).map_err(|_| Error::RecoveryUnrecoverable)
}

fn accepted_plan_json_bytes(record: &AcceptedPlan) -> Result<String, Error> {
    serde_json::to_string_pretty(record).map_err(|_| Error::RecoveryUnrecoverable)
}

// ============================================================================
// Redundant temporary classification (contract section 13)
// ============================================================================

/// Classify recognized producer temps: unambiguous mapping, existing valid
/// target, byte redundancy. Returns the sorted removal list. A differing
/// temp is unrecoverable except the recognized Phase 6 closeout state-write
/// temp during an incomplete closeout: that temp is the interrupted state
/// publication, whose validity is decided by the read-only virtual
/// normalization (`normalize_closeout_state_temp`), never rejected by the
/// live filesystem beforehand.
fn classify_redundant_temps(
    identity: &Identity,
    entries: &[InventoryEntry],
    closeout_incomplete: bool,
) -> Result<Vec<String>, Error> {
    let mut removals: Vec<String> = Vec::new();
    let mut seen_targets: BTreeSet<String> = BTreeSet::new();
    for entry in entries {
        let Some(TempClass::Target(target)) = classify_temp_name(&entry.filename) else {
            continue;
        };
        if !seen_targets.insert(target.clone()) {
            return Err(Error::RecoveryUnrecoverable);
        }
        let target_path = identity.gov_dir.join(&target);
        let target_meta =
            std::fs::symlink_metadata(&target_path).map_err(|_| Error::RecoveryUnrecoverable)?;
        if !target_meta.file_type().is_file() || is_reparse_point(&target_meta) {
            return Err(Error::RecoveryUnrecoverable);
        }
        let target_bytes = std::fs::read(&target_path).map_err(|_| Error::RecoveryUnrecoverable)?;
        let temp_bytes = entry.bytes.as_ref().ok_or(Error::RecoveryUnrecoverable)?;
        if temp_bytes.as_slice() != target_bytes.as_slice() {
            if closeout_incomplete
                && target == "state.json"
                && is_closeout_state_temp_name(&entry.filename)
            {
                continue;
            }
            return Err(Error::RecoveryUnrecoverable);
        }
        removals.push(entry.filename.clone());
    }
    removals.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    Ok(removals)
}

/// Read-only closeout-state temp normalization used by classification: the
/// recognized Phase 6 state-write temp is promoted in the plan-simulation
/// inventory (or removed when redundant); an invalid one fails closed
/// before any classification outcome. Temps already planned as REMOVE
/// actions (byte-redundant with state.json) are left for the action list.
/// The exact planned closeout-state normalization operations for the
/// interrupted Phase 6 state publication (validation only). The promotion
/// itself is deferred to the RESUME_CLOSEOUT action execution — never
/// applied before a leading action — so a crash between a promotion and a
/// leading action can never strand a subject that no stored prefix
/// describes.
fn normalize_closeout_state_temp(
    identity: &Identity,
    entries: &[InventoryEntry],
    planned_removals: &[String],
) -> Result<Vec<NormalizeOp>, Error> {
    let mut ops: Vec<NormalizeOp> = Vec::new();
    for item in entries {
        let Some(TempClass::Target(target)) = classify_temp_name(&item.filename) else {
            continue;
        };
        if target != "state.json" || !is_closeout_state_temp_name(&item.filename) {
            continue;
        }
        if planned_removals.iter().any(|t| t == &item.filename) {
            continue;
        }
        let temp_bytes = item.bytes.as_ref().ok_or(Error::RecoveryUnrecoverable)?;
        ops.push(closeout_state_temp_op(
            identity,
            &item.filename,
            temp_bytes,
        )?);
    }
    Ok(ops)
}

// ============================================================================
// Classification (contract sections 6-12)
// ============================================================================

enum Classification {
    Healthy,
    Recoverable(RecoveryPlan),
    Pending {
        recovery_id: String,
        next_action: usize,
        action_count: usize,
    },
}

struct Analysis {
    subject_sha256: String,
    classification: Classification,
    /// Planned closeout-state temp normalization ops for the fresh-execution
    /// path (empty unless an interrupted Phase 6 state publication exists).
    closeout_ops: Vec<NormalizeOp>,
}

fn analyze(identity: &Identity, journal: Option<&JournalState>) -> Result<Analysis, Error> {
    // 5. capture the complete recovery subject (journal-aware for
    //    recovery-owned temps).
    let (entries, subject_sha256) = capture_subject(identity, journal)?;

    // 7-8. accepted-plan authority or exact reconstruction; plan source.
    let completion_raw = read_completion_raw(identity)?;
    let accepted = derive_accepted_authority(identity, completion_raw.as_ref())?;
    let completion = match &completion_raw {
        Some(ledger) => {
            validate_completion_against(ledger, &accepted)?;
            Some(ledger.clone())
        }
        None => None,
    };

    // 4. journal plan binding: a structurally valid journal bound to
    //    different plan authority is stale.
    if let Some(j) = journal {
        if j.file.accepted_plan_sha256 != accepted.record.sha256
            || j.file.plan_id != accepted.plan.plan_id
        {
            return Err(Error::RecoveryLedgerStale);
        }
    }

    // 9. continuity ledger when present.
    validate_continuity(identity, &accepted, completion.as_ref())?;

    // 10. state and phase-scoped authority. A state that fails strict raw
    // key, type-shape, or semantic validation is a malformed reconstruction
    // candidate, treated exactly like a missing one: both derive one exact
    // state under Sections 9-12.
    let scoped = read_phase_scoped(identity)?;
    let state = std::fs::read(identity.gov_dir.join("state.json"))
        .ok()
        .and_then(|b| strict_parse_state(&b, &accepted).ok());

    let mut derived_state: Option<GovernanceState> = None;
    let mut closeout_phase: Option<String> = None;
    match state {
        Some(state) => {
            // A valid state with no completion ledger must have empty closed
            // phases (missing completion evidence is unrecoverable).
            if completion.is_none() && !state.closed_phases.is_empty() {
                return Err(Error::RecoveryUnrecoverable);
            }
            if let Some(ref ledger) = completion {
                let rel = relation_to_final(&state, ledger);
                match rel {
                    StateRelation::After => {
                        // Completed: phase files may only remain for the
                        // completed phase as an incomplete closeout.
                        if has_phase_files(&scoped) {
                            let final_phase = ledger
                                .completions
                                .last()
                                .unwrap()
                                .completion_receipt
                                .phase_id
                                .clone();
                            let draft_matches = scoped
                                .draft
                                .as_ref()
                                .map(|d| d.phase_id == final_phase)
                                .unwrap_or(false);
                            if draft_matches {
                                check_closeout_preconditions(identity, &scoped, ledger)?;
                                closeout_phase = Some(final_phase);
                            } else {
                                // Phase files without an active phase are
                                // orphaned authority.
                                return Err(Error::RecoveryUnrecoverable);
                            }
                        }
                    }
                    StateRelation::LaterSelected => {
                        // Validate surviving phase-scoped files against the
                        // selected phase.
                        if has_phase_files(&scoped) {
                            let active = state.active_phase.as_deref().unwrap_or_default();
                            validate_phase_scoped_bindings(identity, &scoped, &accepted, active)?;
                        }
                    }
                    StateRelation::PreCloseout => {
                        check_closeout_preconditions(identity, &scoped, ledger)?;
                        closeout_phase = Some(
                            ledger
                                .completions
                                .last()
                                .unwrap()
                                .completion_receipt
                                .phase_id
                                .clone(),
                        );
                    }
                    StateRelation::Other => {
                        if has_phase_files(&scoped) {
                            // Semantically valid but completion-inconsistent
                            // during an incomplete closeout: derive the exact
                            // receipt-bound state correction before any
                            // closeout resumption, so the finalizer never
                            // acts on an unverified state relation.
                            let (derived, closeout) =
                                derive_state(identity, &scoped, &accepted, Some(ledger))?;
                            derived_state = Some(derived);
                            closeout_phase = closeout;
                        } else {
                            // No phase-scoped files remain: the finalizer
                            // removes nothing and rewrites the exact
                            // after-state from the receipt-bound input.
                            check_closeout_preconditions(identity, &scoped, ledger)?;
                            closeout_phase = Some(
                                ledger
                                    .completions
                                    .last()
                                    .unwrap()
                                    .completion_receipt
                                    .phase_id
                                    .clone(),
                            );
                        }
                    }
                }
            } else if has_phase_files(&scoped) {
                // No completion ledger: phase files must bind to the active
                // phase.
                let active = state
                    .active_phase
                    .as_deref()
                    .ok_or(Error::RecoveryUnrecoverable)?;
                validate_phase_scoped_bindings(identity, &scoped, &accepted, active)?;
            }
        }
        None => {
            // Missing or malformed state: derive one exact state.
            let (derived, closeout) =
                derive_state(identity, &scoped, &accepted, completion.as_ref())?;
            derived_state = Some(derived);
            closeout_phase = closeout;
        }
    }

    // 6. classify every permanent and temporary governance entry. A
    //    differing closeout-state temp is tolerated only during an
    //    incomplete closeout (the recognized interrupted state
    //    publication); its validity is decided by the read-only virtual
    //    normalization below, never by the live filesystem beforehand.
    let redundant_temps = classify_redundant_temps(identity, &entries, closeout_phase.is_some())?;

    // 6b. closeout-state temp normalization decision: an invalid one fails
    //     closed before any classification outcome; the resulting ops are
    //     deferred to the RESUME_CLOSEOUT execution (never applied before a
    //     leading action). The plan simulation keeps the live temp in the
    //     inventory, and the RESUME action itself consumes it, so the
    //     stored prefix chain matches the real execution order exactly.
    let closeout_ops = normalize_closeout_state_temp(identity, &entries, &redundant_temps)?;

    // 11. derive one deterministic classification and action list.
    if let Some(j) = journal {
        if let Some(pending_idx) = j.pending {
            let entry = &j.file.recoveries[pending_idx];
            return Ok(Analysis {
                subject_sha256,
                classification: Classification::Pending {
                    recovery_id: entry.recovery_id.clone(),
                    next_action: entry.next_action,
                    action_count: entry.plan.actions.len(),
                },
                closeout_ops,
            });
        }
    }

    let plan = build_recovery_plan(
        identity,
        &accepted,
        &subject_sha256,
        &entries,
        &redundant_temps,
        derived_state.as_ref(),
        closeout_phase.as_ref(),
        completion.as_ref(),
    )?;
    let classification = if plan.seed.actions.is_empty() {
        Classification::Healthy
    } else {
        Classification::Recoverable(plan)
    };
    Ok(Analysis {
        subject_sha256,
        classification,
        closeout_ops,
    })
}

fn has_phase_files(scoped: &PhaseScopedState) -> bool {
    scoped.draft.is_some()
        || scoped.accepted_contract.is_some()
        || scoped.implementation_authority.is_some()
        || scoped.audit_ledger.is_some()
}

#[allow(clippy::too_many_arguments)]
fn build_recovery_plan(
    identity: &Identity,
    accepted: &AcceptedAuthority,
    subject_sha256: &str,
    entries: &[InventoryEntry],
    redundant_temps: &[String],
    derived_state: Option<&GovernanceState>,
    closeout_phase: Option<&String>,
    completion: Option<&CompletionLedger>,
) -> Result<RecoveryPlan, Error> {
    let mut actions: Vec<RecoveryAction> = Vec::new();
    for temp in redundant_temps {
        actions.push(RecoveryAction {
            kind: RecoveryActionKind::RemoveRedundantTemp,
            target: temp.clone(),
            replacement: None,
        });
    }
    if accepted.reconstructed {
        let replacement = accepted_plan_json_bytes(&accepted.record)?;
        actions.push(RecoveryAction {
            kind: RecoveryActionKind::RestoreAcceptedPlan,
            target: "accepted-plan.json".to_string(),
            replacement: Some(replacement),
        });
    }
    if let Some(state) = derived_state {
        let replacement = state_json_bytes(state)?;
        actions.push(RecoveryAction {
            kind: RecoveryActionKind::RestoreState,
            target: "state.json".to_string(),
            replacement: Some(replacement),
        });
    }
    let mut closeout_after: Option<GovernanceState> = None;
    if let Some(phase) = closeout_phase {
        actions.push(RecoveryAction {
            kind: RecoveryActionKind::ResumeCloseout,
            target: phase.clone(),
            replacement: None,
        });
        // The exact after-state written by the Phase 6 finalizer.
        let ledger = completion.ok_or(Error::RecoveryUnrecoverable)?;
        let receipt = &ledger
            .completions
            .last()
            .ok_or(Error::RecoveryUnrecoverable)?
            .completion_receipt;
        closeout_after = Some(GovernanceState {
            schema_version: 1,
            accepted_plan_sha256: accepted.record.sha256.clone(),
            active_phase: None,
            closed_phases: receipt.closed_phases_after.clone(),
        });
    }
    for action in &actions {
        action.validate()?;
    }

    // Deterministic prefix subject hashes: entry zero is the pre-subject;
    // each later entry is the expected subject after the corresponding
    // action prefix.
    let mut prefixes: Vec<String> = vec![subject_sha256.to_string()];
    let mut sim: Vec<InventoryEntry> = entries
        .iter()
        .filter(|e| {
            !matches!(
                classify_temp_name(&e.filename),
                Some(TempClass::RecoveryOwned(_, _))
            )
        })
        .cloned()
        .collect();
    for action in &actions {
        apply_action_to_inventory(&mut sim, action, closeout_after.as_ref())?;
        prefixes.push(subject_sha_from_entries(identity, &sim)?);
    }

    let seed = RecoveryPlanSeed {
        schema_version: 1,
        accepted_plan_sha256: accepted.record.sha256.clone(),
        plan_id: accepted.plan.plan_id.clone(),
        pre_subject_sha256: subject_sha256.to_string(),
        actions,
        prefix_subject_sha256: prefixes,
    };
    let recovery_id = compact_sha(&seed)?;
    Ok(RecoveryPlan { seed, recovery_id })
}

fn apply_action_to_inventory(
    entries: &mut Vec<InventoryEntry>,
    action: &RecoveryAction,
    closeout_after: Option<&GovernanceState>,
) -> Result<(), Error> {
    match action.kind {
        RecoveryActionKind::RemoveRedundantTemp => {
            entries.retain(|e| e.filename != action.target);
        }
        RecoveryActionKind::RestoreAcceptedPlan | RecoveryActionKind::RestoreState => {
            // The action's own replacement bytes: at plan construction they
            // are identical to the authority-derived bytes, and the pending
            // resume path re-verifies them by independent recomputation
            // before any mutation, so the simulation never trusts
            // journal-supplied content.
            let filename = if action.kind == RecoveryActionKind::RestoreAcceptedPlan {
                "accepted-plan.json"
            } else {
                "state.json"
            };
            let replacement = action
                .replacement
                .as_deref()
                .ok_or(Error::RecoveryUnrecoverable)?;
            replace_entry(entries, filename, replacement.as_bytes().to_vec());
        }
        RecoveryActionKind::ResumeCloseout => {
            for name in PHASE_SCOPED_FILES {
                entries.retain(|e| e.filename != name);
                entries.push(InventoryEntry {
                    filename: name.to_string(),
                    kind: "ABSENT",
                    bytes: None,
                });
            }
            // The finalizer writes the exact after-state.
            let after = closeout_after.ok_or(Error::RecoveryUnrecoverable)?;
            let bytes = state_json_bytes(after)?.into_bytes();
            replace_entry(entries, "state.json", bytes);
            // The interrupted state-publication temp is consumed by the
            // RESUME action itself (promoted over the pre-closeout state or
            // removed when redundant), so the simulation drops it exactly
            // when the real execution does.
            entries.retain(|e| !is_closeout_state_temp_name(&e.filename));
        }
    }
    entries.sort_by(|a, b| a.filename.as_bytes().cmp(b.filename.as_bytes()));
    Ok(())
}

fn replace_entry(entries: &mut Vec<InventoryEntry>, filename: &str, bytes: Vec<u8>) {
    entries.retain(|e| e.filename != filename);
    entries.push(InventoryEntry {
        filename: filename.to_string(),
        kind: "REGULAR",
        bytes: Some(bytes),
    });
}

/// Independent recomputation of every stored prefix's deterministic
/// consequences, from the current validated repository authority and the
/// virtual (normalized) inventory, simulating the remaining actions (and,
/// when every action already completed, the final action's fixed-point
/// replay) and requiring exact agreement with the stored plan. Runs
/// read-only before
/// any target
/// creation/replacement, temp promotion/removal, or journal advancement; a
/// semantically false journal — even one with lowercase-valid stored
/// prefixes — fails RECOVERY_LEDGER_INVALID here, never as a later
/// postcondition failure.
fn validate_pending_semantics(
    identity: &Identity,
    virtual_entries: &[InventoryEntry],
    journal: &RecoveryJournalFile,
    entry_index: usize,
    next_action: usize,
) -> Result<(), Error> {
    let entry = &journal.recoveries[entry_index];
    let plan = &entry.plan;
    let action_count = plan.actions.len();

    // 1. Structural uniqueness: no two removal actions may map to the same
    //    permanent target (two different producer temp names bound to one
    //    target is an ambiguous mapping, never a legitimate plan).
    {
        let mut removal_mapping: BTreeMap<String, &str> = BTreeMap::new();
        for action in &plan.actions {
            if action.kind != RecoveryActionKind::RemoveRedundantTemp {
                continue;
            }
            let Some(TempClass::Target(target)) = classify_temp_name(&action.target) else {
                return Err(Error::RecoveryLedgerInvalid);
            };
            if let Some(previous_temp) = removal_mapping.insert(target, &action.target) {
                if previous_temp != action.target.as_str() {
                    return Err(Error::RecoveryLedgerInvalid);
                }
            }
        }
    }

    // Current validated repository authority, independent of the journal.
    let completion_raw = read_completion_raw(identity).map_err(|_| Error::RecoveryLedgerInvalid)?;
    let accepted = derive_accepted_authority(identity, completion_raw.as_ref())
        .map_err(|_| Error::RecoveryLedgerInvalid)?;
    let scoped = read_phase_scoped(identity).map_err(|_| Error::RecoveryLedgerInvalid)?;

    // Deterministic simulation of the remaining action chain. Intermediate
    // subjects after already-completed actions are not reconstructible from
    // the resumed inventory (later actions' targets are already present, and
    // the subject hashes the ordered entry list), so replay starts at
    // `next_action`; every stored next prefix of the remaining actions —
    // including the final one — must equal the simulation result. When
    // every action already completed (next_action == action_count), the
    // final action is replayed over the final inventory as a fixed point:
    // its canonical consequences must reproduce the stored final prefix,
    // closing the finalize-path bypass where only a live-subject comparison
    // would gate. Every replacement is recomputed from the current
    // authority (never from the journal's stored bytes, which are only
    // compared against); this proves the canonical replacement match, the
    // byte-proven temp/target mapping, and the advance path (never trusting
    // a hash match alone).
    let start = if next_action == action_count {
        action_count - 1
    } else {
        next_action
    };
    let mut sim: Vec<InventoryEntry> = virtual_entries.to_vec();
    let mut closeout_after: Option<GovernanceState> = None;
    for i in start..action_count {
        let stored = &plan.actions[i];
        match stored.kind {
            RecoveryActionKind::RemoveRedundantTemp => {
                // Exact target/producer-temp mapping: a temp still present
                // must be byte-proven redundant with its mapped permanent
                // target; an absent temp means the action already completed
                // (proven by the fixed-point subject below).
                let Some(TempClass::Target(target)) = classify_temp_name(&stored.target) else {
                    return Err(Error::RecoveryLedgerInvalid);
                };
                if let Some(temp_entry) = sim.iter().find(|e| e.filename == stored.target) {
                    let temp_bytes = temp_entry
                        .bytes
                        .as_deref()
                        .ok_or(Error::RecoveryLedgerInvalid)?;
                    let target_bytes = std::fs::read(identity.gov_dir.join(target))
                        .map_err(|_| Error::RecoveryLedgerInvalid)?;
                    if temp_bytes != target_bytes.as_slice() {
                        return Err(Error::RecoveryLedgerInvalid);
                    }
                }
            }
            RecoveryActionKind::RestoreAcceptedPlan => {
                // Canonical replacement match: the stored replacement must
                // equal the exact bytes reconstructed from the current
                // completion ledger and plan source (independent of the
                // current accepted-plan file, so the already-restored
                // advance path recomputes identically). A replacement bound
                // to a different plan_id or plan SHA fails here.
                let reconstructed = reconstruct_accepted_plan(identity, completion_raw.as_ref())
                    .map_err(|_| Error::RecoveryLedgerInvalid)?;
                let bytes = accepted_plan_json_bytes(&reconstructed.record)
                    .map_err(|_| Error::RecoveryLedgerInvalid)?;
                if stored.replacement.as_deref() != Some(bytes.as_str()) {
                    return Err(Error::RecoveryLedgerInvalid);
                }
            }
            RecoveryActionKind::RestoreState => {
                // Canonical replacement match: the stored replacement must
                // equal the exact state derived from the current
                // phase-scoped authority and completion ledger (independent
                // of the current state.json file). A replacement bound to a
                // different accepted plan authority fails here.
                let (derived, _closeout) =
                    derive_state(identity, &scoped, &accepted, completion_raw.as_ref())
                        .map_err(|_| Error::RecoveryLedgerInvalid)?;
                let bytes = state_json_bytes(&derived).map_err(|_| Error::RecoveryLedgerInvalid)?;
                if stored.replacement.as_deref() != Some(bytes.as_str()) {
                    return Err(Error::RecoveryLedgerInvalid);
                }
            }
            RecoveryActionKind::ResumeCloseout => {
                // A valid RESUME_CLOSEOUT action necessarily derives from a
                // valid completion ledger and exact final completion
                // receipt: the deterministic receipt-bound after-state must
                // be derivable here, or the stored pending action is not
                // trustworthy. (Binding the label to the receipt remains an
                // execution-stage check: a wrong label with a genuine
                // prefix chain fails there with RECOVERY_ACTION_FAILED
                // before any mutation.)
                if closeout_after.is_none() {
                    closeout_after = Some(
                        expected_after_state(identity).map_err(|_| Error::RecoveryLedgerInvalid)?,
                    );
                }
            }
        }
        apply_action_to_inventory(&mut sim, stored, closeout_after.as_ref())?;
        // Every stored next prefix — intermediate and final alike — must
        // equal the deterministic simulation result: a lowercase-valid
        // stored prefix the simulation cannot reproduce is semantically
        // false. The post-action postcondition check still owns genuine
        // runtime drift after execution; forged journal content never
        // reaches it.
        if subject_sha_from_entries(identity, &sim)? != plan.prefix_subject_sha256[i + 1] {
            return Err(Error::RecoveryLedgerInvalid);
        }
    }
    Ok(())
}

// ============================================================================
// Journal publication (contract sections 19, 26)
// ============================================================================

struct TempFileGuard {
    path: Option<PathBuf>,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }
    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if let Some(p) = self.path.take() {
            let _ = std::fs::remove_file(&p);
        }
    }
}

/// Publish the journal with create-new temp, flush/sync, and atomic
/// replacement. The temp name is the deterministic recovery grammar bound to
/// the recovery ID and the pending entry's action count. `barrier_point`
/// selects the test-only crash barrier for this publication site.
fn publish_journal(
    identity: &Identity,
    journal: &RecoveryJournalFile,
    recovery_id: &str,
    barrier_point: &str,
) -> Result<(), Error> {
    let json = serde_json::to_string_pretty(journal).map_err(|_| Error::PersistenceFailed)?;
    let final_path = identity.gov_dir.join(RECOVERY_LEDGER_FILENAME);
    let pending_count = journal
        .recoveries
        .last()
        .map(|e| e.plan.actions.len())
        .unwrap_or(0);
    let temp_name = recovery_temp_name(recovery_id, pending_count);
    let temp_path = identity.gov_dir.join(&temp_name);
    let mut guard = TempFileGuard::new(temp_path.clone());
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(json.as_bytes())
                .map_err(|_| Error::PersistenceFailed)?;
            file.sync_all().map_err(|_| Error::PersistenceFailed)?;
            drop(file);
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // A pre-existing temp collision is never truncated. The resume
            // path handles recorded collisions; an unexpected one fails
            // closed.
            return Err(Error::RecoveryActionFailed);
        }
        Err(_) => return Err(Error::PersistenceFailed),
    }
    test_only_recovery_barrier(barrier_point)?;
    if pending_published_flag() && test_only_fail_rename_after_publish() {
        return Err(Error::PersistenceFailed);
    }
    state::rename_replace(&temp_path, &final_path).map_err(|_| Error::PersistenceFailed)?;
    guard.disarm();
    Ok(())
}

// ============================================================================
// Action execution (contract sections 12-14, 20, 26)
// ============================================================================

fn execute_action(
    identity: &Identity,
    plan: &RecoveryPlanSeed,
    action_index: usize,
    recovery_id: &str,
    closeout_ops: &[NormalizeOp],
) -> Result<(), Error> {
    let action = &plan.actions[action_index];
    match action.kind {
        RecoveryActionKind::RemoveRedundantTemp => {
            // Re-verify exact redundancy before removal.
            let temp_path = identity.gov_dir.join(&action.target);
            let temp_bytes = std::fs::read(&temp_path).map_err(|_| Error::RecoveryActionFailed)?;
            let Some(TempClass::Target(target)) = classify_temp_name(&action.target) else {
                return Err(Error::RecoveryActionFailed);
            };
            let target_bytes = std::fs::read(identity.gov_dir.join(target))
                .map_err(|_| Error::RecoveryActionFailed)?;
            if temp_bytes != target_bytes {
                return Err(Error::RecoveryActionFailed);
            }
            std::fs::remove_file(&temp_path).map_err(|_| Error::RecoveryActionFailed)?;
        }
        RecoveryActionKind::RestoreAcceptedPlan | RecoveryActionKind::RestoreState => {
            let filename = if action.kind == RecoveryActionKind::RestoreAcceptedPlan {
                "accepted-plan.json"
            } else {
                "state.json"
            };
            let expected = action
                .replacement
                .as_deref()
                .ok_or(Error::RecoveryActionFailed)?;
            // The replacement is always recomputed by apply; caller input
            // never supplies action content. The stored plan replacement is
            // verified by re-deriving it from the surviving authority.
            let recomputed = if action.kind == RecoveryActionKind::RestoreAcceptedPlan {
                recompute_accepted_plan_replacement(identity)?
            } else {
                recompute_state_replacement(identity)?
            };
            let recomputed = recomputed.ok_or(Error::RecoveryActionFailed)?;
            if recomputed != expected {
                return Err(Error::RecoveryActionFailed);
            }
            publish_replacement(identity, filename, expected, recovery_id, action_index)?;
            // Restored bytes are re-read and fully validated before the next
            // action (contract section 26).
            revalidate_restored(identity, filename)?;
        }
        RecoveryActionKind::ResumeCloseout => {
            // The promotion of the interrupted state-publication temp
            // happens inside resume_closeout, strictly after every read-only
            // binding and precondition check: a crafted wrong-label journal
            // fails RECOVERY_ACTION_FAILED before any mutation, and a crash
            // inside the action resumes with the RESUME action still next
            // (the idempotent re-application plus the finalizer remain
            // valid). Leading actions never observe the promotion.
            resume_closeout(identity, &action.target, closeout_ops)?;
        }
    }
    Ok(())
}

/// One authoritative construction of an APPLIED entry's receipt and receipt
/// hash, shared by final publication and resume-temp validation so the
/// expected bytes are exactly what `finalize_recovery` publishes.
fn build_applied_receipt(
    journal: &RecoveryJournalFile,
    entry_index: usize,
    post_subject_sha256: &str,
) -> Result<(RecoveryReceipt, String), Error> {
    let entry = &journal.recoveries[entry_index];
    let plan = &entry.plan;
    let action_count = plan.actions.len();
    let previous_receipt_hash = if entry_index > 0 {
        journal.recoveries[entry_index - 1]
            .recovery_receipt_sha256
            .clone()
    } else {
        None
    };
    let actions_json =
        serde_json::to_string(&plan.actions).map_err(|_| Error::RecoveryActionFailed)?;
    let receipt = RecoveryReceipt {
        schema_version: 1,
        accepted_plan_sha256: journal.accepted_plan_sha256.clone(),
        plan_id: journal.plan_id.clone(),
        recovery_sequence: (entry_index as u32) + 1,
        recovery_id: entry.recovery_id.clone(),
        pre_subject_sha256: plan.pre_subject_sha256.clone(),
        post_subject_sha256: post_subject_sha256.to_string(),
        action_count,
        actions_sha256: sha256_hex(actions_json.as_bytes()),
        previous_recovery_receipt_sha256: previous_receipt_hash,
    };
    let receipt_sha = compact_sha(&receipt)?;
    Ok((receipt, receipt_sha))
}

/// The complete deterministic APPLIED journal for an entry: receipt, receipt
/// SHA-256, post-subject, next_action, status, previous receipt link,
/// sequence, and action hash — byte-identical to the final publication.
fn build_applied_journal(
    journal: &RecoveryJournalFile,
    entry_index: usize,
    post_subject_sha256: &str,
) -> Result<(RecoveryJournalFile, String), Error> {
    let (receipt, receipt_sha) = build_applied_receipt(journal, entry_index, post_subject_sha256)?;
    let mut applied = journal.clone();
    let entry = &mut applied.recoveries[entry_index];
    entry.next_action = entry.plan.actions.len();
    entry.status = "APPLIED".to_string();
    entry.post_subject_sha256 = Some(post_subject_sha256.to_string());
    entry.recovery_receipt = Some(receipt);
    entry.recovery_receipt_sha256 = Some(receipt_sha.clone());
    Ok((applied, receipt_sha))
}

/// Recompute the accepted-plan replacement from the surviving authority.
fn recompute_accepted_plan_replacement(identity: &Identity) -> Result<Option<String>, Error> {
    let completion = read_completion_raw(identity)?;
    let accepted = derive_accepted_authority(identity, completion.as_ref())?;
    if accepted.reconstructed {
        Ok(Some(accepted_plan_json_bytes(&accepted.record)?))
    } else {
        Ok(None)
    }
}

/// Recompute the state replacement from the surviving authority. Uses the
/// same strict validator as analysis: a valid state must pass exact raw
/// keys, type shapes, and the complete Phase 1 validator, and must not be
/// completion-inconsistent (relation Other derives exactly like a malformed
/// state).
fn recompute_state_replacement(identity: &Identity) -> Result<Option<String>, Error> {
    let completion = read_completion_raw(identity)?;
    let accepted = derive_accepted_authority(identity, completion.as_ref())?;
    let scoped = read_phase_scoped(identity)?;
    let strict = std::fs::read(identity.gov_dir.join("state.json"))
        .ok()
        .and_then(|b| strict_parse_state(&b, &accepted).ok());
    let needs_derivation = match strict {
        None => true,
        Some(state) => completion
            .as_ref()
            .map(|ledger| relation_to_final(&state, ledger) == StateRelation::Other)
            .unwrap_or(false),
    };
    if !needs_derivation {
        return Ok(None);
    }
    let (derived, _closeout) = derive_state(identity, &scoped, &accepted, completion.as_ref())?;
    Ok(Some(state_json_bytes(&derived)?))
}

fn publish_replacement(
    identity: &Identity,
    filename: &str,
    content: &str,
    recovery_id: &str,
    action_index: usize,
) -> Result<(), Error> {
    let final_path = identity.gov_dir.join(filename);
    let temp_name = recovery_temp_name(recovery_id, action_index);
    let temp_path = identity.gov_dir.join(&temp_name);
    let mut guard = TempFileGuard::new(temp_path.clone());
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(content.as_bytes())
                .map_err(|_| Error::RecoveryActionFailed)?;
            file.sync_all().map_err(|_| Error::RecoveryActionFailed)?;
            drop(file);
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // A pre-existing recovery-owned temp collision: the temp is never
            // truncated; it may be promoted only when its bytes exactly equal
            // the exact action content.
            let existing = std::fs::read(&temp_path).map_err(|_| Error::RecoveryActionFailed)?;
            if existing.as_slice() != content.as_bytes() {
                return Err(Error::RecoveryActionFailed);
            }
            state::rename_replace(&temp_path, &final_path)
                .map_err(|_| Error::RecoveryActionFailed)?;
            guard.disarm();
            return Ok(());
        }
        Err(_) => return Err(Error::RecoveryActionFailed),
    }
    test_only_recovery_barrier(&format!("after_temp_write:{}", action_index))?;
    state::rename_replace(&temp_path, &final_path).map_err(|_| Error::RecoveryActionFailed)?;
    guard.disarm();
    Ok(())
}

fn revalidate_restored(identity: &Identity, filename: &str) -> Result<(), Error> {
    let bytes =
        std::fs::read(identity.gov_dir.join(filename)).map_err(|_| Error::RecoveryActionFailed)?;
    if filename == "accepted-plan.json" {
        let record: AcceptedPlan =
            serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryActionFailed)?;
        state::validate_accepted_plan_record(&record).map_err(|_| Error::RecoveryActionFailed)?;
        let (plan, sha, _) = validate_plan_source(
            identity,
            &record.plan_path,
            &record.sha256,
            &record.plan_id,
            record.phase_count,
        )
        .map_err(|_| Error::RecoveryActionFailed)?;
        state::validate_plan_consistency(&record, &plan, &sha)
            .map_err(|_| Error::RecoveryActionFailed)?;
    } else {
        let state: GovernanceState =
            serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryActionFailed)?;
        let completion = read_completion_raw(identity).map_err(|_| Error::RecoveryActionFailed)?;
        let accepted = derive_accepted_authority(identity, completion.as_ref())
            .map_err(|_| Error::RecoveryActionFailed)?;
        state::validate_state_record(&state, &accepted.record, &accepted.plan)
            .map_err(|_| Error::RecoveryActionFailed)?;
    }
    Ok(())
}

fn resume_closeout(
    identity: &Identity,
    phase_id: &str,
    closeout_ops: &[NormalizeOp],
) -> Result<(), Error> {
    let completion = read_completion_raw(identity).map_err(|_| Error::RecoveryActionFailed)?;
    let ledger = completion.ok_or(Error::RecoveryActionFailed)?;
    let final_entry = ledger
        .completions
        .last()
        .ok_or(Error::RecoveryActionFailed)?;
    if final_entry.completion_receipt.phase_id != phase_id {
        return Err(Error::RecoveryActionFailed);
    }
    // Phase-scoped leftovers must byte-match the archived copies (the
    // finalizer re-verifies and removes them in the fixed order).
    let scoped = read_phase_scoped(identity).map_err(|_| Error::RecoveryActionFailed)?;
    check_closeout_preconditions(identity, &scoped, &ledger)
        .map_err(|_| Error::RecoveryActionFailed)?;
    // The exact receipt-bound pre-closeout state: the finalizer never
    // derives its state transition from unverified on-disk bytes, and never
    // deletes phase-scoped files from an unverified state relation.
    let accepted = derive_accepted_authority(identity, Some(&ledger))
        .map_err(|_| Error::RecoveryActionFailed)?;
    // Every read-only binding and precondition check passed: only now may
    // the interrupted state-publication temp be consumed (promote the exact
    // after-state over the receipt-bound pre-closeout state, or remove a
    // redundant leftover). A crafted wrong-label journal therefore fails
    // before any mutation, and a crash inside the action resumes with the
    // RESUME action still next (the idempotent re-application remains
    // valid).
    apply_resume_ops(identity, closeout_ops)?;
    let receipt = &final_entry.completion_receipt;
    let mut state = GovernanceState {
        schema_version: 1,
        accepted_plan_sha256: accepted.record.sha256,
        active_phase: receipt.active_phase_before.clone(),
        closed_phases: receipt.closed_phases_before.clone(),
    };
    closeout::resumable_finalization(
        &identity.repo,
        &identity.gov_dir,
        &mut state,
        &ledger,
        phase_id,
    )
    .map_err(|_| Error::RecoveryActionFailed)?;
    Ok(())
}

// ============================================================================
// Resume normalization (contract sections 13, 20)
// ============================================================================

/// Remove or promote authorized leftover recovery temps and remove
/// byte-redundant producer temps before prefix matching. Returns the
/// normalized inventory.
/// A planned, byte-verified normalization mutation. Executed only after the
/// complete candidate state has been accepted against an exact authorized
/// prefix or an exact Phase 6 intermediate state.
#[derive(Debug)]
enum NormalizeOp {
    Promote {
        temp: String,
        target: String,
        bytes: Vec<u8>,
    },
    /// Closeout-state publication resume: atomically replace the exact
    /// receipt-bound pre-closeout state with the temp's after-state bytes
    /// (the interrupted rename). Never discards the only durable
    /// after-state to reconstruct it via a second path.
    Replace {
        temp: String,
        target: String,
        bytes: Vec<u8>,
        expected_old: Vec<u8>,
    },
    Remove {
        temp: String,
    },
}

/// The resume normalization result: the virtual normalized inventory, the
/// operations applied only after the candidate state is accepted, and the
/// closeout-state operations deferred to the RESUME_CLOSEOUT action
/// execution.
struct ResumeNormalization {
    virtual_entries: Vec<InventoryEntry>,
    ops: Vec<NormalizeOp>,
    closeout_ops: Vec<NormalizeOp>,
}

/// Phase 1: read-only validation and virtual normalization of the resume
/// inventory. No filesystem mutation happens here. Every recovery-owned temp
/// is validated (canonical index, unique index, in-range, exact expected
/// bytes), the complete temp set is checked, and only byte-proven producer
/// removals are planned. Returns the virtual normalized inventory plus the
/// planned operations.
fn plan_resume_normalization(
    identity: &Identity,
    entries: Vec<InventoryEntry>,
    journal: &RecoveryJournalFile,
    entry_index: usize,
    expected_next_journal: &RecoveryJournalFile,
    expected_applied_journal: &RecoveryJournalFile,
) -> Result<ResumeNormalization, Error> {
    let entry = &journal.recoveries[entry_index];
    let recovery_id = entry.recovery_id.clone();
    let action_count = entry.plan.actions.len();
    let mut ops: Vec<NormalizeOp> = Vec::new();
    // Closeout-state ops are validated here (fail closed) but always
    // deferred to the RESUME_CLOSEOUT action execution: applying the
    // promotion before a leading action would strand a subject that no
    // stored prefix describes (the plan simulation keeps the temp live
    // through leading actions).
    let mut closeout_ops: Vec<NormalizeOp> = Vec::new();
    let mut recovery_indexes: BTreeMap<usize, &str> = BTreeMap::new();
    // 1. Validate every recovery-owned temp.
    for item in &entries {
        let Some(TempClass::RecoveryOwned(rid, index)) = classify_temp_name(&item.filename) else {
            continue;
        };
        if rid != recovery_id {
            // A temp bound to a different recovery is unrelated.
            return Err(Error::RecoverySubjectStale);
        }
        if recovery_indexes.contains_key(&index) {
            // Duplicate index: ambiguous leftovers are never consumed.
            return Err(Error::RecoveryActionFailed);
        }
        recovery_indexes.insert(index, &item.filename);
        let bytes = item.bytes.as_ref().ok_or(Error::RecoveryActionFailed)?;
        if index < action_count {
            let action = &entry.plan.actions[index];
            let target = match action.kind {
                RecoveryActionKind::RestoreAcceptedPlan => "accepted-plan.json",
                RecoveryActionKind::RestoreState => "state.json",
                // REMOVE and RESUME actions never write action temps.
                _ => return Err(Error::RecoveryActionFailed),
            };
            let expected = action
                .replacement
                .as_deref()
                .ok_or(Error::RecoveryActionFailed)?;
            if bytes.as_slice() != expected.as_bytes() {
                return Err(Error::RecoveryActionFailed);
            }
            let target_path = identity.gov_dir.join(target);
            match std::fs::symlink_metadata(&target_path) {
                Ok(meta) if meta.file_type().is_file() => {
                    // The target already exists: the temp may only be
                    // removed when byte-redundant with it.
                    let target_bytes =
                        std::fs::read(&target_path).map_err(|_| Error::RecoveryActionFailed)?;
                    if target_bytes.as_slice() != expected.as_bytes() {
                        return Err(Error::RecoveryActionFailed);
                    }
                    ops.push(NormalizeOp::Remove {
                        temp: item.filename.clone(),
                    });
                }
                Ok(_) => return Err(Error::FilesystemBoundaryUnsafe),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    ops.push(NormalizeOp::Promote {
                        temp: item.filename.clone(),
                        target: target.to_string(),
                        bytes: expected.as_bytes().to_vec(),
                    });
                }
                Err(_) => return Err(Error::RecoveryActionFailed),
            }
        } else if index == action_count {
            // Journal publication temp: bytes must equal the exact next
            // journal or the exact deterministic applied journal.
            let next_json = serde_json::to_string_pretty(expected_next_journal)
                .map_err(|_| Error::RecoveryActionFailed)?;
            let applied_json = serde_json::to_string_pretty(expected_applied_journal)
                .map_err(|_| Error::RecoveryActionFailed)?;
            if bytes.as_slice() != next_json.as_bytes()
                && bytes.as_slice() != applied_json.as_bytes()
            {
                return Err(Error::RecoveryActionFailed);
            }
            ops.push(NormalizeOp::Remove {
                temp: item.filename.clone(),
            });
        } else {
            // Out-of-range index: not a legitimate leftover of this plan.
            return Err(Error::RecoveryActionFailed);
        }
    }
    // 2. Producer temps: removal is planned only when the bytes are exactly
    //    redundant with an existing valid target; the closeout state-write
    //    temp holding the exact Phase 6 after-state is promoted over the
    //    exact receipt-bound pre-closeout state (interrupted rename) or
    //    removed when already redundant. An unplanned producer temp is never
    //    consumed merely to make the subject match a prefix.
    for item in &entries {
        let Some(TempClass::Target(target)) = classify_temp_name(&item.filename) else {
            continue;
        };
        let temp_bytes = item.bytes.as_ref().ok_or(Error::RecoveryActionFailed)?;
        let target_bytes = std::fs::read(identity.gov_dir.join(&target)).ok();
        let redundant = target_bytes
            .as_deref()
            .map(|tb| tb == temp_bytes.as_slice())
            .unwrap_or(false);
        if redundant {
            ops.push(NormalizeOp::Remove {
                temp: item.filename.clone(),
            });
            continue;
        }
        if target == "state.json" && is_closeout_state_temp_name(&item.filename) {
            // Interrupted Phase 6 state publication: validate the exact
            // after-state relation now (fail closed, never discard the only
            // durable after-state), but defer the promotion to the
            // RESUME_CLOSEOUT action execution, which applies it
            // idempotently. The temp stays live in the virtual inventory so
            // every crash position between the publish and the RESUME
            // action resumes against a stored prefix.
            closeout_ops.push(closeout_state_temp_op(
                identity,
                &item.filename,
                temp_bytes,
            )?);
            continue;
        }
        if target == "state.json" {
            let after = expected_after_state(identity)?;
            let after_json = state_json_bytes(&after).map_err(|_| Error::RecoveryActionFailed)?;
            if temp_bytes.as_slice() == after_json.as_bytes() {
                ops.push(NormalizeOp::Remove {
                    temp: item.filename.clone(),
                });
            }
        }
    }
    // 3. Virtual normalized inventory: apply the planned ops to the captured
    //    entries in order, keeping the first occurrence of every filename.
    let mut virtual_inventory: Vec<InventoryEntry> = Vec::new();
    for item in entries {
        let mut consumed = false;
        for op in &ops {
            match op {
                NormalizeOp::Remove { temp } if temp == &item.filename => {
                    consumed = true;
                }
                NormalizeOp::Promote {
                    temp,
                    target,
                    bytes,
                }
                | NormalizeOp::Replace {
                    temp,
                    target,
                    bytes,
                    ..
                } if temp == &item.filename => {
                    virtual_inventory.push(InventoryEntry {
                        filename: target.clone(),
                        kind: "REGULAR",
                        bytes: Some(bytes.clone()),
                    });
                    consumed = true;
                }
                _ => {}
            }
        }
        if !consumed {
            virtual_inventory.push(item);
        }
    }
    let mut seen = BTreeSet::new();
    virtual_inventory.retain(|e| seen.insert(e.filename.clone()));
    virtual_inventory.sort_by(|a, b| a.filename.as_bytes().cmp(b.filename.as_bytes()));
    Ok(ResumeNormalization {
        virtual_entries: virtual_inventory,
        ops,
        closeout_ops,
    })
}

/// Phase 2: authorized filesystem mutation. Runs only after the complete
/// candidate state has been accepted; every removal error is propagated.
fn apply_resume_ops(identity: &Identity, ops: &[NormalizeOp]) -> Result<(), Error> {
    for op in ops {
        match op {
            NormalizeOp::Promote {
                temp,
                target,
                bytes,
            } => {
                // The target was verified absent in phase 1; re-verify
                // before renaming (no-clobber, no truncation).
                if identity.gov_dir.join(target).exists() {
                    return Err(Error::RecoveryActionFailed);
                }
                state::rename_replace(&identity.gov_dir.join(temp), &identity.gov_dir.join(target))
                    .map_err(|_| Error::RecoveryActionFailed)?;
                // Re-verify the promoted bytes exactly.
                let written = std::fs::read(identity.gov_dir.join(target))
                    .map_err(|_| Error::RecoveryActionFailed)?;
                if written.as_slice() != bytes.as_slice() {
                    return Err(Error::RecoveryActionFailed);
                }
            }
            NormalizeOp::Replace {
                temp,
                target,
                bytes,
                expected_old,
            } => {
                // Phase 1 verified the exact relation; re-verify before
                // replacing (the interrupted rename is atomic and never
                // truncates). The op is idempotent: when the promotion
                // already happened (crash inside RESUME_CLOSEOUT after the
                // rename), the temp is gone and the target must already hold
                // the exact after-state.
                if !identity.gov_dir.join(temp).exists() {
                    let current = std::fs::read(identity.gov_dir.join(target))
                        .map_err(|_| Error::RecoveryActionFailed)?;
                    if current.as_slice() != bytes.as_slice() {
                        return Err(Error::RecoveryActionFailed);
                    }
                    continue;
                }
                let current = std::fs::read(identity.gov_dir.join(target))
                    .map_err(|_| Error::RecoveryActionFailed)?;
                if current.as_slice() != expected_old.as_slice() {
                    return Err(Error::RecoveryActionFailed);
                }
                state::rename_replace(&identity.gov_dir.join(temp), &identity.gov_dir.join(target))
                    .map_err(|_| Error::RecoveryActionFailed)?;
                // Re-verify the promoted bytes exactly.
                let written = std::fs::read(identity.gov_dir.join(target))
                    .map_err(|_| Error::RecoveryActionFailed)?;
                if written.as_slice() != bytes.as_slice() {
                    return Err(Error::RecoveryActionFailed);
                }
            }
            NormalizeOp::Remove { temp } => {
                std::fs::remove_file(identity.gov_dir.join(temp))
                    .map_err(|_| Error::RecoveryActionFailed)?;
            }
        }
    }
    Ok(())
}

/// The exact receipt-bound pre-closeout and after states for the final
/// completion entry. Read-only; never reads state.json, so both are
/// deterministic in every resume path.
fn receipt_bound_states(identity: &Identity) -> Result<(GovernanceState, GovernanceState), Error> {
    let completion = read_completion_raw(identity)?;
    let ledger = completion.ok_or(Error::RecoveryActionFailed)?;
    let receipt = &ledger
        .completions
        .last()
        .ok_or(Error::RecoveryActionFailed)?
        .completion_receipt;
    let accepted = derive_accepted_authority(identity, Some(&ledger))
        .map_err(|_| Error::RecoveryActionFailed)?;
    let pre = GovernanceState {
        schema_version: 1,
        accepted_plan_sha256: accepted.record.sha256.clone(),
        active_phase: receipt.active_phase_before.clone(),
        closed_phases: receipt.closed_phases_before.clone(),
    };
    let after = GovernanceState {
        schema_version: 1,
        accepted_plan_sha256: accepted.record.sha256,
        active_phase: None,
        closed_phases: receipt.closed_phases_after.clone(),
    };
    Ok((pre, after))
}

fn expected_after_state(identity: &Identity) -> Result<GovernanceState, Error> {
    receipt_bound_states(identity).map(|(_, after)| after)
}

/// The deterministic normalization decision for a differing closeout-state
/// temp: Replace (promote) when it holds the exact canonical after-state
/// over the exact receipt-bound pre-closeout state; fail closed otherwise
/// (corrupt evidence or a broken before/after relation).
fn closeout_state_temp_op(
    identity: &Identity,
    temp_name: &str,
    temp_bytes: &[u8],
) -> Result<NormalizeOp, Error> {
    let (pre, after) = receipt_bound_states(identity).map_err(|_| Error::RecoveryUnrecoverable)?;
    let pre_json = state_json_bytes(&pre).map_err(|_| Error::RecoveryUnrecoverable)?;
    let after_json = state_json_bytes(&after).map_err(|_| Error::RecoveryUnrecoverable)?;
    if temp_bytes != after_json.as_bytes() {
        // Neither byte-redundant with state.json nor the exact canonical
        // after-state: corrupt evidence, never consumed.
        return Err(Error::RecoveryUnrecoverable);
    }
    let target_bytes = std::fs::read(identity.gov_dir.join("state.json"))
        .map_err(|_| Error::RecoveryUnrecoverable)?;
    if target_bytes.as_slice() != pre_json.as_bytes() {
        // state.json is not the exact receipt-bound pre-closeout state: the
        // before/after relation is broken.
        return Err(Error::RecoveryUnrecoverable);
    }
    Ok(NormalizeOp::Replace {
        temp: temp_name.to_string(),
        target: "state.json".to_string(),
        bytes: after_json.into_bytes(),
        expected_old: pre_json.into_bytes(),
    })
}

// ============================================================================
// Apply command (contract sections 3, 17, 19-22)
// ============================================================================

pub fn cmd_recovery_apply(
    repo_arg: &str,
    recovery_id_arg: &str,
    subject_sha256_arg: &str,
    decision_arg: &str,
) -> Result<String, Error> {
    // 1. validate exact decision token.
    if decision_arg != "RECOVER" {
        return Err(Error::RecoveryDecisionInvalid);
    }
    // 2. validate both lowercase hashes.
    if !is_valid_sha64(recovery_id_arg) || !is_valid_sha64(subject_sha256_arg) {
        return Err(Error::RecoveryIdInvalid);
    }

    // 3-4. repository identity and .mrgs topology.
    let identity = resolve_identity(repo_arg)?;

    // 4. validate an existing recovery ledger before trusting it.
    let journal = read_journal(&identity.gov_dir)?;

    // 5. capture the complete subject and require exact equality with the
    //    caller's subject SHA-256 (the subject binds object format, branch,
    //    and HEAD, so a changed git identity is rejected here as well).
    let (entries, subject_sha256) = capture_subject(&identity, journal.as_ref())?;
    if subject_sha256 != subject_sha256_arg {
        return Err(Error::RecoverySubjectStale);
    }

    // 6-11. full fail-closed classification even when a pending entry
    // exists.
    let analysis = analyze(&identity, journal.as_ref())?;

    match analysis.classification {
        Classification::Pending {
            recovery_id,
            next_action,
            action_count,
        } => {
            if recovery_id != recovery_id_arg {
                return Err(Error::RecoveryPendingConflict);
            }
            resume_pending(
                &identity,
                journal.as_ref().unwrap(),
                &entries,
                &recovery_id,
                next_action,
                action_count,
            )
        }
        Classification::Healthy => {
            // Idempotent replay across the complete applied recovery
            // history: recovery IDs are unique, so at most one entry can
            // match; the search is not limited to the final entry.
            if let Some(j) = journal.as_ref() {
                for entry in &j.file.recoveries {
                    if entry.recovery_id != recovery_id_arg {
                        continue;
                    }
                    if entry.status == "APPLIED"
                        && entry.post_subject_sha256.as_deref() == Some(&subject_sha256)
                    {
                        let receipt = entry
                            .recovery_receipt
                            .as_ref()
                            .ok_or(Error::RecoverySubjectStale)?;
                        return Ok(recovery_applied_output(
                            entry,
                            receipt,
                            &entry.plan.pre_subject_sha256,
                        ));
                    }
                    // A reused ID with a different subject or status is a
                    // conflict, not an idempotent replay.
                    return Err(Error::RecoverySubjectStale);
                }
            }
            Ok(format!("RECOVERY_NOT_REQUIRED {}", subject_sha256))
        }
        Classification::Recoverable(plan) => {
            if plan.recovery_id != recovery_id_arg {
                return Err(Error::RecoveryIdInvalid);
            }
            execute_fresh(&identity, journal.as_ref(), &plan, &subject_sha256)
        }
    }
}

fn recovery_applied_output(
    entry: &RecoveryJournalEntry,
    receipt: &RecoveryReceipt,
    pre_subject_sha256: &str,
) -> String {
    format!(
        "RECOVERY_APPLIED {} {} {} {} {}",
        receipt.recovery_sequence,
        entry.recovery_id,
        pre_subject_sha256,
        receipt.post_subject_sha256,
        entry.recovery_receipt_sha256.as_deref().unwrap_or_default(),
    )
}

fn execute_fresh(
    identity: &Identity,
    journal: Option<&JournalState>,
    plan: &RecoveryPlan,
    subject_sha256: &str,
) -> Result<String, Error> {
    let recovery_id = plan.recovery_id.clone();
    let action_count = plan.seed.actions.len();

    // Recovery IDs must be unique across the complete history: a fresh plan
    // whose ID already exists is a conflict (the subject necessarily differs
    // from that entry's stored post-subject), never a duplicate append.
    if let Some(j) = journal {
        if j.file
            .recoveries
            .iter()
            .any(|e| e.recovery_id == recovery_id)
        {
            return Err(Error::RecoverySubjectStale);
        }
    }

    // Publish the pending entry before the first target action.
    let mut journal_file = journal
        .map(|j| j.file.clone())
        .unwrap_or(RecoveryJournalFile {
            schema_version: 1,
            accepted_plan_sha256: plan.seed.accepted_plan_sha256.clone(),
            plan_id: plan.seed.plan_id.clone(),
            recoveries: vec![],
        });
    journal_file.recoveries.push(RecoveryJournalEntry {
        recovery_id: recovery_id.clone(),
        plan: plan.seed.clone(),
        next_action: 0,
        status: "PENDING".to_string(),
        post_subject_sha256: None,
        recovery_receipt: None,
        recovery_receipt_sha256: None,
    });
    publish_journal(
        identity,
        &journal_file,
        &recovery_id,
        "after_ledger_temp_write_first",
    )?;
    #[cfg(debug_assertions)]
    PENDING_PUBLISHED.store(true, std::sync::atomic::Ordering::Relaxed);
    test_only_recovery_barrier("after_pending_publish")?;

    // Re-run the fail-closed classification on the pending subject: the
    // journal is excluded from the subject, so the subject is unchanged.
    let reanalysis = analyze(identity, None)?;
    if reanalysis.subject_sha256 != subject_sha256 {
        return Err(Error::RecoveryPostconditionFailed);
    }

    // The closeout-state normalization ops are deferred to the
    // RESUME_CLOSEOUT action itself: a crash after a promotion but before a
    // leading action would otherwise strand a subject that no stored prefix
    // describes and that the intermediate check cannot accept. The action
    // applies them idempotently, so every crash window resumes with the
    // RESUME action still next.
    let closeout_ops = reanalysis.closeout_ops;

    execute_actions(
        identity,
        &plan.seed,
        &recovery_id,
        &mut journal_file,
        0,
        action_count,
        None,
        &closeout_ops,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_actions(
    identity: &Identity,
    plan: &RecoveryPlanSeed,
    recovery_id: &str,
    journal_file: &mut RecoveryJournalFile,
    start_index: usize,
    action_count: usize,
    journal_entry_index: Option<usize>,
    closeout_ops: &[NormalizeOp],
) -> Result<String, Error> {
    let entry_index = journal_entry_index.unwrap_or(journal_file.recoveries.len() - 1);
    for i in start_index..action_count {
        execute_action(identity, plan, i, recovery_id, closeout_ops)?;
        // Test-only barrier between the action mutation and the
        // postcondition capture, used to prove genuine runtime drift (an
        // external change to the just-written target) still yields
        // RECOVERY_POSTCONDITION_FAILED. Debug builds only; release
        // semantics unchanged.
        test_only_recovery_barrier(&format!("after_action_before_postcondition:{}", i))?;
        // Deterministic post-action verification: the recomputed subject
        // must equal the expected prefix hash.
        let (_, sha) = capture_subject(identity, None)?;
        if sha != plan.prefix_subject_sha256[i + 1] {
            return Err(Error::RecoveryPostconditionFailed);
        }
        test_only_recovery_barrier(&format!("after_action:{}", i))?;
        // Atomically advance next_action.
        journal_file.recoveries[entry_index].next_action = i + 1;
        publish_journal(
            identity,
            journal_file,
            recovery_id,
            "after_ledger_temp_write_advance",
        )?;
    }
    // All actions complete: finalize.
    test_only_recovery_barrier("before_finalize")?;
    finalize_recovery(identity, journal_file, entry_index, recovery_id)
}

fn resume_pending(
    identity: &Identity,
    journal: &JournalState,
    entries: &[InventoryEntry],
    recovery_id: &str,
    next_action: usize,
    action_count: usize,
) -> Result<String, Error> {
    let entry_index = journal.pending.ok_or(Error::RecoverySubjectStale)?;
    let entry = &journal.file.recoveries[entry_index];
    let plan = entry.plan.clone();
    // Expected next journal states used to authorize leftover journal temps.
    let mut expected_next = journal.file.clone();
    expected_next.recoveries[entry_index].next_action = expected_next.recoveries[entry_index]
        .next_action
        .saturating_add(1)
        .min(action_count);
    // The expected APPLIED journal is the exact deterministic final
    // publication (same authoritative helper as finalize_recovery).
    let (expected_applied, _) = build_applied_journal(
        &journal.file,
        entry_index,
        &plan.prefix_subject_sha256[action_count],
    )
    .map_err(|_| Error::RecoveryActionFailed)?;
    // Phase 1: read-only validation and virtual normalization. No rename or
    // removal happens before the complete candidate state is accepted. The
    // closeout-state ops are validated here but applied only by the
    // RESUME_CLOSEOUT action itself.
    let normalization = plan_resume_normalization(
        identity,
        entries.to_vec(),
        &journal.file,
        entry_index,
        &expected_next,
        &expected_applied,
    )?;
    let (virtual_entries, ops, closeout_ops) = (
        normalization.virtual_entries,
        normalization.ops,
        normalization.closeout_ops,
    );
    // Independent deterministic recomputation of every stored prefix (the
    // full action chain, replayed from the current validated authority),
    // with exact stored-plan agreement. No promotion/removal, no target
    // mutation, and no journal advancement may happen before this passes: a
    // semantically false pending journal fails RECOVERY_LEDGER_INVALID
    // here, never as a later postcondition failure.
    validate_pending_semantics(
        identity,
        &virtual_entries,
        &journal.file,
        entry_index,
        next_action,
    )?;
    let current_sha = subject_sha_from_entries(identity, &virtual_entries)?;

    if next_action == action_count {
        // All actions done; only finalization remains.
        if current_sha != plan.prefix_subject_sha256[action_count] {
            return Err(Error::RecoverySubjectStale);
        }
        apply_resume_ops(identity, &ops)?;
        let mut journal_file = journal.file.clone();
        return finalize_recovery(identity, &mut journal_file, entry_index, recovery_id);
    }

    // RESUME_CLOSEOUT intermediate states may call the same finalizer again.
    let valid_intermediate = if plan.actions[next_action].kind == RecoveryActionKind::ResumeCloseout
    {
        is_valid_intermediate_closeout(identity, &plan.actions[next_action].target)?
    } else {
        false
    };
    let mut journal_file = journal.file.clone();
    if current_sha == plan.prefix_subject_sha256[next_action]
        || current_sha == plan.prefix_subject_sha256[next_action + 1]
        || valid_intermediate
    {
        // The complete candidate state is accepted: only now may exact
        // recorded temps be promoted or removed.
        apply_resume_ops(identity, &ops)?;
    }
    if current_sha == plan.prefix_subject_sha256[next_action] {
        // Execute the action.
        execute_actions(
            identity,
            &plan,
            recovery_id,
            &mut journal_file,
            next_action,
            action_count,
            Some(entry_index),
            &closeout_ops,
        )
    } else if current_sha == plan.prefix_subject_sha256[next_action + 1] {
        // The action completed before journal advancement: advance without
        // repeating.
        journal_file.recoveries[entry_index].next_action = next_action + 1;
        publish_journal(
            identity,
            &journal_file,
            recovery_id,
            "after_ledger_temp_write_advance",
        )?;
        execute_actions(
            identity,
            &plan,
            recovery_id,
            &mut journal_file,
            next_action + 1,
            action_count,
            Some(entry_index),
            &closeout_ops,
        )
    } else if valid_intermediate {
        execute_actions(
            identity,
            &plan,
            recovery_id,
            &mut journal_file,
            next_action,
            action_count,
            Some(entry_index),
            &closeout_ops,
        )
    } else {
        Err(Error::RecoverySubjectStale)
    }
}

/// An exact valid intermediate Phase 6 cleanup state for the given phase:
/// the full fail-closed classification (ignoring the pending journal) must
/// derive exactly one RESUME_CLOSEOUT action for the same phase.
fn is_valid_intermediate_closeout(identity: &Identity, phase_id: &str) -> Result<bool, Error> {
    let analysis = analyze(identity, None)?;
    match analysis.classification {
        Classification::Recoverable(plan) => Ok(plan.seed.actions.len() == 1
            && plan.seed.actions[0].kind == RecoveryActionKind::ResumeCloseout
            && plan.seed.actions[0].target == phase_id),
        _ => Ok(false),
    }
}

fn finalize_recovery(
    identity: &Identity,
    journal_file: &mut RecoveryJournalFile,
    entry_index: usize,
    recovery_id: &str,
) -> Result<String, Error> {
    let plan = journal_file.recoveries[entry_index].plan.clone();
    let action_count = plan.actions.len();
    // Recompute the post-recovery subject: it must be healthy and equal the
    // final prefix hash.
    let analysis = analyze(identity, None)?;
    match analysis.classification {
        Classification::Healthy => {}
        _ => return Err(Error::RecoveryPostconditionFailed),
    }
    if analysis.subject_sha256 != plan.prefix_subject_sha256[action_count] {
        return Err(Error::RecoveryPostconditionFailed);
    }
    let post = analysis.subject_sha256.clone();
    let (applied_journal, receipt_sha) = build_applied_journal(journal_file, entry_index, &post)
        .map_err(|_| Error::RecoveryPostconditionFailed)?;
    let receipt = applied_journal.recoveries[entry_index]
        .recovery_receipt
        .clone()
        .ok_or(Error::RecoveryPostconditionFailed)?;
    *journal_file = applied_journal;
    publish_journal(
        identity,
        journal_file,
        recovery_id,
        "after_final_ledger_temp_write",
    )?;

    // Re-read and fully validate the published ledger, receipt chain, plan
    // hashes, prefix hashes, and final healthy subject.
    let reloaded = read_journal(&identity.gov_dir)?.ok_or(Error::RecoveryPostconditionFailed)?;
    let reloaded_analysis = analyze(identity, Some(&reloaded))?;
    match reloaded_analysis.classification {
        Classification::Healthy => {}
        _ => return Err(Error::RecoveryPostconditionFailed),
    }
    if reloaded_analysis.subject_sha256 != analysis.subject_sha256 {
        return Err(Error::RecoveryPostconditionFailed);
    }
    let reloaded_entry = &reloaded.file.recoveries[entry_index];
    if reloaded_entry.recovery_receipt_sha256.as_deref() != Some(&receipt_sha) {
        return Err(Error::RecoveryPostconditionFailed);
    }
    Ok(recovery_applied_output(
        reloaded_entry,
        &receipt,
        &plan.pre_subject_sha256,
    ))
}

// ============================================================================
// Inspect command (contract sections 3, 16)
// ============================================================================

pub fn cmd_recovery_inspect(repo_arg: &str) -> Result<String, Error> {
    let identity = resolve_identity(repo_arg)?;
    let journal = read_journal(&identity.gov_dir)?;
    let analysis = analyze(&identity, journal.as_ref())?;
    let mut output = String::new();
    match analysis.classification {
        Classification::Healthy => {
            output.push_str(&format!(
                "RECOVERY_NOT_REQUIRED {}\n",
                analysis.subject_sha256
            ));
        }
        Classification::Recoverable(plan) => {
            output.push_str(&format!(
                "RECOVERY_REQUIRED {} {} {}\n",
                plan.recovery_id,
                analysis.subject_sha256,
                plan.seed.actions.len()
            ));
            for (i, action) in plan.seed.actions.iter().enumerate() {
                output.push_str(&format!(
                    "RECOVERY_ACTION {} {} {}\n",
                    i + 1,
                    action.kind.as_str(),
                    action.target
                ));
            }
        }
        Classification::Pending {
            recovery_id,
            next_action,
            action_count,
        } => {
            output.push_str(&format!(
                "RECOVERY_PENDING {} {} {}\n",
                recovery_id, next_action, action_count
            ));
        }
    }
    if output.ends_with('\n') {
        output.pop();
    }
    Ok(output)
}
