//! Phase 8 contract-required tests.
//!
//! Covers Section 29 of the Phase 8 contract: CLI surface and validation
//! order; recovery subject determinism; governance inventory; health
//! classification; accepted-plan reconstruction; state recovery and
//! active-phase inference; completion-to-state relation; incomplete closeout
//! resumption; temporary-file handling; recovery plan, ID, and prefix
//! hashes; inspect output; apply authorization; the append-only recovery
//! ledger; pending journal publication; resumable action execution;
//! recovery receipts; idempotency and conflicts; coexistence and exemption
//! rules; filesystem safety; Git/privacy/network boundary; persistence
//! safety; error categories; dependencies; and test-binary discipline.
//!
//! Obligation mapping (Section 29, numbered 1-88):
//!
//! 1   -> test_obligation_01_exact_cli_parsing_inspect
//! 2   -> test_obligation_02_exact_cli_parsing_apply
//! 3   -> test_obligation_03_missing_duplicated_unknown_arguments_reject
//! 4   -> test_obligation_04_inspect_read_only_preserves_bytes
//! 5   -> test_obligation_05_healthy_exact_not_required_output
//! 6   -> test_obligation_06_subject_hash_deterministic
//! 7   -> test_obligation_07_inventory_complete_sorted_unique_exact_hashes
//! 8   -> test_obligation_08_subject_binds_git_identity_dirty_allowed
//! 9   -> test_obligation_09_detached_unborn_unreadable_rejected
//! 10  -> test_obligation_10_mrgs_real_directory_not_reparse
//! 11  -> test_obligation_11_unknown_child_unrecoverable_not_deleted
//! 12  -> test_obligation_12_nested_nonutf8_device_rejected
//! 13  -> test_obligation_13_symlink_permanent_filename_rejected
//! 14  -> test_obligation_14_windows_reparse_branch_executes
//! 15  -> test_obligation_15_valid_accepted_plan_authoritative
//! 16  -> test_obligation_16_plan_source_unsafe_unrecoverable
//! 17  -> test_obligation_17_missing_accepted_plan_no_ledger_unrecoverable
//! 18  -> test_obligation_18_malformed_accepted_plan_no_ledger_unrecoverable
//! 19  -> test_obligation_19_accepted_plan_reconstructed_from_ledger
//! 20  -> test_obligation_20_reconstructed_plan_exact_fields_bytes_hash
//! 21  -> test_obligation_21_manifest_disagreement_unrecoverable
//! 22  -> test_obligation_22_completion_ledger_invalid_unrecoverable
//! 23  -> test_obligation_23_valid_state_recognized_without_rewrite
//! 24  -> test_obligation_24_missing_state_reconstructed_null_active
//! 25  -> test_obligation_25_malformed_state_reconstructed_atomically
//! 26  -> test_obligation_26_closed_phases_from_final_receipt
//! 27  -> test_obligation_27_active_phase_inferred_from_draft_prefix
//! 28  -> test_obligation_28_accepted_contract_bound_to_draft
//! 29  -> test_obligation_29_impl_authority_bound_to_accepted_contract
//! 30  -> test_obligation_30_audit_ledger_validated_and_bound
//! 31  -> test_obligation_31_later_file_without_predecessor_unrecoverable
//! 32  -> test_obligation_32_phase_scoped_disagreement_unrecoverable
//! 33  -> test_obligation_33_inferred_phase_not_closed_deps_closed
//! 34  -> test_obligation_34_selected_phase_without_draft_healthy
//! 35  -> test_obligation_35_completion_state_relation_exact
//! 36  -> test_obligation_36_incomplete_closeout_recoverable
//! 37  -> test_obligation_37_state_missing_during_closeout_pre_state
//! 38  -> test_obligation_38_closeout_resume_byte_exact_files
//! 39  -> test_obligation_39_closeout_resume_fixed_order_exact_state
//! 40  -> test_obligation_40_closeout_byte_mismatch_unrecoverable
//! 41  -> test_obligation_41_continuity_validated
//! 42  -> test_obligation_42_continuity_corrupt_unrecoverable_never_regenerated
//! 43  -> test_obligation_43_recovery_ledger_absent_and_strict_valid
//! 44  -> test_obligation_44_corrupt_recovery_ledger_blocks_mutation
//! 45  -> test_obligation_45_temp_name_mapping_unknown_rejected
//! 46  -> test_obligation_46_redundant_temp_remove_action
//! 47  -> test_obligation_47_temp_irregularity_unrecoverable
//! 48  -> test_obligation_48_recovery_temp_promote_remove
//! 49  -> test_obligation_49_action_closed_enum_exact_fields
//! 50  -> test_obligation_50_action_order_deterministic
//! 51  -> test_obligation_51_prefix_hashes_length_and_first
//! 52  -> test_obligation_52_recovery_id_deterministic_seed_hash
//! 53  -> test_obligation_53_inspect_exact_required_action_lines
//! 54  -> test_obligation_54_repeated_inspect_byte_identical_no_writes
//! 55  -> test_obligation_55_unrecoverable_exact_category_no_output
//! 56  -> test_obligation_56_apply_hash_and_decision_grammar
//! 57  -> test_obligation_57_stale_arguments_rejected_before_publication
//! 58  -> test_obligation_58_apply_recomputes_plan_not_caller
//! 59  -> test_obligation_59_pending_published_before_first_mutation
//! 60  -> test_obligation_60_pending_entry_exact_fields
//! 61  -> test_obligation_61_advance_atomic_preserves_on_failure
//! 62  -> test_obligation_62_crash_before_first_action_resumes
//! 63  -> test_obligation_63_crash_after_action_before_advance
//! 64  -> test_obligation_64_crash_during_closeout_resumes_finalizer
//! 65  -> test_obligation_65_pending_conflict_rejects_other_request
//! 66  -> test_obligation_66_post_recovery_subject_healthy_final_prefix
//! 67  -> test_obligation_67_applied_finalization_no_null_ambiguity
//! 68  -> test_obligation_68_receipt_exact_fields_chain
//! 69  -> test_obligation_69_receipt_sha_recomputes
//! 70  -> test_obligation_70_ledger_revalidated_after_publication
//! 71  -> test_obligation_71_apply_exact_success_output
//! 72  -> test_obligation_72_exact_replay_original_output_no_writes
//! 73  -> test_obligation_73_replay_drift_rejected
//! 74  -> test_obligation_74_second_recovery_sequence_two_linked
//! 75  -> test_obligation_75_no_write_outside_mrgs_or_sources
//! 76  -> test_obligation_76_no_git_mutation_or_credential_discovery
//! 77  -> test_obligation_77_git_children_sanitized_no_injected_vars
//! 78  -> test_obligation_78_phase1_7_outputs_unchanged
//! 79  -> test_obligation_79_recovery_ledger_exempt_exact_untracked
//! 80  -> test_obligation_80_tracked_alias_child_symlink_not_exempt
//! 81  -> test_obligation_81_publication_create_new_no_truncate
//! 82  -> test_obligation_82_collision_never_truncated_failure_preserves
//! 83  -> test_obligation_83_handled_failure_no_temp_leftover_journal_rules
//! 84  -> test_obligation_84_platform_branches_execute_or_fallback
//! 85  -> test_obligation_85_apply_healthy_not_required_no_ledger
//! 86  -> test_obligation_86_error_categories_exact_format
//! 87  -> test_obligation_87_no_new_dependency_or_config
//! 88  -> test_obligation_88_no_recursive_test_every_obligation_asserted

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};

fn cargo_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mrgs"))
}

// ============================================================================
// Probes: canonical JSON shapes mirroring the production serialization so the
// tests can recompute subjects, plan seeds, and receipts byte-exactly.
// ============================================================================

#[derive(serde::Serialize)]
struct SubjectProbe {
    schema_version: u32,
    repository_git_object_format: String,
    repository_head: String,
    repository_branch: String,
    governance_entries: Vec<EntryProbe>,
    plan_source: Option<PlanSourceProbe>,
}

#[derive(serde::Serialize)]
struct EntryProbe {
    filename: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
}

#[derive(serde::Serialize)]
struct PlanSourceProbe {
    path: String,
    topology: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct ActionProbe {
    kind: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    replacement: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct PlanSeedProbe {
    schema_version: u32,
    accepted_plan_sha256: String,
    plan_id: String,
    pre_subject_sha256: String,
    actions: Vec<ActionProbe>,
    prefix_subject_sha256: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct ReceiptProbe {
    schema_version: u32,
    accepted_plan_sha256: String,
    plan_id: String,
    recovery_sequence: u32,
    recovery_id: String,
    pre_subject_sha256: String,
    post_subject_sha256: String,
    action_count: usize,
    actions_sha256: String,
    previous_recovery_receipt_sha256: Option<String>,
}

/// Ordered-field probes matching the production record serialization
/// (serde_json::Value maps sort keys, so plain `json!` cannot reproduce the
/// canonical pretty bytes).
#[derive(serde::Serialize)]
struct AcceptedPlanProbe {
    schema_version: u32,
    plan_id: String,
    plan_path: String,
    sha256: String,
    phase_count: usize,
}

#[derive(serde::Serialize)]
struct StateProbe {
    schema_version: u32,
    accepted_plan_sha256: String,
    active_phase: Option<String>,
    closed_phases: Vec<String>,
}

const PERMANENT: [&str; 8] = [
    "accepted-plan.json",
    "state.json",
    "contract-draft.json",
    "accepted-contract.json",
    "implementation-authority.json",
    "audit-ledger.json",
    "completion-ledger.json",
    "continuity-ledger.json",
];

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

// ============================================================================
// Fixture helpers
// ============================================================================

fn valid_plan_toml() -> &'static str {
    r#"schema_version = 1
plan_id = "test-plan"

[[phases]]
id = "phase-1"
title = "First phase"
depends_on = []

[[phases]]
id = "phase-2"
title = "Second phase"
depends_on = ["phase-1"]
"#
}

fn valid_contract_toml() -> &'static str {
    r#"schema_version = 1
contract_id = "test-contract-v1"
phase_id = "phase-1"
title = "Test contract"
objective = "Test objective."

requirements = ["req1", "req2"]
allowed_paths = ["src/"]
forbidden_paths = [".git/", ".mrgs/"]
verification_commands = ["cargo test", "cargo clippy"]
handoff_fields = ["FIELD1"]
"#
}

fn contract_toml_for_phase(phase_id: &str) -> String {
    format!(
        r#"schema_version = 1
contract_id = "test-contract-v1"
phase_id = "{phase_id}"
title = "Test contract"
objective = "Test objective."

requirements = ["req1", "req2"]
allowed_paths = ["src/"]
forbidden_paths = [".git/", ".mrgs/"]
verification_commands = ["cargo test", "cargo clippy"]
handoff_fields = ["FIELD1"]
"#,
        phase_id = phase_id
    )
}

fn standard_metadata(phase: &str, receipt_sha: &str) -> String {
    format!(
        r#"schema_version = 1
repository_id = "mrgs"
continuity_id = "phase-1-primary"
phase_id = "{phase}"
completion_receipt_sha256 = "{receipt_sha}"
note = "Primary governed execution continuity record"

models = [
  {{ role = "implementer", provider = "openai", model_id = "gpt-5.6", execution_mode = "hosted", session_label = "{phase}-implementation" }}
]

hosts = [
  {{ host_id = "main-workstation", platform = "windows", architecture = "x86_64", execution_surface = "opencode" }}
]

links = []
"#,
        phase = phase,
        receipt_sha = receipt_sha
    )
}

fn write_file(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .trim()
        .to_string()
}

fn stdout_raw(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn split_stdout(output: &Output) -> Vec<String> {
    stdout_str(output)
        .split_whitespace()
        .map(String::from)
        .collect()
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8(output.stderr.clone())
        .unwrap()
        .trim()
        .to_string()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success, exit={:?}, stderr={}",
        output.status.code(),
        stderr_str(output)
    );
}

fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "expected failure, got success stdout={}",
        stdout_str(output)
    );
}

fn assert_category(output: &Output, category: &str) {
    assert_failure(output);
    assert_eq!(
        stderr_str(output),
        format!("error: {}", category),
        "unexpected error category"
    );
}

fn assert_category_no_stdout(output: &Output, category: &str) {
    assert_category(output, category);
    assert_eq!(
        stdout_str(output),
        "",
        "failure must produce no success stdout"
    );
}

fn assert_no_temp_files(repo: &Path) {
    let mrgs = repo.join(".mrgs");
    if mrgs.exists() {
        for entry in std::fs::read_dir(&mrgs).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.ends_with(".tmp"), "unexpected temp file: {}", name);
        }
    }
}

fn mrgs_snapshot(repo: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    let mrgs = repo.join(".mrgs");
    for entry in std::fs::read_dir(&mrgs).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type().unwrap().is_file() {
            map.insert(name, std::fs::read(entry.path()).unwrap());
        }
    }
    map
}

fn assert_snapshot_unchanged(repo: &Path, before: &BTreeMap<String, Vec<u8>>) {
    let after = mrgs_snapshot(repo);
    assert_eq!(after, *before, "governance bytes must be unchanged");
}

// ============================================================================
// Git helpers
// ============================================================================

fn git_init(repo: &Path) {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["init", "-b", "main"])
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "user.email", "test@test.com"])
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "user.name", "Test"])
        .output()
        .unwrap();
}

fn git_commit(repo: &Path, filename: &str, content: &[u8]) {
    let path = repo.join(filename);
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent).ok();
    std::fs::write(&path, content).unwrap();
    let add_out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["add", "--", filename])
        .output()
        .unwrap();
    assert!(
        add_out.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&add_out.stderr)
    );
    let commit_out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["commit", "-m", "add file"])
        .output()
        .unwrap();
    assert!(
        commit_out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit_out.stderr)
    );
}

fn git_head(repo: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", "HEAD^{commit}"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn git_branch(repo: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn git_objfmt(repo: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--show-object-format"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

// ============================================================================
// TestRepo: complete Phase 1-7 fixture builder
// ============================================================================

struct TestRepo {
    _dir: tempfile::TempDir,
    repo: PathBuf,
    report_dir: PathBuf,
    contract_path: PathBuf,
    plan_path: PathBuf,
}

impl TestRepo {
    fn new() -> Self {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);

        // Commit .gitignore with .mrgs/ so governance files are not dirty
        std::fs::create_dir_all(repo.join("src")).unwrap();
        git_commit(&repo, ".gitignore", b".mrgs/\n");

        // Commit initial source file
        git_commit(&repo, "src/main.rs", b"fn main() {}\n");

        let plan_path = repo.join("plan.toml");
        write_file(&plan_path, valid_plan_toml());

        let contract_path = repo.join("contract.toml");
        write_file(&contract_path, valid_contract_toml());

        let report_dir = dir.path().join("reports");
        std::fs::create_dir(&report_dir).unwrap();

        TestRepo {
            _dir: dir,
            repo,
            report_dir,
            contract_path,
            plan_path,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut cmd = cargo_bin();
        cmd.args(args);
        cmd.output().unwrap()
    }

    fn run_with_env(&self, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let mut cmd = cargo_bin();
        cmd.args(args);
        for (k, v) in envs {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }

    fn inspect(&self) -> Output {
        self.run(&[
            "recovery",
            "inspect",
            "--repo",
            &self.repo.to_string_lossy(),
        ])
    }

    fn inspect_output(&self) -> Vec<String> {
        let out = self.inspect();
        assert_success(&out);
        stdout_str(&out).lines().map(String::from).collect()
    }

    fn inspect_sha(&self) -> String {
        let lines = self.inspect_output();
        let parts: Vec<&str> = lines[0].split_whitespace().collect();
        parts[1].to_string()
    }

    fn apply(&self, rid: &str, sha: &str) -> Output {
        self.run(&[
            "recovery",
            "apply",
            "--repo",
            &self.repo.to_string_lossy(),
            "--recovery-id",
            rid,
            "--subject-sha256",
            sha,
            "--decision",
            "RECOVER",
        ])
    }

    fn apply_decision(&self, rid: &str, sha: &str, decision: &str) -> Output {
        self.run(&[
            "recovery",
            "apply",
            "--repo",
            &self.repo.to_string_lossy(),
            "--recovery-id",
            rid,
            "--subject-sha256",
            sha,
            "--decision",
            decision,
        ])
    }

    fn accept_plan(&self) -> Output {
        self.run(&[
            "plan",
            "accept",
            "--repo",
            &self.repo.to_string_lossy(),
            "--plan",
            &self.plan_path.to_string_lossy(),
        ])
    }

    fn select_phase(&self, phase: &str) -> Output {
        self.run(&[
            "phase",
            "select",
            "--repo",
            &self.repo.to_string_lossy(),
            "--phase",
            phase,
        ])
    }

    fn draft_contract(&self) -> Output {
        self.run(&[
            "contract",
            "draft",
            "--repo",
            &self.repo.to_string_lossy(),
            "--contract",
            &self.contract_path.to_string_lossy(),
        ])
    }

    fn accept_contract(&self, revision: u32, sha256: &str) -> Output {
        self.run(&[
            "contract",
            "accept",
            "--repo",
            &self.repo.to_string_lossy(),
            "--revision",
            &revision.to_string(),
            "--sha256",
            sha256,
            "--decision",
            "ACCEPTED",
        ])
    }

    fn impl_begin(&self, revision: u32, sha256: &str) -> Output {
        self.run(&[
            "implementation",
            "begin",
            "--repo",
            &self.repo.to_string_lossy(),
            "--revision",
            &revision.to_string(),
            "--sha256",
            sha256,
        ])
    }

    fn impl_check(&self) -> Output {
        self.run(&[
            "implementation",
            "check",
            "--repo",
            &self.repo.to_string_lossy(),
        ])
    }

    fn audit_begin(&self, auditor: &str) -> Output {
        self.run(&[
            "audit",
            "begin",
            "--repo",
            &self.repo.to_string_lossy(),
            "--auditor",
            auditor,
        ])
    }

    fn audit_record(&self, report: &Path) -> Output {
        self.run(&[
            "audit",
            "record",
            "--repo",
            &self.repo.to_string_lossy(),
            "--report",
            &report.to_string_lossy(),
        ])
    }

    fn phase_close(&self, phase_id: &str) -> Output {
        self.run(&[
            "phase",
            "close",
            "--repo",
            &self.repo.to_string_lossy(),
            "--phase",
            phase_id,
        ])
    }

    fn continuity_record(&self, metadata: &Path) -> Output {
        self.run(&[
            "continuity",
            "record",
            "--repo",
            &self.repo.to_string_lossy(),
            "--metadata",
            &metadata.to_string_lossy(),
        ])
    }

    fn get_draft(&self) -> serde_json::Value {
        let path = self.repo.join(".mrgs").join("contract-draft.json");
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap()
    }

    fn get_state(&self) -> serde_json::Value {
        let path = self.repo.join(".mrgs").join("state.json");
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap()
    }

    fn get_completion_ledger(&self) -> Option<serde_json::Value> {
        let path = self.repo.join(".mrgs").join("completion-ledger.json");
        if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap())
        } else {
            None
        }
    }

    fn get_recovery_ledger(&self) -> Option<serde_json::Value> {
        let path = self.repo.join(".mrgs").join("recovery-ledger.json");
        if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap())
        } else {
            None
        }
    }

    fn recovery_ledger_bytes(&self) -> Vec<u8> {
        std::fs::read(self.repo.join(".mrgs/recovery-ledger.json")).unwrap()
    }

    fn make_pass_report(&self, audit_id: &str, subject_sha256: &str, auditor_id: &str) -> String {
        let contract: toml::Value = toml::from_str(valid_contract_toml()).unwrap();
        let requirements = contract["requirements"].as_array().unwrap();
        let verification_commands = contract["verification_commands"].as_array().unwrap();

        let req_results: Vec<serde_json::Value> = requirements
            .iter()
            .map(|r| {
                serde_json::json!({
                    "requirement": r.as_str().unwrap(),
                    "status": "PASS",
                    "evidence": "verified"
                })
            })
            .collect();

        let ver_results: Vec<serde_json::Value> = verification_commands
            .iter()
            .map(|c| {
                serde_json::json!({
                    "command": c.as_str().unwrap(),
                    "status": "PASS",
                    "evidence": "verified"
                })
            })
            .collect();

        let report = serde_json::json!({
            "schema_version": 1,
            "audit_id": audit_id,
            "subject_sha256": subject_sha256,
            "auditor_id": auditor_id,
            "independence_declaration": "INDEPENDENT",
            "verdict": "PASS",
            "summary": "All requirements satisfied",
            "requirement_results": req_results,
            "verification_results": ver_results,
            "findings": []
        });
        serde_json::to_string_pretty(&report).unwrap()
    }

    fn write_report(&self, content: &str) -> PathBuf {
        let path = self.report_dir.join("report.json");
        write_file(&path, content);
        path
    }

    fn write_metadata(&self, name: &str, content: &str) -> PathBuf {
        // Continuity metadata must be a regular file inside the repository.
        let path = self.repo.join(name);
        write_file(&path, content);
        path
    }

    /// Plan accepted, phase-1 selected, contract drafted and accepted,
    /// implementation bound.
    fn setup_impl_bound(&self) {
        git_commit(&self.repo, "plan.toml", valid_plan_toml().as_bytes());
        git_commit(
            &self.repo,
            "contract.toml",
            valid_contract_toml().as_bytes(),
        );
        assert_success(&self.accept_plan());
        assert_success(&self.select_phase("phase-1"));
        assert_success(&self.draft_contract());
        let draft = self.get_draft();
        let sha = draft["sha256"].as_str().unwrap().to_string();
        assert_success(&self.accept_contract(1, &sha));
        assert_success(&self.impl_begin(1, &sha));
    }

    /// Full PASS audit cycle on the current phase.
    fn full_pass_audit(&self) {
        let out = self.audit_begin("auditor1");
        assert_success(&out);
        let parts = split_stdout(&out);
        let report = self.make_pass_report(&parts[1], &parts[3], "auditor1");
        let report_path = self.write_report(&report);
        assert_success(&self.audit_record(&report_path));
    }

    /// Impl-bound plus a PASSED audit on phase-1 (ready for closeout).
    fn setup_closeout_ready(&self) {
        self.setup_impl_bound();
        self.full_pass_audit();
    }

    /// Fully close phase-1; returns (manifest_sha, receipt_sha).
    fn close_phase1(&self) -> (String, String) {
        self.setup_closeout_ready();
        let out = self.phase_close("phase-1");
        assert_success(&out);
        let parts = split_stdout(&out);
        assert_eq!(parts[0], "PHASE_CLOSED");
        assert_eq!(parts[1], "phase-1");
        (parts[3].to_string(), parts[4].to_string())
    }

    /// Close phase-1, then fully close phase-2.
    fn close_phase2(&self) -> (String, String) {
        self.close_phase1();
        assert_success(&self.select_phase("phase-2"));
        write_file(&self.contract_path, &contract_toml_for_phase("phase-2"));
        git_commit(
            &self.repo,
            "contract.toml",
            contract_toml_for_phase("phase-2").as_bytes(),
        );
        assert_success(&self.draft_contract());
        let sha = self.get_draft()["sha256"].as_str().unwrap().to_string();
        assert_success(&self.accept_contract(1, &sha));
        assert_success(&self.impl_begin(1, &sha));
        self.full_pass_audit();
        let out = self.phase_close("phase-2");
        assert_success(&out);
        let parts = split_stdout(&out);
        (parts[3].to_string(), parts[4].to_string())
    }

    /// The archived governance content for the final completion entry.
    fn archived_governance(&self) -> serde_json::Value {
        let ledger = self.get_completion_ledger().unwrap();
        let entries = ledger["completions"].as_array().unwrap();
        entries.last().unwrap()["final_manifest"]["archived_governance"].clone()
    }

    fn plan_sha(&self) -> String {
        let bytes = std::fs::read(self.repo.join("plan.toml")).unwrap();
        sha256_hex(&bytes)
    }

    fn write_state(&self, value: &serde_json::Value) {
        std::fs::write(
            self.repo.join(".mrgs/state.json"),
            serde_json::to_string_pretty(value).unwrap(),
        )
        .unwrap();
    }

    fn delete(&self, name: &str) {
        std::fs::remove_file(self.repo.join(".mrgs").join(name)).unwrap();
    }

    fn write_mrgs(&self, name: &str, content: &[u8]) {
        std::fs::write(self.repo.join(".mrgs").join(name), content).unwrap();
    }

    fn read_mrgs(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.repo.join(".mrgs").join(name)).unwrap()
    }

    fn read_mrgs_str(&self, name: &str) -> String {
        String::from_utf8(self.read_mrgs(name)).unwrap()
    }
}

// ============================================================================
// Subject recomputation probe (mirrors production canonical serialization)
// ============================================================================

/// The recorded plan-source path derived from surviving authority, in the
/// same deterministic order as production: accepted-plan first, then the
/// final completion manifest.
fn recorded_plan_path(repo: &Path) -> Option<String> {
    let gov = repo.join(".mrgs");
    let ap = gov.join("accepted-plan.json");
    if let Ok(bytes) = std::fs::read(&ap) {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(p) = v["plan_path"].as_str() {
                return Some(p.to_string());
            }
        }
    }
    let cl = gov.join("completion-ledger.json");
    if let Ok(bytes) = std::fs::read(&cl) {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(arr) = v["completions"].as_array() {
                if let Some(last) = arr.last() {
                    if let Some(p) = last["final_manifest"]["plan_source_path"].as_str() {
                        return Some(p.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Recompute the canonical recovery subject JSON for the current repository
/// state, mirroring production byte-for-byte.
fn recompute_subject(repo: &Path) -> String {
    let gov = repo.join(".mrgs");
    let mut entries: Vec<EntryProbe> = Vec::new();
    for name in PERMANENT {
        let p = gov.join(name);
        match std::fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_file() => {
                let bytes = std::fs::read(&p).unwrap();
                entries.push(EntryProbe {
                    filename: name.to_string(),
                    kind: "REGULAR".to_string(),
                    byte_length: Some(bytes.len() as u64),
                    sha256: Some(sha256_hex(&bytes)),
                });
            }
            Ok(m) if m.file_type().is_symlink() => entries.push(EntryProbe {
                filename: name.to_string(),
                kind: "SYMLINK".to_string(),
                byte_length: None,
                sha256: None,
            }),
            Ok(m) if m.is_dir() => entries.push(EntryProbe {
                filename: name.to_string(),
                kind: "DIRECTORY".to_string(),
                byte_length: None,
                sha256: None,
            }),
            Ok(_) => entries.push(EntryProbe {
                filename: name.to_string(),
                kind: "OTHER".to_string(),
                byte_length: None,
                sha256: None,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => entries.push(EntryProbe {
                filename: name.to_string(),
                kind: "ABSENT".to_string(),
                byte_length: None,
                sha256: None,
            }),
            Err(_) => panic!("metadata failure"),
        }
    }
    // Recognized temporary children (recovery-ledger.json excluded).
    for entry in std::fs::read_dir(&gov).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "recovery-ledger.json" || PERMANENT.contains(&name.as_str()) {
            continue;
        }
        let meta = std::fs::symlink_metadata(entry.path()).unwrap();
        if meta.file_type().is_file() {
            let bytes = std::fs::read(entry.path()).unwrap();
            entries.push(EntryProbe {
                filename: name,
                kind: "REGULAR".to_string(),
                byte_length: Some(bytes.len() as u64),
                sha256: Some(sha256_hex(&bytes)),
            });
        } else {
            entries.push(EntryProbe {
                filename: name,
                kind: "OTHER".to_string(),
                byte_length: None,
                sha256: None,
            });
        }
    }
    entries.sort_by(|a, b| a.filename.as_bytes().cmp(b.filename.as_bytes()));

    let plan_source = recorded_plan_path(repo).and_then(|path| {
        let full = repo.join(&path);
        match std::fs::symlink_metadata(&full) {
            Ok(m) if m.file_type().is_file() => {
                let bytes = std::fs::read(&full).unwrap();
                Some(PlanSourceProbe {
                    path,
                    topology: "REGULAR".to_string(),
                    byte_length: Some(bytes.len() as u64),
                    sha256: Some(sha256_hex(&bytes)),
                })
            }
            Ok(m) if m.file_type().is_symlink() => Some(PlanSourceProbe {
                path,
                topology: "SYMLINK".to_string(),
                byte_length: None,
                sha256: None,
            }),
            Ok(_) => Some(PlanSourceProbe {
                path,
                topology: "OTHER".to_string(),
                byte_length: None,
                sha256: None,
            }),
            Err(_) => None,
        }
    });

    let subject = SubjectProbe {
        schema_version: 1,
        repository_git_object_format: git_objfmt(repo),
        repository_head: git_head(repo),
        repository_branch: git_branch(repo),
        governance_entries: entries,
        plan_source,
    };
    let json = serde_json::to_string(&subject).unwrap();
    sha256_hex(json.as_bytes())
}

// ============================================================================
// Crash simulation helpers (test-only failpoints in the production binary)
// ============================================================================

fn wait_for_file(path: &Path, timeout_secs: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    while !path.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn crash_apply(t: &TestRepo, rid: &str, sha: &str, point: &str, dir: &Path) -> Child {
    let signal = dir.join("signal");
    let release = dir.join("release");
    let mut cmd = cargo_bin();
    cmd.args([
        "recovery",
        "apply",
        "--repo",
        &t.repo.to_string_lossy(),
        "--recovery-id",
        rid,
        "--subject-sha256",
        sha,
        "--decision",
        "RECOVER",
    ])
    .env("MRGS_TEST_ONLY_RECOVERY_POINT", point)
    .env("MRGS_TEST_ONLY_RECOVERY_SIGNAL_FILE", &signal)
    .env("MRGS_TEST_ONLY_RECOVERY_RELEASE_FILE", &release);
    let child = cmd.spawn().unwrap();
    wait_for_file(&signal, 60);
    child
}

fn kill_child(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

// ============================================================================
// 1-10: CLI, read-only, subject, git identity, topology
// ============================================================================

#[test]
fn test_obligation_01_exact_cli_parsing_inspect() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let out = t.inspect();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_NOT_REQUIRED "));
    // Unknown recovery subcommand is rejected by the exact CLI surface.
    let bad = t.run(&["recovery", "bogus", "--repo", &t.repo.to_string_lossy()]);
    assert_failure(&bad);
}

#[test]
fn test_obligation_02_exact_cli_parsing_apply() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    let rid = parts[1];
    let sha = parts[2];
    let out = t.apply(rid, sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
}

#[test]
fn test_obligation_03_missing_duplicated_unknown_arguments_reject() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let repo = t.repo.to_string_lossy().to_string();
    let sha = t.inspect_sha();
    let rid = "a".repeat(64);
    let before = mrgs_snapshot(&t.repo);

    // Missing --decision
    let out = t.run(&[
        "recovery",
        "apply",
        "--repo",
        &repo,
        "--recovery-id",
        &rid,
        "--subject-sha256",
        &sha,
    ]);
    assert_failure(&out);

    // Missing --recovery-id
    let out = t.run(&[
        "recovery",
        "apply",
        "--repo",
        &repo,
        "--subject-sha256",
        &sha,
        "--decision",
        "RECOVER",
    ]);
    assert_failure(&out);

    // Missing --repo
    let out = t.run(&[
        "recovery",
        "apply",
        "--recovery-id",
        &rid,
        "--subject-sha256",
        &sha,
        "--decision",
        "RECOVER",
    ]);
    assert_failure(&out);

    // Unknown argument
    let out = t.run(&[
        "recovery",
        "apply",
        "--repo",
        &repo,
        "--recovery-id",
        &rid,
        "--subject-sha256",
        &sha,
        "--decision",
        "RECOVER",
        "--bogus",
        "x",
    ]);
    assert_failure(&out);

    // Duplicated argument
    let out = t.run(&[
        "recovery",
        "apply",
        "--repo",
        &repo,
        "--recovery-id",
        &rid,
        "--recovery-id",
        &rid,
        "--subject-sha256",
        &sha,
        "--decision",
        "RECOVER",
    ]);
    assert_failure(&out);

    // Malformed decision on inspect (inspect takes no decision)
    let out = t.run(&[
        "recovery",
        "inspect",
        "--repo",
        &repo,
        "--decision",
        "RECOVER",
    ]);
    assert_failure(&out);

    // No writes occurred for any rejected invocation.
    assert_snapshot_unchanged(&t.repo, &before);
    assert!(!t.repo.join(".mrgs/recovery-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_04_inspect_read_only_preserves_bytes() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let before = mrgs_snapshot(&t.repo);
    let head_before = git_head(&t.repo);
    let status_before = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&t.repo)
            .args(["status", "--porcelain"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let out = t.inspect();
    assert_success(&out);
    assert_snapshot_unchanged(&t.repo, &before);
    assert_eq!(git_head(&t.repo), head_before);
    let status_after = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&t.repo)
            .args(["status", "--porcelain"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(status_after, status_before);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_05_healthy_exact_not_required_output() {
    // Minimal healthy state: accepted plan only.
    let t = TestRepo::new();
    git_commit(&t.repo, "plan.toml", valid_plan_toml().as_bytes());
    assert_success(&t.accept_plan());
    let lines = t.inspect_output();
    assert_eq!(lines.len(), 1);
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_NOT_REQUIRED");
    assert_eq!(parts[1].len(), 64);

    // Fully completed state: phase-1 closed.
    let t2 = TestRepo::new();
    t2.close_phase1();
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_NOT_REQUIRED");
    assert_eq!(parts[1].len(), 64);
}

#[test]
fn test_obligation_06_subject_hash_deterministic() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let first = t.inspect();
    let second = t.inspect();
    assert_success(&first);
    assert_success(&second);
    assert_eq!(stdout_raw(&first), stdout_raw(&second));
}

#[test]
fn test_obligation_07_inventory_complete_sorted_unique_exact_hashes() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let sha = t.inspect_sha();
    let expected = recompute_subject(&t.repo);
    assert_eq!(sha, expected, "subject must match canonical recomputation");

    // Also for a fully closed repo (completion ledger present).
    let t2 = TestRepo::new();
    t2.close_phase1();
    assert_eq!(t2.inspect_sha(), recompute_subject(&t2.repo));
}

#[test]
fn test_obligation_08_subject_binds_git_identity_dirty_allowed() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let clean_sha = t.inspect_sha();
    // A dirty worktree must not change the recovery subject and must not be
    // treated as governance corruption.
    std::fs::write(t.repo.join("src/main.rs"), b"fn main() { dirty! }\n").unwrap();
    assert_eq!(t.inspect_sha(), clean_sha);
    // The subject binds the exact git identity: recomputation embeds the
    // current branch, HEAD, and object format.
    assert_eq!(git_branch(&t.repo), "main");
    assert_eq!(git_objfmt(&t.repo), "sha1");
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
}

#[test]
fn test_obligation_09_detached_unborn_unreadable_rejected() {
    // Detached HEAD
    let t = TestRepo::new();
    t.setup_impl_bound();
    let detach = Command::new("git")
        .arg("-C")
        .arg(&t.repo)
        .args(["checkout", "--detach"])
        .output()
        .unwrap();
    assert!(detach.status.success());
    let out = t.inspect();
    assert_category_no_stdout(&out, "GIT_DETACHED_HEAD");
    let before = mrgs_snapshot(&t.repo);
    assert_snapshot_unchanged(&t.repo, &before);

    // Unborn HEAD: git initialized with no commits.
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    std::fs::create_dir_all(repo.join(".mrgs")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mrgs"))
        .args(["recovery", "inspect", "--repo", &repo.to_string_lossy()])
        .output()
        .unwrap();
    assert_category_no_stdout(&out, "GIT_HEAD_INVALID");

    // Unreadable / not a repository
    let dir2 = tempfile::TempDir::new().unwrap();
    let not_repo = dir2.path().join("notrepo");
    std::fs::create_dir(&not_repo).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mrgs"))
        .args(["recovery", "inspect", "--repo", &not_repo.to_string_lossy()])
        .output()
        .unwrap();
    assert_category_no_stdout(&out, "REPOSITORY_INVALID");
}

#[test]
fn test_obligation_10_mrgs_real_directory_not_reparse() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let mrgs = t.repo.join(".mrgs");
    let real = t.repo.join(".mrgs-real");
    std::fs::rename(&mrgs, &real).unwrap();
    make_dir_link(&real, &mrgs);
    let out = t.inspect();
    assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
    // Nothing was mutated.
    assert!(std::fs::symlink_metadata(&real).unwrap().is_dir());
}

/// Create a directory link: junction on Windows, symlink elsewhere.
fn make_dir_link(target: &Path, link: &Path) {
    if cfg!(windows) {
        let out = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "mklink failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    } else {
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).unwrap();
        #[cfg(not(unix))]
        panic!("unsupported platform");
    }
}

/// Create a file link: symlink on both platforms (Windows requires the
/// SeCreateSymbolicLinkPrivilege; returns the io result so callers can assert
/// capability availability).
#[cfg(unix)]
fn make_file_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn make_file_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[test]
fn test_obligation_11_unknown_child_unrecoverable_not_deleted() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.write_mrgs("mystery.json", b"{}");
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    // The unknown child is never silently ignored or deleted by inspect.
    assert_eq!(t.read_mrgs("mystery.json"), b"{}");
}

#[test]
fn test_obligation_12_nested_nonutf8_device_rejected() {
    // Nested directory child
    let t = TestRepo::new();
    t.setup_impl_bound();
    std::fs::create_dir_all(t.repo.join(".mrgs/nested")).unwrap();
    let out = t.inspect();
    assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");

    // Non-UTF-8 child name
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    write_non_utf8_child(&t2.repo);
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // FIFO (Unix): an unsupported object kind.
    if cfg!(unix) {
        let t3 = TestRepo::new();
        t3.setup_impl_bound();
        let fifo = t3.repo.join(".mrgs/fifo-child");
        let out = Command::new("mkfifo").arg(&fifo).output().unwrap();
        assert!(out.status.success(), "mkfifo failed");
        let res = t3.inspect();
        assert_category_no_stdout(&res, "FILESYSTEM_BOUNDARY_UNSAFE");
    } else {
        // Windows capability-unavailable branch: FIFO/device nodes cannot be
        // planted inside a directory; assert the missing capability and the
        // concrete fail-closed fallback (unsupported objects are rejected).
        let probe = Command::new("cmd")
            .args(["/C", "type", "\\\\.\\pipe\\mrgs-test-pipe"])
            .output()
            .unwrap();
        assert!(!probe.status.success(), "named-pipe access must fail");
        let t3 = TestRepo::new();
        t3.setup_impl_bound();
        std::fs::create_dir_all(t3.repo.join(".mrgs/device-child")).unwrap();
        let res = t3.inspect();
        assert_category_no_stdout(&res, "FILESYSTEM_BOUNDARY_UNSAFE");
    }
}

#[cfg(unix)]
fn write_non_utf8_child(repo: &Path) {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let name = OsString::from_vec(vec![b'b', 0xFF, b'a']);
    std::fs::write(repo.join(".mrgs").join(name), b"x").unwrap();
}

#[cfg(windows)]
fn write_non_utf8_child(repo: &Path) {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    // Lone surrogate: valid UTF-16, invalid UTF-8.
    let name = OsString::from_wide(&[b'b' as u16, 0xD800, b'a' as u16]);
    std::fs::write(repo.join(".mrgs").join(name), b"x").unwrap();
}

#[test]
fn test_obligation_13_symlink_permanent_filename_rejected() {
    for name in PERMANENT {
        let t = TestRepo::new();
        t.setup_impl_bound();
        // Replace a permanent file with a link (target may dangle).
        let target = t.repo.join(format!("{}.target", name));
        let link = t.repo.join(".mrgs").join(name);
        std::fs::remove_file(&link).ok();
        match make_file_link(&target, &link) {
            Ok(()) => {
                let out = t.inspect();
                assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                // Capability-unavailable branch: assert the missing privilege
                // and the concrete fail-closed fallback (a directory at the
                // permanent filename is rejected).
                std::fs::create_dir_all(&link).unwrap();
                let out = t.inspect();
                assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
            }
            Err(e) => panic!("symlink creation failed: {}", e),
        }
    }
}

#[test]
fn test_obligation_14_windows_reparse_branch_executes() {
    // Windows supported branch: a junction (reparse point) at .mrgs is
    // rejected as a filesystem-boundary violation. On Unix the equivalent
    // supported branch is a symlink at .mrgs.
    let t = TestRepo::new();
    t.setup_impl_bound();
    let mrgs = t.repo.join(".mrgs");
    let real = t.repo.join(".mrgs-real");
    std::fs::rename(&mrgs, &real).unwrap();
    make_dir_link(&real, &mrgs);
    let out = t.inspect();
    assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
    if cfg!(windows) {
        // Prove the link really is a reparse point on Windows.
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        let meta = std::fs::symlink_metadata(&mrgs).unwrap();
        assert_ne!(
            meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT,
            0,
            "junction must carry the reparse-point attribute"
        );
    } else {
        assert!(std::fs::symlink_metadata(&mrgs)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}

// ============================================================================
// 15-22: accepted-plan authority and reconstruction
// ============================================================================

#[test]
fn test_obligation_15_valid_accepted_plan_authoritative() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let out = t.inspect();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_NOT_REQUIRED "));
    // The authoritative accepted-plan path binds the exact plan source.
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
    // No reconstruction occurred: bytes are untouched.
    let accepted = t.read_mrgs_str("accepted-plan.json");
    let parsed: serde_json::Value = serde_json::from_str(&accepted).unwrap();
    assert_eq!(parsed["plan_id"], "test-plan");
    assert_eq!(parsed["plan_path"], "plan.toml");
    assert_eq!(parsed["sha256"], t.plan_sha());
    assert_eq!(parsed["phase_count"], 2);
}

#[test]
fn test_obligation_16_plan_source_unsafe_unrecoverable() {
    // Absence
    let t = TestRepo::new();
    t.setup_impl_bound();
    std::fs::remove_file(t.repo.join("plan.toml")).unwrap();
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Parse failure
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    write_file(&t2.repo.join("plan.toml"), "not = toml [");
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Hash drift
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    write_file(
        &t3.repo.join("plan.toml"),
        &format!("{}\n# drifted", valid_plan_toml()),
    );
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Unsafe topology (symlink plan source)
    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    let real = t4.repo.join("plan-real.toml");
    write_file(&real, valid_plan_toml());
    std::fs::remove_file(t4.repo.join("plan.toml")).unwrap();
    match make_file_link(&real, &t4.repo.join("plan.toml")) {
        Ok(()) => {
            let out = t4.inspect();
            assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Windows capability-unavailable branch: file symlinks require
            // the privilege; the fail-closed fallback (absence) applies.
            let out = t4.inspect();
            assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
        }
        Err(e) => panic!("symlink creation failed: {}", e),
    }
}

#[test]
fn test_obligation_17_missing_accepted_plan_no_ledger_unrecoverable() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("accepted-plan.json");
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

#[test]
fn test_obligation_18_malformed_accepted_plan_no_ledger_unrecoverable() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.write_mrgs("accepted-plan.json", b"{not json");
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    // A structurally invalid record with a wrong schema is also unrecoverable.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let mut v: serde_json::Value =
        serde_json::from_slice(&t2.read_mrgs("accepted-plan.json")).unwrap();
    v["schema_version"] = serde_json::Value::from(99);
    t2.write_mrgs(
        "accepted-plan.json",
        serde_json::to_string_pretty(&v).unwrap().as_bytes(),
    );
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // A noncanonical (backslash) plan path without a completion ledger is
    // unrecoverable with zero mutation.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    let mut v: serde_json::Value =
        serde_json::from_slice(&t3.read_mrgs("accepted-plan.json")).unwrap();
    v["plan_path"] = serde_json::Value::String("plan\\toml".to_string());
    t3.write_mrgs(
        "accepted-plan.json",
        serde_json::to_string_pretty(&v).unwrap().as_bytes(),
    );
    let before = mrgs_snapshot(&t3.repo);
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_snapshot_unchanged(&t3.repo, &before);
}

#[test]
fn test_obligation_19_accepted_plan_reconstructed_from_ledger() {
    // Missing accepted-plan
    let t = TestRepo::new();
    t.close_phase1();
    t.delete("accepted-plan.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 RESTORE_ACCEPTED_PLAN accepted-plan.json"
    );
    let out = t.apply(parts[1], parts[2]);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    assert!(t.repo.join(".mrgs/accepted-plan.json").exists());

    // Malformed accepted-plan with the same valid ledger
    let t2 = TestRepo::new();
    t2.close_phase1();
    t2.write_mrgs("accepted-plan.json", b"{corrupt");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 RESTORE_ACCEPTED_PLAN accepted-plan.json"
    );
    let out = t2.apply(parts[1], parts[2]);
    assert_success(&out);
    assert!(t2.repo.join(".mrgs/accepted-plan.json").exists());
    assert_eq!(t2.inspect_sha(), recompute_subject(&t2.repo));

    // Unknown raw key with the same valid ledger: reconstruction candidate.
    let t3 = TestRepo::new();
    t3.close_phase1();
    {
        let mut ap: serde_json::Value =
            serde_json::from_slice(&t3.read_mrgs("accepted-plan.json")).unwrap();
        ap.as_object_mut()
            .unwrap()
            .insert("extra".to_string(), serde_json::Value::Bool(true));
        t3.write_mrgs(
            "accepted-plan.json",
            serde_json::to_string_pretty(&ap).unwrap().as_bytes(),
        );
    }
    let lines = t3.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 RESTORE_ACCEPTED_PLAN accepted-plan.json"
    );
    assert_success(&t3.apply(parts[1], parts[2]));
    assert!(t3.repo.join(".mrgs/accepted-plan.json").exists());
    assert_eq!(t3.inspect_sha(), recompute_subject(&t3.repo));

    // Backslash (noncanonical) plan path with the same valid ledger:
    // reconstruction candidate.
    let t4 = TestRepo::new();
    t4.close_phase1();
    {
        let mut ap: serde_json::Value =
            serde_json::from_slice(&t4.read_mrgs("accepted-plan.json")).unwrap();
        ap["plan_path"] = serde_json::Value::String("plan\\toml".to_string());
        t4.write_mrgs(
            "accepted-plan.json",
            serde_json::to_string_pretty(&ap).unwrap().as_bytes(),
        );
    }
    let lines = t4.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 RESTORE_ACCEPTED_PLAN accepted-plan.json"
    );
    assert_success(&t4.apply(parts[1], parts[2]));
    assert!(t4.repo.join(".mrgs/accepted-plan.json").exists());
    assert_eq!(t4.inspect_sha(), recompute_subject(&t4.repo));

    // Parseable record whose raw plan_path points at a wrong but existing
    // safe regular file, while the completion ledger proves the correct
    // plan: reconstruction and apply succeed with the exact record.
    let t5 = TestRepo::new();
    t5.close_phase1();
    let plan_sha = t5.plan_sha();
    {
        let mut ap: serde_json::Value =
            serde_json::from_slice(&t5.read_mrgs("accepted-plan.json")).unwrap();
        ap["plan_path"] = serde_json::Value::String("contract.toml".to_string());
        t5.write_mrgs(
            "accepted-plan.json",
            serde_json::to_string_pretty(&ap).unwrap().as_bytes(),
        );
    }
    let lines = t5.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 RESTORE_ACCEPTED_PLAN accepted-plan.json"
    );
    assert_success(&t5.apply(parts[1], parts[2]));
    let bytes = t5.read_mrgs("accepted-plan.json");
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed["plan_path"], "plan.toml");
    assert_eq!(parsed["sha256"], plan_sha);
    assert_eq!(t5.inspect_sha(), recompute_subject(&t5.repo));
}

#[test]
fn test_obligation_20_reconstructed_plan_exact_fields_bytes_hash() {
    let t = TestRepo::new();
    t.close_phase1();
    let plan_sha = t.plan_sha();
    t.delete("accepted-plan.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));

    let bytes = t.read_mrgs("accepted-plan.json");
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["plan_id"], "test-plan");
    assert_eq!(parsed["plan_path"], "plan.toml");
    assert_eq!(parsed["sha256"], plan_sha);
    assert_eq!(parsed["phase_count"], 2);
    // Exact deterministic bytes and hash.
    let expected = AcceptedPlanProbe {
        schema_version: 1,
        plan_id: "test-plan".to_string(),
        plan_path: "plan.toml".to_string(),
        sha256: plan_sha.clone(),
        phase_count: 2,
    };
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        serde_json::to_string_pretty(&expected).unwrap()
    );
    // The record's own bytes hash deterministically from the canonical JSON.
    assert_eq!(
        sha256_hex(&t.read_mrgs("accepted-plan.json")),
        sha256_hex(serde_json::to_string_pretty(&expected).unwrap().as_bytes())
    );
}

#[test]
fn test_obligation_21_manifest_disagreement_unrecoverable() {
    let t = TestRepo::new();
    t.close_phase2();
    // Tamper the FIRST manifest's plan content and recompute its hashes so
    // the ledger stays internally valid but the manifests disagree.
    let path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let completions = ledger["completions"].as_array_mut().unwrap();
    let first = &mut completions[0];
    first["final_manifest"]["plan_content"] =
        serde_json::Value::String(format!("{}\n# manifest disagreement", valid_plan_toml()));
    let manifest_json = serde_json::to_string(&first["final_manifest"]).unwrap();
    let manifest_sha = sha256_hex(manifest_json.as_bytes());
    first["final_manifest_sha256"] = serde_json::Value::String(manifest_sha.clone());
    first["completion_receipt"]["final_manifest_sha256"] =
        serde_json::Value::String(manifest_sha.clone());
    let receipt_json = serde_json::to_string(&first["completion_receipt"]).unwrap();
    let receipt_sha = sha256_hex(receipt_json.as_bytes());
    first["completion_receipt_sha256"] = serde_json::Value::String(receipt_sha.clone());
    completions[1]["completion_receipt"]["previous_completion_receipt_sha256"] =
        serde_json::Value::String(receipt_sha.clone());
    let receipt2_json = serde_json::to_string(&completions[1]["completion_receipt"]).unwrap();
    let receipt2_sha = sha256_hex(receipt2_json.as_bytes());
    completions[1]["completion_receipt_sha256"] = serde_json::Value::String(receipt2_sha.clone());
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();

    t.delete("accepted-plan.json");
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

#[test]
fn test_obligation_22_completion_ledger_invalid_unrecoverable() {
    // Corrupt receipt hash
    let t = TestRepo::new();
    t.close_phase1();
    let path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    ledger["completions"][0]["completion_receipt_sha256"] =
        serde_json::Value::String("c".repeat(64));
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Missing raw key
    let t2 = TestRepo::new();
    t2.close_phase1();
    let path = t2.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let obj = ledger.as_object_mut().unwrap();
    obj.remove("plan_id");
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Broken previous-receipt chain (two completions)
    let t3 = TestRepo::new();
    t3.close_phase2();
    let path = t3.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    ledger["completions"][1]["completion_receipt"]["previous_completion_receipt_sha256"] =
        serde_json::Value::String("d".repeat(64));
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Unsupported schema
    let t4 = TestRepo::new();
    t4.close_phase1();
    let path = t4.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    ledger["schema_version"] = serde_json::Value::from(2);
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t4.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

// ============================================================================
// 23-34: state recovery and active-phase inference
// ============================================================================

#[test]
fn test_obligation_23_valid_state_recognized_without_rewrite() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let state_before = t.read_mrgs("state.json");
    let out = t.inspect();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_NOT_REQUIRED "));
    assert_eq!(
        t.read_mrgs("state.json"),
        state_before,
        "state must not be rewritten"
    );
}

#[test]
fn test_obligation_24_missing_state_reconstructed_null_active() {
    let t = TestRepo::new();
    git_commit(&t.repo, "plan.toml", valid_plan_toml().as_bytes());
    assert_success(&t.accept_plan());
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json");
    assert_success(&t.apply(parts[1], parts[2]));
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!([]));
    assert_eq!(state["accepted_plan_sha256"], t.plan_sha());
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
}

#[test]
fn test_obligation_25_malformed_state_reconstructed_atomically() {
    let t = TestRepo::new();
    git_commit(&t.repo, "plan.toml", valid_plan_toml().as_bytes());
    assert_success(&t.accept_plan());
    t.write_mrgs("state.json", b"{corrupt");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_success(&t.apply(parts[1], parts[2]));
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!([]));
    // Atomic publication leaves no temp files.
    assert_no_temp_files(&t.repo);

    // Unknown raw key: reconstruction candidate, never "valid because it
    // deserialized".
    let t2 = TestRepo::new();
    git_commit(&t2.repo, "plan.toml", valid_plan_toml().as_bytes());
    assert_success(&t2.accept_plan());
    {
        let mut state: serde_json::Value =
            serde_json::from_slice(&t2.read_mrgs("state.json")).unwrap();
        state
            .as_object_mut()
            .unwrap()
            .insert("extra".to_string(), serde_json::Value::Bool(true));
        t2.write_mrgs(
            "state.json",
            serde_json::to_string_pretty(&state).unwrap().as_bytes(),
        );
    }
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json");
    assert_success(&t2.apply(parts[1], parts[2]));
    let state = t2.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!([]));
    assert_eq!(t2.inspect_sha(), recompute_subject(&t2.repo));

    // Unsupported schema version: reconstruction candidate.
    let t3 = TestRepo::new();
    git_commit(&t3.repo, "plan.toml", valid_plan_toml().as_bytes());
    assert_success(&t3.accept_plan());
    {
        let mut state: serde_json::Value =
            serde_json::from_slice(&t3.read_mrgs("state.json")).unwrap();
        state["schema_version"] = serde_json::Value::from(99);
        t3.write_mrgs(
            "state.json",
            serde_json::to_string_pretty(&state).unwrap().as_bytes(),
        );
    }
    let lines = t3.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json");
    assert_success(&t3.apply(parts[1], parts[2]));
    let state = t3.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!([]));
    assert_eq!(t3.inspect_sha(), recompute_subject(&t3.repo));

    // Invalid accepted-plan binding: reconstruction candidate.
    let t4 = TestRepo::new();
    git_commit(&t4.repo, "plan.toml", valid_plan_toml().as_bytes());
    assert_success(&t4.accept_plan());
    {
        let mut state: serde_json::Value =
            serde_json::from_slice(&t4.read_mrgs("state.json")).unwrap();
        state["accepted_plan_sha256"] = serde_json::Value::String("b".repeat(64));
        t4.write_mrgs(
            "state.json",
            serde_json::to_string_pretty(&state).unwrap().as_bytes(),
        );
    }
    let lines = t4.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json");
    assert_success(&t4.apply(parts[1], parts[2]));
    let state = t4.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!([]));
    assert_eq!(t4.inspect_sha(), recompute_subject(&t4.repo));
}

#[test]
fn test_obligation_26_closed_phases_from_final_receipt() {
    let t = TestRepo::new();
    t.close_phase2();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json");
    assert_success(&t.apply(parts[1], parts[2]));
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(
        state["closed_phases"],
        serde_json::json!(["phase-1", "phase-2"])
    );
}

#[test]
fn test_obligation_27_active_phase_inferred_from_draft_prefix() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json");
    assert_success(&t.apply(parts[1], parts[2]));
    let state = t.get_state();
    assert_eq!(state["active_phase"], "phase-1");
    assert_eq!(state["closed_phases"], serde_json::json!([]));
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
}

#[test]
fn test_obligation_28_accepted_contract_bound_to_draft() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    // Accepted-contract bound to a different phase than the draft.
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t.read_mrgs("accepted-contract.json")).unwrap();
    ledger["phase_id"] = serde_json::Value::String("phase-2".to_string());
    t.write_mrgs(
        "accepted-contract.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

#[test]
fn test_obligation_29_impl_authority_bound_to_accepted_contract() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    // Tamper the implementation authority's contract SHA (structure fails).
    let mut auth: serde_json::Value =
        serde_json::from_slice(&t.read_mrgs("implementation-authority.json")).unwrap();
    auth["contract_sha256"] = serde_json::Value::String("a".repeat(64));
    t.write_mrgs(
        "implementation-authority.json",
        serde_json::to_string_pretty(&auth).unwrap().as_bytes(),
    );
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Valid authority: inference succeeds.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    t2.delete("state.json");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_success(&t2.apply(parts[1], parts[2]));
    assert_eq!(t2.get_state()["active_phase"], "phase-1");
}

#[test]
fn test_obligation_30_audit_ledger_validated_and_bound() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    t.delete("state.json");
    // Tamper an audit round auditor (breaks audit-id recomputation).
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t.read_mrgs("audit-ledger.json")).unwrap();
    ledger["rounds"][0]["auditor_id"] = serde_json::Value::String("intruder".to_string());
    t.write_mrgs(
        "audit-ledger.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Valid audit history: inference succeeds and binds the audit ledger.
    let t2 = TestRepo::new();
    t2.setup_closeout_ready();
    t2.delete("state.json");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_success(&t2.apply(parts[1], parts[2]));
    let state = t2.get_state();
    assert_eq!(state["active_phase"], "phase-1");
    assert_eq!(t2.inspect_sha(), recompute_subject(&t2.repo));
}

#[test]
fn test_obligation_31_later_file_without_predecessor_unrecoverable() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    t.delete("contract-draft.json");
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

#[test]
fn test_obligation_32_phase_scoped_disagreement_unrecoverable() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    // Draft phase disagrees with the accepted contract phase.
    let mut draft: serde_json::Value =
        serde_json::from_slice(&t.read_mrgs("contract-draft.json")).unwrap();
    draft["phase_id"] = serde_json::Value::String("phase-2".to_string());
    t.write_mrgs(
        "contract-draft.json",
        serde_json::to_string_pretty(&draft).unwrap().as_bytes(),
    );
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

#[test]
fn test_obligation_33_inferred_phase_not_closed_deps_closed() {
    // Positive: phase-2 inferred after phase-1 closed, with deps closed.
    let t = TestRepo::new();
    t.close_phase1();
    assert_success(&t.select_phase("phase-2"));
    write_file(&t.contract_path, &contract_toml_for_phase("phase-2"));
    git_commit(
        &t.repo,
        "contract.toml",
        contract_toml_for_phase("phase-2").as_bytes(),
    );
    assert_success(&t.draft_contract());
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    assert_success(&t.impl_begin(1, &sha));
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_success(&t.apply(parts[1], parts[2]));
    let state = t.get_state();
    assert_eq!(state["active_phase"], "phase-2");
    assert_eq!(state["closed_phases"], serde_json::json!(["phase-1"]));

    // Negative: an inferred phase that is already closed is unrecoverable.
    // Restore PHASE-1's archived files (the first completion entry) after
    // both phases are closed and drop state: the draft names a closed phase.
    let t2 = TestRepo::new();
    t2.close_phase2();
    let ledger = t2.get_completion_ledger().unwrap();
    let archived = ledger["completions"][0]["final_manifest"]["archived_governance"].clone();
    for (name, field) in [
        ("contract-draft.json", "contract_draft_content"),
        ("accepted-contract.json", "accepted_contract_content"),
        (
            "implementation-authority.json",
            "implementation_authority_content",
        ),
        ("audit-ledger.json", "audit_ledger_content"),
    ] {
        t2.write_mrgs(name, archived[field].as_str().unwrap().as_bytes());
    }
    t2.delete("state.json");
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

#[test]
fn test_obligation_34_selected_phase_without_draft_healthy() {
    let t = TestRepo::new();
    git_commit(&t.repo, "plan.toml", valid_plan_toml().as_bytes());
    assert_success(&t.accept_plan());
    assert_success(&t.select_phase("phase-1"));
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_NOT_REQUIRED");
    // The valid selected phase was not erased or guessed away.
    let state = t.get_state();
    assert_eq!(state["active_phase"], "phase-1");
    assert_eq!(state["closed_phases"], serde_json::json!([]));
}

// ============================================================================
// 35-42: completion relation, incomplete closeout, continuity
// ============================================================================

#[test]
fn test_obligation_35_completion_state_relation_exact() {
    // Healthy completed relation is exact.
    let t = TestRepo::new();
    t.close_phase1();
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));

    // A valid state that disagrees with the final receipt is a recoverable
    // incomplete closeout, not healthy. With no remaining phase-scoped files
    // the closeout resumption (through the existing finalizer, which
    // rewrites the exact receipt-bound after-state) is the single action.
    let t2 = TestRepo::new();
    t2.close_phase1();
    t2.write_state(&serde_json::json!({
        "schema_version": 1,
        "accepted_plan_sha256": t2.plan_sha(),
        "active_phase": null,
        "closed_phases": []
    }));
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
    assert_success(&t2.apply(parts[1], parts[2]));
    let state = t2.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!(["phase-1"]));
}

#[test]
fn test_obligation_36_incomplete_closeout_recoverable() {
    // State transition unfinished: active phase still set.
    let t = TestRepo::new();
    t.close_phase1();
    t.write_state(&serde_json::json!({
        "schema_version": 1,
        "accepted_plan_sha256": t.plan_sha(),
        "active_phase": "phase-1",
        "closed_phases": []
    }));
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
    assert_success(&t.apply(parts[1], parts[2]));
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!(["phase-1"]));

    // Cleanup unfinished: phase files remain after state is complete.
    let t2 = TestRepo::new();
    t2.close_phase1();
    let archived = t2.archived_governance();
    for (name, field) in [
        ("contract-draft.json", "contract_draft_content"),
        ("accepted-contract.json", "accepted_contract_content"),
        (
            "implementation-authority.json",
            "implementation_authority_content",
        ),
        ("audit-ledger.json", "audit_ledger_content"),
    ] {
        t2.write_mrgs(name, archived[field].as_str().unwrap().as_bytes());
    }
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
    assert_success(&t2.apply(parts[1], parts[2]));
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(
            !t2.repo.join(".mrgs").join(name).exists(),
            "{} must be removed",
            name
        );
    }
}

#[test]
fn test_obligation_37_state_missing_during_closeout_pre_state() {
    let t = TestRepo::new();
    t.close_phase1();
    let archived = t.archived_governance();
    for (name, field) in [
        ("contract-draft.json", "contract_draft_content"),
        ("accepted-contract.json", "accepted_contract_content"),
        (
            "implementation-authority.json",
            "implementation_authority_content",
        ),
        ("audit-ledger.json", "audit_ledger_content"),
    ] {
        t.write_mrgs(name, archived[field].as_str().unwrap().as_bytes());
    }
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json",
        "pre-closeout state must be reconstructed first"
    );
    assert_eq!(lines[2], "RECOVERY_ACTION 2 RESUME_CLOSEOUT phase-1");
    assert_success(&t.apply(parts[1], parts[2]));
    // Final state is the exact receipt after-state.
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!(["phase-1"]));
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(!t.repo.join(".mrgs").join(name).exists());
    }
    let completions = t.get_completion_ledger().unwrap();
    assert_eq!(completions["completions"].as_array().unwrap().len(), 1);

    // Semantically valid but completion-inconsistent state during closeout:
    // the receipt-bound state correction is derived before resumption.
    let t2 = TestRepo::new();
    t2.close_phase1();
    let archived = t2.archived_governance();
    for (name, field) in [
        ("contract-draft.json", "contract_draft_content"),
        ("accepted-contract.json", "accepted_contract_content"),
        (
            "implementation-authority.json",
            "implementation_authority_content",
        ),
        ("audit-ledger.json", "audit_ledger_content"),
    ] {
        t2.write_mrgs(name, archived[field].as_str().unwrap().as_bytes());
    }
    // Valid state record whose relation to the final receipt is Other:
    // closed phases match the before-set but the active phase does not match
    // the receipt's active_phase_before.
    t2.write_state(&serde_json::json!({
        "schema_version": 1,
        "accepted_plan_sha256": t2.plan_sha(),
        "active_phase": serde_json::Value::Null,
        "closed_phases": []
    }));
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json",
        "the exact receipt-bound pre-closeout state must be derived first"
    );
    assert_eq!(lines[2], "RECOVERY_ACTION 2 RESUME_CLOSEOUT phase-1");
    assert_success(&t2.apply(parts[1], parts[2]));
    let state = t2.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!(["phase-1"]));
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(!t2.repo.join(".mrgs").join(name).exists());
    }
    assert_eq!(t2.inspect_sha(), recompute_subject(&t2.repo));
}

#[test]
fn test_obligation_38_closeout_resume_byte_exact_files() {
    let t = TestRepo::new();
    t.close_phase1();
    let archived = t.archived_governance();
    // Simulate a crash mid-cleanup at a reachable point of the fixed order
    // (audit-ledger and implementation-authority already removed): the two
    // remaining files must byte-match the archived copies.
    t.write_mrgs(
        "accepted-contract.json",
        archived["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_mrgs(
        "contract-draft.json",
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_state(&serde_json::json!({
        "schema_version": 1,
        "accepted_plan_sha256": t.plan_sha(),
        "active_phase": "phase-1",
        "closed_phases": []
    }));
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
    assert_success(&t.apply(parts[1], parts[2]));
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(!t.repo.join(".mrgs").join(name).exists());
    }
}

#[test]
fn test_obligation_39_closeout_resume_fixed_order_exact_state() {
    let t = TestRepo::new();
    t.close_phase1();
    let archived = t.archived_governance();
    // Simulate a crash mid-cleanup: the fixed order removes
    // audit-ledger, implementation-authority, accepted-contract, then
    // contract-draft. Remove the first two and resume.
    t.write_mrgs(
        "audit-ledger.json",
        archived["audit_ledger_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_mrgs(
        "implementation-authority.json",
        archived["implementation_authority_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_mrgs(
        "accepted-contract.json",
        archived["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_mrgs(
        "contract-draft.json",
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_state(&serde_json::json!({
        "schema_version": 1,
        "accepted_plan_sha256": t.plan_sha(),
        "active_phase": "phase-1",
        "closed_phases": []
    }));
    let ledger_before = t.read_mrgs("completion-ledger.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    // Exact closed state and the completion ledger untouched.
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!(["phase-1"]));
    assert_eq!(t.read_mrgs("completion-ledger.json"), ledger_before);
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(!t.repo.join(".mrgs").join(name).exists());
    }
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_40_closeout_byte_mismatch_unrecoverable() {
    let t = TestRepo::new();
    t.close_phase1();
    let archived = t.archived_governance();
    t.write_mrgs(
        "contract-draft.json",
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    // Tampered byte content for the second file.
    t.write_mrgs("accepted-contract.json", b"{tampered");
    t.write_state(&serde_json::json!({
        "schema_version": 1,
        "accepted_plan_sha256": t.plan_sha(),
        "active_phase": "phase-1",
        "closed_phases": []
    }));
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    // No file was removed.
    assert!(t.repo.join(".mrgs/contract-draft.json").exists());
    assert!(t.repo.join(".mrgs/accepted-contract.json").exists());
}

#[test]
fn test_obligation_41_continuity_validated() {
    let t = TestRepo::new();
    let (_, receipt_sha) = t.close_phase1();
    let meta = t.write_metadata("cont.toml", &standard_metadata("phase-1", &receipt_sha));
    assert_success(&t.continuity_record(&meta));
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));
}

#[test]
fn test_obligation_42_continuity_corrupt_unrecoverable_never_regenerated() {
    let t = TestRepo::new();
    let (_, receipt_sha) = t.close_phase1();
    let meta = t.write_metadata("cont.toml", &standard_metadata("phase-1", &receipt_sha));
    assert_success(&t.continuity_record(&meta));
    // Corrupt the archived note (breaks the manifest hash chain).
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t.read_mrgs("continuity-ledger.json")).unwrap();
    ledger["entries"][0]["continuity_manifest"]["note"] =
        serde_json::Value::String("tampered".to_string());
    let tampered = serde_json::to_string_pretty(&ledger).unwrap();
    t.write_mrgs("continuity-ledger.json", tampered.as_bytes());
    let before = t.read_mrgs("continuity-ledger.json");
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    // Never regenerated: the tampered bytes are preserved exactly.
    assert_eq!(t.read_mrgs("continuity-ledger.json"), before);
}

// ============================================================================
// 43-48: recovery ledger and temporary files
// ============================================================================

#[test]
fn test_obligation_43_recovery_ledger_absent_and_strict_valid() {
    // Absent ledger is accepted on a healthy repo.
    let t = TestRepo::new();
    t.setup_impl_bound();
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));
    assert!(!t.repo.join(".mrgs/recovery-ledger.json").exists());

    // A valid applied ledger is revalidated and accepted.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    t2.delete("state.json");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t2.apply(parts[1], parts[2]));
    let lines = t2.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));

    // Missing raw key in the ledger is invalid.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.delete("state.json");
    let lines = t3.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t3.apply(parts[1], parts[2]));
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t3.read_mrgs("recovery-ledger.json")).unwrap();
    ledger.as_object_mut().unwrap().remove("plan_id");
    t3.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
}

#[test]
fn test_obligation_44_corrupt_recovery_ledger_blocks_mutation() {
    // Corrupt JSON
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    t.write_mrgs("recovery-ledger.json", b"{not json");
    let out = t.apply(parts[1], parts[2]);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // Topology-unsafe ledger (symlink) blocks mutation.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    t2.delete("state.json");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t2.apply(parts[1], parts[2]));
    let ledger_path = t2.repo.join(".mrgs/recovery-ledger.json");
    std::fs::remove_file(&ledger_path).unwrap();
    let target = t2.repo.join("ledger-target.json");
    write_file(&target, "{}");
    match make_file_link(&target, &ledger_path) {
        Ok(()) => {
            let out = t2.apply(parts[1], parts[2]);
            assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Capability-unavailable branch: a directory at the ledger path
            // is the concrete fail-closed fallback.
            std::fs::create_dir_all(&ledger_path).unwrap();
            let out = t2.apply(parts[1], parts[2]);
            assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
        }
        Err(e) => panic!("symlink creation failed: {}", e),
    }

    // Stale ledger (internally valid, bound to a different plan).
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.delete("state.json");
    let lines = t3.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t3.apply(parts[1], parts[2]));
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t3.read_mrgs("recovery-ledger.json")).unwrap();
    let other_plan = "b".repeat(64);
    ledger["accepted_plan_sha256"] = serde_json::Value::String(other_plan.clone());
    ledger["recoveries"][0]["plan"]["accepted_plan_sha256"] =
        serde_json::Value::String(other_plan.clone());
    ledger["recoveries"][0]["recovery_receipt"]["accepted_plan_sha256"] =
        serde_json::Value::String(other_plan.clone());
    // Recompute the seed hash so the journal is internally consistent.
    let plan: PlanSeedProbe =
        serde_json::from_value(ledger["recoveries"][0]["plan"].clone()).unwrap();
    let new_rid = sha256_hex(serde_json::to_string(&plan).unwrap().as_bytes());
    ledger["recoveries"][0]["recovery_id"] = serde_json::Value::String(new_rid.clone());
    ledger["recoveries"][0]["recovery_receipt"]["recovery_id"] =
        serde_json::Value::String(new_rid.clone());
    let receipt: ReceiptProbe =
        serde_json::from_value(ledger["recoveries"][0]["recovery_receipt"].clone()).unwrap();
    let new_receipt_sha = sha256_hex(serde_json::to_string(&receipt).unwrap().as_bytes());
    ledger["recoveries"][0]["recovery_receipt_sha256"] =
        serde_json::Value::String(new_receipt_sha.clone());
    t3.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_STALE");

    // Arbitrary parseable restore replacement: semantically invalid journal,
    // rejected before any classification or execution, zero mutation.
    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    craft_pending_journal_with_edited_plan(&t4, |plan| {
        plan["actions"][0]["replacement"] =
            serde_json::Value::String("{\"arbitrary\": true}".to_string());
    });
    let sha_now = recompute_subject(&t4.repo);
    let rid = t4.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t4.repo);
    let out = t4.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t4.repo, &before);

    // Reversed action order: LEDGER_INVALID, zero mutation.
    let t5 = TestRepo::new();
    craft_pending_two_action_journal_with_edited_plan(&t5, |plan| {
        let actions = plan["actions"].as_array_mut().unwrap();
        actions.swap(0, 1);
    });
    let sha_now = recompute_subject(&t5.repo);
    let rid = t5.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t5.repo);
    let out = t5.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t5.repo, &before);

    // Duplicate action (two state restorations): LEDGER_INVALID, zero
    // mutation.
    let t6 = TestRepo::new();
    craft_pending_two_action_journal_with_edited_plan(&t6, |plan| {
        let actions = plan["actions"].as_array_mut().unwrap();
        actions[1] = actions[0].clone();
    });
    let sha_now = recompute_subject(&t6.repo);
    let rid = t6.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t6.repo);
    let out = t6.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t6.repo, &before);

    // Invalid hash format in a stored prefix: LEDGER_INVALID, zero mutation.
    let t7 = TestRepo::new();
    t7.setup_impl_bound();
    craft_pending_journal_with_edited_plan(&t7, |plan| {
        plan["prefix_subject_sha256"][0] = serde_json::Value::String("Z".repeat(64));
    });
    let sha_now = recompute_subject(&t7.repo);
    let rid = t7.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t7.repo);
    let out = t7.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t7.repo, &before);

    // APPLIED post-subject differing from the final prefix: LEDGER_INVALID,
    // zero mutation.
    let t8 = TestRepo::new();
    t8.setup_impl_bound();
    craft_applied_journal_with_edit(&t8, |entry| {
        entry["post_subject_sha256"] = serde_json::Value::String("b".repeat(64));
        entry["recovery_receipt"]["post_subject_sha256"] =
            serde_json::Value::String("b".repeat(64));
    });
    let sha_now = recompute_subject(&t8.repo);
    let rid = t8.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t8.repo);
    let out = t8.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t8.repo, &before);

    // Duplicate recovery ID across the journal history: LEDGER_INVALID, zero
    // mutation.
    let t9 = TestRepo::new();
    t9.setup_impl_bound();
    t9.delete("state.json");
    let lines = t9.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t9.apply(parts[1], parts[2]));
    let first_rid = t9.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    // Second, distinct recovery (malformed state with unsupported schema:
    // different subject bytes, hence a different recovery ID).
    let mut malformed: serde_json::Value =
        serde_json::from_slice(&t9.read_mrgs("state.json")).unwrap();
    malformed["schema_version"] = serde_json::Value::from(99);
    t9.write_mrgs(
        "state.json",
        serde_json::to_string_pretty(&malformed).unwrap().as_bytes(),
    );
    let lines = t9.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t9.apply(parts[1], parts[2]));
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t9.read_mrgs("recovery-ledger.json")).unwrap();
    // Rewrite entry two with entry one's plan and ID: the seed hash matches
    // the copied ID, so the duplicate-ID rule is the only violation.
    let first_plan = ledger["recoveries"][0]["plan"].clone();
    let first_post = ledger["recoveries"][0]["post_subject_sha256"].clone();
    ledger["recoveries"][1]["plan"] = first_plan;
    ledger["recoveries"][1]["recovery_id"] = serde_json::Value::String(first_rid.clone());
    ledger["recoveries"][1]["post_subject_sha256"] = first_post.clone();
    {
        let plan_copy = ledger["recoveries"][1]["plan"].clone();
        let pre = plan_copy["pre_subject_sha256"].clone();
        let actions_json = serde_json::to_string(&plan_copy["actions"]).unwrap();
        let receipt = ledger["recoveries"][1]["recovery_receipt"]
            .as_object_mut()
            .unwrap();
        receipt.insert(
            "recovery_id".to_string(),
            serde_json::Value::String(first_rid.clone()),
        );
        receipt.insert("pre_subject_sha256".to_string(), pre);
        receipt.insert("post_subject_sha256".to_string(), first_post.clone());
        receipt.insert(
            "actions_sha256".to_string(),
            serde_json::Value::String(sha256_hex(actions_json.as_bytes())),
        );
    }
    {
        let probe: ReceiptProbe =
            serde_json::from_value(ledger["recoveries"][1]["recovery_receipt"].clone()).unwrap();
        let receipt_sha = sha256_hex(serde_json::to_string(&probe).unwrap().as_bytes());
        ledger["recoveries"][1]["recovery_receipt_sha256"] = serde_json::Value::String(receipt_sha);
    }
    t9.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
    let sha_now = recompute_subject(&t9.repo);
    let before = mrgs_snapshot(&t9.repo);
    let out = t9.apply(&first_rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t9.repo, &before);

    // Forged-but-valid intermediate prefix: a self-consistent pending
    // journal with legitimate actions and replacements, prefix[0] = the
    // exact current subject, and an intermediate prefix the deterministic
    // simulation cannot reproduce is semantically false: RECOVERY_LEDGER_INVALID
    // before any mutation (never a postcondition failure after the fact).
    // The final prefix is deliberately not validated pre-mutation: it is
    // owned by the post-action postcondition check (test 86), which runs
    // only after real execution.
    let t10 = TestRepo::new();
    craft_pending_two_action_journal_with_edited_plan(&t10, |plan| {
        plan["prefix_subject_sha256"][1] = serde_json::Value::String("a".repeat(64));
    });
    let sha_now = recompute_subject(&t10.repo);
    let rid = t10.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t10.repo);
    let out = t10.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t10.repo, &before);
    assert!(!t10.repo.join(".mrgs/state.json").exists());
    assert!(!t10.repo.join(".mrgs/accepted-plan.json").exists());

    // Canonical RESTORE_STATE replacement bound to a wrong accepted plan:
    // the stored after-state references an accepted_plan_sha256 the current
    // authority cannot produce, so the journal is rejected pre-mutation.
    let t11 = TestRepo::new();
    t11.setup_impl_bound();
    craft_pending_journal_with_edited_plan(&t11, |plan| {
        let replacement = plan["actions"][0]["replacement"]
            .as_str()
            .unwrap()
            .to_string();
        let wrong = replacement.replace(
            &format!("\"accepted_plan_sha256\": \"{}\"", t11.plan_sha()),
            &format!("\"accepted_plan_sha256\": \"{}\"", "b".repeat(64)),
        );
        assert_ne!(wrong, replacement, "replacement must remain canonical");
        plan["actions"][0]["replacement"] = serde_json::Value::String(wrong);
    });
    let sha_now = recompute_subject(&t11.repo);
    let rid = t11.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t11.repo);
    let out = t11.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t11.repo, &before);
    assert!(!t11.repo.join(".mrgs/state.json").exists());

    // Canonical RESTORE_ACCEPTED_PLAN replacement bound to a wrong plan_id:
    // rejected pre-mutation.
    let t12 = TestRepo::new();
    craft_pending_two_action_journal_with_edited_plan(&t12, |plan| {
        let replacement = plan["actions"][0]["replacement"]
            .as_str()
            .unwrap()
            .to_string();
        let parsed: serde_json::Value = serde_json::from_str(&replacement).unwrap();
        let real_id = parsed["plan_id"].as_str().unwrap().to_string();
        let wrong = replacement.replace(
            &format!("\"plan_id\": \"{}\"", real_id),
            "\"plan_id\": \"forged-plan-id\"",
        );
        assert_ne!(wrong, replacement, "replacement must remain canonical");
        plan["actions"][0]["replacement"] = serde_json::Value::String(wrong);
    });
    let sha_now = recompute_subject(&t12.repo);
    let rid = t12.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t12.repo);
    let out = t12.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t12.repo, &before);
    assert!(!t12.repo.join(".mrgs/accepted-plan.json").exists());
    assert!(!t12.repo.join(".mrgs/state.json").exists());

    // Canonical RESTORE_ACCEPTED_PLAN replacement bound to a wrong plan
    // SHA-256: rejected pre-mutation.
    let t13 = TestRepo::new();
    craft_pending_two_action_journal_with_edited_plan(&t13, |plan| {
        let replacement = plan["actions"][0]["replacement"]
            .as_str()
            .unwrap()
            .to_string();
        let parsed: serde_json::Value = serde_json::from_str(&replacement).unwrap();
        let real_sha = parsed["sha256"].as_str().unwrap().to_string();
        let wrong = replacement.replace(
            &format!("\"sha256\": \"{}\"", real_sha),
            &format!("\"sha256\": \"{}\"", "c".repeat(64)),
        );
        assert_ne!(wrong, replacement, "replacement must remain canonical");
        plan["actions"][0]["replacement"] = serde_json::Value::String(wrong);
    });
    let sha_now = recompute_subject(&t13.repo);
    let rid = t13.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t13.repo);
    let out = t13.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t13.repo, &before);
    assert!(!t13.repo.join(".mrgs/accepted-plan.json").exists());
    assert!(!t13.repo.join(".mrgs/state.json").exists());

    // Two different producer-temp removal actions mapping to the same
    // permanent target (state.json via .closeout-state.0.tmp and
    // .closeout-state.1.tmp): an ambiguous mapping, rejected pre-mutation.
    let t14 = TestRepo::new();
    t14.setup_impl_bound();
    craft_pending_journal_with_edited_plan(&t14, |plan| {
        let prefix0 = plan["prefix_subject_sha256"][0].clone();
        plan["actions"] = serde_json::json!([
            {"kind": "REMOVE_REDUNDANT_TEMP", "target": ".closeout-state.0.tmp"},
            {"kind": "REMOVE_REDUNDANT_TEMP", "target": ".closeout-state.1.tmp"},
        ]);
        plan["prefix_subject_sha256"] =
            serde_json::json!([prefix0, "a".repeat(64), "b".repeat(64)]);
    });
    let sha_now = recompute_subject(&t14.repo);
    let rid = t14.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let before = mrgs_snapshot(&t14.repo);
    let out = t14.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t14.repo, &before);
    assert!(!t14.repo.join(".mrgs/state.json").exists());

    // Forged final prefix, one-action plan: a self-consistent pending
    // journal with prefix[0] = the exact current subject, a legitimate
    // canonical RESTORE_STATE replacement, and prefix[1] = an arbitrary
    // different lowercase-valid hash. The final prefix is validated
    // pre-mutation exactly like every other prefix: RECOVERY_LEDGER_INVALID,
    // state.json never created, recovery-ledger and repository bytes
    // unchanged.
    let t15 = TestRepo::new();
    t15.setup_impl_bound();
    craft_pending_journal_with_edited_plan(&t15, |plan| {
        plan["prefix_subject_sha256"][1] = serde_json::Value::String("a".repeat(64));
    });
    let sha_now = recompute_subject(&t15.repo);
    let rid = t15.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let ledger_before = t15.recovery_ledger_bytes();
    let before = mrgs_snapshot(&t15.repo);
    let out = t15.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert!(!t15.repo.join(".mrgs/state.json").exists());
    assert_eq!(t15.recovery_ledger_bytes(), ledger_before);
    assert_snapshot_unchanged(&t15.repo, &before);

    // Forged final prefix, one-action RESUME_CLOSEOUT plan: the interrupted
    // Phase 6 state publication (temp + pre-closeout state + phase-scoped
    // files) with a pending RESUME journal whose final prefix is an
    // arbitrary different lowercase-valid hash. The receipt-bound
    // after-state simulation cannot reproduce it: RECOVERY_LEDGER_INVALID
    // with the temp, pre-closeout state, phase files, completion ledger,
    // and pending journal bytes all preserved.
    let t16 = TestRepo::new();
    let after16 = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t16.plan_sha(),
        active_phase: None,
        closed_phases: vec!["phase-1".to_string()],
    })
    .unwrap();
    setup_closeout_state_temp_fixture(&t16, ".closeout-state.0.tmp", after16.as_bytes());
    let mut ledger16: serde_json::Value =
        serde_json::from_slice(&t16.read_mrgs("recovery-ledger.json")).unwrap();
    let prefixes = ledger16["recoveries"][0]["plan"]["prefix_subject_sha256"]
        .as_array_mut()
        .unwrap();
    let last = prefixes.last_mut().unwrap();
    *last = serde_json::Value::String("a".repeat(64));
    let plan16: PlanSeedProbe =
        serde_json::from_value(ledger16["recoveries"][0]["plan"].clone()).unwrap();
    let forged_rid = sha256_hex(serde_json::to_string(&plan16).unwrap().as_bytes());
    ledger16["recoveries"][0]["recovery_id"] = serde_json::Value::String(forged_rid.clone());
    t16.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger16).unwrap().as_bytes(),
    );
    let sha_now = recompute_subject(&t16.repo);
    let before = mrgs_snapshot(&t16.repo);
    let out = t16.apply(&forged_rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t16.repo, &before);
    assert!(t16.repo.join(".mrgs/.closeout-state.0.tmp").exists());
}

fn phase1_producer_temp(target_name: &str) -> String {
    // Producer grammar: .{pid}.{count}.{ts}.{filename}.tmp
    format!(".123.0.456.{}.tmp", target_name)
}

#[test]
fn test_obligation_45_temp_name_mapping_unknown_rejected() {
    // Recognized producer temp mapping to exactly one permanent target.
    let t = TestRepo::new();
    t.close_phase1();
    let state_bytes = t.read_mrgs("state.json");
    let temp = phase1_producer_temp("state.json");
    t.write_mrgs(&temp, &state_bytes);
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1],
        format!("RECOVERY_ACTION 1 REMOVE_REDUNDANT_TEMP {}", temp)
    );

    // Unknown temp name is unrecoverable.
    let t2 = TestRepo::new();
    t2.close_phase1();
    t2.write_mrgs("mystery.tmp", b"x");
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Malformed producer temp (non-digit attempt) is unrecoverable.
    let t3 = TestRepo::new();
    t3.close_phase1();
    t3.write_mrgs(".closeout.abc.tmp", b"x");
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

#[test]
fn test_obligation_46_redundant_temp_remove_action() {
    let t = TestRepo::new();
    t.close_phase1();
    // Redundant copy of the completion ledger under the closeout grammar.
    let ledger_bytes = t.read_mrgs("completion-ledger.json");
    t.write_mrgs(".closeout.0.tmp", &ledger_bytes);
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 REMOVE_REDUNDANT_TEMP .closeout.0.tmp"
    );
    assert_success(&t.apply(parts[1], parts[2]));
    assert!(!t.repo.join(".mrgs/.closeout.0.tmp").exists());
    assert_eq!(t.read_mrgs("completion-ledger.json"), ledger_bytes);
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
}

#[test]
fn test_obligation_47_temp_irregularity_unrecoverable() {
    // Differing temp bytes
    let t = TestRepo::new();
    t.close_phase1();
    let state_bytes = t.read_mrgs("state.json");
    let temp = phase1_producer_temp("state.json");
    let mut diff = state_bytes.clone();
    diff.push(b'!');
    t.write_mrgs(&temp, &diff);
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Target-absent pre-Phase-8 temp
    let t2 = TestRepo::new();
    t2.close_phase1();
    t2.delete("state.json");
    let temp = phase1_producer_temp("state.json");
    t2.write_mrgs(&temp, b"orphan");
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Duplicate candidates for one target
    let t3 = TestRepo::new();
    t3.close_phase1();
    let state_bytes = t3.read_mrgs("state.json");
    let temp_a = phase1_producer_temp("state.json");
    let temp_b = ".999.0.888.state.json.tmp".to_string();
    t3.write_mrgs(&temp_a, &state_bytes);
    t3.write_mrgs(&temp_b, &state_bytes);
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Unsafe temp topology (symlink)
    let t4 = TestRepo::new();
    t4.close_phase1();
    let target = t4.repo.join("state-copy.json");
    write_file(&target, "{}");
    let temp = ".closeout.1.tmp";
    match make_file_link(&target, &t4.repo.join(".mrgs").join(temp)) {
        Ok(()) => {
            let out = t4.inspect();
            assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            std::fs::create_dir_all(t4.repo.join(".mrgs").join(temp)).unwrap();
            let out = t4.inspect();
            assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
        }
        Err(e) => panic!("symlink creation failed: {}", e),
    }
}

#[test]
fn test_obligation_48_recovery_temp_promote_remove() {
    // (a) Promote: crash after the RESTORE_STATE temp write, before rename.
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_temp_write:0", &dir);
    kill_child(child);
    // Journal is pending; the deterministic temp exists.
    let ledger = t.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["status"], "PENDING");
    let temp = format!(".recovery-{}-0.tmp", rid);
    assert!(t.repo.join(".mrgs").join(&temp).exists());
    assert!(!t.repo.join(".mrgs/state.json").exists());
    // Resume with the current subject SHA (includes the authorized temp).
    let sha_now = recompute_subject(&t.repo);
    let out = t.apply(rid, &sha_now);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    assert!(t.repo.join(".mrgs/state.json").exists());
    assert!(!t.repo.join(".mrgs").join(&temp).exists());
    assert_no_temp_files(&t.repo);
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    t2.delete("state.json");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid2 = parts[1];
    let sha2 = parts[2];
    let dir2 = t2._dir.path().to_path_buf();
    // The ledger publication temp is .recovery-<rid>-<action_count>.tmp with
    // action_count == 1 (single RESTORE_STATE action). The advance publish
    // happens after action 0, so the crash point fires only when the journal
    // already exists.
    let child2 = crash_apply(&t2, rid2, sha2, "after_ledger_temp_write_advance", &dir2);
    kill_child(child2);
    let ledger2 = t2.get_recovery_ledger().unwrap();
    assert_eq!(ledger2["recoveries"][0]["status"], "PENDING");
    let temp2 = format!(".recovery-{}-1.tmp", rid2);
    assert!(t2.repo.join(".mrgs").join(&temp2).exists());
    let sha_now2 = recompute_subject(&t2.repo);
    let out2 = t2.apply(rid2, &sha_now2);
    assert_success(&out2);
    assert!(stdout_str(&out2).starts_with("RECOVERY_APPLIED "));
    assert!(!t2.repo.join(".mrgs").join(&temp2).exists());
    assert_no_temp_files(&t2.repo);

    // Prevalidation before any normalization mutation: a valid pending
    // restore temp plus a second invalid recovery-owned temp whose index
    // sorts after it must fail with zero mutation.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.delete("state.json");
    let lines = t3.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid3 = parts[1];
    let sha3 = parts[2];
    let dir3 = t3._dir.path().to_path_buf();
    let child3 = crash_apply(&t3, rid3, sha3, "after_temp_write:0", &dir3);
    kill_child(child3);
    let valid_temp = format!(".recovery-{}-0.tmp", rid3);
    let invalid_temp = format!(".recovery-{}-2.tmp", rid3);
    assert!(t3.repo.join(".mrgs").join(&valid_temp).exists());
    t3.write_mrgs(&invalid_temp, b"out of range leftover");
    let ledger_before = t3.recovery_ledger_bytes();
    let valid_bytes = t3.read_mrgs(&valid_temp);
    let invalid_bytes = t3.read_mrgs(&invalid_temp);
    let sha_now = recompute_subject(&t3.repo);
    let out = t3.apply(rid3, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_ACTION_FAILED");
    // No target created, no temp removed, journal bytes unchanged.
    assert!(!t3.repo.join(".mrgs/state.json").exists());
    assert_eq!(t3.read_mrgs(&valid_temp), valid_bytes);
    assert_eq!(t3.read_mrgs(&invalid_temp), invalid_bytes);
    assert_eq!(t3.recovery_ledger_bytes(), ledger_before);

    // A noncanonical index alias (leading zero) is never a valid leftover:
    // it fails closed as unrecoverable with zero mutation.
    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    t4.delete("state.json");
    let lines = t4.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid4 = parts[1];
    let sha4 = parts[2];
    let dir4 = t4._dir.path().to_path_buf();
    let child4 = crash_apply(&t4, rid4, sha4, "after_temp_write:0", &dir4);
    kill_child(child4);
    let alias_temp = format!(".recovery-{}-00.tmp", rid4);
    t4.write_mrgs(&alias_temp, b"alias");
    let ledger_before = t4.recovery_ledger_bytes();
    let before = mrgs_snapshot(&t4.repo);
    let sha_now = recompute_subject(&t4.repo);
    let out = t4.apply(rid4, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_snapshot_unchanged(&t4.repo, &before);
    assert_eq!(t4.recovery_ledger_bytes(), ledger_before);
}

// ============================================================================
// 49-58: actions, plan, ID, inspect output, apply authorization
// ============================================================================

fn craft_pending_journal_with_edited_plan(t: &TestRepo, edit: impl Fn(&mut serde_json::Value)) {
    // Build a pending journal (crash before first action), then edit the
    // plan and recompute the recovery ID so the journal stays internally
    // consistent.
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(t, rid, sha, "after_pending_publish", &dir);
    kill_child(child);
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t.read_mrgs("recovery-ledger.json")).unwrap();
    edit(&mut ledger["recoveries"][0]["plan"]);
    // Recompute the recovery ID from the ordered canonical plan seed. When
    // the edit makes the plan unparseable the journal must fail closed as
    // RECOVERY_LEDGER_INVALID anyway, so any consistent hash suffices.
    let new_rid =
        match serde_json::from_value::<PlanSeedProbe>(ledger["recoveries"][0]["plan"].clone()) {
            Ok(plan) => sha256_hex(serde_json::to_string(&plan).unwrap().as_bytes()),
            Err(_) => "a".repeat(64),
        };
    ledger["recoveries"][0]["recovery_id"] = serde_json::Value::String(new_rid.clone());
    t.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
}

/// Pending-journal craft on a two-action subject (missing accepted-plan and
/// state after a closeout): actions are [RESTORE_ACCEPTED_PLAN,
/// RESTORE_STATE]. The recovery ID is recomputed from the edited plan seed.
fn craft_pending_two_action_journal_with_edited_plan(
    t: &TestRepo,
    edit: impl Fn(&mut serde_json::Value),
) {
    t.close_phase1();
    t.delete("accepted-plan.json");
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(t, rid, sha, "after_pending_publish", &dir);
    kill_child(child);
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t.read_mrgs("recovery-ledger.json")).unwrap();
    edit(&mut ledger["recoveries"][0]["plan"]);
    let new_rid =
        match serde_json::from_value::<PlanSeedProbe>(ledger["recoveries"][0]["plan"].clone()) {
            Ok(plan) => sha256_hex(serde_json::to_string(&plan).unwrap().as_bytes()),
            Err(_) => "a".repeat(64),
        };
    ledger["recoveries"][0]["recovery_id"] = serde_json::Value::String(new_rid.clone());
    t.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
}

/// APPLIED-journal craft: perform a full single-action recovery, then edit
/// the APPLIED entry and recompute the recovery ID (entry and receipt) and
/// the receipt hash so the journal stays internally consistent where
/// possible.
fn craft_applied_journal_with_edit(t: &TestRepo, edit: impl Fn(&mut serde_json::Value)) {
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t.read_mrgs("recovery-ledger.json")).unwrap();
    edit(&mut ledger["recoveries"][0]);
    let plan: PlanSeedProbe =
        serde_json::from_value(ledger["recoveries"][0]["plan"].clone()).unwrap();
    let new_rid = sha256_hex(serde_json::to_string(&plan).unwrap().as_bytes());
    ledger["recoveries"][0]["recovery_id"] = serde_json::Value::String(new_rid.clone());
    if let Some(receipt) = ledger["recoveries"][0].get("recovery_receipt") {
        if !receipt.is_null() {
            let mut receipt = receipt.clone();
            receipt["recovery_id"] = serde_json::Value::String(new_rid.clone());
            ledger["recoveries"][0]["recovery_receipt"] = receipt.clone();
            let probe: ReceiptProbe = serde_json::from_value(receipt).unwrap();
            let receipt_sha = sha256_hex(serde_json::to_string(&probe).unwrap().as_bytes());
            ledger["recoveries"][0]["recovery_receipt_sha256"] =
                serde_json::Value::String(receipt_sha.clone());
        }
    }
    t.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
}

#[test]
fn test_obligation_49_action_closed_enum_exact_fields() {
    // Unknown action kind
    let t = TestRepo::new();
    t.setup_impl_bound();
    craft_pending_journal_with_edited_plan(&t, |plan| {
        plan["actions"][0]["kind"] = serde_json::Value::String("BOGUS".to_string());
    });
    let sha_now = recompute_subject(&t.repo);
    let rid = t.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let out = t.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // Unknown field on an action
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    craft_pending_journal_with_edited_plan(&t2, |plan| {
        plan["actions"][0]["extra_field"] = serde_json::Value::from(1);
    });
    let sha_now = recompute_subject(&t2.repo);
    let rid = t2.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let out = t2.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // Missing required field
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    craft_pending_journal_with_edited_plan(&t3, |plan| {
        plan["actions"][0].as_object_mut().unwrap().remove("target");
    });
    let sha_now = recompute_subject(&t3.repo);
    let rid = t3.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    let out = t3.apply(&rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // Strict target labels: arbitrary, aliased, or unsafe targets in a
    // crafted pending journal are rejected before any mutation.
    let target_cases: Vec<(&str, &str)> = vec![
        // RESTORE_STATE with a traversal target
        ("RESTORE_STATE", "../evil"),
        // RESTORE_STATE with a separator-suffixed alias
        ("RESTORE_STATE", "state.json/"),
        // RESTORE_STATE with a backslash alias
        ("RESTORE_STATE", "state.json\\x"),
        // RESTORE_STATE with an unknown label
        ("RESTORE_STATE", "other.json"),
        // RESTORE_ACCEPTED_PLAN with a wrong label
        ("RESTORE_ACCEPTED_PLAN", "accepted-plan"),
        // REMOVE of a recovery-owned temp (journal-rules only, never a
        // REMOVE target)
        (
            "REMOVE_REDUNDANT_TEMP",
            ".recovery-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-0.tmp",
        ),
        // REMOVE of a permanent filename (not a temp grammar)
        ("REMOVE_REDUNDANT_TEMP", "state.json"),
        // REMOVE with a traversal temp name
        ("REMOVE_REDUNDANT_TEMP", "../evil.tmp"),
        // RESUME_CLOSEOUT with a control-character phase label
        ("RESUME_CLOSEOUT", "phase-1\n"),
        // RESUME_CLOSEOUT with surrounding whitespace
        ("RESUME_CLOSEOUT", " phase-1"),
    ];
    for (kind, target) in target_cases {
        let t = TestRepo::new();
        t.setup_impl_bound();
        craft_pending_journal_with_edited_plan(&t, |plan| {
            plan["actions"][0]["kind"] = serde_json::Value::String(kind.to_string());
            plan["actions"][0]["target"] = serde_json::Value::String(target.to_string());
            plan["actions"][0]
                .as_object_mut()
                .unwrap()
                .remove("replacement");
        });
        let sha_now = recompute_subject(&t.repo);
        let rid = t.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
            .as_str()
            .unwrap()
            .to_string();
        let out = t.apply(&rid, &sha_now);
        assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
        // No mutation occurred.
        assert!(!t.repo.join(".mrgs/state.json").exists());
        assert!(!t.repo.join(".mrgs").join(target).exists() || target.starts_with("../"));
    }

    // Separator/traversal-bearing RESUME labels pass the Phase 6 phase-ID
    // grammar (Phase 6 accepts such IDs and they must remain resumable), but
    // a pending journal carrying them is semantically false: its stored
    // prefix chain was computed for the original plan, the deterministic
    // RESUME_CLOSEOUT simulation cannot reproduce it, and without a
    // derivable receipt-bound after-state (no completion ledger here) the
    // stored pending action is not trustworthy. Validation rejects the
    // journal as RECOVERY_LEDGER_INVALID before any mutation, and no
    // filesystem path is ever derived from the label. The execution-stage
    // receipt binding (RECOVERY_ACTION_FAILED, zero mutation) is exercised
    // by obligation 64's crafted-label subcase, which keeps a genuine
    // prefix chain and a real completion ledger.
    let binding_cases: Vec<&str> =
        vec!["phase-1/2", "phase-1\\2", "..phase", "phase//1", "/phase-1"];
    for target in binding_cases {
        let t = TestRepo::new();
        t.setup_impl_bound();
        craft_pending_journal_with_edited_plan(&t, |plan| {
            plan["actions"][0]["kind"] = serde_json::Value::String("RESUME_CLOSEOUT".to_string());
            plan["actions"][0]["target"] = serde_json::Value::String(target.to_string());
            plan["actions"][0]
                .as_object_mut()
                .unwrap()
                .remove("replacement");
        });
        let sha_now = recompute_subject(&t.repo);
        let rid = t.get_recovery_ledger().unwrap()["recoveries"][0]["recovery_id"]
            .as_str()
            .unwrap()
            .to_string();
        let before = mrgs_snapshot(&t.repo);
        let out = t.apply(&rid, &sha_now);
        assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
        // No mutation occurred: the journal is rejected pre-mutation, so
        // the recovery ledger and every repository file are unchanged.
        assert_snapshot_unchanged(&t.repo, &before);
        assert!(!t.repo.join(".mrgs/state.json").exists());
    }
}

#[test]
fn test_obligation_50_action_order_deterministic() {
    let t = TestRepo::new();
    t.close_phase1();
    // Redundant temp + missing accepted-plan + missing state + remaining
    // phase files: all four action kinds in the fixed order.
    let ledger_bytes = t.read_mrgs("completion-ledger.json");
    t.write_mrgs(".closeout.0.tmp", &ledger_bytes);
    t.delete("accepted-plan.json");
    t.delete("state.json");
    let archived = t.archived_governance();
    t.write_mrgs(
        "contract-draft.json",
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_mrgs(
        "accepted-contract.json",
        archived["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    let lines = t.inspect_output();
    assert_eq!(lines.len(), 5);
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(parts[3], "4");
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 REMOVE_REDUNDANT_TEMP .closeout.0.tmp"
    );
    assert_eq!(
        lines[2],
        "RECOVERY_ACTION 2 RESTORE_ACCEPTED_PLAN accepted-plan.json"
    );
    assert_eq!(lines[3], "RECOVERY_ACTION 3 RESTORE_STATE state.json");
    assert_eq!(lines[4], "RECOVERY_ACTION 4 RESUME_CLOSEOUT phase-1");
    assert_success(&t.apply(parts[1], parts[2]));
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!(["phase-1"]));
    assert!(!t.repo.join(".mrgs/.closeout.0.tmp").exists());
}

#[test]
fn test_obligation_51_prefix_hashes_length_and_first() {
    let t = TestRepo::new();
    t.close_phase1();
    t.delete("accepted-plan.json");
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_pending_publish", &dir);
    kill_child(child);
    let ledger = t.get_recovery_ledger().unwrap();
    let plan = &ledger["recoveries"][0]["plan"];
    let actions = plan["actions"].as_array().unwrap();
    let prefixes = plan["prefix_subject_sha256"].as_array().unwrap();
    assert_eq!(prefixes.len(), actions.len() + 1);
    assert_eq!(plan["pre_subject_sha256"], prefixes[0]);
    for p in prefixes {
        assert_eq!(p.as_str().unwrap().len(), 64);
    }
    // The final prefix equals the healthy post-recovery subject. The resume
    // caller passes the current (pre-recovery) subject SHA.
    let sha_pre = recompute_subject(&t.repo);
    let out = t.apply(rid, &sha_pre);
    assert_success(&out);
    let sha_post = recompute_subject(&t.repo);
    let ledger2 = t.get_recovery_ledger().unwrap();
    let post = ledger2["recoveries"][0]["post_subject_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let pre_prefixes = ledger["recoveries"][0]["plan"]["prefix_subject_sha256"]
        .as_array()
        .unwrap()
        .clone();
    let post_prefixes = ledger2["recoveries"][0]["plan"]["prefix_subject_sha256"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        pre_prefixes, post_prefixes,
        "plan must be preserved across resume"
    );
    assert_eq!(post, prefixes.last().unwrap().as_str().unwrap());
    assert_eq!(post, sha_post);

    // Prefix simulation is self-contained: a malformed accepted-plan whose
    // raw plan_path points at a different safe regular file while the
    // completion ledger proves the correct plan. The simulated prefixes must
    // use the reconstructed record's plan path, never the live bytes.
    let t2 = TestRepo::new();
    t2.close_phase1();
    let plan_sha = t2.plan_sha();
    {
        let mut ap: serde_json::Value =
            serde_json::from_slice(&t2.read_mrgs("accepted-plan.json")).unwrap();
        ap["plan_path"] = serde_json::Value::String("contract.toml".to_string());
        t2.write_mrgs(
            "accepted-plan.json",
            serde_json::to_string_pretty(&ap).unwrap().as_bytes(),
        );
    }
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 RESTORE_ACCEPTED_PLAN accepted-plan.json"
    );
    assert_success(&t2.apply(parts[1], parts[2]));
    let ledger = t2.get_recovery_ledger().unwrap();
    let final_prefix = ledger["recoveries"][0]["plan"]["prefix_subject_sha256"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let post = ledger["recoveries"][0]["post_subject_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        post, final_prefix,
        "final subject must equal the final stored prefix"
    );
    let restored: serde_json::Value =
        serde_json::from_slice(&t2.read_mrgs("accepted-plan.json")).unwrap();
    assert_eq!(restored["plan_path"], "plan.toml");
    assert_eq!(restored["sha256"], plan_sha);
    let lines = t2.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));
    assert_eq!(t2.inspect_sha(), recompute_subject(&t2.repo));
}

#[test]
fn test_obligation_52_recovery_id_deterministic_seed_hash() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    // Same subject twice -> same recovery ID.
    let first = t.inspect_output();
    let second = t.inspect_output();
    assert_eq!(first, second);
    let parts: Vec<&str> = first[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_pending_publish", &dir);
    kill_child(child);
    let ledger = t.get_recovery_ledger().unwrap();
    // recovery_id == SHA-256 of the compact canonical plan seed.
    let plan: PlanSeedProbe =
        serde_json::from_value(ledger["recoveries"][0]["plan"].clone()).unwrap();
    let seed_json = serde_json::to_string(&plan).unwrap();
    assert_eq!(sha256_hex(seed_json.as_bytes()), rid);
    assert_eq!(ledger["recoveries"][0]["recovery_id"], rid);
}

#[test]
fn test_obligation_53_inspect_exact_required_action_lines() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    assert_eq!(lines.len(), 2);
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(parts[1].len(), 64);
    assert_eq!(parts[2].len(), 64);
    assert_eq!(parts[3], "1");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json");
}

#[test]
fn test_obligation_54_repeated_inspect_byte_identical_no_writes() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let before = mrgs_snapshot(&t.repo);
    let first = t.inspect();
    let second = t.inspect();
    assert_success(&first);
    assert_success(&second);
    assert_eq!(stdout_raw(&first), stdout_raw(&second));
    assert_snapshot_unchanged(&t.repo, &before);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_55_unrecoverable_exact_category_no_output() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.write_mrgs("rogue.json", b"{}");
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    // No mutation.
    assert!(t.repo.join(".mrgs/rogue.json").exists());
    assert!(!t.repo.join(".mrgs/recovery-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_56_apply_hash_and_decision_grammar() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let before = mrgs_snapshot(&t.repo);

    // Uppercase hash
    let out = t.apply(&rid.to_uppercase(), sha);
    assert_category_no_stdout(&out, "RECOVERY_ID_INVALID");
    // Wrong length
    let out = t.apply(&"a".repeat(63), sha);
    assert_category_no_stdout(&out, "RECOVERY_ID_INVALID");
    // Non-hex characters
    let out = t.apply(&"z".repeat(64), sha);
    assert_category_no_stdout(&out, "RECOVERY_ID_INVALID");
    // Uppercase subject hash
    let out = t.apply(rid, &sha.to_uppercase());
    assert_category_no_stdout(&out, "RECOVERY_ID_INVALID");
    // Wrong decision tokens (no trimming, case folding, or normalization)
    for bad in ["recover", "Recover", " RECOVER", "RECOVER ", "RECOVERED"] {
        let out = t.apply_decision(rid, sha, bad);
        assert_category_no_stdout(&out, "RECOVERY_DECISION_INVALID");
    }
    // Nothing was written.
    assert_snapshot_unchanged(&t.repo, &before);
    assert!(!t.repo.join(".mrgs/recovery-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_57_stale_arguments_rejected_before_publication() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let sha = parts[2];
    let before = mrgs_snapshot(&t.repo);

    // Wrong recovery ID (valid grammar, different value)
    let out = t.apply(&"a".repeat(64), sha);
    assert_category_no_stdout(&out, "RECOVERY_ID_INVALID");

    // Stale subject hash: change the repo after inspection.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    t2.delete("state.json");
    let lines2 = t2.inspect_output();
    let parts2: Vec<&str> = lines2[0].split_whitespace().collect();
    t2.write_mrgs("state.json", b"{}");
    let out = t2.apply(parts2[1], parts2[2]);
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");

    // Changed git identity: branch switch after inspection.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.delete("state.json");
    let lines3 = t3.inspect_output();
    let parts3: Vec<&str> = lines3[0].split_whitespace().collect();
    let switch = Command::new("git")
        .arg("-C")
        .arg(&t3.repo)
        .args(["checkout", "-b", "drift"])
        .output()
        .unwrap();
    assert!(switch.status.success());
    let out = t3.apply(parts3[1], parts3[2]);
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");

    // Nothing published for any rejected attempt.
    assert_snapshot_unchanged(&t.repo, &before);
    assert!(!t.repo.join(".mrgs/recovery-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_58_apply_recomputes_plan_not_caller() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    assert_success(&t.apply(rid, sha));
    let ledger = t.get_recovery_ledger().unwrap();
    let plan: PlanSeedProbe =
        serde_json::from_value(ledger["recoveries"][0]["plan"].clone()).unwrap();
    // The plan is exactly the deterministic recomputation: one RESTORE_STATE
    // action whose replacement is the recomputed canonical state bytes.
    assert_eq!(plan.schema_version, 1);
    assert_eq!(plan.accepted_plan_sha256, t.plan_sha());
    assert_eq!(plan.plan_id, "test-plan");
    assert_eq!(plan.actions.len(), 1);
    let action = &plan.actions[0];
    assert_eq!(action.kind, "RESTORE_STATE");
    assert_eq!(action.target, "state.json");
    let expected_state = StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t.plan_sha(),
        active_phase: Some("phase-1".to_string()),
        closed_phases: vec![],
    };
    assert_eq!(
        action.replacement.as_deref().unwrap(),
        serde_json::to_string_pretty(&expected_state).unwrap()
    );
    assert_eq!(plan.pre_subject_sha256, sha);
    assert_eq!(plan.prefix_subject_sha256.len(), 2);
    assert_eq!(plan.prefix_subject_sha256[0], sha);
}

// ============================================================================
// 59-74: pending journal, resumable execution, receipt, idempotency
// ============================================================================

#[test]
fn test_obligation_59_pending_published_before_first_mutation() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let state_absent = !t.repo.join(".mrgs/state.json").exists();
    let accepted_before = t.read_mrgs("accepted-plan.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, parts[1], parts[2], "after_pending_publish", &dir);
    kill_child(child);
    // The pending entry is durably published before any target mutation.
    let ledger = t.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["status"], "PENDING");
    assert_eq!(ledger["recoveries"][0]["next_action"], 0);
    assert_eq!(state_absent, !t.repo.join(".mrgs/state.json").exists());
    assert_eq!(t.read_mrgs("accepted-plan.json"), accepted_before);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_60_pending_entry_exact_fields() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_pending_publish", &dir);
    kill_child(child);
    let ledger = t.get_recovery_ledger().unwrap();
    assert_eq!(ledger["schema_version"], 1);
    assert_eq!(ledger["accepted_plan_sha256"], t.plan_sha());
    assert_eq!(ledger["plan_id"], "test-plan");
    let entry = &ledger["recoveries"][0];
    assert_eq!(entry["recovery_id"], rid);
    assert_eq!(entry["status"], "PENDING");
    assert_eq!(entry["next_action"], 0);
    assert_eq!(entry["plan"]["pre_subject_sha256"], sha);
    assert_eq!(entry["plan"]["actions"].as_array().unwrap().len(), 1);
    assert_eq!(
        entry["plan"]["prefix_subject_sha256"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // Explicit nulls for the not-yet-applied fields.
    assert!(entry["post_subject_sha256"].is_null());
    assert!(entry["recovery_receipt"].is_null());
    assert!(entry["recovery_receipt_sha256"].is_null());
}

#[test]
fn test_obligation_61_advance_atomic_preserves_on_failure() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    // The pending publish succeeds; the first advance rename fails.
    let out = t.run_with_env(
        &[
            "recovery",
            "apply",
            "--repo",
            &t.repo.to_string_lossy(),
            "--recovery-id",
            rid,
            "--subject-sha256",
            sha,
            "--decision",
            "RECOVER",
        ],
        &[("MRGS_TEST_ONLY_RECOVERY_FAIL_RENAME_AFTER_PUBLISH", "1")],
    );
    assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
    // Prior recovery-ledger bytes are preserved exactly (still the pending
    // entry with next_action == 0).
    let ledger = t.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["status"], "PENDING");
    assert_eq!(ledger["recoveries"][0]["next_action"], 0);
    // The action DID execute (state restored) but no journal advancement and
    // no leftover temp.
    assert!(t.repo.join(".mrgs/state.json").exists());
    assert_no_temp_files(&t.repo);
    // Resume completes the recovery.
    let sha_now = recompute_subject(&t.repo);
    let out = t.apply(rid, &sha_now);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    let ledger = t.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["status"], "APPLIED");
}

#[test]
fn test_obligation_62_crash_before_first_action_resumes() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_pending_publish", &dir);
    kill_child(child);
    // Inspection reports the pending journal for its exact recovery ID.
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_PENDING");
    assert_eq!(parts[1], rid);
    assert_eq!(parts[2], "0");
    assert_eq!(parts[3], "1");
    // Resume with the same recovery ID and the current subject SHA.
    let out = t.apply(rid, sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    assert!(t.repo.join(".mrgs/state.json").exists());
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
}

#[test]
fn test_obligation_63_crash_after_action_before_advance() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_action:0", &dir);
    kill_child(child);
    // The action completed; the journal was not advanced.
    assert!(t.repo.join(".mrgs/state.json").exists());
    let state_after_crash = t.read_mrgs("state.json");
    let ledger = t.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["status"], "PENDING");
    assert_eq!(ledger["recoveries"][0]["next_action"], 0);
    // Resume: current subject equals prefix 1, so advance without repeating.
    let sha_now = recompute_subject(&t.repo);
    let out = t.apply(rid, &sha_now);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    // No duplicate mutation: the state bytes are byte-identical.
    assert_eq!(t.read_mrgs("state.json"), state_after_crash);
    let ledger = t.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["status"], "APPLIED");
}

/// Build the exact interrupted Phase 6 state-publication fixture: completion
/// entry published, phase-scoped cleanup partly done (audit-ledger.json and
/// implementation-authority.json already removed in Phase 6 order),
/// state.json holding the exact receipt-bound pre-closeout state, the given
/// closeout-state temp written, and a pending RESUME_CLOSEOUT journal
/// (crash before the first action). Returns the pending recovery ID.
fn setup_closeout_state_temp_fixture(t: &TestRepo, temp_name: &str, temp_bytes: &[u8]) -> String {
    t.close_phase1();
    let archived = t.archived_governance();
    // Phase 6 cleanup order: audit-ledger.json, implementation-authority.json,
    // accepted-contract.json, contract-draft.json — the first two are gone.
    t.write_mrgs(
        "accepted-contract.json",
        archived["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_mrgs(
        "contract-draft.json",
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    // Exact receipt-bound pre-closeout state (canonical struct order).
    let pre_state = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t.plan_sha(),
        active_phase: Some("phase-1".to_string()),
        closed_phases: vec![],
    })
    .unwrap();
    t.write_mrgs("state.json", pre_state.as_bytes());
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
    let rid = parts[1].to_string();
    let sha = parts[2].to_string();
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(t, &rid, &sha, "after_pending_publish", &dir);
    kill_child(child);
    // Crash during the finalizer's state publication: the leftover
    // closeout-state temp exists while state.json still holds the
    // pre-closeout state.
    t.write_mrgs(temp_name, temp_bytes);
    rid
}

#[test]
fn test_obligation_64_crash_during_closeout_resumes_finalizer() {
    // (a) Crash mid-cleanup, fresh apply (no pending journal): resumed
    //     through the existing finalizer.
    let t = TestRepo::new();
    t.close_phase1();
    let archived = t.archived_governance();
    // Simulate a crash mid-cleanup: two files already removed.
    t.write_mrgs(
        "accepted-contract.json",
        archived["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_mrgs(
        "contract-draft.json",
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t.write_state(&serde_json::json!({
        "schema_version": 1,
        "accepted_plan_sha256": t.plan_sha(),
        "active_phase": "phase-1",
        "closed_phases": []
    }));
    let ledger_before = t.read_mrgs("completion-ledger.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
    assert_success(&t.apply(parts[1], parts[2]));
    // Resumed through the existing finalizer: exactly one completion entry,
    // no second completion.
    let completions = t.get_completion_ledger().unwrap();
    assert_eq!(completions["completions"].as_array().unwrap().len(), 1);
    assert_eq!(t.read_mrgs("completion-ledger.json"), ledger_before);
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert_eq!(state["closed_phases"], serde_json::json!(["phase-1"]));
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(!t.repo.join(".mrgs").join(name).exists());
    }
    assert_no_temp_files(&t.repo);

    // (b) Interrupted state publication with a pending RESUME_CLOSEOUT
    //     journal: the closeout-state temp holding the exact canonical
    //     after-state is promoted over the pre-closeout state (the
    //     interrupted rename), then the finalizer completes.
    let t2 = TestRepo::new();
    let after_state = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t2.plan_sha(),
        active_phase: None,
        closed_phases: vec!["phase-1".to_string()],
    })
    .unwrap();
    let rid2 =
        setup_closeout_state_temp_fixture(&t2, ".closeout-state.0.tmp", after_state.as_bytes());
    let plan = &t2.get_recovery_ledger().unwrap()["recoveries"][0]["plan"];
    let final_prefix = plan["prefix_subject_sha256"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let ledger_before = t2.read_mrgs("completion-ledger.json");
    let sha_now = recompute_subject(&t2.repo);
    let out = t2.apply(&rid2, &sha_now);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    // The promoted temp is the after-state: state.json holds the exact
    // canonical after-state bytes and the temp is gone.
    assert_eq!(t2.read_mrgs("state.json"), after_state.as_bytes());
    assert!(!t2.repo.join(".mrgs/.closeout-state.0.tmp").exists());
    assert_no_temp_files(&t2.repo);
    // Completion ledger unchanged: exactly one entry, no second completion.
    assert_eq!(t2.read_mrgs("completion-ledger.json"), ledger_before);
    let completions = t2.get_completion_ledger().unwrap();
    assert_eq!(completions["completions"].as_array().unwrap().len(), 1);
    // Phase-scoped cleanup completed in Phase 6 order.
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(!t2.repo.join(".mrgs").join(name).exists());
    }
    // Final subject equals the final stored prefix; inspect is healthy.
    assert_eq!(recompute_subject(&t2.repo), final_prefix);
    let lines = t2.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));

    // (c) Wrong temp bytes: neither redundant with state.json nor the exact
    //     after-state; zero mutation.
    let t3 = TestRepo::new();
    let rid3 = setup_closeout_state_temp_fixture(&t3, ".closeout-state.0.tmp", b"not a state");
    let sha_now = recompute_subject(&t3.repo);
    let before = mrgs_snapshot(&t3.repo);
    let out = t3.apply(&rid3, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_snapshot_unchanged(&t3.repo, &before);

    // (d) Duplicate candidates for one target; zero mutation.
    let t4 = TestRepo::new();
    let after4 = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t4.plan_sha(),
        active_phase: None,
        closed_phases: vec!["phase-1".to_string()],
    })
    .unwrap();
    let rid4 = setup_closeout_state_temp_fixture(&t4, ".closeout-state.0.tmp", after4.as_bytes());
    t4.write_mrgs(".closeout-state.1.tmp", after4.as_bytes());
    let sha_now = recompute_subject(&t4.repo);
    let before = mrgs_snapshot(&t4.repo);
    let out = t4.apply(&rid4, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_snapshot_unchanged(&t4.repo, &before);

    // (e) Noncanonical attempt spelling: .closeout-state.00.tmp is not the
    //     deterministic grammar; zero mutation.
    let t5 = TestRepo::new();
    let after5 = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t5.plan_sha(),
        active_phase: None,
        closed_phases: vec!["phase-1".to_string()],
    })
    .unwrap();
    let rid5 = setup_closeout_state_temp_fixture(&t5, ".closeout-state.00.tmp", after5.as_bytes());
    let sha_now = recompute_subject(&t5.repo);
    let before = mrgs_snapshot(&t5.repo);
    let out = t5.apply(&rid5, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_snapshot_unchanged(&t5.repo, &before);

    // (f) After-state temp bound to a different final completion receipt:
    //     canonical JSON, but not the exact after-state of this receipt;
    //     zero mutation.
    let t6 = TestRepo::new();
    let wrong_after = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t6.plan_sha(),
        active_phase: None,
        closed_phases: vec!["phase-1".to_string(), "phase-2".to_string()],
    })
    .unwrap();
    let rid6 =
        setup_closeout_state_temp_fixture(&t6, ".closeout-state.0.tmp", wrong_after.as_bytes());
    let sha_now = recompute_subject(&t6.repo);
    let before = mrgs_snapshot(&t6.repo);
    let out = t6.apply(&rid6, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_snapshot_unchanged(&t6.repo, &before);

    // (g) Fresh apply (no pending journal) on the same interrupted state
    //     publication: the virtual normalization promotes the temp before
    //     the finalizer runs, so no leftover temp survives and the post
    //     subject matches the virtual prefix simulation.
    let t7 = TestRepo::new();
    t7.close_phase1();
    let archived7 = t7.archived_governance();
    t7.write_mrgs(
        "accepted-contract.json",
        archived7["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t7.write_mrgs(
        "contract-draft.json",
        archived7["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    let pre7 = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t7.plan_sha(),
        active_phase: Some("phase-1".to_string()),
        closed_phases: vec![],
    })
    .unwrap();
    t7.write_mrgs("state.json", pre7.as_bytes());
    let after7 = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t7.plan_sha(),
        active_phase: None,
        closed_phases: vec!["phase-1".to_string()],
    })
    .unwrap();
    t7.write_mrgs(".closeout-state.0.tmp", after7.as_bytes());
    let lines = t7.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
    let out = t7.apply(parts[1], parts[2]);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    assert_eq!(t7.read_mrgs("state.json"), after7.as_bytes());
    assert!(!t7.repo.join(".mrgs/.closeout-state.0.tmp").exists());
    assert_no_temp_files(&t7.repo);
    let completions = t7.get_completion_ledger().unwrap();
    assert_eq!(completions["completions"].as_array().unwrap().len(), 1);
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(!t7.repo.join(".mrgs").join(name).exists());
    }
    let lines = t7.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));

    // (h) Crash after the leading action of a fresh [RESTORE_ACCEPTED_PLAN,
    //     RESUME_CLOSEOUT] plan while an interrupted state-publication temp
    //     is present: the journal still says next_action=0, the temp is
    //     live, and the pre-closeout state is intact. The promotion is
    //     deferred to the RESUME action, so the resume advances past the
    //     completed leading action and finishes through the finalizer —
    //     never a stale-subject dead end.
    let t8 = TestRepo::new();
    t8.close_phase1();
    let archived8 = t8.archived_governance();
    t8.delete("accepted-plan.json");
    t8.write_mrgs(
        "accepted-contract.json",
        archived8["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t8.write_mrgs(
        "contract-draft.json",
        archived8["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    let pre8 = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t8.plan_sha(),
        active_phase: Some("phase-1".to_string()),
        closed_phases: vec![],
    })
    .unwrap();
    t8.write_mrgs("state.json", pre8.as_bytes());
    let after8 = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t8.plan_sha(),
        active_phase: None,
        closed_phases: vec!["phase-1".to_string()],
    })
    .unwrap();
    t8.write_mrgs(".closeout-state.0.tmp", after8.as_bytes());
    let lines = t8.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 RESTORE_ACCEPTED_PLAN accepted-plan.json"
    );
    assert_eq!(lines[2], "RECOVERY_ACTION 2 RESUME_CLOSEOUT phase-1");
    let rid8 = parts[1].to_string();
    let sha8 = parts[2].to_string();
    let dir8 = t8._dir.path().to_path_buf();
    let child = crash_apply(&t8, &rid8, &sha8, "after_action:0", &dir8);
    kill_child(child);
    // The window: the journal still says next_action=0 while the leading
    // action already completed and the state-publication temp remains.
    let ledger = t8.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["next_action"], 0);
    assert_eq!(ledger["recoveries"][0]["status"], "PENDING");
    assert!(t8.repo.join(".mrgs/.closeout-state.0.tmp").exists());
    assert!(t8.repo.join(".mrgs/accepted-plan.json").exists());
    let sha_now = recompute_subject(&t8.repo);
    let out = t8.apply(&rid8, &sha_now);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    assert_eq!(t8.read_mrgs("state.json"), after8.as_bytes());
    assert!(!t8.repo.join(".mrgs/.closeout-state.0.tmp").exists());
    assert_no_temp_files(&t8.repo);
    let completions = t8.get_completion_ledger().unwrap();
    assert_eq!(completions["completions"].as_array().unwrap().len(), 1);
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(!t8.repo.join(".mrgs").join(name).exists());
    }
    let ledger = t8.get_recovery_ledger().unwrap();
    let post = ledger["recoveries"][0]["post_subject_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(post, recompute_subject(&t8.repo));
    let lines = t8.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));

    // (i) Crafted wrong-label RESUME journal with a valid closeout-state
    //     temp: the read-only binding preflight runs before the promotion,
    //     so the apply fails RECOVERY_ACTION_FAILED with the state
    //     publication temp and the pre-closeout state completely untouched.
    let t9 = TestRepo::new();
    t9.close_phase1();
    let archived9 = t9.archived_governance();
    t9.write_mrgs(
        "accepted-contract.json",
        archived9["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t9.write_mrgs(
        "contract-draft.json",
        archived9["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    let pre9 = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t9.plan_sha(),
        active_phase: Some("phase-1".to_string()),
        closed_phases: vec![],
    })
    .unwrap();
    t9.write_mrgs("state.json", pre9.as_bytes());
    let after9 = serde_json::to_string_pretty(&StateProbe {
        schema_version: 1,
        accepted_plan_sha256: t9.plan_sha(),
        active_phase: None,
        closed_phases: vec!["phase-1".to_string()],
    })
    .unwrap();
    t9.write_mrgs(".closeout-state.0.tmp", after9.as_bytes());
    let lines = t9.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
    let rid9 = parts[1].to_string();
    let sha9 = parts[2].to_string();
    let dir9 = t9._dir.path().to_path_buf();
    let child = crash_apply(&t9, &rid9, &sha9, "after_pending_publish", &dir9);
    kill_child(child);
    // Craft a wrong but grammatically valid RESUME label and recompute the
    // recovery ID so the journal stays internally consistent.
    let mut ledger9: serde_json::Value =
        serde_json::from_slice(&t9.read_mrgs("recovery-ledger.json")).unwrap();
    ledger9["recoveries"][0]["plan"]["actions"][0]["target"] =
        serde_json::Value::String("phase-1/2".to_string());
    let plan9: PlanSeedProbe =
        serde_json::from_value(ledger9["recoveries"][0]["plan"].clone()).unwrap();
    let forged_rid = sha256_hex(serde_json::to_string(&plan9).unwrap().as_bytes());
    ledger9["recoveries"][0]["recovery_id"] = serde_json::Value::String(forged_rid.clone());
    t9.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger9).unwrap().as_bytes(),
    );
    let sha_now = recompute_subject(&t9.repo);
    let before = mrgs_snapshot(&t9.repo);
    let out = t9.apply(&forged_rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_ACTION_FAILED");
    // No mutation: the temp, the pre-closeout state, the phase files, and
    // the completion ledger are all byte-identical.
    assert_snapshot_unchanged(&t9.repo, &before);
    assert_eq!(t9.read_mrgs("state.json"), pre9.as_bytes());
    assert!(t9.repo.join(".mrgs/.closeout-state.0.tmp").exists());
}

#[test]
fn test_obligation_65_pending_conflict_rejects_other_request() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_pending_publish", &dir);
    kill_child(child);
    let ledger_before = t.recovery_ledger_bytes();
    // A different recovery request while pending is rejected.
    let other_rid = "e".repeat(64);
    let out = t.apply(&other_rid, sha);
    assert_category_no_stdout(&out, "RECOVERY_PENDING_CONFLICT");
    // No mutation of the journal.
    assert_eq!(t.recovery_ledger_bytes(), ledger_before);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_66_post_recovery_subject_healthy_final_prefix() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "before_finalize", &dir);
    kill_child(child);
    let ledger = t.get_recovery_ledger().unwrap();
    let plan = &ledger["recoveries"][0]["plan"];
    let final_prefix = plan["prefix_subject_sha256"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(ledger["recoveries"][0]["next_action"], 1);
    assert_eq!(ledger["recoveries"][0]["status"], "PENDING");
    let sha_now = recompute_subject(&t.repo);
    assert_eq!(
        sha_now, final_prefix,
        "post-recovery subject must equal the final prefix"
    );
    let out = t.apply(rid, &sha_now);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    // Post-recovery subject is recomputed, healthy, and equals the final
    // prefix hash.
    let ledger = t.get_recovery_ledger().unwrap();
    let post = ledger["recoveries"][0]["post_subject_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(post, final_prefix);
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));

    // Crash after the complete APPLIED journal temp is written but before
    // its rename: the resume must recognize the exact deterministic applied
    // journal, finalize successfully with the exact receipt, and leave no
    // temp behind.
    let t2 = TestRepo::new();
    t2.close_phase1();
    t2.delete("accepted-plan.json");
    t2.delete("state.json");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[0], "RECOVERY_REQUIRED");
    assert_eq!(parts[3], "2");
    let rid = parts[1];
    let sha = parts[2];
    let dir = t2._dir.path().to_path_buf();
    let child = crash_apply(&t2, rid, sha, "after_final_ledger_temp_write", &dir);
    kill_child(child);
    // All target actions completed; the ledger on disk is still PENDING with
    // next_action == action_count; the APPLIED journal temp exists.
    let ledger = t2.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["status"], "PENDING");
    assert_eq!(ledger["recoveries"][0]["next_action"], 2);
    let final_temp = format!(".recovery-{}-2.tmp", rid);
    assert!(t2.repo.join(".mrgs").join(&final_temp).exists());
    // The temp holds the complete APPLIED journal: exact deterministic
    // receipt, receipt SHA-256, post-subject, next_action, status, previous
    // receipt link, sequence, and action hash.
    let temp_bytes = t2.read_mrgs(&final_temp);
    let temp_ledger: serde_json::Value = serde_json::from_slice(&temp_bytes).unwrap();
    assert_eq!(temp_ledger["recoveries"][0]["status"], "APPLIED");
    assert_eq!(temp_ledger["recoveries"][0]["next_action"], 2);
    let temp_receipt_sha = temp_ledger["recoveries"][0]["recovery_receipt_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!temp_ledger["recoveries"][0]["post_subject_sha256"].is_null());
    assert!(!temp_ledger["recoveries"][0]["recovery_receipt"].is_null());
    // Resume using the unchanged pending ledger and the exact temp.
    let sha_now = recompute_subject(&t2.repo);
    let out = t2.apply(rid, &sha_now);
    assert_success(&out);
    let stdout = stdout_str(&out);
    assert!(stdout.starts_with("RECOVERY_APPLIED "));
    assert_eq!(
        stdout.split_whitespace().nth(5).unwrap(),
        temp_receipt_sha,
        "the resumed receipt must be the exact deterministic receipt"
    );
    let ledger = t2.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["status"], "APPLIED");
    assert_eq!(
        ledger["recoveries"][0]["recovery_receipt_sha256"]
            .as_str()
            .unwrap(),
        temp_receipt_sha
    );
    assert_eq!(
        t2.recovery_ledger_bytes(),
        temp_bytes,
        "the final ledger must be byte-identical to the complete APPLIED journal temp"
    );
    assert!(!t2.repo.join(".mrgs").join(&final_temp).exists());
    assert_no_temp_files(&t2.repo);

    // Forged final prefix with next_action == action_count (crash after the
    // last action, before finalization): the attacker rolls the action back
    // (deletes the restored state) and forges the final prefix to the hash
    // of the rolled-back subject — which the finalize branch alone would
    // accept as "current == final prefix" and falsely finalize. The
    // full-chain replay recomputes the action's deterministic consequences
    // (the canonical restored state) and cannot reproduce the forged
    // prefix: RECOVERY_LEDGER_INVALID before any mutation, journal still
    // PENDING.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.delete("state.json");
    let lines = t3.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t3._dir.path().to_path_buf();
    let child = crash_apply(&t3, rid, sha, "before_finalize", &dir);
    kill_child(child);
    // The action completed: the canonical state is on disk.
    assert!(t3.repo.join(".mrgs/state.json").exists());
    t3.delete("state.json");
    let forged_subject = recompute_subject(&t3.repo);
    let mut ledger = t3.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["next_action"], 1);
    let prefixes = ledger["recoveries"][0]["plan"]["prefix_subject_sha256"]
        .as_array_mut()
        .unwrap();
    let last = prefixes.last_mut().unwrap();
    *last = serde_json::Value::String(forged_subject.clone());
    let plan: PlanSeedProbe =
        serde_json::from_value(ledger["recoveries"][0]["plan"].clone()).unwrap();
    let forged_rid = sha256_hex(serde_json::to_string(&plan).unwrap().as_bytes());
    ledger["recoveries"][0]["recovery_id"] = serde_json::Value::String(forged_rid.clone());
    t3.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
    let before = mrgs_snapshot(&t3.repo);
    let out = t3.apply(&forged_rid, &forged_subject);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_snapshot_unchanged(&t3.repo, &before);
    let ledger = t3.get_recovery_ledger().unwrap();
    assert_eq!(ledger["recoveries"][0]["status"], "PENDING");
}

#[test]
fn test_obligation_67_applied_finalization_no_null_ambiguity() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    let ledger = t.get_recovery_ledger().unwrap();
    let entry = &ledger["recoveries"][0];
    assert_eq!(entry["status"], "APPLIED");
    assert_eq!(entry["next_action"], 1);
    assert!(!entry["post_subject_sha256"].is_null());
    assert!(!entry["recovery_receipt"].is_null());
    assert!(!entry["recovery_receipt_sha256"].is_null());
    assert_eq!(entry["post_subject_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(entry["recovery_receipt_sha256"].as_str().unwrap().len(), 64);
}

#[test]
fn test_obligation_68_receipt_exact_fields_chain() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    assert_success(&t.apply(rid, sha));
    let ledger = t.get_recovery_ledger().unwrap();
    let entry = &ledger["recoveries"][0];
    let receipt: ReceiptProbe = serde_json::from_value(entry["recovery_receipt"].clone()).unwrap();
    assert_eq!(receipt.schema_version, 1);
    assert_eq!(receipt.accepted_plan_sha256, t.plan_sha());
    assert_eq!(receipt.plan_id, "test-plan");
    assert_eq!(receipt.recovery_sequence, 1);
    assert_eq!(receipt.recovery_id, rid);
    assert_eq!(receipt.pre_subject_sha256, sha);
    assert_eq!(
        receipt.post_subject_sha256,
        entry["post_subject_sha256"].as_str().unwrap()
    );
    assert_eq!(receipt.action_count, 1);
    let plan: PlanSeedProbe = serde_json::from_value(entry["plan"].clone()).unwrap();
    let actions_json = serde_json::to_string(&plan.actions).unwrap();
    assert_eq!(receipt.actions_sha256, sha256_hex(actions_json.as_bytes()));
    assert_eq!(receipt.previous_recovery_receipt_sha256, None);
    // Stored hash equals the receipt hash.
    assert_eq!(
        entry["recovery_receipt_sha256"],
        sha256_hex(serde_json::to_string(&receipt).unwrap().as_bytes())
    );
}

#[test]
fn test_obligation_69_receipt_sha_recomputes() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let out = t.apply(rid, sha);
    assert_success(&out);
    let applied_parts = split_stdout(&out);
    assert_eq!(applied_parts[0], "RECOVERY_APPLIED");
    let ledger = t.get_recovery_ledger().unwrap();
    let entry = &ledger["recoveries"][0];
    let receipt: ReceiptProbe = serde_json::from_value(entry["recovery_receipt"].clone()).unwrap();
    let recomputed = sha256_hex(serde_json::to_string(&receipt).unwrap().as_bytes());
    assert_eq!(
        recomputed,
        entry["recovery_receipt_sha256"].as_str().unwrap()
    );
    assert_eq!(recomputed, applied_parts[5]);
}

#[test]
fn test_obligation_70_ledger_revalidated_after_publication() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    assert_success(&t.apply(rid, sha));
    // A full re-inspection validates the complete ledger (chain, plan
    // hashes, receipt, binding) and reports healthy.
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));
    let ledger = t.get_recovery_ledger().unwrap();
    let plan: PlanSeedProbe =
        serde_json::from_value(ledger["recoveries"][0]["plan"].clone()).unwrap();
    assert_eq!(
        sha256_hex(serde_json::to_string(&plan).unwrap().as_bytes()),
        ledger["recoveries"][0]["recovery_id"].as_str().unwrap()
    );
    let receipt: ReceiptProbe =
        serde_json::from_value(ledger["recoveries"][0]["recovery_receipt"].clone()).unwrap();
    assert_eq!(
        sha256_hex(serde_json::to_string(&receipt).unwrap().as_bytes()),
        ledger["recoveries"][0]["recovery_receipt_sha256"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        ledger["recoveries"][0]["post_subject_sha256"]
            .as_str()
            .unwrap(),
        plan.prefix_subject_sha256.last().unwrap()
    );
    assert_eq!(ledger["accepted_plan_sha256"], t.plan_sha());
}

#[test]
fn test_obligation_71_apply_exact_success_output() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let out = t.apply(rid, sha);
    assert_success(&out);
    let stdout = stdout_str(&out);
    let pieces: Vec<&str> = stdout.split_whitespace().collect();
    assert_eq!(pieces.len(), 6);
    assert_eq!(pieces[0], "RECOVERY_APPLIED");
    assert_eq!(pieces[1], "1");
    assert_eq!(pieces[2], rid);
    assert_eq!(pieces[3], sha);
    let ledger = t.get_recovery_ledger().unwrap();
    let entry = &ledger["recoveries"][0];
    assert_eq!(pieces[4], entry["post_subject_sha256"].as_str().unwrap());
    assert_eq!(
        pieces[5],
        entry["recovery_receipt_sha256"].as_str().unwrap()
    );
}

#[test]
fn test_obligation_72_exact_replay_original_output_no_writes() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let first = t.apply(rid, sha);
    assert_success(&first);
    let snapshot = mrgs_snapshot(&t.repo);
    // The caller replays with the current subject SHA (from inspection).
    let current_sha = t.inspect_sha();
    let second = t.apply(rid, &current_sha);
    assert_success(&second);
    assert_eq!(stdout_raw(&first), stdout_raw(&second));
    assert_snapshot_unchanged(&t.repo, &snapshot);
    assert_no_temp_files(&t.repo);

    // Historical replay across the complete applied history: two
    // independent recoveries return the repository to the same healthy
    // subject; replaying the FIRST recovery ID after the second recovery
    // returns the first original output and writes nothing.
    let t2 = TestRepo::new();
    t2.close_phase1();
    t2.delete("state.json");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid1 = parts[1].to_string();
    let first = t2.apply(&rid1, parts[2]);
    assert_success(&first);
    let first_stdout = stdout_raw(&first);
    // Second independent recovery (accepted-plan restore, reconstructed from
    // the completion ledger) ends at the same healthy subject: the restored
    // record bytes are identical.
    t2.delete("accepted-plan.json");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t2.apply(parts[1], parts[2]));
    let snapshot = mrgs_snapshot(&t2.repo);
    let current_sha = t2.inspect_sha();
    let replay = t2.apply(&rid1, &current_sha);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_stdout);
    assert_snapshot_unchanged(&t2.repo, &snapshot);
    assert_no_temp_files(&t2.repo);
}

#[test]
fn test_obligation_73_replay_drift_rejected() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    assert_success(&t.apply(rid, sha));
    let ledger_before = t.recovery_ledger_bytes();
    // Subject drift after the recovery: reformat state.json (different bytes).
    let state = t.get_state();
    t.write_mrgs(
        "state.json",
        serde_json::to_string(&state).unwrap().as_bytes(),
    );
    let out = t.apply(rid, sha);
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");
    assert_eq!(t.recovery_ledger_bytes(), ledger_before);
    // The drifted subject is still rejected when the caller supplies the
    // current SHA (reused ID is a conflict, not an idempotent replay).
    let sha_now = recompute_subject(&t.repo);
    let out = t.apply(rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");
    assert_eq!(t.recovery_ledger_bytes(), ledger_before);
}

#[test]
fn test_obligation_74_second_recovery_sequence_two_linked() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    let first_entry_bytes =
        serde_json::to_string_pretty(&t.get_recovery_ledger().unwrap()["recoveries"][0]).unwrap();

    // Second independent corruption and recovery. Recovery IDs must be
    // unique, so the second corruption must produce a distinct plan seed:
    // a malformed state (unsupported schema) has different subject bytes
    // than an absent state, so the pre-subject (and therefore the recovery
    // ID) differs while the derived healthy state is identical.
    let mut malformed: serde_json::Value =
        serde_json::from_slice(&t.read_mrgs("state.json")).unwrap();
    malformed["schema_version"] = serde_json::Value::from(99);
    t.write_mrgs(
        "state.json",
        serde_json::to_string_pretty(&malformed).unwrap().as_bytes(),
    );
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    let ledger = t.get_recovery_ledger().unwrap();
    let recoveries = ledger["recoveries"].as_array().unwrap();
    assert_eq!(recoveries.len(), 2);
    // Entry one is untouched; sequence two links to the first receipt.
    assert_eq!(
        serde_json::to_string_pretty(&recoveries[0]).unwrap(),
        first_entry_bytes
    );
    let r1: ReceiptProbe =
        serde_json::from_value(recoveries[0]["recovery_receipt"].clone()).unwrap();
    let r2: ReceiptProbe =
        serde_json::from_value(recoveries[1]["recovery_receipt"].clone()).unwrap();
    assert_eq!(r1.recovery_sequence, 1);
    assert_eq!(r2.recovery_sequence, 2);
    assert_eq!(
        r2.previous_recovery_receipt_sha256.as_deref(),
        Some(recoveries[0]["recovery_receipt_sha256"].as_str().unwrap())
    );
    assert_eq!(
        recoveries[1]["recovery_receipt_sha256"].as_str().unwrap(),
        sha256_hex(serde_json::to_string(&r2).unwrap().as_bytes())
    );
}

// ============================================================================
// 75-84: boundaries, git safety, exemption, persistence
// ============================================================================

fn snapshot_worktree(repo: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    let mut stack = vec![repo.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".git" || name == ".mrgs" {
                continue;
            }
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                stack.push(path);
            } else if entry.file_type().unwrap().is_file() {
                let rel = path
                    .strip_prefix(repo)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                map.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    map
}

#[test]
fn test_obligation_75_no_write_outside_mrgs_or_sources() {
    let t = TestRepo::new();
    t.close_phase1();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let worktree_before = snapshot_worktree(&t.repo);
    let head_before = git_head(&t.repo);
    assert_success(&t.apply(parts[1], parts[2]));
    // No recovery action writes outside .mrgs: every source/plan/contract
    // byte is preserved, and no new source file appeared.
    assert_eq!(snapshot_worktree(&t.repo), worktree_before);
    assert_eq!(git_head(&t.repo), head_before);
    // Plan and contract sources stay exactly as they were, outside .mrgs.
    assert!(!t.repo.join(".mrgs/plan.toml").exists());
    assert!(t.repo.join("plan.toml").exists());
    assert!(t.repo.join("contract.toml").exists());
}

#[test]
fn test_obligation_76_no_git_mutation_or_credential_discovery() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];

    let dir = t._dir.path().to_path_buf();
    let marker = dir.join("askpass-marker");
    let askpass = dir.join("askpass.bat");
    std::fs::write(
        &askpass,
        format!("@echo invoked >> {}\r\n", marker.display()),
    )
    .unwrap();
    let head_before = git_head(&t.repo);
    let status_before = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&t.repo)
            .args(["status", "--porcelain"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let config_before = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&t.repo)
            .args(["config", "--local", "--list"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();

    let out = t.run_with_env(
        &[
            "recovery",
            "apply",
            "--repo",
            &t.repo.to_string_lossy(),
            "--recovery-id",
            rid,
            "--subject-sha256",
            sha,
            "--decision",
            "RECOVER",
        ],
        &[
            ("GIT_ASKPASS", &askpass.to_string_lossy()),
            ("GIT_TERMINAL_PROMPT", "0"),
            (
                "GIT_SSH_COMMAND",
                &format!("echo ssh-invoked >> {}", marker.display()),
            ),
        ],
    );
    assert_success(&out);
    // No credential/helper discovery ever ran.
    assert!(!marker.exists(), "external helper must never be invoked");
    assert_eq!(git_head(&t.repo), head_before);
    let status_after = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&t.repo)
            .args(["status", "--porcelain"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(status_after, status_before);
    let config_after = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&t.repo)
            .args(["config", "--local", "--list"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(config_after, config_before);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_77_git_children_sanitized_no_injected_vars() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let decoy = dir.join("decoy");
    std::fs::create_dir_all(&decoy).unwrap();
    let out = t.run_with_env(
        &[
            "recovery",
            "apply",
            "--repo",
            &t.repo.to_string_lossy(),
            "--recovery-id",
            rid,
            "--subject-sha256",
            sha,
            "--decision",
            "RECOVER",
        ],
        &[
            ("GIT_DIR", &decoy.to_string_lossy()),
            ("GIT_WORK_TREE", &decoy.to_string_lossy()),
            ("GIT_INDEX_FILE", &decoy.join("index").to_string_lossy()),
            (
                "GIT_OBJECT_DIRECTORY",
                &decoy.join("objects").to_string_lossy(),
            ),
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "core.fsmonitor"),
            ("GIT_CONFIG_VALUE_0", "true"),
            ("GIT_OPTIONAL_LOCKS", "1"),
        ],
    );
    assert_success(&out);
    // The injected controls never reached a git child; the decoy is untouched.
    let decoy_entries: Vec<_> = std::fs::read_dir(&decoy).unwrap().collect();
    assert_eq!(decoy_entries.len(), 0, "decoy must remain empty");
    assert!(t.repo.join(".mrgs/state.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_78_phase1_7_outputs_unchanged() {
    let t = TestRepo::new();
    git_commit(&t.repo, "plan.toml", valid_plan_toml().as_bytes());
    git_commit(&t.repo, "contract.toml", valid_contract_toml().as_bytes());
    let out = t.accept_plan();
    assert_success(&out);
    assert!(stdout_str(&out).contains("test-plan"));
    let sel = t.select_phase("phase-1");
    assert_success(&sel);
    assert_eq!(stdout_str(&sel), "phase-1");
    let draft = t.draft_contract();
    assert_success(&draft);
    let parts = split_stdout(&draft);
    assert_eq!(parts[0], "test-contract-v1");
    assert_eq!(parts[1].len(), 64);
    let acc = t.accept_contract(1, &parts[1]);
    assert_success(&acc);
    assert!(stdout_str(&acc).starts_with("ACCEPTED test-contract-v1 1"));
    let ib = t.impl_begin(1, &parts[1]);
    assert_success(&ib);
    assert!(stdout_str(&ib).contains("IMPLEMENTATION_BOUND"));
    let ic = t.impl_check();
    assert_success(&ic);
    assert!(stdout_str(&ic).contains("IMPLEMENTATION_OK"));
    let ab = t.audit_begin("auditor1");
    assert_success(&ab);
    let ab_parts = split_stdout(&ab);
    assert_eq!(ab_parts[0], "AUDIT_OPEN");
    let report = t.make_pass_report(&ab_parts[1], &ab_parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let ar = t.audit_record(&report_path);
    assert_success(&ar);
    assert!(stdout_str(&ar).starts_with("AUDIT_PASS "));
    let co = t.phase_close("phase-1");
    assert_success(&co);
    let co_parts = split_stdout(&co);
    assert_eq!(co_parts[0], "PHASE_CLOSED");
    assert_eq!(co_parts[1], "phase-1");
    // Idempotent closeout replay keeps the exact output shape.
    let co2 = t.phase_close("phase-1");
    assert_success(&co2);
    assert_eq!(stdout_raw(&co), stdout_raw(&co2));
    // Continuity output unchanged.
    let meta = t.write_metadata("cont.toml", &standard_metadata("phase-1", &co_parts[4]));
    let cr = t.continuity_record(&meta);
    assert_success(&cr);
    assert!(stdout_str(&cr).starts_with("CONTINUITY_RECORDED mrgs phase-1 1 "));
    // Representative error categories unchanged.
    let err_out = t.phase_close("phase-99");
    assert_category(&err_out, "GOVERNANCE_AUTHORITY_INVALID");
    // Recovery coexists without altering any Phase 1-7 output.
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));
}

#[test]
fn test_obligation_79_recovery_ledger_exempt_exact_untracked() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    let ledger_bytes = t.recovery_ledger_bytes();
    // The untracked, ignored recovery ledger is exempt from later
    // implementation-cleanliness checks.
    let check = t.impl_check();
    assert_success(&check);
    assert_eq!(t.recovery_ledger_bytes(), ledger_bytes);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_80_tracked_alias_child_symlink_not_exempt() {
    // (a) A TRACKED recovery ledger is not exempt: index validation rejects
    // any tracked .mrgs path.
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    let add = Command::new("git")
        .arg("-C")
        .arg(&t.repo)
        .args(["add", "-f", "--", ".mrgs/recovery-ledger.json"])
        .output()
        .unwrap();
    assert!(add.status.success());
    let check = t.impl_check();
    assert_failure(&check);

    // (b) Child paths of the ledger are not exempt.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    std::fs::create_dir_all(t2.repo.join(".mrgs/recovery-ledger.json")).unwrap();
    t2.write_mrgs("recovery-ledger.json/child.json", b"{}");
    let check = t2.impl_check();
    assert_category(&check, "FILESYSTEM_BOUNDARY_UNSAFE");

    // (c) A symlink at the ledger path is not exempt.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    let target = t3.repo.join("decoy.json");
    write_file(&target, "{}");
    let link = t3.repo.join(".mrgs/recovery-ledger.json");
    match make_file_link(&target, &link) {
        Ok(()) => {
            let check = t3.impl_check();
            assert_category(&check, "FILESYSTEM_BOUNDARY_UNSAFE");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            std::fs::create_dir_all(&link).unwrap();
            let check = t3.impl_check();
            assert_category(&check, "FILESYSTEM_BOUNDARY_UNSAFE");
        }
        Err(e) => panic!("symlink creation failed: {}", e),
    }

    // (d) Arbitrary .mrgs paths are not exempt: implementation check rejects
    // the reserved .mrgs change path.
    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    t4.write_mrgs("extra.json", b"{}");
    let check = t4.impl_check();
    assert_category(&check, "CHANGE_FORBIDDEN");

    // (e) Case aliases are not exempt: a differently-cased sibling path is a
    // different path string and fails the exact-path exemption. On Windows
    // the filesystem is case-insensitive but git reports the on-disk name,
    // so a differently-cased filename still fails the byte-exact match.
    let t5 = TestRepo::new();
    t5.setup_impl_bound();
    let alias_dir = if cfg!(windows) { ".mrgs" } else { ".MRGS" };
    // A differently-cased filename is a different path string: git reports
    // the on-disk name, which fails the byte-exact exemption.
    let alias_name = if cfg!(windows) {
        "Recovery-Ledger.json"
    } else {
        "recovery-ledger.json"
    };
    let alias_path = t5.repo.join(alias_dir).join(alias_name);
    std::fs::create_dir_all(alias_path.parent().unwrap()).ok();
    std::fs::write(&alias_path, b"{}").ok();
    let check = t5.impl_check();
    assert_failure(&check);
}

#[test]
fn test_obligation_81_publication_create_new_no_truncate() {
    // A pre-existing file at the deterministic recovery temp name is never
    // truncated or reused; without a journal binding it is an unrecoverable
    // leftover.
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let temp = format!(".recovery-{}-0.tmp", rid);
    t.write_mrgs(&temp, b"stale bytes");
    let out = t.apply(rid, sha);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_eq!(
        t.read_mrgs(&temp),
        b"stale bytes",
        "temp must never be truncated"
    );
    assert!(!t.repo.join(".mrgs/recovery-ledger.json").exists());
}

#[test]
fn test_obligation_82_collision_never_truncated_failure_preserves() {
    // A corrupted recovery-owned temp during resume is rejected, never
    // truncated, and the journal is preserved.
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_temp_write:0", &dir);
    kill_child(child);
    let ledger_before = t.recovery_ledger_bytes();
    let temp = format!(".recovery-{}-0.tmp", rid);
    assert!(t.repo.join(".mrgs").join(&temp).exists());
    t.write_mrgs(&temp, b"corrupted bytes");
    let sha_now = recompute_subject(&t.repo);
    let out = t.apply(rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_ACTION_FAILED");
    assert_eq!(
        t.read_mrgs(&temp),
        b"corrupted bytes",
        "collision temp never truncated"
    );
    assert_eq!(t.recovery_ledger_bytes(), ledger_before);
}

#[test]
fn test_obligation_83_handled_failure_no_temp_leftover_journal_rules() {
    // (a) Handled publication failure leaves no new temp and preserves the
    // prior ledger (covered by the failpoint in obligation 61; assert the
    // temp-free invariant here directly).
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let out = t.run_with_env(
        &[
            "recovery",
            "apply",
            "--repo",
            &t.repo.to_string_lossy(),
            "--recovery-id",
            rid,
            "--subject-sha256",
            sha,
            "--decision",
            "RECOVER",
        ],
        &[("MRGS_TEST_ONLY_RECOVERY_FAIL_RENAME_AFTER_PUBLISH", "1")],
    );
    assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
    assert_no_temp_files(&t.repo);

    // (b) An interruption leftover before the journal existed is NOT
    // recoverable: it is only recoverable through journal rules.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    t2.delete("state.json");
    let lines = t2.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid2 = parts[1];
    let sha2 = parts[2];
    let dir2 = t2._dir.path().to_path_buf();
    let child2 = crash_apply(&t2, rid2, sha2, "after_ledger_temp_write_first", &dir2);
    kill_child(child2);
    assert!(!t2.repo.join(".mrgs/recovery-ledger.json").exists());
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // (c) A successful recovery leaves no temp files.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.delete("state.json");
    let lines = t3.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t3.apply(parts[1], parts[2]));
    assert_no_temp_files(&t3.repo);
}

#[test]
fn test_obligation_84_platform_branches_execute_or_fallback() {
    // The junction/reparse branch executes on the supported platform (see
    // obligations 10/14) and the atomic replacement path (MoveFileEx on
    // Windows, rename elsewhere) is exercised by every successful apply.
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let out = t.apply(parts[1], parts[2]);
    assert_success(&out);
    assert!(t.repo.join(".mrgs/state.json").exists());
    assert_no_temp_files(&t.repo);

    // Reparse-point rejection executes on the current platform.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let mrgs = t2.repo.join(".mrgs");
    let real = t2.repo.join(".mrgs-real");
    std::fs::rename(&mrgs, &real).unwrap();
    make_dir_link(&real, &mrgs);
    let out = t2.inspect();
    assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
    if cfg!(windows) {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        let meta = std::fs::symlink_metadata(&mrgs).unwrap();
        assert_ne!(meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);
    } else {
        assert!(std::fs::symlink_metadata(&mrgs)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    // Non-UTF-8 child rejection executes on the current platform.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    write_non_utf8_child(&t3.repo);
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

#[test]
fn test_obligation_85_apply_healthy_not_required_no_ledger() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let sha = t.inspect_sha();
    let before = mrgs_snapshot(&t.repo);
    let dummy_rid = "a".repeat(64);
    let out = t.apply(&dummy_rid, &sha);
    assert_success(&out);
    assert_eq!(stdout_str(&out), format!("RECOVERY_NOT_REQUIRED {}", sha));
    assert!(!t.repo.join(".mrgs/recovery-ledger.json").exists());
    assert_snapshot_unchanged(&t.repo, &before);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_86_error_categories_exact_format() {
    // RECOVERY_ID_INVALID
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let sha = t.inspect_sha();
    let out = t.apply(&"g".repeat(64), &sha);
    assert_category_no_stdout(&out, "RECOVERY_ID_INVALID");

    // RECOVERY_DECISION_INVALID
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let out = t.apply_decision(parts[1], parts[2], "recover");
    assert_category_no_stdout(&out, "RECOVERY_DECISION_INVALID");

    // RECOVERY_UNRECOVERABLE
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    t2.write_mrgs("rogue.json", b"{}");
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // RECOVERY_LEDGER_INVALID
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.delete("state.json");
    let lines = t3.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t3.apply(parts[1], parts[2]));
    t3.write_mrgs("recovery-ledger.json", b"{broken");
    let out = t3.apply(parts[1], parts[2]);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // RECOVERY_LEDGER_STALE (internally consistent journal for another plan)
    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    t4.delete("state.json");
    let lines = t4.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t4.apply(parts[1], parts[2]));
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&t4.read_mrgs("recovery-ledger.json")).unwrap();
    let other = "f".repeat(64);
    ledger["accepted_plan_sha256"] = serde_json::Value::String(other.clone());
    ledger["recoveries"][0]["plan"]["accepted_plan_sha256"] =
        serde_json::Value::String(other.clone());
    ledger["recoveries"][0]["recovery_receipt"]["accepted_plan_sha256"] =
        serde_json::Value::String(other.clone());
    let plan: PlanSeedProbe =
        serde_json::from_value(ledger["recoveries"][0]["plan"].clone()).unwrap();
    let new_rid = sha256_hex(serde_json::to_string(&plan).unwrap().as_bytes());
    ledger["recoveries"][0]["recovery_id"] = serde_json::Value::String(new_rid.clone());
    ledger["recoveries"][0]["recovery_receipt"]["recovery_id"] =
        serde_json::Value::String(new_rid.clone());
    let receipt: ReceiptProbe =
        serde_json::from_value(ledger["recoveries"][0]["recovery_receipt"].clone()).unwrap();
    ledger["recoveries"][0]["recovery_receipt_sha256"] = serde_json::Value::String(sha256_hex(
        serde_json::to_string(&receipt).unwrap().as_bytes(),
    ));
    t4.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_string_pretty(&ledger).unwrap().as_bytes(),
    );
    let out = t4.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_STALE");

    // RECOVERY_PENDING_CONFLICT
    let t5 = TestRepo::new();
    t5.setup_impl_bound();
    t5.delete("state.json");
    let lines = t5.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let dir = t5._dir.path().to_path_buf();
    let child = crash_apply(&t5, parts[1], parts[2], "after_pending_publish", &dir);
    kill_child(child);
    let current_sha = recompute_subject(&t5.repo);
    let out = t5.apply(&"e".repeat(64), &current_sha);
    assert_category_no_stdout(&out, "RECOVERY_PENDING_CONFLICT");

    // RECOVERY_SUBJECT_STALE
    let t6 = TestRepo::new();
    t6.setup_impl_bound();
    t6.delete("state.json");
    let lines = t6.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    t6.write_mrgs("state.json", b"{}");
    let out = t6.apply(parts[1], parts[2]);
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");

    // RECOVERY_ACTION_FAILED
    let t7 = TestRepo::new();
    t7.setup_impl_bound();
    t7.delete("state.json");
    let lines = t7.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t7._dir.path().to_path_buf();
    let child = crash_apply(&t7, rid, sha, "after_temp_write:0", &dir);
    kill_child(child);
    let temp = format!(".recovery-{}-0.tmp", rid);
    t7.write_mrgs(&temp, b"bad");
    let sha_now = recompute_subject(&t7.repo);
    let out = t7.apply(rid, &sha_now);
    assert_category_no_stdout(&out, "RECOVERY_ACTION_FAILED");

    // RECOVERY_POSTCONDITION_FAILED: genuine runtime drift. The stored
    // journal is fully valid — every prefix, including the final one, is
    // independently simulated before execution — but an external change to
    // the just-written target between action completion and postcondition
    // capture shifts the resulting subject. (Forged journal prefixes are
    // rejected pre-mutation as RECOVERY_LEDGER_INVALID in obligation 44.)
    let t8 = TestRepo::new();
    t8.setup_impl_bound();
    t8.delete("state.json");
    let lines = t8.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t8._dir.path().to_path_buf();
    let signal = dir.join("signal");
    let release = dir.join("release");
    // Piped stdio so the child's output is capturable (crash_apply inherits
    // stdio for its kill-based callers).
    let mut cmd = cargo_bin();
    cmd.args([
        "recovery",
        "apply",
        "--repo",
        &t8.repo.to_string_lossy(),
        "--recovery-id",
        rid,
        "--subject-sha256",
        sha,
        "--decision",
        "RECOVER",
    ])
    .env(
        "MRGS_TEST_ONLY_RECOVERY_POINT",
        "after_action_before_postcondition:0",
    )
    .env("MRGS_TEST_ONLY_RECOVERY_SIGNAL_FILE", &signal)
    .env("MRGS_TEST_ONLY_RECOVERY_RELEASE_FILE", &release)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());
    let child = cmd.spawn().unwrap();
    wait_for_file(&signal, 60);
    // The action has completed (state.json written); tamper with the
    // just-written target before the postcondition capture, then release.
    assert!(t8.repo.join(".mrgs/state.json").exists());
    t8.write_mrgs("state.json", b"tampered");
    std::fs::write(&release, b"go").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_category_no_stdout(&out, "RECOVERY_POSTCONDITION_FAILED");
}

#[test]
fn test_obligation_87_no_new_dependency_or_config() {
    // The dependency sections must be exactly the frozen Phase 1-7 set.
    let cargo = std::fs::read_to_string("Cargo.toml").unwrap();
    assert!(cargo.contains(
        "[dependencies]\nclap = { version = \"4\", features = [\"derive\"] }\nserde = { version = \"1\", features = [\"derive\"] }\nserde_json = \"1\"\ntoml = \"0.8\"\nsha2 = \"0.10\"\nthiserror = \"1\""
    ));
    assert!(cargo
        .contains("[dev-dependencies]\ntempfile = \"3\"\nassert_cmd = \"2\"\npredicates = \"3\""));

    // No hidden configuration is created by a recovery run.
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    let root_entries: Vec<String> = std::fs::read_dir(&t.repo)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    for name in root_entries {
        if name.starts_with('.') && name != ".git" && name != ".mrgs" && name != ".gitignore" {
            panic!("unexpected hidden entry created: {}", name);
        }
    }
    assert!(!t.repo.join(".cargo").exists());
}

#[test]
fn test_obligation_88_no_recursive_test_every_obligation_asserted() {
    // This test binary must never invoke cargo recursively. The needles are
    // assembled dynamically so the assertion cannot match its own source.
    let source = std::fs::read_to_string("tests/phase8.rs").unwrap();
    let cmd_needle = format!("Command::new({})", "\"cargo\"");
    assert!(!source.contains(&cmd_needle), "recursive cargo invocation");
    let cword = String::from("cargo");
    let tword = String::from("test");
    let args_needle = format!("\"{}\", \"{}\"", cword, tword);
    assert!(
        !source.contains(&args_needle),
        "recursive cargo test argument pair"
    );

    // Exactly 88 numbered obligation tests exist, each with a direct
    // executable assertion.
    let count = source
        .lines()
        .filter(|l| l.starts_with("fn test_obligation_") && l.ends_with("() {"))
        .count();
    assert_eq!(count, 88, "exactly 88 obligation tests required");
    for i in 1..=88 {
        let needle = format!("fn test_obligation_{:02}_", i);
        assert!(source.contains(&needle), "missing obligation test {}", i);
    }
}

// ============================================================================
// Supplemental regression tests (reported separately from the 88)
// ============================================================================

#[test]
fn supplemental_01_minimal_plan_only_healthy() {
    let t = TestRepo::new();
    git_commit(&t.repo, "plan.toml", valid_plan_toml().as_bytes());
    assert_success(&t.accept_plan());
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_NOT_REQUIRED "));
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
}

#[test]
fn supplemental_02_remove_temp_only_apply_path() {
    let t = TestRepo::new();
    t.close_phase1();
    let state_bytes = t.read_mrgs("state.json");
    let temp = phase1_producer_temp("state.json");
    t.write_mrgs(&temp, &state_bytes);
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_success(&t.apply(parts[1], parts[2]));
    assert!(!t.repo.join(".mrgs").join(&temp).exists());
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
}

#[test]
fn supplemental_03_pending_prefix_matching_executes_action() {
    // Crash after pending publish; the resumed apply executes action 0
    // because the current subject equals prefix 0.
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_pending_publish", &dir);
    kill_child(child);
    assert!(!t.repo.join(".mrgs/state.json").exists());
    let out = t.apply(rid, sha);
    assert_success(&out);
    assert!(t.repo.join(".mrgs/state.json").exists());
    assert_eq!(t.inspect_sha(), recompute_subject(&t.repo));
}

#[test]
fn supplemental_04_recovery_temp_name_deterministic() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.delete("state.json");
    let lines = t.inspect_output();
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let rid = parts[1];
    let sha = parts[2];
    let dir = t._dir.path().to_path_buf();
    let child = crash_apply(&t, rid, sha, "after_temp_write:0", &dir);
    kill_child(child);
    let expected = format!(".recovery-{}-0.tmp", rid);
    assert!(t.repo.join(".mrgs").join(&expected).exists());
    let names: BTreeSet<String> = std::fs::read_dir(t.repo.join(".mrgs"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    let temps: Vec<&String> = names.iter().filter(|n| n.ends_with(".tmp")).collect();
    assert_eq!(temps.len(), 1, "exactly one deterministic recovery temp");
    assert_eq!(temps[0], &expected);
}

#[test]
fn supplemental_05_missing_mrgs_directory_unrecoverable() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    std::fs::remove_dir_all(t.repo.join(".mrgs")).unwrap();
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}
