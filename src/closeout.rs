use crate::audit::{self, AuditLedger};
use crate::error::Error;
use crate::implementation::{self, ValidatedAuthority};
use crate::state::{self, GovernanceState, ImplementationAuthority};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Validate that all required keys are present in a JSON object.
/// This catches omitted required fields that serde's Option<T> would silently accept.
fn require_keys(obj: &serde_json::Value, keys: &[&str]) -> Result<(), Error> {
    if let serde_json::Value::Object(map) = obj {
        for key in keys {
            if !map.contains_key(*key) {
                return Err(Error::CloseoutLedgerInvalid);
            }
        }
        Ok(())
    } else {
        Err(Error::CloseoutLedgerInvalid)
    }
}

// ============================================================================
// Phase-scoped governance filenames (fixed order for cleanup)
// ============================================================================

const PHASE_SCOPED_FILES: [&str; 4] = [
    "contract-draft.json",
    "accepted-contract.json",
    "implementation-authority.json",
    "audit-ledger.json",
];

const AUDIT_LEDGER_CLEANUP_ORDER: [&str; 4] = [
    "audit-ledger.json",
    "implementation-authority.json",
    "accepted-contract.json",
    "contract-draft.json",
];

// ============================================================================
// Completion Ledger Schema (Section 13)
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionLedger {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub plan_id: String,
    pub completions: Vec<CompletionEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionEntry {
    pub final_manifest: FinalManifest,
    pub final_manifest_sha256: String,
    pub completion_receipt: CompletionReceipt,
    pub completion_receipt_sha256: String,
}

// ============================================================================
// Final Manifest (Section 8)
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalManifest {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub plan_id: String,
    pub plan_source_path: String,
    pub plan_content: String,
    pub phase_id: String,
    pub phase_title: String,
    pub phase_dependencies: Vec<String>,
    pub plan_phase_index: usize,
    pub completion_sequence: u32,
    pub contract_id: String,
    pub contract_revision: u32,
    pub contract_source_path: String,
    pub contract_sha256: String,
    pub contract_content: String,
    pub implementation_baseline_head: String,
    pub implementation_baseline_branch: String,
    pub git_object_format: String,
    pub final_head: String,
    pub final_branch: String,
    pub final_audit_id: String,
    pub final_audit_round: u32,
    pub final_auditor_id: String,
    pub final_subject_sha256: String,
    pub final_subject: audit::AuditSubject,
    pub final_report_source_path: String,
    pub final_report_sha256: String,
    pub final_report_content: String,
    pub archived_governance: ArchivedGovernance,
}

// ============================================================================
// Archived Governance (Section 9)
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchivedGovernance {
    pub contract_draft_sha256: String,
    pub contract_draft_content: String,
    pub accepted_contract_sha256: String,
    pub accepted_contract_content: String,
    pub implementation_authority_sha256: String,
    pub implementation_authority_content: String,
    pub audit_ledger_sha256: String,
    pub audit_ledger_content: String,
}

// ============================================================================
// Completion Receipt (Section 11)
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionReceipt {
    pub schema_version: u32,
    pub accepted_plan_sha256: String,
    pub plan_id: String,
    pub phase_id: String,
    pub phase_title: String,
    pub plan_phase_index: usize,
    pub completion_sequence: u32,
    pub final_manifest_sha256: String,
    pub previous_completion_receipt_sha256: Option<String>,
    pub closed_phases_before: Vec<String>,
    pub closed_phases_after: Vec<String>,
    pub active_phase_before: Option<String>,
    pub active_phase_after: Option<serde_json::Value>,
}

// ============================================================================
// Deterministic Compact Serialization (Section 10 / 12)
// ============================================================================
// Hashes are computed over compact canonical JSON using serde_json::to_string
// (no whitespace, no trailing newline). Persisted ledger/state files use
// serde_json::to_string_pretty for human-readable §22 output.

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn compute_manifest_hash(m: &FinalManifest) -> Result<String, Error> {
    let json = serde_json::to_string(m).map_err(|_| Error::CloseoutLedgerInvalid)?;
    Ok(sha256_hex(json.as_bytes()))
}

// ============================================================================
// Completion Receipt Serialization (Section 11 field order)
// ============================================================================

fn compute_receipt_hash(r: &CompletionReceipt) -> Result<String, Error> {
    let json = serde_json::to_string(r).map_err(|_| Error::CloseoutLedgerInvalid)?;
    Ok(sha256_hex(json.as_bytes()))
}

// ============================================================================
// Completion Ledger Serialization (Section 13)
// ============================================================================

fn serialize_ledger(ledger: &CompletionLedger) -> String {
    serde_json::to_string_pretty(ledger).unwrap_or_default()
}

fn read_governance_file_bytes(gov_dir: &Path, filename: &str) -> Result<(String, String), Error> {
    let path = gov_dir.join(filename);
    let bytes = std::fs::read(&path).map_err(|_| Error::CloseoutNotReady)?;
    let content = String::from_utf8(bytes.clone()).map_err(|_| Error::CloseoutNotReady)?;
    let hash = sha256_hex(&bytes);
    Ok((content, hash))
}

fn validate_governance_file_safety(gov_dir: &Path, filename: &str) -> Result<PathBuf, Error> {
    let path = gov_dir.join(filename);
    if !path.exists() {
        return Err(Error::CloseoutNotReady);
    }
    let meta = std::fs::symlink_metadata(&path).map_err(|_| Error::CloseoutNotReady)?;
    if !meta.file_type().is_file() {
        return Err(Error::CloseoutNotReady);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(Error::CloseoutNotReady);
        }
    }
    Ok(path)
}

fn file_exists(gov_dir: &Path, filename: &str) -> bool {
    let path = gov_dir.join(filename);
    path.exists() && path.is_file()
}

// ============================================================================
// Completion Ledger Reading and Validation
// ============================================================================

fn read_completion_ledger(gov_dir: &Path) -> Result<Option<CompletionLedger>, Error> {
    let path = gov_dir.join("completion-ledger.json");
    if !path.exists() {
        return Ok(None);
    }
    // Safe topology: must be a regular file, not a symlink/junction/device.
    let meta = std::fs::symlink_metadata(&path).map_err(|_| Error::CloseoutLedgerInvalid)?;
    if !meta.file_type().is_file() {
        return Err(Error::CloseoutLedgerInvalid);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(Error::CloseoutLedgerInvalid);
        }
    }
    let bytes = std::fs::read(&path).map_err(|_| Error::CloseoutLedgerInvalid)?;
    // Strict raw JSON key presence validation (§13: all fields required).
    // serde Option<T> silently accepts missing keys; we catch that here.
    let raw: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| Error::CloseoutLedgerInvalid)?;
    require_keys(
        &raw,
        &[
            "schema_version",
            "accepted_plan_sha256",
            "plan_id",
            "completions",
        ],
    )?;
    if let Some(entries) = raw["completions"].as_array() {
        let receipt_keys = &[
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
        let manifest_keys = &[
            "schema_version",
            "accepted_plan_sha256",
            "plan_id",
            "plan_source_path",
            "plan_content",
            "phase_id",
            "phase_title",
            "phase_dependencies",
            "plan_phase_index",
            "completion_sequence",
            "contract_id",
            "contract_revision",
            "contract_source_path",
            "contract_sha256",
            "contract_content",
            "implementation_baseline_head",
            "implementation_baseline_branch",
            "git_object_format",
            "final_head",
            "final_branch",
            "final_audit_id",
            "final_audit_round",
            "final_auditor_id",
            "final_subject_sha256",
            "final_subject",
            "final_report_source_path",
            "final_report_sha256",
            "final_report_content",
            "archived_governance",
        ];
        for entry in entries {
            require_keys(
                entry,
                &[
                    "final_manifest",
                    "final_manifest_sha256",
                    "completion_receipt",
                    "completion_receipt_sha256",
                ],
            )?;
            if let Some(m) = entry.get("final_manifest") {
                require_keys(m, manifest_keys)?;
            }
            if let Some(r) = entry.get("completion_receipt") {
                require_keys(r, receipt_keys)?;
            }
        }
    }
    let ledger: CompletionLedger =
        serde_json::from_slice(&bytes).map_err(|_| Error::CloseoutLedgerInvalid)?;
    Ok(Some(ledger))
}

fn validate_completion_ledger(
    ledger: &CompletionLedger,
    accepted_plan_sha256: &str,
    plan_id: &str,
) -> Result<(), Error> {
    if ledger.schema_version != 1 {
        return Err(Error::CloseoutLedgerInvalid);
    }
    if ledger.accepted_plan_sha256 != accepted_plan_sha256 {
        return Err(Error::CloseoutLedgerStale);
    }
    if ledger.plan_id != plan_id {
        return Err(Error::CloseoutLedgerStale);
    }

    let mut seen_phases: BTreeSet<&str> = BTreeSet::new();
    let mut prev_receipt_hash: Option<String> = None;
    let mut closed_before: Vec<String> = vec![];

    for (i, entry) in ledger.completions.iter().enumerate() {
        // Contiguous sequence from 1
        let expected_seq = (i as u32) + 1;
        if entry.completion_receipt.completion_sequence != expected_seq {
            return Err(Error::CloseoutLedgerInvalid);
        }

        // Manifest plan authority matches accepted plan
        if entry.final_manifest.accepted_plan_sha256 != accepted_plan_sha256 {
            return Err(Error::CloseoutLedgerStale);
        }
        if entry.final_manifest.plan_id != plan_id {
            return Err(Error::CloseoutLedgerStale);
        }

        // Manifest hash recomputes
        let recomputed_manifest = compute_manifest_hash(&entry.final_manifest)?;
        if recomputed_manifest != entry.final_manifest_sha256 {
            return Err(Error::CloseoutLedgerInvalid);
        }
        // Receipt hash recomputes
        let recomputed_receipt = compute_receipt_hash(&entry.completion_receipt)?;
        if recomputed_receipt != entry.completion_receipt_sha256 {
            return Err(Error::CloseoutLedgerInvalid);
        }

        // Receipt names its manifest hash
        if entry.completion_receipt.final_manifest_sha256 != entry.final_manifest_sha256 {
            return Err(Error::CloseoutLedgerInvalid);
        }

        // Previous receipt hash chain
        if entry.completion_receipt.previous_completion_receipt_sha256 != prev_receipt_hash {
            return Err(Error::CloseoutLedgerInvalid);
        }
        prev_receipt_hash = Some(entry.completion_receipt_sha256.clone());

        // closed_phases_before matches previous closed_phases_after
        if entry.completion_receipt.closed_phases_before != closed_before {
            return Err(Error::CloseoutLedgerInvalid);
        }

        // closed_phases_after appends exactly the receipt phase
        let mut expected_after = closed_before.clone();
        expected_after.push(entry.completion_receipt.phase_id.clone());
        if entry.completion_receipt.closed_phases_after != expected_after {
            return Err(Error::CloseoutLedgerInvalid);
        }

        // active_phase_after is null (serde deserializes JSON null as None for Option<Value>)
        let apha_null = entry
            .completion_receipt
            .active_phase_after
            .as_ref()
            .map(|v| v.is_null())
            .unwrap_or(true); // None also counts as null
        if !apha_null {
            return Err(Error::CloseoutLedgerInvalid);
        }

        // Each phase appears at most once
        if !seen_phases.insert(&entry.completion_receipt.phase_id) {
            return Err(Error::CloseoutLedgerInvalid);
        }

        // Dependencies completed before
        for dep in &entry.final_manifest.phase_dependencies {
            if !seen_phases.contains(dep.as_str()) {
                return Err(Error::CloseoutLedgerInvalid);
            }
        }

        // Manifest and receipt phase identity agree
        if entry.final_manifest.phase_id != entry.completion_receipt.phase_id {
            return Err(Error::CloseoutLedgerInvalid);
        }
        if entry.final_manifest.completion_sequence != entry.completion_receipt.completion_sequence
        {
            return Err(Error::CloseoutLedgerInvalid);
        }
        if entry.final_manifest.plan_phase_index != entry.completion_receipt.plan_phase_index {
            return Err(Error::CloseoutLedgerInvalid);
        }
        if entry.final_manifest.phase_title != entry.completion_receipt.phase_title {
            return Err(Error::CloseoutLedgerInvalid);
        }

        // Subject hash recomputes from stored subject
        if let Ok(recomputed) = audit::compute_subject_sha256(&entry.final_manifest.final_subject) {
            if recomputed != entry.final_manifest.final_subject_sha256 {
                return Err(Error::CloseoutLedgerInvalid);
            }
        }

        // Report hash recomputes from stored report content
        let recomputed_report = sha256_hex(entry.final_manifest.final_report_content.as_bytes());
        if recomputed_report != entry.final_manifest.final_report_sha256 {
            return Err(Error::CloseoutLedgerInvalid);
        }

        // Archived governance records parse and revalidate
        let ag = &entry.final_manifest.archived_governance;
        let _: state::ContractDraft = serde_json::from_str(&ag.contract_draft_content)
            .map_err(|_| Error::CloseoutLedgerInvalid)?;
        let _: state::AcceptedContractLedger = serde_json::from_str(&ag.accepted_contract_content)
            .map_err(|_| Error::CloseoutLedgerInvalid)?;
        let _: state::ImplementationAuthority =
            serde_json::from_str(&ag.implementation_authority_content)
                .map_err(|_| Error::CloseoutLedgerInvalid)?;
        let _: crate::audit::AuditLedger = serde_json::from_str(&ag.audit_ledger_content)
            .map_err(|_| Error::CloseoutLedgerInvalid)?;

        // Archived governance hashes recompute
        if sha256_hex(ag.contract_draft_content.as_bytes()) != ag.contract_draft_sha256 {
            return Err(Error::CloseoutLedgerInvalid);
        }
        if sha256_hex(ag.accepted_contract_content.as_bytes()) != ag.accepted_contract_sha256 {
            return Err(Error::CloseoutLedgerInvalid);
        }
        if sha256_hex(ag.implementation_authority_content.as_bytes())
            != ag.implementation_authority_sha256
        {
            return Err(Error::CloseoutLedgerInvalid);
        }
        if sha256_hex(ag.audit_ledger_content.as_bytes()) != ag.audit_ledger_sha256 {
            return Err(Error::CloseoutLedgerInvalid);
        }

        closed_before = entry.completion_receipt.closed_phases_after.clone();
    }

    Ok(())
}

// ============================================================================
// State-to-Ledger Relation (Section 14)
// ============================================================================

fn validate_state_ledger_relation(
    state: &GovernanceState,
    ledger: Option<&CompletionLedger>,
) -> Result<(), Error> {
    match ledger {
        None => {
            // Missing ledger is legal only when closed_phases is empty
            if !state.closed_phases.is_empty() {
                return Err(Error::CloseoutLedgerStale);
            }
        }
        Some(ledger) => {
            let ledger_phases: Vec<String> = ledger
                .completions
                .iter()
                .map(|e| e.completion_receipt.phase_id.clone())
                .collect();

            if state.closed_phases == ledger_phases {
                // Stable relation: active_phase names a different open phase or is null
                // Both are legal
            } else if ledger_phases.len() == state.closed_phases.len() + 1 {
                // In-progress finalization: ledger == closed_phases + [active_phase]
                if let Some(active) = &state.active_phase {
                    let mut expected = state.closed_phases.clone();
                    expected.push(active.clone());
                    if ledger_phases != expected {
                        return Err(Error::CloseoutStateMismatch);
                    }
                } else {
                    return Err(Error::CloseoutStateMismatch);
                }
            } else {
                return Err(Error::CloseoutStateMismatch);
            }
        }
    }
    Ok(())
}

// ============================================================================
// Phase ID Validation (Section 5)
// ============================================================================

fn validate_phase_id(phase_id: &str) -> Result<(), Error> {
    if phase_id.is_empty() {
        return Err(Error::CloseoutConflict);
    }
    if phase_id.len() > 128 {
        return Err(Error::CloseoutConflict);
    }
    if phase_id != phase_id.trim() {
        return Err(Error::CloseoutConflict);
    }
    if phase_id.chars().any(|c| c.is_control()) {
        return Err(Error::CloseoutConflict);
    }
    Ok(())
}

// ============================================================================
// Closeout Readiness (Section 7)
// ============================================================================

struct CloseoutContext {
    repo: PathBuf,
    gov_dir: PathBuf,
    accepted_plan_sha256: String,
    plan_id: String,
    plan_phase_index: usize,
    plan_phase_title: String,
    plan_phase_dependencies: Vec<String>,
    state: GovernanceState,
    phase_id: String,
}

fn validate_closeout_readiness(ctx: &CloseoutContext) -> Result<ValidatedAuthority, Error> {
    // 1. requested phase is active phase
    let active = ctx
        .state
        .active_phase
        .as_ref()
        .ok_or(Error::CloseoutNotReady)?;
    if *active != ctx.phase_id {
        return Err(Error::CloseoutConflict);
    }

    // 2. phase not in closed_phases
    if ctx.state.closed_phases.contains(&ctx.phase_id) {
        return Err(Error::CloseoutConflict);
    }

    // 3. all dependencies closed
    for dep in &ctx.plan_phase_dependencies {
        if !ctx.state.closed_phases.contains(dep) {
            return Err(Error::CloseoutNotReady);
        }
    }

    // 4. contract draft exists and is valid
    if !file_exists(&ctx.gov_dir, "contract-draft.json") {
        return Err(Error::CloseoutNotReady);
    }
    let draft = state::read_contract_draft(&ctx.gov_dir)?;
    state::validate_contract_draft_record(
        &draft,
        &ctx.accepted_plan_sha256,
        &ctx.phase_id,
        &draft.contract_id,
    )?;

    // 5. accepted contract exists and is valid
    if !file_exists(&ctx.gov_dir, "accepted-contract.json") {
        return Err(Error::CloseoutNotReady);
    }
    let ledger = state::read_accepted_contract_ledger(&ctx.gov_dir)?;
    state::validate_accepted_contract_ledger(
        &ledger,
        &ctx.accepted_plan_sha256,
        &ctx.phase_id,
        Some(&draft),
    )?;

    // 6. implementation authority exists and is valid
    if !file_exists(&ctx.gov_dir, "implementation-authority.json") {
        return Err(Error::CloseoutNotReady);
    }
    let impl_auth: ImplementationAuthority = {
        let bytes = std::fs::read(ctx.gov_dir.join("implementation-authority.json"))
            .map_err(|_| Error::ImplementationAuthorityInvalid)?;
        serde_json::from_slice(&bytes).map_err(|_| Error::ImplementationAuthorityInvalid)?
    };
    implementation::validate_impl_record_structure(&impl_auth, &impl_auth.git_object_format)?;

    // 7. full Phase 4 authority validation (includes Git context, branch, HEAD,
    //    operation state, index structure, sparse config, and implementation check)
    let auth = implementation::validate_phase4_authority(
        ctx.repo.to_str().ok_or(Error::CloseoutNotReady)?,
    )?;

    // 8. audit ledger exists and is valid
    let audit_ledger = audit::read_audit_ledger(&ctx.gov_dir)?.ok_or(Error::CloseoutNotReady)?;
    audit::validate_ledger_history(&audit_ledger, &auth)?;

    // 9. Phase 5 lifecycle is PASSED
    let lifecycle = audit::infer_lifecycle(Some(&audit_ledger));
    if lifecycle != "PASSED" {
        return Err(Error::CloseoutNotReady);
    }

    // 10. final audit round is PASS with no repair route
    let final_round = audit_ledger.rounds.last().ok_or(Error::CloseoutNotReady)?;
    if final_round.status != "PASS" {
        return Err(Error::CloseoutNotReady);
    }
    if final_round.repair.is_some() {
        return Err(Error::CloseoutNotReady);
    }

    // 11. report bytes and SHA-256 revalidate
    let report_bytes = final_round
        .report_content
        .as_ref()
        .ok_or(Error::CloseoutNotReady)?;
    let report_hash = sha256_hex(report_bytes.as_bytes());
    if report_hash
        != *final_round
            .report_sha256
            .as_ref()
            .ok_or(Error::CloseoutNotReady)?
    {
        return Err(Error::CloseoutNotReady);
    }

    // 12. every requirement and verification result is PASS
    let report: audit::AuditReport =
        serde_json::from_str(report_bytes).map_err(|_| Error::CloseoutNotReady)?;
    for req in &report.requirement_results {
        if req.status != "PASS" {
            return Err(Error::CloseoutNotReady);
        }
    }
    for v in &report.verification_results {
        if v.status != "PASS" {
            return Err(Error::CloseoutNotReady);
        }
    }

    // 13-14. subject hash equality and subject drift proof
    let passed_subject_hash = &final_round.subject_sha256;
    let recomputed = audit::compute_subject_sha256(&final_round.subject)?;
    if recomputed != *passed_subject_hash {
        return Err(Error::CloseoutNotReady);
    }

    // Rebuild the current subject and compare with the stored passed subject.
    // This detects any drift: changed HEAD, branch, worktree, or index.
    let git = crate::git::GitRunner::new(&ctx.repo);
    let impl_auth: crate::state::ImplementationAuthority = {
        let bytes = std::fs::read(ctx.gov_dir.join("implementation-authority.json"))
            .map_err(|_| Error::ImplementationAuthorityInvalid)?;
        serde_json::from_slice(&bytes).map_err(|_| Error::ImplementationAuthorityInvalid)?
    };
    let tracked_gov = implementation::tracked_governance_paths(&git, &impl_auth.git_object_format)?;
    let change_paths = implementation::build_change_inventory(
        &git,
        &impl_auth,
        &auth,
        &impl_auth.git_object_format,
        &tracked_gov,
    )?;
    let current_subject =
        audit::build_audit_subject(&auth, &git, &impl_auth, &change_paths, &tracked_gov)?;
    let current_subject_hash = audit::compute_subject_sha256(&current_subject)?;
    if current_subject_hash != *passed_subject_hash {
        return Err(Error::CloseoutNotReady);
    }

    Ok(auth)
}

// ============================================================================
// Manifest Construction (Section 8)
// ============================================================================

fn build_final_manifest(
    ctx: &CloseoutContext,
    _auth: &ValidatedAuthority,
    audit_ledger: &AuditLedger,
    impl_auth: &ImplementationAuthority,
    archived: ArchivedGovernance,
    completion_sequence: u32,
) -> Result<FinalManifest, Error> {
    let final_round = audit_ledger.rounds.last().ok_or(Error::CloseoutNotReady)?;
    let report_content = final_round
        .report_content
        .clone()
        .ok_or(Error::CloseoutNotReady)?;
    let report_sha256 = final_round
        .report_sha256
        .clone()
        .ok_or(Error::CloseoutNotReady)?;
    let report_source_path = final_round
        .report_source_path
        .clone()
        .ok_or(Error::CloseoutNotReady)?;

    // Plan phase metadata from pre-extracted context fields
    let plan_phase_index = ctx.plan_phase_index;
    let phase_title = ctx.plan_phase_title.clone();
    let phase_dependencies = ctx.plan_phase_dependencies.clone();

    // Contract content from implementation authority
    let contract_content = impl_auth.contract_content.clone();

    // Plan content: use the accepted plan's recorded path
    let plan_source_path = {
        let accepted = state::read_accepted_plan(&ctx.repo)?;
        accepted.plan_path
    };
    let plan_file = crate::path::resolve_safe_plan_path(&ctx.repo, &plan_source_path)?;
    let plan_content = std::fs::read_to_string(&plan_file).map_err(|_| Error::CloseoutNotReady)?;

    let git = crate::git::GitRunner::new(&ctx.repo);
    let current_head = {
        let out = git.run(["rev-parse", "--verify", "HEAD^{commit}"])?;
        let stdout = String::from_utf8(out.stdout).map_err(|_| Error::CloseoutNotReady)?;
        stdout.trim().to_string()
    };
    let current_branch = {
        let out = git.run(["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        let stdout = String::from_utf8(out.stdout).map_err(|_| Error::CloseoutNotReady)?;
        stdout.trim().to_string()
    };

    let manifest = FinalManifest {
        schema_version: 1,
        accepted_plan_sha256: ctx.accepted_plan_sha256.clone(),
        plan_id: ctx.plan_id.clone(),
        plan_source_path,
        plan_content,
        phase_id: ctx.phase_id.clone(),
        phase_title,
        phase_dependencies,
        plan_phase_index,
        completion_sequence,
        contract_id: impl_auth.contract_id.clone(),
        contract_revision: impl_auth.contract_revision,
        contract_source_path: impl_auth.contract_source_path.clone(),
        contract_sha256: impl_auth.contract_sha256.clone(),
        contract_content,
        implementation_baseline_head: impl_auth.baseline_head.clone(),
        implementation_baseline_branch: impl_auth.baseline_branch.clone(),
        git_object_format: impl_auth.git_object_format.clone(),
        final_head: current_head,
        final_branch: current_branch,
        final_audit_id: final_round.audit_id.clone(),
        final_audit_round: final_round.round,
        final_auditor_id: final_round.auditor_id.clone(),
        final_subject_sha256: final_round.subject_sha256.clone(),
        final_subject: final_round.subject.clone(),
        final_report_source_path: report_source_path,
        final_report_sha256: report_sha256,
        final_report_content: report_content,
        archived_governance: archived,
    };

    Ok(manifest)
}

// ============================================================================
// Completion Receipt Construction (Section 11)
// ============================================================================

fn build_completion_receipt(
    ctx: &CloseoutContext,
    manifest_hash: &str,
    prev_receipt_hash: Option<String>,
    sequence: u32,
) -> Result<CompletionReceipt, Error> {
    let plan_phase_index = ctx.plan_phase_index;
    let phase_title = ctx.plan_phase_title.clone();

    let closed_phases_before = ctx.state.closed_phases.clone();
    let mut closed_phases_after = closed_phases_before.clone();
    closed_phases_after.push(ctx.phase_id.clone());

    let receipt = CompletionReceipt {
        schema_version: 1,
        accepted_plan_sha256: ctx.accepted_plan_sha256.clone(),
        plan_id: ctx.plan_id.clone(),
        phase_id: ctx.phase_id.clone(),
        phase_title,
        plan_phase_index,
        completion_sequence: sequence,
        final_manifest_sha256: manifest_hash.to_string(),
        previous_completion_receipt_sha256: prev_receipt_hash,
        closed_phases_before,
        closed_phases_after,
        active_phase_before: ctx.state.active_phase.clone(),
        active_phase_after: Some(serde_json::Value::Null),
    };

    Ok(receipt)
}

// ============================================================================
// Atomic File Operations (Section 22)
// ============================================================================

/// RAII guard that removes a temp file on drop. Used to satisfy §22's
/// requirement that pre-publication failure leaves no temporary file.
struct TempFileGuard {
    path: Option<PathBuf>,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }
    /// Consumes the guard without removing the file (on success path).
    fn disarm(&mut self) {
        self.path = None;
    }
    fn path(&self) -> &Path {
        self.path.as_ref().unwrap()
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if let Some(p) = self.path.take() {
            let _ = std::fs::remove_file(&p);
        }
    }
}

fn atomic_publish_completion_ledger(
    gov_dir: &Path,
    ledger: &CompletionLedger,
) -> Result<(), Error> {
    let json = serialize_ledger(ledger);
    let final_path = gov_dir.join("completion-ledger.json");

    let tmp_path = create_temp_file(gov_dir, ".closeout")?;
    let mut guard = TempFileGuard::new(tmp_path);

    // Write and sync directly to the create_new handle.
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(guard.path())
            .map_err(|_| Error::PersistenceFailed)?;
        use std::io::Write;
        file.write_all(json.as_bytes())
            .map_err(|_| Error::PersistenceFailed)?;
        file.sync_all().map_err(|_| Error::PersistenceFailed)?;
    }

    // Guard stays armed through rename; disarmed only on success.
    state::rename_replace(guard.path(), &final_path).map_err(|_| Error::PersistenceFailed)?;
    guard.disarm();
    Ok(())
}

fn atomic_replace_state(gov_dir: &Path, state: &GovernanceState) -> Result<(), Error> {
    let json = serde_json::to_string_pretty(state).map_err(|_| Error::PersistenceFailed)?;
    let final_path = gov_dir.join("state.json");

    let tmp_path = create_temp_file(gov_dir, ".closeout-state")?;
    let mut guard = TempFileGuard::new(tmp_path);

    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(guard.path())
            .map_err(|_| Error::PersistenceFailed)?;
        use std::io::Write;
        file.write_all(json.as_bytes())
            .map_err(|_| Error::PersistenceFailed)?;
        file.sync_all().map_err(|_| Error::PersistenceFailed)?;
    }

    state::rename_replace(guard.path(), &final_path).map_err(|_| Error::PersistenceFailed)?;
    guard.disarm();
    Ok(())
}

/// Create a unique temp file with no-clobber semantics.
/// Returns the path on success; the caller is responsible for cleanup.
fn create_temp_file(gov_dir: &Path, prefix: &str) -> Result<PathBuf, Error> {
    for attempt in 0..16u32 {
        let name = format!("{}.{}.tmp", prefix, attempt);
        let candidate = gov_dir.join(&name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                drop(file);
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(Error::PersistenceFailed),
        }
    }
    Err(Error::PersistenceFailed)
}

// ============================================================================
// Cleanup (Section 16 - fixed order)
// ============================================================================

fn cleanup_phase_scoped_files(gov_dir: &Path, archived: &ArchivedGovernance) -> Result<(), Error> {
    for filename in &AUDIT_LEDGER_CLEANUP_ORDER {
        let path = gov_dir.join(filename);
        // Use symlink_metadata to detect symlinks/junctions (§16 safe topology).
        // path.exists() follows symlinks and treats dangling symlinks as absent.
        let meta = std::fs::symlink_metadata(&path);
        match meta {
            Ok(m) => {
                // Reject non-regular-file objects: symlinks, directories, devices, etc.
                if !m.file_type().is_file() {
                    return Err(Error::CloseoutArchiveMismatch);
                }
                // Verify exact bytes match archived content
                let bytes = std::fs::read(&path).map_err(|_| Error::CloseoutArchiveMismatch)?;
                let content =
                    String::from_utf8(bytes.clone()).map_err(|_| Error::CloseoutArchiveMismatch)?;
                let hash = sha256_hex(&bytes);

                let expected_content = match *filename {
                    "contract-draft.json" => &archived.contract_draft_content,
                    "accepted-contract.json" => &archived.accepted_contract_content,
                    "implementation-authority.json" => &archived.implementation_authority_content,
                    "audit-ledger.json" => &archived.audit_ledger_content,
                    _ => return Err(Error::CloseoutArchiveMismatch),
                };
                let expected_hash = match *filename {
                    "contract-draft.json" => &archived.contract_draft_sha256,
                    "accepted-contract.json" => &archived.accepted_contract_sha256,
                    "implementation-authority.json" => &archived.implementation_authority_sha256,
                    "audit-ledger.json" => &archived.audit_ledger_sha256,
                    _ => return Err(Error::CloseoutArchiveMismatch),
                };

                if content != *expected_content || hash != *expected_hash {
                    return Err(Error::CloseoutArchiveMismatch);
                }

                // Check topology safety
                validate_governance_file_safety(gov_dir, filename)?;

                // Remove
                std::fs::remove_file(&path).map_err(|_| Error::CloseoutArchiveMismatch)?;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File absent — tolerate on replay
            }
            Err(_) => {
                // Permission denied, I/O error, or other metadata failure — reject
                return Err(Error::CloseoutArchiveMismatch);
            }
        }
    }
    Ok(())
}

// ============================================================================
// Main Entry Point
// ============================================================================

pub fn cmd_phase_close(repo_arg: &str, phase_id: &str) -> Result<String, Error> {
    // 1. CLI token grammar (Section 5)
    validate_phase_id(phase_id)?;

    // 2. Canonical repository path (Section 4)
    let repo_path = std::path::Path::new(repo_arg);
    crate::path::assert_existing_dir(repo_path)?;
    let repo = std::fs::canonicalize(repo_path).map_err(|_| Error::RepositoryInvalid)?;

    // 3. Safe .mrgs directory (Section 4)
    let gov_dir = crate::path::validate_gov_dir_exists(&repo)?;

    // 4. Accepted plan record (Section 4)
    let accepted = state::read_accepted_plan(&repo)?;
    state::validate_accepted_plan_record(&accepted)?;

    let plan_file = crate::path::resolve_safe_plan_path(&repo, &accepted.plan_path)?;
    let plan_bytes = std::fs::read(&plan_file)?;
    let plan_sha256 = sha256_hex(&plan_bytes);
    let plan_str = String::from_utf8(plan_bytes)?;
    let plan: crate::plan::Plan = toml::from_str(&plan_str)?;
    plan.validate()?;

    state::validate_plan_consistency(&accepted, &plan, &plan_sha256)?;

    // 5. State structure (Section 4)
    let mut gov_state = state::read_state(&repo)?;
    state::validate_state_record(&gov_state, &accepted, &plan)?;

    // 6. Requested phase existence (Section 4)
    if !plan.phases.iter().any(|p| p.id == phase_id) {
        return Err(Error::UnknownPhase(phase_id.to_string()));
    }

    // 7. Completion ledger topology and validation (Section 4)
    let completion_ledger = read_completion_ledger(&gov_dir)?;
    if let Some(ref ledger) = completion_ledger {
        validate_completion_ledger(ledger, &accepted.sha256, &plan.plan_id)?;
    }

    // 8. State-to-ledger relation (Section 14)
    validate_state_ledger_relation(&gov_state, completion_ledger.as_ref())?;

    // 9. Lifecycle detection
    // ARCHIVED_PENDING_FINALIZATION: ledger has entry AND state.active_phase == requested
    // CLOSED: ledger has entry AND state.active_phase != requested
    // OPEN: no entry
    let ledger_has_entry = completion_ledger
        .as_ref()
        .map(|l| {
            l.completions
                .iter()
                .any(|e| e.completion_receipt.phase_id == phase_id)
        })
        .unwrap_or(false);

    if ledger_has_entry {
        let state_has_active = gov_state.active_phase.as_deref() == Some(phase_id);
        if state_has_active {
            // Section 16: Resumable finalization
            return resumable_finalization(
                &repo,
                &gov_dir,
                &mut gov_state,
                completion_ledger.as_ref().unwrap(),
                phase_id,
            );
        } else {
            // Section 17: Completed idempotency
            return completed_idempotent(
                &repo,
                &gov_dir,
                &gov_state,
                completion_ledger.as_ref().unwrap(),
                phase_id,
            );
        }
    }

    // OPEN: first closeout
    // Validate all closeout readiness requirements
    let plan_phase_index = plan
        .phases
        .iter()
        .position(|p| p.id == phase_id)
        .ok_or(Error::CloseoutConflict)?;
    let plan_phase_title = plan
        .phases
        .iter()
        .find(|p| p.id == phase_id)
        .map(|p| p.title.clone())
        .ok_or(Error::CloseoutConflict)?;
    let plan_phase_dependencies = plan
        .phases
        .iter()
        .find(|p| p.id == phase_id)
        .map(|p| p.depends_on.clone())
        .unwrap_or_default();

    let ctx = CloseoutContext {
        repo: repo.clone(),
        gov_dir: gov_dir.clone(),
        accepted_plan_sha256: accepted.sha256.clone(),
        plan_id: plan.plan_id.clone(),
        plan_phase_index,
        plan_phase_title,
        plan_phase_dependencies,
        state: gov_state.clone(),
        phase_id: phase_id.to_string(),
    };

    let auth = validate_closeout_readiness(&ctx)?;

    // 10-11. Read and archive phase-scoped governance files
    let mut archived_contents = std::collections::HashMap::new();
    for filename in &PHASE_SCOPED_FILES {
        validate_governance_file_safety(&gov_dir, filename)?;
        let (content, hash) = read_governance_file_bytes(&gov_dir, filename)?;
        // Verify parsed record revalidates
        match *filename {
            "contract-draft.json" => {
                let _: state::ContractDraft =
                    serde_json::from_str(&content).map_err(|_| Error::CloseoutArchiveMismatch)?;
            }
            "accepted-contract.json" => {
                let _: state::AcceptedContractLedger =
                    serde_json::from_str(&content).map_err(|_| Error::CloseoutArchiveMismatch)?;
            }
            "implementation-authority.json" => {
                let _: state::ImplementationAuthority =
                    serde_json::from_str(&content).map_err(|_| Error::CloseoutArchiveMismatch)?;
            }
            "audit-ledger.json" => {
                let _: AuditLedger =
                    serde_json::from_str(&content).map_err(|_| Error::CloseoutArchiveMismatch)?;
            }
            _ => {}
        }
        archived_contents.insert(filename.to_string(), (content, hash));
    }

    let archived = ArchivedGovernance {
        contract_draft_sha256: archived_contents["contract-draft.json"].1.clone(),
        contract_draft_content: archived_contents["contract-draft.json"].0.clone(),
        accepted_contract_sha256: archived_contents["accepted-contract.json"].1.clone(),
        accepted_contract_content: archived_contents["accepted-contract.json"].0.clone(),
        implementation_authority_sha256: archived_contents["implementation-authority.json"]
            .1
            .clone(),
        implementation_authority_content: archived_contents["implementation-authority.json"]
            .0
            .clone(),
        audit_ledger_sha256: archived_contents["audit-ledger.json"].1.clone(),
        audit_ledger_content: archived_contents["audit-ledger.json"].0.clone(),
    };

    // Read implementation authority for manifest construction
    let impl_auth: state::ImplementationAuthority = {
        let bytes = std::fs::read(gov_dir.join("implementation-authority.json"))
            .map_err(|_| Error::ImplementationAuthorityInvalid)?;
        serde_json::from_slice(&bytes).map_err(|_| Error::ImplementationAuthorityInvalid)?
    };

    let audit_ledger = audit::read_audit_ledger(&gov_dir)?.ok_or(Error::CloseoutNotReady)?;

    // Compute sequence and chain from existing completions
    let (sequence, prev_receipt_hash) = match completion_ledger.as_ref() {
        Some(existing) if !existing.completions.is_empty() => {
            let last = existing.completions.last().unwrap();
            (
                last.completion_receipt.completion_sequence + 1,
                Some(last.completion_receipt_sha256.clone()),
            )
        }
        _ => (1, None),
    };

    // 10. Construct final manifest
    let manifest = build_final_manifest(
        &ctx,
        &auth,
        &audit_ledger,
        &impl_auth,
        archived.clone(),
        sequence,
    )?;
    let manifest_hash = compute_manifest_hash(&manifest)?;

    // 11. Construct completion receipt
    let receipt = build_completion_receipt(&ctx, &manifest_hash, prev_receipt_hash, sequence)?;
    let receipt_hash = compute_receipt_hash(&receipt)?;

    // Create completion entry
    let entry = CompletionEntry {
        final_manifest: manifest,
        final_manifest_sha256: manifest_hash.clone(),
        completion_receipt: receipt,
        completion_receipt_sha256: receipt_hash.clone(),
    };

    // Build prospective ledger: append to existing completions or start fresh
    let mut completions: Vec<CompletionEntry> = completion_ledger
        .as_ref()
        .map(|l| l.completions.clone())
        .unwrap_or_default();
    completions.push(entry);

    let new_ledger = CompletionLedger {
        schema_version: 1,
        accepted_plan_sha256: accepted.sha256.clone(),
        plan_id: plan.plan_id.clone(),
        completions,
    };

    // Validate prospective ledger
    validate_completion_ledger(&new_ledger, &accepted.sha256, &plan.plan_id)?;

    // 12. Atomic completion-ledger publication
    atomic_publish_completion_ledger(&gov_dir, &new_ledger)?;

    // 13-14. Resumable finalization
    resumable_finalization(&repo, &gov_dir, &mut gov_state, &new_ledger, phase_id)
}

// ============================================================================
// Completed Idempotency (Section 17)
// ============================================================================

fn completed_idempotent(
    _repo: &Path,
    _gov_dir: &Path,
    state: &GovernanceState,
    ledger: &CompletionLedger,
    phase_id: &str,
) -> Result<String, Error> {
    // 1. state.active_phase must be null
    if state.active_phase.is_some() {
        return Err(Error::CloseoutConflict);
    }

    // 2. requested phase is final closed phase and final completion entry
    let last_entry = ledger
        .completions
        .last()
        .ok_or(Error::CloseoutLedgerInvalid)?;
    if last_entry.completion_receipt.phase_id != phase_id {
        return Err(Error::CloseoutConflict);
    }
    if state.closed_phases.last().map(|s| s.as_str()) != Some(phase_id) {
        return Err(Error::CloseoutConflict);
    }

    // 3. all four phase-scoped governance files absent
    for filename in &PHASE_SCOPED_FILES {
        let path = _gov_dir.join(filename);
        if path.exists() {
            return Err(Error::CloseoutConflict);
        }
    }

    // 4. recompute and validate every manifest, receipt, and chain hash
    validate_completion_ledger(ledger, &ledger.accepted_plan_sha256, &ledger.plan_id)?;

    // 5. return exact original output
    Ok(format!(
        "PHASE_CLOSED {} {} {} {}",
        phase_id,
        last_entry.completion_receipt.completion_sequence,
        last_entry.final_manifest_sha256,
        last_entry.completion_receipt_sha256,
    ))
}

// ============================================================================
// Resumable Finalization (Section 16)
// ============================================================================

fn resumable_finalization(
    _repo: &Path,
    gov_dir: &Path,
    state: &mut GovernanceState,
    ledger: &CompletionLedger,
    phase_id: &str,
) -> Result<String, Error> {
    // 1. load final completion entry
    let entry = ledger
        .completions
        .last()
        .ok_or(Error::CloseoutLedgerInvalid)?;

    // 2. revalidate complete ledger
    validate_completion_ledger(ledger, &ledger.accepted_plan_sha256, &ledger.plan_id)?;

    // 3. verify requested phase, manifest, receipt, and state relation
    if entry.completion_receipt.phase_id != phase_id {
        return Err(Error::CloseoutConflict);
    }

    // 4-6. remove phase-scoped files that still exist (with exact byte verification)
    let archived = &entry.final_manifest.archived_governance;
    cleanup_phase_scoped_files(gov_dir, archived)?;

    // 7-8. verify all four files absent after cleanup
    for filename in &PHASE_SCOPED_FILES {
        let path = gov_dir.join(filename);
        if path.exists() {
            return Err(Error::CloseoutArchiveMismatch);
        }
    }

    // 9. construct new state
    let mut new_state = state.clone();
    new_state.active_phase = None;
    if !new_state.closed_phases.contains(&phase_id.to_string()) {
        new_state.closed_phases.push(phase_id.to_string());
    }

    // 10. validate prospective state against accepted plan
    {
        let accepted = state::read_accepted_plan(_repo)?;
        let plan_file = crate::path::resolve_safe_plan_path(_repo, &accepted.plan_path)?;
        let plan_bytes = std::fs::read(&plan_file)?;
        let plan_str = String::from_utf8(plan_bytes)?;
        let plan: crate::plan::Plan = toml::from_str(&plan_str)?;
        state::validate_state_record(&new_state, &accepted, &plan)?;
    }

    // 11. atomic state replacement
    atomic_replace_state(gov_dir, &new_state)?;

    // 12. re-read and validate completion ledger and state relation
    *state = new_state.clone();
    let reloaded_state = state::read_state(_repo)?;
    let reloaded_ledger = read_completion_ledger(gov_dir)?.ok_or(Error::CloseoutLedgerInvalid)?;
    validate_state_ledger_relation(&reloaded_state, Some(&reloaded_ledger))?;

    // 13. verify all four files remain absent
    for filename in &PHASE_SCOPED_FILES {
        let path = gov_dir.join(filename);
        if path.exists() {
            return Err(Error::CloseoutArchiveMismatch);
        }
    }

    // 14. success output
    Ok(format!(
        "PHASE_CLOSED {} {} {} {}",
        phase_id,
        entry.completion_receipt.completion_sequence,
        entry.final_manifest_sha256,
        entry.completion_receipt_sha256,
    ))
}
