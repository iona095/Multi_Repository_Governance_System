use crate::plan::Plan;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct AcceptedPlan {
    pub schema_version: u32,
    pub plan_id: String,
    pub plan_path: String,
    pub sha256: String,
    pub phase_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractDraftPreimage {
    pub revision: u32,
    #[serde(deserialize_with = "deserialize_lowercase_sha")]
    pub sha256: String,
}

fn deserialize_lowercase_sha<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let s = String::deserialize(deserializer)?;
    if !is_valid_sha64(&s) {
        return Err(serde::de::Error::custom(
            "sha256 must be lowercase 64-character hex",
        ));
    }
    Ok(s)
}

fn deserialize_preimage<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<ContractDraftPreimage>, D::Error> {
    use serde::de::Visitor;
    struct PreimageVisitor;
    impl<'de> Visitor<'de> for PreimageVisitor {
        type Value = Option<ContractDraftPreimage>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a preimage object or absent")
        }
        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Err(serde::de::Error::custom("null is not allowed for preimage"))
        }
        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Err(serde::de::Error::custom("null is not allowed for preimage"))
        }
        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            ContractDraftPreimage::deserialize(deserializer).map(Some)
        }
    }
    deserializer.deserialize_option(PreimageVisitor)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractDraft {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub phase_id: String,
    pub contract_id: String,
    pub revision: u32,
    #[serde(
        default,
        deserialize_with = "deserialize_preimage",
        skip_serializing_if = "Option::is_none"
    )]
    pub preimage: Option<ContractDraftPreimage>,
    pub source_path: String,
    pub sha256: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedContractLedger {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub phase_id: String,
    pub contract_id: String,
    pub revisions: Vec<AcceptedRevision>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedRevision {
    pub revision: u32,
    pub source_path: String,
    pub sha256: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationAuthority {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub phase_id: String,
    pub contract_id: String,
    pub contract_revision: u32,
    pub contract_source_path: String,
    pub contract_sha256: String,
    pub contract_content: String,
    pub git_object_format: String,
    pub baseline_head: String,
    pub baseline_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceState {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub active_phase: Option<String>,
    pub closed_phases: Vec<String>,
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
static NEXT_TEMP_NAME: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(test)]
#[allow(dead_code)]
pub fn set_next_temp_name(name: &str) {
    *NEXT_TEMP_NAME.lock().unwrap() = Some(name.to_string());
}

fn unique_temp_name(filename: &str) -> String {
    #[cfg(test)]
    {
        let mut next = NEXT_TEMP_NAME.lock().unwrap();
        if let Some(name) = next.take() {
            return name;
        }
    }
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(".{}.{}.{}.{}.tmp", pid, count, ts, filename)
}

#[cfg(windows)]
pub(crate) fn rename_replace(src: &Path, dst: &Path) -> std::io::Result<()> {
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

    let src_wide: Vec<u16> = OsStr::new(src)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let dst_wide: Vec<u16> = OsStr::new(dst)
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
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn rename_replace(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::rename(src, dst)
}

fn validate_governance_filename(filename: &str) -> Result<(), crate::error::Error> {
    match filename {
        "accepted-plan.json"
        | "state.json"
        | "contract-draft.json"
        | "accepted-contract.json"
        | "implementation-authority.json"
        | "audit-ledger.json"
        | "completion-ledger.json" => {}
        _ => {
            return Err(crate::error::Error::UnauthorizedFilename(
                filename.to_string(),
            ))
        }
    }
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err(crate::error::Error::UnauthorizedFilename(
            filename.to_string(),
        ));
    }
    Ok(())
}

pub fn atomic_write_json<T: Serialize>(
    dir: &Path,
    filename: &str,
    value: &T,
) -> Result<(), crate::error::Error> {
    validate_governance_filename(filename)?;

    if !dir.exists() {
        return Err(crate::error::Error::GovDirNotExists(dir.to_path_buf()));
    }
    if !dir.is_dir() {
        return Err(crate::error::Error::GovDirNotDirectory(dir.to_path_buf()));
    }

    let final_path = dir.join(filename);
    let json = serde_json::to_string_pretty(value)?;

    let mut tmp_path = None;
    let mut attempts = 0usize;
    while attempts < 16 {
        attempts += 1;
        let name = unique_temp_name(filename);
        let candidate = dir.join(&name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                let mut f = file;
                let write_result = (|| -> std::io::Result<()> {
                    f.write_all(json.as_bytes())?;
                    f.sync_all()?;
                    drop(f);
                    Ok(())
                })();
                match write_result {
                    Ok(()) => {
                        tmp_path = Some(candidate);
                        break;
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&candidate);
                        return Err(e.into());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }

    let tmp_path = match tmp_path {
        Some(p) => p,
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "temporary file name collision after 16 attempts",
            )
            .into());
        }
    };

    let replace_result = rename_replace(&tmp_path, &final_path);
    if let Err(e) = replace_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }

    Ok(())
}

fn is_valid_sha64(s: &str) -> bool {
    s.len() == 64
        && s.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

pub fn read_accepted_plan(repo_path: &Path) -> Result<AcceptedPlan, crate::error::Error> {
    let path = repo_path.join(".mrgs").join("accepted-plan.json");
    if !path.exists() {
        return Err(crate::error::Error::NoAcceptedPlan(repo_path.to_path_buf()));
    }
    let bytes =
        std::fs::read(&path).map_err(|_| crate::error::Error::GovernanceAuthorityInvalid)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn read_state(repo_path: &Path) -> Result<GovernanceState, crate::error::Error> {
    let path = repo_path.join(".mrgs").join("state.json");
    if !path.exists() {
        return Err(crate::error::Error::NoState(repo_path.to_path_buf()));
    }
    let bytes =
        std::fs::read(&path).map_err(|_| crate::error::Error::GovernanceAuthorityInvalid)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn validate_accepted_plan_record(record: &AcceptedPlan) -> Result<(), crate::error::Error> {
    if record.schema_version != 1 {
        return Err(crate::error::Error::AcceptedSchemaVersion(
            record.schema_version,
        ));
    }
    if record.plan_id.is_empty() {
        return Err(crate::error::Error::EmptyPlanId);
    }
    let _ = crate::path::validate_safe_relative_path(&record.plan_path)?;
    if !is_valid_sha64(&record.sha256) {
        return Err(crate::error::Error::InvalidSha256);
    }
    if record.phase_count == 0 {
        return Err(crate::error::Error::ZeroPhases);
    }
    Ok(())
}

pub fn validate_state_record(
    state: &GovernanceState,
    accepted: &AcceptedPlan,
    plan: &Plan,
) -> Result<(), crate::error::Error> {
    if state.schema_version != 1 {
        return Err(crate::error::Error::StateSchemaVersion(
            state.schema_version,
        ));
    }
    if !is_valid_sha64(&state.accepted_plan_sha256) {
        return Err(crate::error::Error::InvalidSha256);
    }
    if state.accepted_plan_sha256 != accepted.sha256 {
        return Err(crate::error::Error::StateShaMismatch);
    }

    let phase_ids: std::collections::HashSet<&str> =
        plan.phases.iter().map(|p| p.id.as_str()).collect();

    let mut closed_seen = std::collections::HashSet::new();
    for cp in &state.closed_phases {
        if !phase_ids.contains(cp.as_str()) {
            return Err(crate::error::Error::UnknownClosedPhase(cp.clone()));
        }
        if !closed_seen.insert(cp.as_str()) {
            return Err(crate::error::Error::DuplicateClosedPhase(cp.clone()));
        }
    }

    for cp in &state.closed_phases {
        if let Some(phase) = plan.phases.iter().find(|p| p.id == *cp) {
            for dep in &phase.depends_on {
                if !state.closed_phases.contains(dep) {
                    return Err(crate::error::Error::InconsistentClosedDep(
                        cp.clone(),
                        dep.clone(),
                    ));
                }
            }
        }
    }

    if let Some(ref active) = state.active_phase {
        if !phase_ids.contains(active.as_str()) {
            return Err(crate::error::Error::UnknownActivePhase(active.clone()));
        }
        if state.closed_phases.contains(active) {
            return Err(crate::error::Error::ActivePhaseAlsoClosed(active.clone()));
        }
        if let Some(phase) = plan.phases.iter().find(|p| p.id == *active) {
            for dep in &phase.depends_on {
                if !state.closed_phases.contains(dep) {
                    return Err(crate::error::Error::ActivePhaseDependencyUnmet(
                        active.clone(),
                        dep.clone(),
                    ));
                }
            }
        }
    }

    Ok(())
}

pub fn validate_plan_consistency(
    accepted: &AcceptedPlan,
    plan: &Plan,
    sha256: &str,
) -> Result<(), crate::error::Error> {
    if accepted.plan_id != plan.plan_id {
        return Err(crate::error::Error::PlanIdMismatch(plan.plan_id.clone()));
    }
    if accepted.phase_count != plan.phases.len() {
        return Err(crate::error::Error::PhaseCountMismatch {
            expected: accepted.phase_count,
            actual: plan.phases.len(),
        });
    }
    if accepted.sha256 != sha256 {
        return Err(crate::error::Error::PlanDrift {
            expected: accepted.sha256.clone(),
            actual: sha256.to_string(),
        });
    }
    Ok(())
}

pub fn validate_contract_draft_record(
    draft: &ContractDraft,
    accepted_sha: &str,
    active_phase: &str,
    contract_id: &str,
) -> Result<(), crate::error::Error> {
    if draft.schema_version != 1 {
        return Err(crate::error::Error::UnsupportedDraftSchema(
            draft.schema_version,
        ));
    }
    if draft.revision < 1 {
        return Err(crate::error::Error::DraftRevisionZero);
    }
    if !is_valid_sha64(&draft.sha256) {
        return Err(crate::error::Error::InvalidSha256);
    }
    if !is_valid_sha64(&draft.accepted_plan_sha256) {
        return Err(crate::error::Error::InvalidSha256);
    }
    if draft.accepted_plan_sha256 != accepted_sha {
        return Err(crate::error::Error::DraftFieldMismatch(
            "accepted_plan_sha256".into(),
        ));
    }
    if draft.phase_id != active_phase {
        return Err(crate::error::Error::DraftFieldMismatch("phase_id".into()));
    }
    if draft.contract_id != contract_id {
        return Err(crate::error::Error::DraftFieldMismatch(
            "contract_id".into(),
        ));
    }
    // Preimage validation
    match &draft.preimage {
        Some(pre) => {
            if draft.revision == 1 {
                return Err(crate::error::Error::DraftPreimageUnexpected);
            }
            if pre.revision < 1 {
                return Err(crate::error::Error::DraftPreimageRevisionZero);
            }
            let expected = draft.revision.checked_sub(1).ok_or_else(|| {
                let zero: u32 = 0;
                crate::error::Error::DraftPreimageRevisionMismatch {
                    preimage_revision: pre.revision,
                    draft_revision: draft.revision,
                    expected: zero,
                }
            })?;
            if pre.revision != expected {
                return Err(crate::error::Error::DraftPreimageRevisionMismatch {
                    preimage_revision: pre.revision,
                    draft_revision: draft.revision,
                    expected,
                });
            }
            if !is_valid_sha64(&pre.sha256) {
                return Err(crate::error::Error::DraftPreimageShaInvalid);
            }
        }
        None => {
            if draft.revision > 1 {
                return Err(crate::error::Error::DraftPreimageRequired {
                    draft_revision: draft.revision,
                });
            }
        }
    }
    crate::path::validate_strict_normalized_path(&draft.source_path)?;
    let parsed: crate::contract::Contract = toml::from_str(&draft.content)?;
    parsed.validate()?;
    if draft.contract_id != parsed.contract_id {
        return Err(crate::error::Error::DraftFieldMismatch(
            "contract_id".into(),
        ));
    }
    if draft.phase_id != parsed.phase_id {
        return Err(crate::error::Error::DraftFieldMismatch("phase_id".into()));
    }
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(draft.content.as_bytes());
    let computed = format!("{:x}", hasher.finalize());
    if computed != draft.sha256 {
        return Err(crate::error::Error::DraftContentHashMismatch);
    }
    Ok(())
}

pub fn is_draft_idempotent(
    existing_sha: &str,
    existing_content: &str,
    submitted_sha: &str,
    submitted_bytes: &[u8],
) -> bool {
    existing_sha == submitted_sha && existing_content.as_bytes() == submitted_bytes
}

pub fn read_contract_draft(gov_dir: &Path) -> Result<ContractDraft, crate::error::Error> {
    let path = gov_dir.join("contract-draft.json");
    let bytes =
        std::fs::read(&path).map_err(|_| crate::error::Error::GovernanceAuthorityInvalid)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn read_accepted_contract_ledger(
    gov_dir: &Path,
) -> Result<AcceptedContractLedger, crate::error::Error> {
    let path = gov_dir.join("accepted-contract.json");
    let bytes =
        std::fs::read(&path).map_err(|_| crate::error::Error::GovernanceAuthorityInvalid)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn validate_accepted_contract_ledger(
    ledger: &AcceptedContractLedger,
    accepted_plan_sha: &str,
    active_phase: &str,
    draft: Option<&ContractDraft>,
) -> Result<(), crate::error::Error> {
    if ledger.schema_version != 1 {
        return Err(crate::error::Error::AcceptedSchemaVersion(
            ledger.schema_version,
        ));
    }
    if !is_valid_sha64(&ledger.accepted_plan_sha256) {
        return Err(crate::error::Error::InvalidSha256);
    }
    if ledger.accepted_plan_sha256 != accepted_plan_sha {
        return Err(crate::error::Error::AcceptedContractPlanShaMismatch);
    }
    if ledger.phase_id.is_empty() {
        return Err(crate::error::Error::EmptyContractField("phase_id".into()));
    }
    if ledger.phase_id != active_phase {
        return Err(crate::error::Error::AcceptedContractPhaseMismatch {
            expected: active_phase.to_string(),
            actual: ledger.phase_id.clone(),
        });
    }
    if ledger.contract_id.is_empty() {
        return Err(crate::error::Error::EmptyContractField(
            "contract_id".into(),
        ));
    }
    if ledger.revisions.is_empty() {
        return Err(crate::error::Error::AcceptedContractEmptyRevisions);
    }
    let mut prev_revision: Option<u32> = None;
    let mut seen_revisions = std::collections::HashSet::new();
    for rev in &ledger.revisions {
        if rev.revision < 1 {
            return Err(crate::error::Error::AcceptedContractRevisionZero);
        }
        if let Some(prev) = prev_revision {
            if rev.revision <= prev {
                return Err(crate::error::Error::AcceptedContractNonIncreasingRevision(
                    rev.revision,
                    prev,
                ));
            }
        }
        if !seen_revisions.insert(rev.revision) {
            return Err(crate::error::Error::AcceptedContractDuplicateRevision(
                rev.revision,
            ));
        }
        prev_revision = Some(rev.revision);

        crate::path::validate_strict_normalized_path(&rev.source_path)?;
        if rev.source_path == ".mrgs" || rev.source_path.starts_with(".mrgs/") {
            return Err(crate::error::Error::AcceptedContractSourceUnderMrgs);
        }
        if !is_valid_sha64(&rev.sha256) {
            return Err(crate::error::Error::InvalidSha256);
        }
        let parsed: crate::contract::Contract = match toml::from_str(&rev.content) {
            Ok(c) => c,
            Err(_) => return Err(crate::error::Error::AcceptedContractContentParse),
        };
        parsed.validate()?;
        if parsed.phase_id != ledger.phase_id {
            return Err(crate::error::Error::AcceptedContractContentPhaseMismatch);
        }
        if parsed.contract_id != ledger.contract_id {
            return Err(crate::error::Error::AcceptedContractContentIdMismatch);
        }
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(rev.content.as_bytes());
        let computed = format!("{:x}", hasher.finalize());
        if computed != rev.sha256 {
            return Err(crate::error::Error::AcceptedContractContentHashMismatch);
        }
    }
    if let Some(draft) = draft {
        if ledger.contract_id != draft.contract_id {
            return Err(
                crate::error::Error::AcceptedContractDraftContractIdMismatch {
                    accepted: ledger.contract_id.clone(),
                    draft: draft.contract_id.clone(),
                },
            );
        }
        let final_rev = ledger.revisions.last().unwrap();
        if final_rev.revision > draft.revision {
            return Err(
                crate::error::Error::AcceptedContractFinalRevisionExceedsDraft {
                    revision: final_rev.revision,
                    draft_revision: draft.revision,
                },
            );
        }
        if final_rev.revision == draft.revision
            && (final_rev.sha256 != draft.sha256
                || final_rev.source_path != draft.source_path
                || final_rev.content != draft.content)
        {
            return Err(crate::error::Error::AcceptedContractEqualRevisionContentMismatch);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_sha64_rejects_uppercase() {
        let upper = "ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEF";
        assert!(!is_valid_sha64(upper));
    }

    #[test]
    fn test_is_valid_sha64_accepts_lowercase() {
        let lower = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";
        assert!(is_valid_sha64(lower));
    }

    #[test]
    fn test_is_valid_sha64_accepts_digits() {
        let digits = "0123456789012345678901234567890123456789012345678901234567890123";
        assert!(is_valid_sha64(digits));
    }

    #[test]
    fn test_is_valid_sha64_rejects_wrong_length() {
        assert!(!is_valid_sha64("abc"));
        assert!(!is_valid_sha64(""));
    }

    #[test]
    fn test_unique_temp_names_differ() {
        let a = unique_temp_name("state.json");
        let b = unique_temp_name("state.json");
        assert_ne!(a, b);
        assert!(a.contains(".tmp"));
        assert!(b.contains(".tmp"));
    }

    #[test]
    fn test_validate_governance_filename_accepts_valid() {
        assert!(validate_governance_filename("accepted-plan.json").is_ok());
        assert!(validate_governance_filename("state.json").is_ok());
        assert!(validate_governance_filename("contract-draft.json").is_ok());
        assert!(validate_governance_filename("accepted-contract.json").is_ok());
        assert!(validate_governance_filename("implementation-authority.json").is_ok());
    }

    #[test]
    fn test_validate_governance_filename_rejects_invalid() {
        assert!(validate_governance_filename("other.json").is_err());
        assert!(validate_governance_filename("accepted-plan.json/").is_err());
        assert!(validate_governance_filename("../state.json").is_err());
    }

    #[test]
    fn test_is_draft_idempotent_requires_exact_bytes_beyond_digest() {
        let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let existing_content = "content version a";
        let submitted_bytes = b"content version b";
        // Equal digest metadata but different byte content
        assert!(!is_draft_idempotent(
            digest,
            existing_content,
            digest,
            submitted_bytes,
        ));
        // Equal digest AND equal bytes succeeds
        assert!(is_draft_idempotent(
            digest,
            existing_content,
            digest,
            existing_content.as_bytes(),
        ));
    }

    #[test]
    fn test_is_draft_idempotent_rejects_sha_mismatch() {
        let sha_a = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let sha_b = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        assert!(!is_draft_idempotent(sha_a, "content", sha_b, b"content",));
    }

    #[test]
    fn test_create_new_does_not_truncate_existing() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("explicit_test_file.tmp");
        std::fs::write(&path, b"original content").unwrap();
        let result = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"original content");
    }
}
