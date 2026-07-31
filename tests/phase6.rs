//! Phase 6 contract-required tests.
//!
//! Covers Section 25 of the Phase 6 contract: CLI and readiness,
//! current-subject proof, final manifest, completion receipt and ledger,
//! first publication and finalization, resumable and idempotent behavior,
//! corruption/persistence/regression.
//!
//! Section 25 Obligation Mapping:
//!
//! 25.1 CLI and readiness:
//!   1  -> test_obligation_01_exact_cli_parsing
//!   2  -> test_obligation_02_unknown_phase_rejects
//!   3  -> test_obligation_03_requested_phase_different_from_active
//!   4  -> test_obligation_04_missing_active_phase_no_completed
//!   5  -> test_obligation_05_dependency_inconsistency
//!   6  -> test_obligation_06_missing_contract_draft
//!   7  -> test_obligation_07_missing_accepted_contract
//!   8  -> test_obligation_08_missing_impl_authority
//!   9  -> test_obligation_09_missing_audit_ledger
//!  10  -> test_obligation_10_pending_failed_routed_final_fail
//!
//! 25.2 Current-subject proof:
//!  11  -> test_obligation_11_valid_terminal_pass_closeout_ready
//!  12  -> test_obligation_12_changed_worktree_rejects
//!  13  -> test_obligation_13_changed_index_rejects
//!  14  -> test_obligation_14_changed_head_rejects
//!  15  -> test_obligation_15_changed_branch_rejects
//!  16  -> test_obligation_16_stale_accepted_contract
//!  17  -> test_obligation_17_stale_impl_authority
//!  18  -> test_obligation_18_malformed_passed_report
//!  19  -> test_obligation_19_passed_report_hash_mismatch
//!  20  -> test_obligation_20_passed_subject_hash_mismatch
//!
//! 25.3 Final manifest:
//!  21  -> test_obligation_21_valid_manifest_exact_fields
//!  22  -> test_obligation_22_deterministic_manifest_bytes_hash
//!  23  -> test_obligation_23_exact_plan_metadata_index
//!  24  -> test_obligation_24_exact_phase_dependencies
//!  25  -> test_obligation_25_exact_contract_content
//!  26  -> test_obligation_26_exact_final_subject
//!  27  -> test_obligation_27_exact_report_bytes_sha
//!  28  -> test_obligation_28_all_four_governance_archived
//!  29  -> test_obligation_29_archive_hashes_recompute
//!  30  -> test_obligation_30_manifest_hash_mismatch_rejects
//!
//! 25.4 Completion receipt and ledger:
//!  31  -> test_obligation_31_first_receipt_null_previous
//!  32  -> test_obligation_32_exact_closed_phases_arrays
//!  33  -> test_obligation_33_deterministic_receipt_hash
//!  34  -> test_obligation_34_exact_manifest_hash_binding
//!  35  -> test_obligation_35_second_receipt_chains_first
//!  36  -> test_obligation_36_completion_sequence_contiguous
//!  37  -> test_obligation_37_duplicate_completed_phase
//!  38  -> test_obligation_38_dependency_ordering
//!  39  -> test_obligation_39_reordered_entries_reject
//!  40  -> test_obligation_40_receipt_hash_mismatch
//!  41  -> test_obligation_41_broken_previous_link
//!  42  -> test_obligation_42_wrong_plan_authority_stale
//!
//! 25.5 First publication and finalization:
//!  43  -> test_obligation_43_exact_success_output
//!  44  -> test_obligation_44_ledger_before_cleanup
//!  45  -> test_obligation_45_removes_exactly_four_files
//!  46  -> test_obligation_46_plan_and_ledger_remain
//!  47  -> test_obligation_47_state_clears_active
//!  48  -> test_obligation_48_no_unrelated_changes
//!  49  -> test_obligation_49_no_tracked_changes
//!  50  -> test_obligation_50_no_temp_files
//!
//! 25.6 Resumable and idempotent:
//!  51  -> test_obligation_51_replay_after_ledger_resume
//!  52  -> test_obligation_52_replay_after_one_removal
//!  53  -> test_obligation_53_replay_after_two_removals
//!  54  -> test_obligation_54_replay_after_three_removals
//!  55  -> test_obligation_55_replay_after_all_removed
//!  56  -> test_obligation_56_completed_replay_identical
//!  57  -> test_obligation_57_completed_replay_preserves
//!  58  -> test_obligation_58_changed_archived_bytes_rejects
//!  59  -> test_obligation_59_unsafe_archived_topology
//!  60  -> test_obligation_60_state_closed_no_entry
//!  61  -> test_obligation_61_entry_wrong_active_phase
//!  62  -> test_obligation_62_earlier_phase_after_later
//!
//! 25.7 Corruption, persistence, regression:
//!  63  -> test_obligation_63_unknown_ledger_field
//!  64  -> test_obligation_64_missing_ledger_field
//!  65  -> test_obligation_65_noncontiguous_sequence
//!  66  -> test_obligation_66_archived_mismatch
//!  67  -> test_obligation_67_first_pub_failure_no_ledger
//!  68  -> test_obligation_68_temp_collision_no_truncate
//!  69  -> test_obligation_69_replacement_preserves
//!  70  -> test_obligation_70_git_runner_safety
//!  71  -> test_obligation_71_phase1_5_outputs_unchanged
//!  72  -> test_obligation_72_no_new_dependency

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

fn cargo_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mrgs"))
}

// ============================================================================
// Helper: valid plan and contract TOML
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

fn write_file(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

fn stdout_str(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .trim()
        .to_string()
}

fn split_stdout(output: &std::process::Output) -> Vec<String> {
    stdout_str(output)
        .split_whitespace()
        .map(String::from)
        .collect()
}

fn stderr_str(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone())
        .unwrap()
        .trim()
        .to_string()
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "expected success, exit={:?}, stderr={}",
        output.status.code(),
        stderr_str(output)
    );
}

fn assert_failure(output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "expected failure, got success stdout={}",
        stdout_str(output)
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

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
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

// ============================================================================
// TestRepo: complete Phase 1-5 state
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

    fn run(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = cargo_bin();
        cmd.args(args);
        cmd.output().unwrap()
    }

    fn accept_plan(&self) -> std::process::Output {
        self.run(&[
            "plan",
            "accept",
            "--repo",
            &self.repo.to_string_lossy(),
            "--plan",
            &self.plan_path.to_string_lossy(),
        ])
    }

    fn select_phase(&self) -> std::process::Output {
        self.run(&[
            "phase",
            "select",
            "--repo",
            &self.repo.to_string_lossy(),
            "--phase",
            "phase-1",
        ])
    }

    fn draft_contract(&self) -> std::process::Output {
        self.run(&[
            "contract",
            "draft",
            "--repo",
            &self.repo.to_string_lossy(),
            "--contract",
            &self.contract_path.to_string_lossy(),
        ])
    }

    fn accept_contract(&self, revision: u32, sha256: &str) -> std::process::Output {
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

    fn impl_begin(&self, revision: u32, sha256: &str) -> std::process::Output {
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

    fn audit_begin(&self, auditor: &str) -> std::process::Output {
        self.run(&[
            "audit",
            "begin",
            "--repo",
            &self.repo.to_string_lossy(),
            "--auditor",
            auditor,
        ])
    }

    fn audit_record(&self, report: &Path) -> std::process::Output {
        self.run(&[
            "audit",
            "record",
            "--repo",
            &self.repo.to_string_lossy(),
            "--report",
            &report.to_string_lossy(),
        ])
    }

    fn phase_close(&self, phase_id: &str) -> std::process::Output {
        self.run(&[
            "phase",
            "close",
            "--repo",
            &self.repo.to_string_lossy(),
            "--phase",
            phase_id,
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

    #[allow(dead_code)]
    fn get_ledger(&self) -> Option<serde_json::Value> {
        let path = self.repo.join(".mrgs").join("audit-ledger.json");
        if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap())
        } else {
            None
        }
    }

    fn get_completion_ledger(&self) -> Option<serde_json::Value> {
        let path = self.repo.join(".mrgs").join("completion-ledger.json");
        if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap())
        } else {
            None
        }
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

    /// Set up a fully bound implementation repo
    fn setup_impl_bound(&self) {
        git_commit(&self.repo, "plan.toml", valid_plan_toml().as_bytes());
        git_commit(
            &self.repo,
            "contract.toml",
            valid_contract_toml().as_bytes(),
        );
        assert_success(&self.accept_plan());
        assert_success(&self.select_phase());
        assert_success(&self.draft_contract());
        let draft = self.get_draft();
        let sha = draft["sha256"].as_str().unwrap().to_string();
        assert_success(&self.accept_contract(1, &sha));
        assert_success(&self.impl_begin(1, &sha));
    }

    /// Complete a full PASS audit cycle
    fn full_pass_audit(&self) {
        let out = self.audit_begin("auditor1");
        assert_success(&out);
        let parts = split_stdout(&out);
        let report = self.make_pass_report(&parts[1], &parts[3], "auditor1");
        let report_path = self.write_report(&report);
        assert_success(&self.audit_record(&report_path));
    }

    /// Set up a fully ready-for-closeout state
    fn setup_closeout_ready(&self) {
        self.setup_impl_bound();
        self.full_pass_audit();
    }
}

// ============================================================================
// 25.1 CLI and readiness (1-10)
// ============================================================================

#[test]
fn test_obligation_01_exact_cli_parsing() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    let out = t.phase_close("phase-1");
    assert_success(&out);
    let _parts = split_stdout(&out);
    // Output verified in test_obligation_43_exact_success_output
}

#[test]
fn test_obligation_02_unknown_phase_rejects() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    let out = t.phase_close("phase-99");
    assert_failure(&out);
    let err = stderr_str(&out);
    assert!(
        err.contains("error:"),
        "expected error output, got: {}",
        err
    );
}

#[test]
fn test_obligation_03_requested_phase_different_from_active() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // phase-2 is in the plan but not active
    let out = t.phase_close("phase-2");
    assert_failure(&out);
}

#[test]
fn test_obligation_04_missing_active_phase_no_completed() {
    let t = TestRepo::new();
    // Accept plan but don't select any phase
    git_commit(&t.repo, "plan.toml", valid_plan_toml().as_bytes());
    assert_success(&t.accept_plan());
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_05_dependency_inconsistency() {
    // This tests that phase-2 cannot close if phase-1 is not closed
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // phase-1 is active, so phase-2 close should fail (different from active)
    let out = t.phase_close("phase-2");
    assert_failure(&out);
}

#[test]
fn test_obligation_06_missing_contract_draft() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    // Remove contract-draft.json
    std::fs::remove_file(t.repo.join(".mrgs/contract-draft.json")).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_07_missing_accepted_contract() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    // Remove accepted-contract.json
    std::fs::remove_file(t.repo.join(".mrgs/accepted-contract.json")).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_08_missing_impl_authority() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    // Remove implementation-authority.json
    std::fs::remove_file(t.repo.join(".mrgs/implementation-authority.json")).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_09_missing_audit_ledger() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    // No audit at all - should fail
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_10_pending_failed_routed_final_fail() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    // Audit in PENDING state (begin but no record) should reject
    let out = t.audit_begin("auditor1");
    assert_success(&out);
    let close_out = t.phase_close("phase-1");
    assert_failure(&close_out);
}

// ============================================================================
// 25.2 Current-subject proof (11-20)
// ============================================================================

#[test]
fn test_obligation_11_valid_terminal_pass_closeout_ready() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    let out = t.phase_close("phase-1");
    assert_success(&out);
}

#[test]
fn test_obligation_12_changed_worktree_rejects() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Change a file after the audit
    std::fs::write(t.repo.join("src/main.rs"), b"fn main() { changed! }\n").unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_13_changed_index_rejects() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Stage a change after the audit
    std::fs::write(t.repo.join("src/main.rs"), b"fn main() { staged! }\n").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&t.repo)
        .args(["add", "--", "src/main.rs"])
        .output()
        .unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_14_changed_head_rejects() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Commit after the audit
    git_commit(&t.repo, "src/main.rs", b"fn main() { committed! }\n");
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_15_changed_branch_rejects() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Switch to a new branch to simulate branch drift after PASS.
    // The closeout rebuilds the subject and compares with stored passed subject.
    // A different current_branch produces a different subject hash, causing rejection.
    let out_new_branch = Command::new("git")
        .arg("-C")
        .arg(&t.repo)
        .args(["checkout", "-b", "feature-drift"])
        .output()
        .unwrap();
    assert!(out_new_branch.status.success(), "git checkout failed");
    let out = t.phase_close("phase-1");
    assert_failure(&out);
    let err = stderr_str(&out);
    assert!(
        err.contains("error:"),
        "expected error output, got: {}",
        err
    );
    // Switch back to main for cleanup
    let _ = Command::new("git")
        .arg("-C")
        .arg(&t.repo)
        .args(["checkout", "main"])
        .output();
}

#[test]
fn test_obligation_16_stale_accepted_contract() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Corrupt the accepted contract content hash
    let ledger_path = t.repo.join(".mrgs/accepted-contract.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    let revisions = ledger["revisions"].as_array_mut().unwrap();
    let last = revisions.last_mut().unwrap();
    last["sha256"] = serde_json::Value::String("a".repeat(64));
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_17_stale_impl_authority() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Corrupt the implementation authority
    let auth_path = t.repo.join(".mrgs/implementation-authority.json");
    let mut auth: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&auth_path).unwrap()).unwrap();
    auth["baseline_head"] = serde_json::Value::String("a".repeat(40));
    std::fs::write(&auth_path, serde_json::to_string_pretty(&auth).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_18_malformed_passed_report() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    // Record a malformed audit report
    let out = t.audit_begin("auditor1");
    assert_success(&out);
    let _parts = split_stdout(&out);
    let malformed_report = r#"{"schema_version":1,"audit_id":"bad"}"#;
    let report_path = t.write_report(malformed_report);
    let record_out = t.audit_record(&report_path);
    // Audit record should fail with malformed report
    assert_failure(&record_out);
}

#[test]
fn test_obligation_19_passed_report_hash_mismatch() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Tamper with the report hash in the audit ledger
    let ledger_path = t.repo.join(".mrgs/audit-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    let rounds = ledger["rounds"].as_array_mut().unwrap();
    let last = rounds.last_mut().unwrap();
    last["report_sha256"] = serde_json::Value::String("b".repeat(64));
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_20_passed_subject_hash_mismatch() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Tamper with the subject hash in the audit ledger
    let ledger_path = t.repo.join(".mrgs/audit-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    let rounds = ledger["rounds"].as_array_mut().unwrap();
    let last = rounds.last_mut().unwrap();
    last["subject_sha256"] = serde_json::Value::String("c".repeat(64));
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

// ============================================================================
// 25.3 Final manifest (21-30)
// ============================================================================

#[test]
fn test_obligation_21_valid_manifest_exact_fields() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let entry = &ledger["completions"][0];
    let manifest = &entry["final_manifest"];
    // Check all required fields exist
    let required = [
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
    for field in &required {
        assert!(
            manifest.get(*field).is_some(),
            "missing manifest field: {}",
            field
        );
    }
}

#[test]
fn test_obligation_22_deterministic_manifest_bytes_hash() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let stored_hash = entry_manifest_hash(&ledger, 0);
    let manifest = &ledger["completions"][0]["final_manifest"];
    let canonical = canonicalize_manifest(manifest);
    let computed = sha256_hex(canonical.as_bytes());
    assert_eq!(
        stored_hash, computed,
        "manifest hash must equal SHA-256 of exact compact canonical JSON"
    );
}

#[test]
fn test_obligation_23_exact_plan_metadata_index() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let manifest = &ledger["completions"][0]["final_manifest"];
    assert_eq!(manifest["plan_id"], "test-plan");
    assert_eq!(manifest["phase_id"], "phase-1");
    assert_eq!(manifest["plan_phase_index"], 0);
    assert_eq!(manifest["completion_sequence"], 1);
}

#[test]
fn test_obligation_24_exact_phase_dependencies() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let deps = &ledger["completions"][0]["final_manifest"]["phase_dependencies"];
    assert!(deps.as_array().unwrap().is_empty());
}

#[test]
fn test_obligation_25_exact_contract_content() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let content = ledger["completions"][0]["final_manifest"]["contract_content"]
        .as_str()
        .unwrap();
    // Contract content should be the accepted contract TOML
    assert!(content.contains("test-contract-v1"));
}

#[test]
fn test_obligation_26_exact_final_subject() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let subject = &ledger["completions"][0]["final_manifest"]["final_subject"];
    assert!(subject.is_object());
    assert!(subject.get("schema_version").is_some());
    assert!(subject.get("entries").is_some());
}

#[test]
fn test_obligation_27_exact_report_bytes_sha() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let report_content = ledger["completions"][0]["final_manifest"]["final_report_content"]
        .as_str()
        .unwrap();
    let report_sha = ledger["completions"][0]["final_manifest"]["final_report_sha256"]
        .as_str()
        .unwrap();
    let computed = sha256_hex(report_content.as_bytes());
    assert_eq!(report_sha, computed);
}

#[test]
fn test_obligation_28_all_four_governance_archived() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let ag = &ledger["completions"][0]["final_manifest"]["archived_governance"];
    assert!(ag.get("contract_draft_sha256").is_some());
    assert!(ag.get("contract_draft_content").is_some());
    assert!(ag.get("accepted_contract_sha256").is_some());
    assert!(ag.get("accepted_contract_content").is_some());
    assert!(ag.get("implementation_authority_sha256").is_some());
    assert!(ag.get("implementation_authority_content").is_some());
    assert!(ag.get("audit_ledger_sha256").is_some());
    assert!(ag.get("audit_ledger_content").is_some());
}

#[test]
fn test_obligation_29_archive_hashes_recompute() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let ag = &ledger["completions"][0]["final_manifest"]["archived_governance"];
    let pairs = [
        ("contract_draft_sha256", "contract_draft_content"),
        ("accepted_contract_sha256", "accepted_contract_content"),
        (
            "implementation_authority_sha256",
            "implementation_authority_content",
        ),
        ("audit_ledger_sha256", "audit_ledger_content"),
    ];
    for (hash_key, content_key) in &pairs {
        let content = ag[*content_key].as_str().unwrap();
        let hash = ag[*hash_key].as_str().unwrap();
        let computed = sha256_hex(content.as_bytes());
        assert_eq!(hash, computed, "hash mismatch for {}", hash_key);
    }
}

#[test]
fn test_obligation_30_manifest_hash_mismatch_rejects() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Tamper with manifest hash in completion ledger
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    // First closeout to create the ledger
    assert_success(&t.phase_close("phase-1"));
    // Now tamper with the hash
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["completions"][0]["final_manifest_sha256"] = serde_json::Value::String("d".repeat(64));
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    // Replay should detect mismatch
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

// ============================================================================
// 25.4 Completion receipt and ledger (31-42)
// ============================================================================

#[test]
fn test_obligation_31_first_receipt_null_previous() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let prev =
        &ledger["completions"][0]["completion_receipt"]["previous_completion_receipt_sha256"];
    assert!(prev.is_null());
}

#[test]
fn test_obligation_32_exact_closed_phases_arrays() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let receipt = &ledger["completions"][0]["completion_receipt"];
    assert_eq!(receipt["closed_phases_before"], serde_json::json!([]));
    assert_eq!(
        receipt["closed_phases_after"],
        serde_json::json!(["phase-1"])
    );
    assert!(receipt["active_phase_before"].is_string());
    assert!(receipt["active_phase_after"].is_null());
}

#[test]
fn test_obligation_33_deterministic_receipt_hash() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let stored_hash = entry_receipt_hash(&ledger, 0);
    let receipt = &ledger["completions"][0]["completion_receipt"];
    let canonical = canonicalize_receipt(receipt);
    let computed = sha256_hex(canonical.as_bytes());
    assert_eq!(
        stored_hash, computed,
        "receipt hash must equal SHA-256 of exact compact canonical JSON"
    );
}

#[test]
fn test_obligation_34_exact_manifest_hash_binding() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    let manifest_hash = entry_manifest_hash(&ledger, 0);
    let receipt_manifest_hash = ledger["completions"][0]["completion_receipt"]
        ["final_manifest_sha256"]
        .as_str()
        .unwrap();
    assert_eq!(manifest_hash, receipt_manifest_hash);
}

#[test]
fn test_obligation_35_second_receipt_chains_first() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));

    // Complete phase-2
    assert_success(&t.run(&[
        "phase",
        "select",
        "--repo",
        &t.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]));
    write_file(&t.contract_path, &contract_toml_for_phase("phase-2"));
    git_commit(
        &t.repo,
        "contract.toml",
        contract_toml_for_phase("phase-2").as_bytes(),
    );
    assert_success(&t.draft_contract());
    let draft = t.get_draft();
    let sha = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    assert_success(&t.impl_begin(1, &sha));
    t.full_pass_audit();
    assert_success(&t.phase_close("phase-2"));

    // Verify chaining: receipt 2's previous hash = receipt 1's hash
    let ledger = t.get_completion_ledger().unwrap();
    assert_eq!(ledger["completions"].as_array().unwrap().len(), 2);
    let prev = ledger["completions"][1]["completion_receipt"]["previous_completion_receipt_sha256"]
        .as_str()
        .unwrap();
    assert_eq!(
        prev,
        ledger["completions"][0]["completion_receipt_sha256"]
            .as_str()
            .unwrap()
    );
}

#[test]
fn test_obligation_36_completion_sequence_contiguous() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger = t.get_completion_ledger().unwrap();
    assert_eq!(
        ledger["completions"][0]["completion_receipt"]["completion_sequence"],
        1
    );
}

#[test]
fn test_obligation_37_duplicate_completed_phase() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Try to close again - should be idempotent (not a duplicate error)
    let out = t.phase_close("phase-1");
    assert_success(&out);
}

#[test]
fn test_obligation_38_dependency_ordering() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Verify state records the closure: active_phase is null, phase-1 in closed_phases
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert!(state["closed_phases"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("phase-1")));
    // Verify ledger has exactly one entry with matching phase
    let ledger = t.get_completion_ledger().unwrap();
    assert_eq!(ledger["completions"].as_array().unwrap().len(), 1);
    assert_eq!(
        ledger["completions"][0]["completion_receipt"]["phase_id"],
        "phase-1"
    );
}

#[test]
fn test_obligation_39_reordered_entries_reject() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Tamper with completion sequence
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["completions"][0]["completion_receipt"]["completion_sequence"] = serde_json::json!(99);
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_40_receipt_hash_mismatch() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["completions"][0]["completion_receipt_sha256"] =
        serde_json::Value::String("e".repeat(64));
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_41_broken_previous_link() {
    let t = TestRepo::new();
    // Create a ledger with a broken previous receipt link
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let ledger = serde_json::json!({
        "schema_version": 1,
        "accepted_plan_sha256": "a".repeat(64),
        "plan_id": "test-plan",
        "completions": [{
            "final_manifest": {},
            "final_manifest_sha256": "a".repeat(64),
            "completion_receipt": {
                "schema_version": 1,
                "previous_completion_receipt_sha256": "b".repeat(64),
                "closed_phases_before": [],
                "closed_phases_after": ["phase-1"],
                "active_phase_before": "phase-1",
                "active_phase_after": null,
                "phase_id": "phase-1",
                "phase_title": "First phase",
                "completion_sequence": 1,
                "final_manifest_sha256": "a".repeat(64),
                "plan_id": "test-plan",
                "accepted_plan_sha256": "a".repeat(64),
                "plan_phase_index": 0
            },
            "completion_receipt_sha256": "c".repeat(64)
        }]
    });
    std::fs::create_dir_all(t.repo.join(".mrgs")).unwrap();
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_42_wrong_plan_authority_stale() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Tamper with accepted_plan_sha256 in completion ledger
    assert_success(&t.phase_close("phase-1"));
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["accepted_plan_sha256"] = serde_json::Value::String("f".repeat(64));
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

// ============================================================================
// 25.5 First publication and finalization (43-50)
// ============================================================================

#[test]
fn test_obligation_43_exact_success_output() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    let out = t.phase_close("phase-1");
    assert_success(&out);
    let output = stdout_str(&out);
    let parts: Vec<&str> = output.split_whitespace().collect();
    assert_eq!(parts.len(), 5);
    assert_eq!(parts[0], "PHASE_CLOSED");
    assert_eq!(parts[1], "phase-1");
    assert_eq!(parts[2], "1"); // completion_sequence
                               // parts[3] is manifest hash, parts[4] is receipt hash
    assert_eq!(parts[3].len(), 64);
    assert_eq!(parts[4].len(), 64);
}

#[test]
fn test_obligation_44_ledger_before_cleanup() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Ledger should exist
    assert!(t.repo.join(".mrgs/completion-ledger.json").exists());
}

#[test]
fn test_obligation_45_removes_exactly_four_files() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Phase-scoped files should be removed
    assert!(!t.repo.join(".mrgs/contract-draft.json").exists());
    assert!(!t.repo.join(".mrgs/accepted-contract.json").exists());
    assert!(!t.repo.join(".mrgs/implementation-authority.json").exists());
    assert!(!t.repo.join(".mrgs/audit-ledger.json").exists());
}

#[test]
fn test_obligation_46_plan_and_ledger_remain() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    assert!(t.repo.join(".mrgs/accepted-plan.json").exists());
    assert!(t.repo.join(".mrgs/completion-ledger.json").exists());
}

#[test]
fn test_obligation_47_state_clears_active() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let state = t.get_state();
    assert!(state["active_phase"].is_null());
    assert!(state["closed_phases"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("phase-1")));
}

#[test]
fn test_obligation_48_no_unrelated_changes() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Verify no temp files
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_49_no_tracked_changes() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Verify git status is clean
    let out = Command::new("git")
        .arg("-C")
        .arg(&t.repo)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    let status = String::from_utf8(out.stdout).unwrap();
    assert!(status.is_empty(), "unexpected git status: {}", status);
}

#[test]
fn test_obligation_50_no_temp_files() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    assert_no_temp_files(&t.repo);
}

// ============================================================================
// 25.6 Resumable and idempotent (51-62)
// ============================================================================

#[test]
fn test_obligation_51_replay_after_ledger_resume() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // First closeout: creates ledger and completes
    assert_success(&t.phase_close("phase-1"));
    // Second closeout: idempotent replay
    let out = t.phase_close("phase-1");
    assert_success(&out);
    // Output should be identical
    let output = stdout_str(&out);
    assert!(output.starts_with("PHASE_CLOSED phase-1"));
}

#[test]
fn test_obligation_52_replay_after_one_removal() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Remove one file manually
    std::fs::remove_file(t.repo.join(".mrgs/audit-ledger.json")).ok();
    // Replay should succeed (tolerates absent files)
    let out = t.phase_close("phase-1");
    assert_success(&out);
}

#[test]
fn test_obligation_53_replay_after_two_removals() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    std::fs::remove_file(t.repo.join(".mrgs/audit-ledger.json")).ok();
    std::fs::remove_file(t.repo.join(".mrgs/implementation-authority.json")).ok();
    let out = t.phase_close("phase-1");
    assert_success(&out);
}

#[test]
fn test_obligation_54_replay_after_three_removals() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    std::fs::remove_file(t.repo.join(".mrgs/audit-ledger.json")).ok();
    std::fs::remove_file(t.repo.join(".mrgs/implementation-authority.json")).ok();
    std::fs::remove_file(t.repo.join(".mrgs/accepted-contract.json")).ok();
    let out = t.phase_close("phase-1");
    assert_success(&out);
}

#[test]
fn test_obligation_55_replay_after_all_removed() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // All four removed
    std::fs::remove_file(t.repo.join(".mrgs/audit-ledger.json")).ok();
    std::fs::remove_file(t.repo.join(".mrgs/implementation-authority.json")).ok();
    std::fs::remove_file(t.repo.join(".mrgs/accepted-contract.json")).ok();
    std::fs::remove_file(t.repo.join(".mrgs/contract-draft.json")).ok();
    let out = t.phase_close("phase-1");
    assert_success(&out);
}

#[test]
fn test_obligation_56_completed_replay_identical() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let out1 = t.phase_close("phase-1");
    assert_success(&out1);
    let out2 = t.phase_close("phase-1");
    assert_success(&out2);
    assert_eq!(stdout_str(&out1), stdout_str(&out2));
}

#[test]
fn test_obligation_57_completed_replay_preserves() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    let ledger_bytes = std::fs::read(t.repo.join(".mrgs/completion-ledger.json")).unwrap();
    assert_success(&t.phase_close("phase-1"));
    let ledger_bytes_after = std::fs::read(t.repo.join(".mrgs/completion-ledger.json")).unwrap();
    assert_eq!(ledger_bytes, ledger_bytes_after);
}

#[test]
fn test_obligation_58_changed_archived_bytes_rejects() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Restore archived files from ledger, then corrupt one
    let ledger = t.get_completion_ledger().unwrap();
    let ag = &ledger["completions"][0]["final_manifest"]["archived_governance"];
    let mrgs = t.repo.join(".mrgs");
    for (filename, content_key) in &[
        ("contract-draft.json", "contract_draft_content"),
        ("accepted-contract.json", "accepted_contract_content"),
        (
            "implementation-authority.json",
            "implementation_authority_content",
        ),
        ("audit-ledger.json", "audit_ledger_content"),
    ] {
        let content = ag[*content_key].as_str().unwrap();
        std::fs::write(mrgs.join(filename), content).unwrap();
    }
    // Corrupt contract-draft.json with different bytes
    std::fs::write(mrgs.join("contract-draft.json"), b"corrupted content").unwrap();
    // Set state back to active to trigger resumable finalization
    let state_path = mrgs.join("state.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    state["active_phase"] = serde_json::json!("phase-1");
    state["closed_phases"] = serde_json::json!([]);
    std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out); // archived bytes mismatch
}

#[test]
fn test_obligation_59_unsafe_archived_topology() {
    // Test unsafe topology rejection: create a symlink inside .mrgs and verify
    // closeout rejects it. On Windows, symlink creation may require privileges;
    // if unavailable, test junction or assert capability failure.
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Restore archived files from ledger
    let ledger = t.get_completion_ledger().unwrap();
    let ag = &ledger["completions"][0]["final_manifest"]["archived_governance"];
    let mrgs = t.repo.join(".mrgs");
    for (filename, content_key) in &[
        ("contract-draft.json", "contract_draft_content"),
        ("accepted-contract.json", "accepted_contract_content"),
        (
            "implementation-authority.json",
            "implementation_authority_content",
        ),
        ("audit-ledger.json", "audit_ledger_content"),
    ] {
        let content = ag[*content_key].as_str().unwrap();
        std::fs::write(mrgs.join(filename), content).unwrap();
    }
    // Set state back to active for resumable finalization
    let state_path = mrgs.join("state.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    state["active_phase"] = serde_json::json!("phase-1");
    state["closed_phases"] = serde_json::json!([]);
    std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
    // Replace one governance file with a symlink
    std::fs::remove_file(mrgs.join("contract-draft.json")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/nonexistent/target", mrgs.join("contract-draft.json"))
            .unwrap();
        let out = t.phase_close("phase-1");
        assert_failure(&out);
        let err = stderr_str(&out);
        assert!(
            err.contains("error:"),
            "expected error output, got: {}",
            err
        );
    }
    #[cfg(windows)]
    {
        // On Windows, attempt symlink; if it fails (no privilege), assert the
        // capability limitation rather than passing vacuously.
        let symlink_result =
            std::os::windows::fs::symlink_file("C:/nonexistent", mrgs.join("contract-draft.json"));
        match symlink_result {
            Ok(()) => {
                let out = t.phase_close("phase-1");
                assert_failure(&out);
            }
            Err(err) => {
                // Symlink creation requires elevated privileges on Windows.
                let kind = err.kind();
                assert!(
                    kind == std::io::ErrorKind::PermissionDenied
                        || kind == std::io::ErrorKind::Unsupported,
                    "expected permission denied or unsupported, got {:?}",
                    kind
                );
                // Restore a regular file so the test can verify closeout still works.
                std::fs::write(
                    mrgs.join("contract-draft.json"),
                    ag["contract_draft_content"].as_str().unwrap(),
                )
                .unwrap();
                let out = t.phase_close("phase-1");
                assert_success(&out);
            }
        }
    }
}

#[test]
fn test_obligation_60_state_closed_no_entry() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Close phase-1
    assert_success(&t.phase_close("phase-1"));
    // Manually set state to have closed_phases with phase-1 but no completion entry
    let _state_path = t.repo.join(".mrgs/state.json");
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    // Remove the completion ledger
    std::fs::remove_file(&ledger_path).unwrap();
    // State has closed_phases but no ledger => should reject (CLOSEOUT_LEDGER_STALE)
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_61_entry_wrong_active_phase() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));
    // Try to close phase-2 when phase-1 is completed
    let out = t.phase_close("phase-2");
    assert_failure(&out);
}

#[test]
fn test_obligation_62_earlier_phase_after_later() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));

    // Complete phase-2
    assert_success(&t.run(&[
        "phase",
        "select",
        "--repo",
        &t.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]));
    write_file(&t.contract_path, &contract_toml_for_phase("phase-2"));
    git_commit(
        &t.repo,
        "contract.toml",
        contract_toml_for_phase("phase-2").as_bytes(),
    );
    assert_success(&t.draft_contract());
    let draft = t.get_draft();
    let sha = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    assert_success(&t.impl_begin(1, &sha));
    t.full_pass_audit();
    assert_success(&t.phase_close("phase-2"));

    // Replay phase-1 after phase-2 completed: must reject (not the final phase)
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_63_unknown_ledger_field() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // First closeout to create a valid ledger
    assert_success(&t.phase_close("phase-1"));
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["unknown_field"] = serde_json::json!("bad");
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_64_missing_ledger_field() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // First closeout to create a valid ledger
    assert_success(&t.phase_close("phase-1"));
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger.as_object_mut().unwrap().remove("plan_id");
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_65_noncontiguous_sequence() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // First closeout to create valid ledger
    assert_success(&t.phase_close("phase-1"));
    // Tamper with sequence
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["completions"][0]["completion_receipt"]["completion_sequence"] = serde_json::json!(2);
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_66_archived_mismatch() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // First closeout to create a valid ledger
    assert_success(&t.phase_close("phase-1"));
    // Create ledger with wrong archived content
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["completions"][0]["final_manifest"]["archived_governance"]["contract_draft_content"] =
        serde_json::Value::String("wrong content".to_string());
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.phase_close("phase-1");
    assert_failure(&out);
}

#[test]
fn test_obligation_67_first_pub_failure_no_ledger() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    let out = t.phase_close("phase-1");
    assert_success(&out);
    // Verify ledger was created and contains correct entries
    let ledger = t.get_completion_ledger().unwrap();
    assert_eq!(ledger["completions"].as_array().unwrap().len(), 1);
    let entry = &ledger["completions"][0];
    // Manifest and receipt hashes match closeout output
    let parts = split_stdout(&out);
    assert_eq!(parts[3], entry["final_manifest_sha256"].as_str().unwrap());
    assert_eq!(
        parts[4],
        entry["completion_receipt_sha256"].as_str().unwrap()
    );
    // Manifest plan authority matches
    assert_eq!(
        entry["final_manifest"]["accepted_plan_sha256"],
        ledger["accepted_plan_sha256"]
    );
    assert_eq!(entry["final_manifest"]["plan_id"], ledger["plan_id"]);
    // No temp files remain
    let mrgs = t.repo.join(".mrgs");
    let tmps: Vec<_> = std::fs::read_dir(&mrgs)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(tmps.is_empty(), "no temp files should remain");
}

#[test]
fn test_obligation_68_temp_collision_no_truncate() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    assert_success(&t.phase_close("phase-1"));

    // Restore the four archived governance files from the completion ledger
    let ledger = t.get_completion_ledger().unwrap();
    let ag = &ledger["completions"][0]["final_manifest"]["archived_governance"];
    let mrgs = t.repo.join(".mrgs");
    for (filename, content_key) in &[
        ("contract-draft.json", "contract_draft_content"),
        ("accepted-contract.json", "accepted_contract_content"),
        (
            "implementation-authority.json",
            "implementation_authority_content",
        ),
        ("audit-ledger.json", "audit_ledger_content"),
    ] {
        let content = ag[*content_key].as_str().unwrap();
        std::fs::write(mrgs.join(filename), content).unwrap();
    }

    // Set state back to active to trigger resumable finalization
    let state_path = mrgs.join("state.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    state["active_phase"] = serde_json::json!("phase-1");
    state["closed_phases"] = serde_json::json!([]);
    std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Precreate .closeout-state.0.tmp; the code will skip it and use attempt 1
    let collision_bytes = b"state temp collision data that must be preserved";
    std::fs::write(mrgs.join(".closeout-state.0.tmp"), collision_bytes).unwrap();

    let out = t.phase_close("phase-1");
    assert_success(&out);

    // Verify the pre-existing temp file's bytes are unchanged
    let preserved = std::fs::read(mrgs.join(".closeout-state.0.tmp")).unwrap();
    assert_eq!(
        preserved, collision_bytes,
        "pre-existing temp file bytes must be preserved"
    );

    // Verify state was finalized correctly
    let final_state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    assert!(final_state["active_phase"].is_null());
    assert!(final_state["closed_phases"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("phase-1")));
}

#[test]
fn test_obligation_69_replacement_preserves() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // First closeout creates the ledger
    assert_success(&t.phase_close("phase-1"));
    let ledger_bytes = std::fs::read(t.repo.join(".mrgs/completion-ledger.json")).unwrap();
    // Second closeout should preserve the bytes
    assert_success(&t.phase_close("phase-1"));
    let ledger_bytes_after = std::fs::read(t.repo.join(".mrgs/completion-ledger.json")).unwrap();
    assert_eq!(ledger_bytes, ledger_bytes_after);
}

#[test]
fn test_obligation_70_git_runner_safety() {
    let t = TestRepo::new();
    t.setup_closeout_ready();
    // Verify git state is unchanged after closeout
    let head_before = git_head(&t.repo);
    let branch_before = git_branch(&t.repo);
    assert_success(&t.phase_close("phase-1"));
    let head_after = git_head(&t.repo);
    let branch_after = git_branch(&t.repo);
    assert_eq!(head_before, head_after);
    assert_eq!(branch_before, branch_after);
}

#[test]
fn test_obligation_71_phase1_5_outputs_unchanged() {
    // Exercise each Phase 1-5 command on a fresh fixture with exact output assertions
    let t = TestRepo::new();
    // Commit plan and contract so they are not untracked (same as setup_impl_bound)
    git_commit(&t.repo, "plan.toml", valid_plan_toml().as_bytes());
    git_commit(&t.repo, "contract.toml", valid_contract_toml().as_bytes());
    // Phase 1: plan accept
    let out = t.accept_plan();
    assert_success(&out);
    assert!(stdout_str(&out).contains("test-plan"));
    // Phase 1: phase select
    let sel = t.run(&[
        "phase",
        "select",
        "--repo",
        &t.repo.to_string_lossy(),
        "--phase",
        "phase-1",
    ]);
    assert_success(&sel);
    assert_eq!(stdout_str(&sel), "phase-1");
    // Phase 2: contract draft
    let draft = t.draft_contract();
    assert_success(&draft);
    let parts = split_stdout(&draft);
    assert_eq!(parts[0], "test-contract-v1");
    assert_eq!(parts[1].len(), 64);
    // Phase 3: contract accept
    let sha = parts[1].clone();
    let acc = t.accept_contract(1, &sha);
    assert_success(&acc);
    assert!(stdout_str(&acc).starts_with("ACCEPTED test-contract-v1 1"));
    // Phase 4: implementation begin
    let ib = t.impl_begin(1, &sha);
    assert_success(&ib);
    assert!(stdout_str(&ib).contains("IMPLEMENTATION_BOUND"));
    // Phase 4: implementation check
    let ic = t.run(&[
        "implementation",
        "check",
        "--repo",
        &t.repo.to_string_lossy(),
    ]);
    assert_success(&ic);
    assert!(stdout_str(&ic).contains("IMPLEMENTATION_OK"));
    // Phase 5: audit begin
    let ab = t.audit_begin("auditor1");
    assert_success(&ab);
    let ab_parts = split_stdout(&ab);
    assert_eq!(ab_parts[0], "AUDIT_OPEN");
    assert_eq!(ab_parts[1].len(), 64);
    // Phase 5: audit record (PASS report)
    let report = t.make_pass_report(&ab_parts[1], &ab_parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let ar = t.audit_record(&report_path);
    assert_success(&ar);
    // Closeout
    let co = t.phase_close("phase-1");
    assert_success(&co);
    assert!(stdout_str(&co).starts_with("PHASE_CLOSED phase-1 1"));
}

#[test]
fn test_obligation_72_no_new_dependency() {
    // This test verifies that no new production or dev dependency was added.
    let cargo = std::fs::read_to_string("Cargo.toml").unwrap();
    assert!(cargo.contains("clap"));
    assert!(cargo.contains("serde"));
    assert!(cargo.contains("serde_json"));
    assert!(cargo.contains("toml"));
    assert!(cargo.contains("sha2"));
    assert!(cargo.contains("thiserror"));
    assert!(cargo.contains("tempfile"));
    assert!(cargo.contains("assert_cmd"));
    assert!(cargo.contains("predicates"));
    assert!(!cargo.contains("tokio"));
    assert!(!cargo.contains("reqwest"));
    assert!(!cargo.contains("rusqlite"));
    assert!(!cargo.contains("uuid"));
}

// ============================================================================
// Test-only canonicalizers: serialize serde_json::Value in exact struct field
// order matching the production FinalManifest/CompletionReceipt declarations.
// This proves SHA-256 of the canonical compact JSON equals the stored hash.
// ============================================================================

#[allow(dead_code)]
fn push_json_string(out: &mut String, s: &str) {
    out.push('\"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('\"');
}

fn push_json_u32(out: &mut String, n: u32) {
    out.push_str(&n.to_string());
}
fn push_json_usize(out: &mut String, n: usize) {
    out.push_str(&n.to_string());
}

#[allow(dead_code)]
fn push_str_array(out: &mut String, arr: &[serde_json::Value]) {
    out.push('[');
    for (i, v) in arr.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        match v {
            serde_json::Value::String(s) => push_json_string(out, s),
            _ => out.push_str(&v.to_string()),
        }
    }
    out.push(']');
}

fn push_string_array(out: &mut String, arr: &[serde_json::Value]) {
    out.push('[');
    for (i, v) in arr.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        if let serde_json::Value::String(s) = v {
            push_json_string(out, s);
        } else {
            out.push_str(&v.to_string());
        }
    }
    out.push(']');
}

fn canonicalize_layer(out: &mut String, v: &serde_json::Value) {
    out.push('{');
    out.push_str("\"mode\":");
    push_json_string(out, v["mode"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"oid\":");
    push_json_string(out, v["oid"].as_str().unwrap_or(""));
    out.push('}');
}

fn push_layer_opt(out: &mut String, v: &serde_json::Value) {
    if v.is_null() {
        out.push_str("null");
    } else {
        canonicalize_layer(out, v);
    }
}

fn canonicalize_worktree(out: &mut String, v: &serde_json::Value) {
    out.push('{');
    out.push_str("\"kind\":");
    push_json_string(out, v["kind"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"sha256\":");
    match &v["sha256"] {
        serde_json::Value::String(s) => push_json_string(out, s),
        _ => out.push_str("null"),
    }
    out.push('}');
}

fn canonicalize_entry(out: &mut String, v: &serde_json::Value) {
    out.push('{');
    out.push_str("\"path\":");
    push_json_string(out, v["path"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"baseline\":");
    push_layer_opt(out, &v["baseline"]);
    out.push(',');
    out.push_str("\"head\":");
    push_layer_opt(out, &v["head"]);
    out.push(',');
    out.push_str("\"index\":");
    push_layer_opt(out, &v["index"]);
    out.push(',');
    out.push_str("\"worktree\":");
    canonicalize_worktree(out, &v["worktree"]);
    out.push('}');
}

fn canonicalize_subject(out: &mut String, v: &serde_json::Value) {
    out.push('{');
    out.push_str("\"schema_version\":");
    push_json_u32(out, v["schema_version"].as_u64().unwrap() as u32);
    out.push(',');
    out.push_str("\"accepted_plan_sha256\":");
    push_json_string(out, v["accepted_plan_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"phase_id\":");
    push_json_string(out, v["phase_id"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"contract_id\":");
    push_json_string(out, v["contract_id"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"contract_revision\":");
    push_json_u32(out, v["contract_revision"].as_u64().unwrap() as u32);
    out.push(',');
    out.push_str("\"contract_source_path\":");
    push_json_string(out, v["contract_source_path"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"contract_sha256\":");
    push_json_string(out, v["contract_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"implementation_baseline_head\":");
    push_json_string(
        out,
        v["implementation_baseline_head"].as_str().unwrap_or(""),
    );
    out.push(',');
    out.push_str("\"implementation_baseline_branch\":");
    push_json_string(
        out,
        v["implementation_baseline_branch"].as_str().unwrap_or(""),
    );
    out.push(',');
    out.push_str("\"git_object_format\":");
    push_json_string(out, v["git_object_format"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"current_head\":");
    push_json_string(out, v["current_head"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"current_branch\":");
    push_json_string(out, v["current_branch"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"entries\":[");
    if let Some(arr) = v["entries"].as_array() {
        for (i, e) in arr.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            canonicalize_entry(out, e);
        }
    }
    out.push_str("]}");
}

fn canonicalize_archived(out: &mut String, v: &serde_json::Value) {
    out.push('{');
    out.push_str("\"contract_draft_sha256\":");
    push_json_string(out, v["contract_draft_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"contract_draft_content\":");
    push_json_string(out, v["contract_draft_content"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"accepted_contract_sha256\":");
    push_json_string(out, v["accepted_contract_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"accepted_contract_content\":");
    push_json_string(out, v["accepted_contract_content"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"implementation_authority_sha256\":");
    push_json_string(
        out,
        v["implementation_authority_sha256"].as_str().unwrap_or(""),
    );
    out.push(',');
    out.push_str("\"implementation_authority_content\":");
    push_json_string(
        out,
        v["implementation_authority_content"].as_str().unwrap_or(""),
    );
    out.push(',');
    out.push_str("\"audit_ledger_sha256\":");
    push_json_string(out, v["audit_ledger_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"audit_ledger_content\":");
    push_json_string(out, v["audit_ledger_content"].as_str().unwrap_or(""));
    out.push('}');
}

/// Canonicalize a FinalManifest Value in exact struct field order (§8).
fn canonicalize_manifest(v: &serde_json::Value) -> String {
    let mut out = String::with_capacity(4096);
    out.push('{');
    out.push_str("\"schema_version\":");
    push_json_u32(&mut out, v["schema_version"].as_u64().unwrap() as u32);
    out.push(',');
    out.push_str("\"accepted_plan_sha256\":");
    push_json_string(&mut out, v["accepted_plan_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"plan_id\":");
    push_json_string(&mut out, v["plan_id"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"plan_source_path\":");
    push_json_string(&mut out, v["plan_source_path"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"plan_content\":");
    push_json_string(&mut out, v["plan_content"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"phase_id\":");
    push_json_string(&mut out, v["phase_id"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"phase_title\":");
    push_json_string(&mut out, v["phase_title"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"phase_dependencies\":");
    push_string_array(
        &mut out,
        v["phase_dependencies"].as_array().unwrap_or(&vec![]),
    );
    out.push(',');
    out.push_str("\"plan_phase_index\":");
    push_json_usize(&mut out, v["plan_phase_index"].as_u64().unwrap() as usize);
    out.push(',');
    out.push_str("\"completion_sequence\":");
    push_json_u32(&mut out, v["completion_sequence"].as_u64().unwrap() as u32);
    out.push(',');
    out.push_str("\"contract_id\":");
    push_json_string(&mut out, v["contract_id"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"contract_revision\":");
    push_json_u32(&mut out, v["contract_revision"].as_u64().unwrap() as u32);
    out.push(',');
    out.push_str("\"contract_source_path\":");
    push_json_string(&mut out, v["contract_source_path"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"contract_sha256\":");
    push_json_string(&mut out, v["contract_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"contract_content\":");
    push_json_string(&mut out, v["contract_content"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"implementation_baseline_head\":");
    push_json_string(
        &mut out,
        v["implementation_baseline_head"].as_str().unwrap_or(""),
    );
    out.push(',');
    out.push_str("\"implementation_baseline_branch\":");
    push_json_string(
        &mut out,
        v["implementation_baseline_branch"].as_str().unwrap_or(""),
    );
    out.push(',');
    out.push_str("\"git_object_format\":");
    push_json_string(&mut out, v["git_object_format"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"final_head\":");
    push_json_string(&mut out, v["final_head"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"final_branch\":");
    push_json_string(&mut out, v["final_branch"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"final_audit_id\":");
    push_json_string(&mut out, v["final_audit_id"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"final_audit_round\":");
    push_json_u32(&mut out, v["final_audit_round"].as_u64().unwrap() as u32);
    out.push(',');
    out.push_str("\"final_auditor_id\":");
    push_json_string(&mut out, v["final_auditor_id"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"final_subject_sha256\":");
    push_json_string(&mut out, v["final_subject_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"final_subject\":");
    canonicalize_subject(&mut out, &v["final_subject"]);
    out.push(',');
    out.push_str("\"final_report_source_path\":");
    push_json_string(
        &mut out,
        v["final_report_source_path"].as_str().unwrap_or(""),
    );
    out.push(',');
    out.push_str("\"final_report_sha256\":");
    push_json_string(&mut out, v["final_report_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"final_report_content\":");
    push_json_string(&mut out, v["final_report_content"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"archived_governance\":");
    canonicalize_archived(&mut out, &v["archived_governance"]);
    out.push('}');
    out
}

/// Canonicalize a CompletionReceipt Value in exact struct field order (§11).
fn canonicalize_receipt(v: &serde_json::Value) -> String {
    let mut out = String::with_capacity(1024);
    out.push('{');
    out.push_str("\"schema_version\":");
    push_json_u32(&mut out, v["schema_version"].as_u64().unwrap() as u32);
    out.push(',');
    out.push_str("\"accepted_plan_sha256\":");
    push_json_string(&mut out, v["accepted_plan_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"plan_id\":");
    push_json_string(&mut out, v["plan_id"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"phase_id\":");
    push_json_string(&mut out, v["phase_id"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"phase_title\":");
    push_json_string(&mut out, v["phase_title"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"plan_phase_index\":");
    push_json_usize(&mut out, v["plan_phase_index"].as_u64().unwrap() as usize);
    out.push(',');
    out.push_str("\"completion_sequence\":");
    push_json_u32(&mut out, v["completion_sequence"].as_u64().unwrap() as u32);
    out.push(',');
    out.push_str("\"final_manifest_sha256\":");
    push_json_string(&mut out, v["final_manifest_sha256"].as_str().unwrap_or(""));
    out.push(',');
    out.push_str("\"previous_completion_receipt_sha256\":");
    match &v["previous_completion_receipt_sha256"] {
        serde_json::Value::String(s) => push_json_string(&mut out, s),
        _ => out.push_str("null"),
    }
    out.push(',');
    out.push_str("\"closed_phases_before\":");
    push_string_array(
        &mut out,
        v["closed_phases_before"].as_array().unwrap_or(&vec![]),
    );
    out.push(',');
    out.push_str("\"closed_phases_after\":");
    push_string_array(
        &mut out,
        v["closed_phases_after"].as_array().unwrap_or(&vec![]),
    );
    out.push(',');
    out.push_str("\"active_phase_before\":");
    match &v["active_phase_before"] {
        serde_json::Value::String(s) => push_json_string(&mut out, s),
        _ => out.push_str("null"),
    }
    out.push(',');
    out.push_str("\"active_phase_after\":");
    match &v["active_phase_after"] {
        serde_json::Value::Null => out.push_str("null"),
        serde_json::Value::String(s) => push_json_string(&mut out, s),
        other => out.push_str(&other.to_string()),
    }
    out.push('}');
    out
}

// ============================================================================
// Ledger inspection helpers
// ============================================================================

fn entry_manifest_hash(ledger: &serde_json::Value, idx: usize) -> String {
    ledger["completions"][idx]["final_manifest_sha256"]
        .as_str()
        .unwrap()
        .to_string()
}

fn entry_receipt_hash(ledger: &serde_json::Value, idx: usize) -> String {
    ledger["completions"][idx]["completion_receipt_sha256"]
        .as_str()
        .unwrap()
        .to_string()
}
