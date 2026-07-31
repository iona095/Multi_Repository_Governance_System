//! Phase 7 contract-required tests.
//!
//! Covers Section 27 of the Phase 7 contract: CLI, source path, and metadata
//! structure; model and host metadata; target completion binding;
//! cross-repository resolution; manifest, receipt, and ledger; publication,
//! idempotency, and conflicts; safety and Phase 1-6 regression.
//!
//! Section 27 Obligation Mapping:
//!
//! 27.1 CLI, source path, and metadata structure:
//!   1  -> test_obligation_01_exact_cli_parsing_without_sources
//!   2  -> test_obligation_02_repeated_source_repo_parsing
//!   3  -> test_obligation_03_missing_unknown_arguments_reject
//!   4  -> test_obligation_04_metadata_outside_repo_rejects
//!   5  -> test_obligation_05_metadata_under_git_or_mrgs_rejects
//!   6  -> test_obligation_06_unsafe_metadata_topology_rejects
//!   7  -> test_obligation_07_invalid_utf8_metadata_rejects
//!   8  -> test_obligation_08_malformed_toml_rejects
//!   9  -> test_obligation_09_unknown_fields_reject
//!  10  -> test_obligation_10_required_fields_enforced
//!  11  -> test_obligation_11_unsupported_schema_version_rejects
//!  12  -> test_obligation_12_scalar_grammar_enforced
//!
//! 27.2 Model and host metadata:
//!  13  -> test_obligation_13_single_model_host_accepted
//!  14  -> test_obligation_14_multiple_sorted_entries_preserved
//!  15  -> test_obligation_15_zero_models_reject
//!  16  -> test_obligation_16_zero_hosts_reject
//!  17  -> test_obligation_17_unsorted_models_reject
//!  18  -> test_obligation_18_duplicate_models_reject
//!  19  -> test_obligation_19_invalid_model_forms_reject
//!  20  -> test_obligation_20_unsorted_hosts_reject
//!  21  -> test_obligation_21_duplicate_host_ids_reject
//!  22  -> test_obligation_22_invalid_host_forms_reject
//!
//! 27.3 Target completion binding:
//!  23  -> test_obligation_23_closed_phase_exact_receipt_accepted
//!  24  -> test_obligation_24_unknown_phase_rejects
//!  25  -> test_obligation_25_active_or_unclosed_phase_rejects
//!  26  -> test_obligation_26_wrong_completion_receipt_hash_rejects
//!  27  -> test_obligation_27_malformed_completion_ledger_rejects
//!  28  -> test_obligation_28_completion_ledger_other_plan_stale
//!  29  -> test_obligation_29_phase_missing_from_closed_state_rejects
//!  30  -> test_obligation_30_changed_completion_binding_rejects
//!  31  -> test_obligation_31_final_manifest_hash_mismatch_rejects
//!  32  -> test_obligation_32_completion_ordering_chain_rejects
//!
//! 27.4 Cross-repository resolution:
//!  33  -> test_obligation_33_empty_links_zero_sources_accepted
//!  34  -> test_obligation_34_nonempty_links_require_sources
//!  35  -> test_obligation_35_unreferenced_source_rejects
//!  36  -> test_obligation_36_duplicate_source_roots_reject
//!  37  -> test_obligation_37_source_equal_target_rejects
//!  38  -> test_obligation_38_invalid_source_plan_authority_rejects
//!  39  -> test_obligation_39_invalid_source_ledger_state_rejects
//!  40  -> test_obligation_40_source_repository_id_mismatch_rejects
//!  41  -> test_obligation_41_source_plan_sha_mismatch_rejects
//!  42  -> test_obligation_42_missing_source_phase_rejects
//!  43  -> test_obligation_43_source_receipt_mismatch_rejects
//!  44  -> test_obligation_44_source_manifest_chain_mismatch_rejects
//!  45  -> test_obligation_45_omitted_source_continuity_accepted
//!  46  -> test_obligation_46_source_continuity_resolved_exactly
//!  47  -> test_obligation_47_missing_stale_mismatched_source_continuity
//!  48  -> test_obligation_48_link_relation_sorting_uniqueness_resolution
//!
//! 27.5 Manifest, receipt, and ledger:
//!  49  -> test_obligation_49_manifest_exact_fields_and_order
//!  50  -> test_obligation_50_metadata_path_bytes_sha_preserved
//!  51  -> test_obligation_51_note_models_hosts_preserved
//!  52  -> test_obligation_52_resolved_links_exact_no_source_path
//!  53  -> test_obligation_53_manifest_bytes_hash_deterministic
//!  54  -> test_obligation_54_first_receipt_null_previous
//!  55  -> test_obligation_55_later_receipt_chains_previous
//!  56  -> test_obligation_56_continuity_sequence_contiguous
//!  57  -> test_obligation_57_target_sequence_receipt_binding_exact
//!  58  -> test_obligation_58_receipt_bytes_hash_deterministic
//!  59  -> test_obligation_59_ledger_top_level_immutable_repo_id
//!  60  -> test_obligation_60_reordered_entries_reject
//!  61  -> test_obligation_61_duplicate_phase_continuity_id_rejects
//!  62  -> test_obligation_62_broken_hash_binding_chain_rejects
//!
//! 27.6 Publication, idempotency, and conflicts:
//!  63  -> test_obligation_63_first_record_exact_output
//!  64  -> test_obligation_64_first_publication_only_ledger_file
//!  65  -> test_obligation_65_exact_replay_identical_output_bytes
//!  66  -> test_obligation_66_replay_with_links_without_sources
//!  67  -> test_obligation_67_same_id_changed_metadata_rejects
//!  68  -> test_obligation_68_same_phase_different_id_rejects
//!  69  -> test_obligation_69_earlier_sequence_after_later_rejects
//!  70  -> test_obligation_70_temp_collision_no_truncate
//!  71  -> test_obligation_71_replacement_failure_preserves_bytes
//!  72  -> test_obligation_72_no_temp_files_after_success_failure
//!
//! 27.7 Safety and Phase 1-6 regression:
//!  73  -> test_obligation_73_unsafe_target_ledger_topology_rejects
//!  74  -> test_obligation_74_unsafe_source_topology_rejects
//!  75  -> test_obligation_75_boundaries_exempt_exact_untracked_path
//!  76  -> test_obligation_76_no_paths_env_or_host_persisted
//!  77  -> test_obligation_77_phase1_6_preserve_safe_ledger
//!  78  -> test_obligation_78_phase1_6_outputs_unchanged
//!  79  -> test_obligation_79_no_git_mutation_or_observation
//!  80  -> test_obligation_80_no_new_dependency_no_recursive_test

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

fn stdout_str(output: &Output) -> String {
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .trim()
        .to_string()
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

/// Flip the first hex character to a different valid hex value.
fn flip_first_hex_char(s: &str) -> String {
    match s.strip_prefix('a') {
        Some(rest) => format!("b{}", rest),
        None => format!("a{}", &s[1..]),
    }
}

fn mrgs_listing(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(repo.join(".mrgs"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
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
// TestRepo: complete Phase 1-6 state plus Phase 7 continuity support
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

    fn repair_check(&self) -> Output {
        self.run(&["repair", "check", "--repo", &self.repo.to_string_lossy()])
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

    fn continuity_record(&self, metadata: &Path, sources: &[&str]) -> Output {
        let mut args: Vec<String> = vec![
            "continuity".to_string(),
            "record".to_string(),
            "--repo".to_string(),
            self.repo.to_string_lossy().to_string(),
            "--metadata".to_string(),
            metadata.to_string_lossy().to_string(),
        ];
        for s in sources {
            args.push("--source-repo".to_string());
            args.push(s.to_string());
        }
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run(&arg_refs)
    }

    // ---- Governance chain setup ----

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
        let sha = self.get_draft_sha();
        assert_success(&self.accept_contract(1, &sha));
        assert_success(&self.impl_begin(1, &sha));
    }

    fn full_pass_audit(&self) {
        let out = self.audit_begin("auditor1");
        assert_success(&out);
        let parts = split_stdout(&out);
        let report = self.make_pass_report(&parts[1], &parts[3], "auditor1");
        let report_path = self.write_report(&report);
        assert_success(&self.audit_record(&report_path));
    }

    fn setup_closeout_ready(&self) {
        self.setup_impl_bound();
        self.full_pass_audit();
    }

    /// Close phase-1 and return its completion receipt SHA-256.
    fn close_phase1(&self) -> String {
        self.setup_closeout_ready();
        let out = self.phase_close("phase-1");
        assert_success(&out);
        let ledger = self.get_completion_ledger().unwrap();
        ledger["completions"][0]["completion_receipt_sha256"]
            .as_str()
            .unwrap()
            .to_string()
    }

    /// Complete phase-2 through closeout; returns its completion receipt SHA-256.
    fn complete_phase2(&self) -> String {
        assert_success(&self.select_phase("phase-2"));
        write_file(&self.contract_path, &contract_toml_for_phase("phase-2"));
        git_commit(
            &self.repo,
            "contract.toml",
            contract_toml_for_phase("phase-2").as_bytes(),
        );
        assert_success(&self.draft_contract());
        let sha = self.get_draft_sha();
        assert_success(&self.accept_contract(1, &sha));
        assert_success(&self.impl_begin(1, &sha));
        self.full_pass_audit();
        let out = self.phase_close("phase-2");
        assert_success(&out);
        let ledger = self.get_completion_ledger().unwrap();
        ledger["completions"][1]["completion_receipt_sha256"]
            .as_str()
            .unwrap()
            .to_string()
    }

    /// Build a source repo with phase-1 completed and, optionally, a
    /// continuity record (links = []). Returns the repo plus its key hashes.
    fn build_source(record_continuity: bool) -> TestRepo {
        let s = TestRepo::new();
        let receipt = s.close_phase1();
        if record_continuity {
            let meta = s.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
            assert_success(&s.continuity_record(&meta, &[]));
        }
        s
    }

    // ---- Recorded file accessors ----

    fn get_draft_sha(&self) -> String {
        let path = self.repo.join(".mrgs").join("contract-draft.json");
        let draft: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        draft["sha256"].as_str().unwrap().to_string()
    }

    fn get_completion_ledger(&self) -> Option<serde_json::Value> {
        let path = self.repo.join(".mrgs").join("completion-ledger.json");
        if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap())
        } else {
            None
        }
    }

    fn get_continuity_ledger(&self) -> Option<serde_json::Value> {
        let path = self.repo.join(".mrgs").join("continuity-ledger.json");
        if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap())
        } else {
            None
        }
    }

    fn continuity_ledger_bytes(&self) -> Vec<u8> {
        std::fs::read(self.repo.join(".mrgs/continuity-ledger.json")).unwrap()
    }

    fn plan_sha(&self) -> String {
        let path = self.repo.join(".mrgs").join("accepted-plan.json");
        let accepted: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        accepted["sha256"].as_str().unwrap().to_string()
    }

    fn completion_receipt_sha(&self, phase: &str) -> String {
        let ledger = self.get_completion_ledger().unwrap();
        for entry in ledger["completions"].as_array().unwrap() {
            if entry["completion_receipt"]["phase_id"].as_str().unwrap() == phase {
                return entry["completion_receipt_sha256"]
                    .as_str()
                    .unwrap()
                    .to_string();
            }
        }
        panic!("phase {} not in completion ledger", phase);
    }

    fn continuity_receipt_sha(&self, phase: &str) -> String {
        let ledger = self.get_continuity_ledger().unwrap();
        for entry in ledger["entries"].as_array().unwrap() {
            if entry["continuity_receipt"]["phase_id"].as_str().unwrap() == phase {
                return entry["continuity_receipt_sha256"]
                    .as_str()
                    .unwrap()
                    .to_string();
            }
        }
        panic!("phase {} not in continuity ledger", phase);
    }

    fn completion_sequence(&self, phase: &str) -> u32 {
        let ledger = self.get_completion_ledger().unwrap();
        for entry in ledger["completions"].as_array().unwrap() {
            if entry["completion_receipt"]["phase_id"].as_str().unwrap() == phase {
                return entry["completion_receipt"]["completion_sequence"]
                    .as_u64()
                    .unwrap() as u32;
            }
        }
        panic!("phase {} not in completion ledger", phase);
    }

    // ---- Reports ----

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

    fn make_fail_report(&self, audit_id: &str, subject_sha256: &str, auditor_id: &str) -> String {
        let contract: toml::Value = toml::from_str(valid_contract_toml()).unwrap();
        let requirements = contract["requirements"].as_array().unwrap();
        let verification_commands = contract["verification_commands"].as_array().unwrap();

        let req_results: Vec<serde_json::Value> = requirements
            .iter()
            .enumerate()
            .map(|(i, r)| {
                if i == 0 {
                    serde_json::json!({
                        "requirement": r.as_str().unwrap(),
                        "status": "FAIL",
                        "evidence": "not satisfied"
                    })
                } else {
                    serde_json::json!({
                        "requirement": r.as_str().unwrap(),
                        "status": "PASS",
                        "evidence": "ok"
                    })
                }
            })
            .collect();

        let ver_results: Vec<serde_json::Value> = verification_commands
            .iter()
            .map(|c| {
                serde_json::json!({
                    "command": c.as_str().unwrap(),
                    "status": "PASS",
                    "evidence": "ok"
                })
            })
            .collect();

        let report = serde_json::json!({
            "schema_version": 1,
            "audit_id": audit_id,
            "subject_sha256": subject_sha256,
            "auditor_id": auditor_id,
            "independence_declaration": "INDEPENDENT",
            "verdict": "FAIL",
            "summary": "Requirement 1 failed",
            "requirement_results": req_results,
            "verification_results": ver_results,
            "findings": [{
                "id": "F-001",
                "severity": "BLOCKER",
                "claim_kind": "REQUIREMENT",
                "claim_index": 1,
                "summary": "req1 failed",
                "evidence": "no evidence",
                "repair_paths": ["src/main.rs"]
            }]
        });
        serde_json::to_string_pretty(&report).unwrap()
    }

    fn write_report(&self, content: &str) -> PathBuf {
        let path = self.report_dir.join("report.json");
        write_file(&path, content);
        path
    }

    // ---- Metadata files ----

    fn write_metadata(&self, name: &str, content: &str) -> PathBuf {
        let path = self.repo.join(name);
        write_file(&path, content);
        path
    }

    fn commit_file(&self, name: &str, content: &str) {
        git_commit(&self.repo, name, content.as_bytes());
    }
}

// ============================================================================
// Metadata TOML builders
// ============================================================================

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
        receipt_sha = receipt_sha,
    )
}

fn metadata_with_links(phase: &str, receipt_sha: &str, links: &str) -> String {
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

links = [
{links}
]
"#,
        phase = phase,
        receipt_sha = receipt_sha,
        links = links,
    )
}

/// Metadata whose target identity differs from the source identity label.
/// Required for links that carry a source continuity receipt: the link must
/// name the source ledger's repository ID and must not name the target's.
fn metadata_with_links_target_repo(phase: &str, receipt_sha: &str, links: &str) -> String {
    // Replace only the top-level target identity (first occurrence); the link
    // fragments' own repository_id values must survive untouched.
    metadata_with_links(phase, receipt_sha, links).replacen(
        "repository_id = \"mrgs\"",
        "repository_id = \"target-repo\"",
        1,
    )
}

/// link fragment for `links = [...]` (no leading indentation constraints)
fn link_fragment(
    repository_id: &str,
    plan_sha: &str,
    phase: &str,
    receipt_sha: &str,
    continuity_sha: Option<&str>,
) -> String {
    match continuity_sha {
        Some(c) => format!(
            "  {{ relation = \"continues_from\", repository_id = \"{rid}\", accepted_plan_sha256 = \"{ps}\", phase_id = \"{ph}\", completion_receipt_sha256 = \"{rs}\", source_continuity_receipt_sha256 = \"{cs}\" }}",
            rid = repository_id,
            ps = plan_sha,
            ph = phase,
            rs = receipt_sha,
            cs = c,
        ),
        None => format!(
            "  {{ relation = \"continues_from\", repository_id = \"{rid}\", accepted_plan_sha256 = \"{ps}\", phase_id = \"{ph}\", completion_receipt_sha256 = \"{rs}\" }}",
            rid = repository_id,
            ps = plan_sha,
            ph = phase,
            rs = receipt_sha,
        ),
    }
}

// ============================================================================
// Raw JSON helpers for deterministic byte/shape assertions
// ============================================================================

/// Return the byte range of the first `"key": { ... }` object: the start is
/// the opening brace, the end is exclusive after the matching closing brace.
fn json_object_range(text: &str, key: &str) -> (usize, usize) {
    let marker = format!("\"{}\": {{", key);
    let key_pos = text.find(&marker).unwrap_or_else(|| {
        panic!("key {} not found in ledger", key);
    });
    let open = key_pos + marker.find('{').unwrap();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut esc = false;
    for (i, c) in text[open..].char_indices() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
        } else {
            match c {
                '"' => in_str = true,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return (open, open + i + 1);
                    }
                }
                _ => {}
            }
        }
    }
    panic!("unbalanced JSON object for key {}", key);
}

/// Assert that the keys appear exactly in the given order inside the region.
fn assert_key_order_in_region(text: &str, region: (usize, usize), keys: &[&str]) {
    let mut last = region.0;
    for k in keys {
        let needle = format!("\"{}\":", k);
        let search = &text[last..region.1];
        let pos = search.find(&needle).unwrap_or_else(|| {
            panic!("key {} not found after position {} in region", k, last);
        });
        assert!(
            pos + needle.len() <= region.1 - last,
            "key {} spills past region end",
            k
        );
        last += pos;
    }
}

fn manifest_region(ledger_text: &str) -> (usize, usize) {
    json_object_range(ledger_text, "continuity_manifest")
}

fn receipt_region(ledger_text: &str) -> (usize, usize) {
    json_object_range(ledger_text, "continuity_receipt")
}

/// Compact JSON of an object extracted from pretty text: remove whitespace
/// outside strings only. This reproduces serde_json's compact serialization
/// byte-for-byte while preserving the declaration order of the typed struct
/// (a `serde_json::Value` round-trip would reorder keys and is never used).
fn compact_json_of_range(text: &str, region: (usize, usize)) -> String {
    let mut out = String::with_capacity(region.1 - region.0);
    let mut in_str = false;
    let mut esc = false;
    for c in text[region.0..region.1].chars() {
        if in_str {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
        } else {
            match c {
                '"' => {
                    in_str = true;
                    out.push(c);
                }
                c if c.is_whitespace() => {}
                c => out.push(c),
            }
        }
    }
    out
}

// ============================================================================
// 27.1 CLI, source path, and metadata structure (1-12)
// ============================================================================

#[test]
fn test_obligation_01_exact_cli_parsing_without_sources() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_success(&out);
    let parts = split_stdout(&out);
    assert_eq!(parts.len(), 6, "output: {}", stdout_str(&out));
    assert_eq!(parts[0], "CONTINUITY_RECORDED");
    assert_eq!(parts[1], "mrgs");
    assert_eq!(parts[2], "phase-1");
    assert_eq!(parts[3], "1");
    assert_eq!(parts[4].len(), 64);
    assert_eq!(parts[5].len(), 64);
    // Output must not contain any path or metadata content
    assert!(!stdout_str(&out).contains("\\"));
    assert!(!stdout_str(&out).contains("continuity.toml"));
}

#[test]
fn test_obligation_02_repeated_source_repo_parsing() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source_a = TestRepo::build_source(false);
    let source_b = TestRepo::build_source(false);
    let sa_receipt = source_a.completion_receipt_sha("phase-1");
    let sb_receipt = source_b.completion_receipt_sha("phase-1");
    let link_a = link_fragment(
        "source-a",
        &source_a.plan_sha(),
        "phase-1",
        &sa_receipt,
        None,
    );
    let link_b = link_fragment(
        "source-b",
        &source_b.plan_sha(),
        "phase-1",
        &sb_receipt,
        None,
    );
    let meta_text = metadata_with_links("phase-1", &receipt, &format!("{},\n{}", link_a, link_b));
    let meta = t.write_metadata("continuity.toml", &meta_text);
    // Both --source-repo arguments must be preserved; source argument order is
    // non-authoritative (supplied in reverse).
    let out = t.continuity_record(
        &meta,
        &[
            &source_b.repo.to_string_lossy(),
            &source_a.repo.to_string_lossy(),
        ],
    );
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let resolved = ledger["entries"][0]["continuity_manifest"]["resolved_links"]
        .as_array()
        .unwrap();
    assert_eq!(resolved.len(), 2);
    // Metadata link order preserved in resolved links
    assert_eq!(
        resolved[0]["source_repository_id"].as_str().unwrap(),
        "source-a"
    );
    assert_eq!(
        resolved[1]["source_repository_id"].as_str().unwrap(),
        "source-b"
    );
}

#[test]
fn test_obligation_03_missing_unknown_arguments_reject() {
    // Missing --metadata
    let out = cargo_bin()
        .args(["continuity", "record", "--repo", "somewhere"])
        .output()
        .unwrap();
    assert_failure(&out);
    // Unknown flag
    let out = cargo_bin()
        .args([
            "continuity",
            "record",
            "--repo",
            "somewhere",
            "--metadata",
            "m.toml",
            "--bogus",
            "x",
        ])
        .output()
        .unwrap();
    assert_failure(&out);
    // Unknown subcommand under continuity
    let out = cargo_bin()
        .args(["continuity", "begin", "--repo", "somewhere"])
        .output()
        .unwrap();
    assert_failure(&out);
    // Unknown top-level subcommand
    let out = cargo_bin().args(["wat"]).output().unwrap();
    assert_failure(&out);
    assert!(!stderr_str(&out).is_empty());
}

#[test]
fn test_obligation_04_metadata_outside_repo_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let dir = tempfile::TempDir::new().unwrap();
    let outside = dir.path().join("outside.toml");
    write_file(&outside, &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&outside, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    // Nothing published
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_05_metadata_under_git_or_mrgs_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // Under .mrgs
    let under_mrgs = t.write_metadata(".mrgs/meta.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&under_mrgs, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    // Under .git
    let under_git = t.write_metadata(".git/meta.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&under_git, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

/// Create a symlink (or, on capability-unavailable platforms, a directory)
/// at `link` pointing to `target`.
fn make_symlink_or_fallback(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).unwrap();
        true
    }
    #[cfg(windows)]
    {
        // Directory junction: no administrator privilege required.
        let out = Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        if out.status.success() {
            true
        } else {
            // Capability unavailable: fall back to a directory at the path.
            std::fs::create_dir(link).unwrap();
            false
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        std::fs::create_dir(link).unwrap();
        false
    }
}

#[test]
fn test_obligation_06_unsafe_metadata_topology_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let real = t.write_metadata("real.toml", &standard_metadata("phase-1", &receipt));
    let link = t.repo.join("meta-link.toml");
    let created_symlink = make_symlink_or_fallback(&link, &real);
    if created_symlink {
        // Executed platform branch: symlink/junction leaf must reject with the
        // filesystem-boundary category.
        let out = t.continuity_record(&link, &[]);
        assert_category(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
    } else {
        // Capability-unavailable branch with explicit capability assertion and
        // a concrete fallback safety assertion: a directory at the metadata
        // path must also reject.
        assert!(!cfg!(windows) || !link_exists_as_symlink(&link));
        let out = t.continuity_record(&link, &[]);
        assert_failure(&out);
        assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    }
    assert_no_temp_files(&t.repo);
}

#[cfg(windows)]
fn link_exists_as_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn link_exists_as_symlink(_path: &Path) -> bool {
    false
}

#[test]
fn test_obligation_07_invalid_utf8_metadata_rejects() {
    let t = TestRepo::new();
    let _receipt = t.close_phase1();
    let path = t.repo.join("bad.toml");
    std::fs::write(&path, [0xffu8, 0xfe, 0xfd, 0x00]).unwrap();
    let out = t.continuity_record(&path, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_08_malformed_toml_rejects() {
    let t = TestRepo::new();
    let _receipt = t.close_phase1();
    let path = t.write_metadata("bad.toml", "this is {{{ not toml");
    let out = t.continuity_record(&path, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    let path2 = t.write_metadata("bad2.toml", "schema_version = 1\nrepository_id = [1,2]\n");
    let out = t.continuity_record(&path2, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_09_unknown_fields_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let base = standard_metadata("phase-1", &receipt);

    // Unknown top-level field
    let text = format!("{}\nextra_top = 1\n", base);
    let out = t.continuity_record(&t.write_metadata("m1.toml", &text), &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");

    // Unknown model field
    let text = base.replace(
        "execution_mode = \"hosted\", session_label",
        "execution_mode = \"hosted\", session_label, extra_model = 1",
    );
    let out = t.continuity_record(&t.write_metadata("m2.toml", &text), &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");

    // Unknown host field
    let text = base.replace(
        "execution_surface = \"opencode\" }",
        "execution_surface = \"opencode\", extra_host = 1 }",
    );
    let out = t.continuity_record(&t.write_metadata("m3.toml", &text), &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");

    // Unknown link field
    let text = metadata_with_links(
        "phase-1",
        &receipt,
        "  { relation = \"continues_from\", repository_id = \"x\", accepted_plan_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\", phase_id = \"p\", completion_receipt_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\", extra_link = 1 }",
    );
    let out = t.continuity_record(&t.write_metadata("m4.toml", &text), &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");

    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

fn all_zeros_sha() -> &'static str {
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
}

/// Remove a TOML array block (`name = [` through its closing `]`) entirely,
/// leaving otherwise valid TOML so a missing field is the only defect.
fn remove_toml_array_block(text: &str, name: &str) -> String {
    let opener = format!("{} = [", name);
    let mut out_lines: Vec<String> = Vec::new();
    let mut skipping = false;
    let mut removed = false;
    for line in text.lines() {
        if !skipping && line.trim_start().starts_with(&opener) {
            skipping = true;
            removed = true;
            if line.trim_end().ends_with(']') {
                skipping = false;
            }
            continue;
        }
        if skipping {
            if line.trim_start().starts_with(']') {
                skipping = false;
            }
            continue;
        }
        out_lines.push(line.to_string());
    }
    assert!(removed, "array block {} not found", name);
    out_lines.join("\n")
}

#[test]
fn test_obligation_10_required_fields_enforced() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let base = standard_metadata("phase-1", &receipt);

    // Every required top-level field, removed one at a time so the only
    // defect is the missing field (valid TOML otherwise).
    for (label, key) in [
        ("schema_version", "schema_version = 1"),
        ("repository_id", "repository_id = \"mrgs\""),
        ("continuity_id", "continuity_id = \"phase-1-primary\""),
        ("phase_id", "phase_id = \"phase-1\""),
        ("completion_receipt_sha256", ""),
        (
            "note",
            "note = \"Primary governed execution continuity record\"",
        ),
    ] {
        let needle = if label == "completion_receipt_sha256" {
            format!("completion_receipt_sha256 = \"{}\"", receipt)
        } else {
            key.to_string()
        };
        let text = base.replace(&needle, "");
        assert!(
            text != base,
            "field {} replacement did not change text",
            label
        );
        let out = t.continuity_record(&t.write_metadata(&format!("m_{}.toml", label), &text), &[]);
        assert_category(&out, "CONTINUITY_METADATA_INVALID");
    }
    // Array-valued top-level fields, removed as complete blocks.
    for label in ["models", "hosts", "links"] {
        let text = remove_toml_array_block(&base, label);
        let out = t.continuity_record(&t.write_metadata(&format!("m_{}.toml", label), &text), &[]);
        assert_category(&out, "CONTINUITY_METADATA_INVALID");
    }

    // Required nested model fields: remove the exact key/value pair, leaving
    // a valid inline table missing one field.
    let model_removals = [
        "role = \"implementer\", ",
        "provider = \"openai\", ",
        "model_id = \"gpt-5.6\", ",
        "execution_mode = \"hosted\", ",
        ", session_label = \"phase-1-implementation\"",
    ];
    for needle in model_removals {
        let text = base.replace(needle, "");
        assert!(text != base, "model field {} did not change text", needle);
        let out = t.continuity_record(&t.write_metadata("mn.toml", &text), &[]);
        assert_category(&out, "CONTINUITY_METADATA_INVALID");
    }
    // Required nested host fields
    let host_removals = [
        "host_id = \"main-workstation\", ",
        "platform = \"windows\", ",
        "architecture = \"x86_64\", ",
        ", execution_surface = \"opencode\"",
    ];
    for needle in host_removals {
        let text = base.replace(needle, "");
        assert!(text != base, "host field {} did not change text", needle);
        let out = t.continuity_record(&t.write_metadata("mh.toml", &text), &[]);
        assert_category(&out, "CONTINUITY_METADATA_INVALID");
    }
    // Required nested link fields
    let z = all_zeros_sha();
    let link_text = format!(
        "  {{ relation = \"continues_from\", repository_id = \"x\", accepted_plan_sha256 = \"{z}\", phase_id = \"p\", completion_receipt_sha256 = \"{z}\" }}"
    );
    let link_removals = [
        "relation = \"continues_from\", ".to_string(),
        "repository_id = \"x\", ".to_string(),
        format!("accepted_plan_sha256 = \"{z}\", "),
        "phase_id = \"p\", ".to_string(),
        format!(", completion_receipt_sha256 = \"{z}\""),
    ];
    for needle in link_removals {
        let text = metadata_with_links("phase-1", &receipt, &link_text).replace(&needle, "");
        let out = t.continuity_record(&t.write_metadata("ml.toml", &text), &[]);
        assert_category(&out, "CONTINUITY_METADATA_INVALID");
    }

    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_11_unsupported_schema_version_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let text =
        standard_metadata("phase-1", &receipt).replace("schema_version = 1", "schema_version = 2");
    let out = t.continuity_record(&t.write_metadata("m.toml", &text), &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");

    // Ledger schema version is enforced independently: write a valid first
    // ledger, then bump its schema_version and expect the existing-ledger
    // validation to fail closed.
    let meta = t.write_metadata("ok.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta, &[]));
    let ledger_path = t.repo.join(".mrgs/continuity-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["schema_version"] = serde_json::json!(2);
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_LEDGER_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_12_scalar_grammar_enforced() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let base = standard_metadata("phase-1", &receipt);

    let note_overlong = format!("note = \"{}\"", "x".repeat(1025));
    let sha_line = format!("completion_receipt_sha256 = \"{}\"", receipt);
    let sha_upper = sha_line.to_uppercase();
    let sha_short = "completion_receipt_sha256 = \"abc\"".to_string();
    let sha_nonhex = format!(
        "completion_receipt_sha256 = \"{}z{}z{}z\"",
        "0".repeat(20),
        "0".repeat(20),
        "0".repeat(20)
    );
    let model_overlong = format!("model_id = \"{}\"", "a".repeat(257));
    // Each case replaces a present substring with an invalid form.
    let cases: Vec<(&str, &str, &str)> = vec![
        // trimming
        (
            "note trimmed",
            "note = \"Primary governed",
            "note = \" Primary governed",
        ),
        // control characters
        (
            "note control",
            "Primary governed execution continuity record",
            "Primary\\u{0001} governed execution continuity record",
        ),
        // length limits
        (
            "note overlong",
            "note = \"Primary governed execution continuity record\"",
            &note_overlong,
        ),
        (
            "empty note",
            "note = \"Primary governed execution continuity record\"",
            "note = \"\"",
        ),
        (
            "empty repository_id",
            "repository_id = \"mrgs\"",
            "repository_id = \"\"",
        ),
        // token grammar
        (
            "repo slash",
            "repository_id = \"mrgs\"",
            "repository_id = \"mrgs/\"",
        ),
        (
            "repo dash start",
            "repository_id = \"mrgs\"",
            "repository_id = \"-mrgs\"",
        ),
        (
            "repo space",
            "repository_id = \"mrgs\"",
            "repository_id = \"mr gs\"",
        ),
        (
            "repo colon",
            "repository_id = \"mrgs\"",
            "repository_id = \"mrgs:1\"",
        ),
        // extended grammar
        (
            "provider trailing space",
            "provider = \"openai\"",
            "provider = \"openai \"",
        ),
        (
            "provider repeated space",
            "provider = \"openai\"",
            "provider = \"open  ai\"",
        ),
        (
            "model backslash",
            "model_id = \"gpt-5.6\"",
            "model_id = \"gpt\\\\5.6\"",
        ),
        ("model overlong", "model_id = \"gpt-5.6\"", &model_overlong),
        // SHA grammar
        ("sha uppercase", &sha_line, &sha_upper),
        ("sha short", &sha_line, &sha_short),
        ("sha nonhex", &sha_line, &sha_nonhex),
    ];
    for (label, from, to) in &cases {
        let text = base.replace(from, to);
        assert!(text != base, "grammar case {} did not change text", label);
        let out = t.continuity_record(&t.write_metadata(&format!("g_{}.toml", label), &text), &[]);
        assert_category(&out, "CONTINUITY_METADATA_INVALID");
    }

    // Positive extended grammar: provider/model/execution_surface may contain
    // / : @ and single internal spaces.
    let text = base
        .replace("provider = \"openai\"", "provider = \"openai/chat:v2\"")
        .replace("model_id = \"gpt-5.6\"", "model_id = \"gpt 5.6@beta\"")
        .replace(
            "execution_surface = \"opencode\"",
            "execution_surface = \"opencode:cli\"",
        );
    let out = t.continuity_record(&t.write_metadata("g_ok.toml", &text), &[]);
    assert_success(&out);

    assert_no_temp_files(&t.repo);
}

// ============================================================================
// 27.2 Model and host metadata (13-22)
// ============================================================================

#[test]
fn test_obligation_13_single_model_host_accepted() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let manifest = &ledger["entries"][0]["continuity_manifest"];
    let models = manifest["models"].as_array().unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["role"].as_str().unwrap(), "implementer");
    let hosts = manifest["hosts"].as_array().unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0]["host_id"].as_str().unwrap(), "main-workstation");
}

#[test]
fn test_obligation_14_multiple_sorted_entries_preserved() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let text = format!(
        r#"schema_version = 1
repository_id = "mrgs"
continuity_id = "phase-1-primary"
phase_id = "phase-1"
completion_receipt_sha256 = "{receipt}"
note = "Multi model and host continuity record"

models = [
  {{ role = "implementer", provider = "openai", model_id = "gpt-5.6", execution_mode = "hosted", session_label = "phase-1-impl" }},
  {{ role = "reviewer", provider = "anthropic", model_id = "claude-4.6", execution_mode = "hosted", session_label = "phase-1-review" }}
]

hosts = [
  {{ host_id = "main-workstation", platform = "windows", architecture = "x86_64", execution_surface = "opencode" }},
  {{ host_id = "secondary-workstation", platform = "linux", architecture = "aarch64", execution_surface = "terminal" }}
]

links = []
"#,
        receipt = receipt,
    );
    let meta = t.write_metadata("continuity.toml", &text);
    let out = t.continuity_record(&meta, &[]);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let manifest = &ledger["entries"][0]["continuity_manifest"];
    let models = manifest["models"].as_array().unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(models[0]["role"].as_str().unwrap(), "implementer");
    assert_eq!(models[1]["role"].as_str().unwrap(), "reviewer");
    assert_eq!(models[1]["provider"].as_str().unwrap(), "anthropic");
    let hosts = manifest["hosts"].as_array().unwrap();
    assert_eq!(hosts.len(), 2);
    assert_eq!(hosts[0]["host_id"].as_str().unwrap(), "main-workstation");
    assert_eq!(
        hosts[1]["host_id"].as_str().unwrap(),
        "secondary-workstation"
    );
    // Exact order preservation: serialized manifest text shows model 0 first
    let ledger_text = String::from_utf8(t.continuity_ledger_bytes()).unwrap();
    let (start, end) = manifest_region(&ledger_text);
    let pos_impl = ledger_text[start..end].find("\"implementer\"").unwrap();
    let pos_rev = ledger_text[start..end].find("\"reviewer\"").unwrap();
    assert!(pos_impl < pos_rev, "model order must be preserved");
}

#[test]
fn test_obligation_15_zero_models_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let text = standard_metadata("phase-1", &receipt).replace(
        "models = [\n  { role = \"implementer\", provider = \"openai\", model_id = \"gpt-5.6\", execution_mode = \"hosted\", session_label = \"phase-1-phase-1-implementation\" }\n]",
        "models = []",
    );
    // The standard builder parametrizes the session label; replace robustly:
    let text = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("{ role = "))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("models = [\n  ]", "models = []");
    let _ = text;
    let text = standard_metadata("phase-1", &receipt);
    // Build a zero-models variant by removing the model table between markers.
    let mut lines: Vec<String> = Vec::new();
    let mut skipping = false;
    for line in text.lines() {
        if line.starts_with("models = [") {
            skipping = true;
            lines.push("models = []".to_string());
            continue;
        }
        if skipping {
            if line.trim_start().starts_with('}') || line.trim().is_empty() {
                skipping = false;
                continue;
            }
            continue;
        }
        lines.push(line.to_string());
    }
    let zero = lines.join("\n");
    let meta = t.write_metadata("m.toml", &zero);
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_16_zero_hosts_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let text = standard_metadata("phase-1", &receipt);
    let mut lines: Vec<String> = Vec::new();
    let mut skipping = false;
    for line in text.lines() {
        if line.starts_with("hosts = [") {
            skipping = true;
            lines.push("hosts = []".to_string());
            continue;
        }
        if skipping {
            if line.trim_start().starts_with('}') || line.trim().is_empty() {
                skipping = false;
                continue;
            }
            continue;
        }
        lines.push(line.to_string());
    }
    let zero = lines.join("\n");
    let meta = t.write_metadata("m.toml", &zero);
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_17_unsorted_models_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // reviewer sorts before implementer -> unsorted
    let text = format!(
        r#"schema_version = 1
repository_id = "mrgs"
continuity_id = "phase-1-primary"
phase_id = "phase-1"
completion_receipt_sha256 = "{receipt}"
note = "Unsorted models"

models = [
  {{ role = "reviewer", provider = "anthropic", model_id = "claude-4.6", execution_mode = "hosted", session_label = "phase-1-review" }},
  {{ role = "implementer", provider = "openai", model_id = "gpt-5.6", execution_mode = "hosted", session_label = "phase-1-impl" }}
]

hosts = [
  {{ host_id = "main-workstation", platform = "windows", architecture = "x86_64", execution_surface = "opencode" }}
]

links = []
"#,
        receipt = receipt,
    );
    let meta = t.write_metadata("m.toml", &text);
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_18_duplicate_models_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let text = format!(
        r#"schema_version = 1
repository_id = "mrgs"
continuity_id = "phase-1-primary"
phase_id = "phase-1"
completion_receipt_sha256 = "{receipt}"
note = "Duplicate models"

models = [
  {{ role = "implementer", provider = "openai", model_id = "gpt-5.6", execution_mode = "hosted", session_label = "phase-1-impl" }},
  {{ role = "implementer", provider = "openai", model_id = "gpt-5.6", execution_mode = "hosted", session_label = "phase-1-impl" }}
]

hosts = [
  {{ host_id = "main-workstation", platform = "windows", architecture = "x86_64", execution_surface = "opencode" }}
]

links = []
"#,
        receipt = receipt,
    );
    let meta = t.write_metadata("m.toml", &text);
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_19_invalid_model_forms_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let base = standard_metadata("phase-1", &receipt);
    let label_long = format!("session_label = \"{}\"", "x".repeat(257));
    let cases: Vec<(&str, &str, &str)> = vec![
        (
            "role space",
            "role = \"implementer\"",
            "role = \"imple menter\"",
        ),
        (
            "role dash start",
            "role = \"implementer\"",
            "role = \"-implementer\"",
        ),
        ("role empty", "role = \"implementer\"", "role = \"\""),
        (
            "provider backslash",
            "provider = \"openai\"",
            "provider = \"openai\\\\bad\"",
        ),
        (
            "model colon start",
            "model_id = \"gpt-5.6\"",
            "model_id = \":gpt-5.6\"",
        ),
        (
            "mode slash",
            "execution_mode = \"hosted\"",
            "execution_mode = \"host/ed\"",
        ),
        (
            "label control",
            "session_label = \"phase-1-implementation\"",
            "session_label = \"phase-1\\u{0002}-implementation\"",
        ),
        (
            "label long",
            "session_label = \"phase-1-implementation\"",
            &label_long,
        ),
    ];
    for (label, from, to) in &cases {
        let text = base.replace(from, to);
        assert!(text != base, "model case {} did not change text", label);
        let out = t.continuity_record(&t.write_metadata("mm.toml", &text), &[]);
        assert_category(&out, "CONTINUITY_METADATA_INVALID");
    }
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_20_unsorted_hosts_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let text = format!(
        r#"schema_version = 1
repository_id = "mrgs"
continuity_id = "phase-1-primary"
phase_id = "phase-1"
completion_receipt_sha256 = "{receipt}"
note = "Unsorted hosts"

models = [
  {{ role = "implementer", provider = "openai", model_id = "gpt-5.6", execution_mode = "hosted", session_label = "phase-1-impl" }}
]

hosts = [
  {{ host_id = "secondary-workstation", platform = "linux", architecture = "aarch64", execution_surface = "terminal" }},
  {{ host_id = "main-workstation", platform = "windows", architecture = "x86_64", execution_surface = "opencode" }}
]

links = []
"#,
        receipt = receipt,
    );
    let meta = t.write_metadata("m.toml", &text);
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_21_duplicate_host_ids_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let text = format!(
        r#"schema_version = 1
repository_id = "mrgs"
continuity_id = "phase-1-primary"
phase_id = "phase-1"
completion_receipt_sha256 = "{receipt}"
note = "Duplicate hosts"

models = [
  {{ role = "implementer", provider = "openai", model_id = "gpt-5.6", execution_mode = "hosted", session_label = "phase-1-impl" }}
]

hosts = [
  {{ host_id = "main-workstation", platform = "windows", architecture = "x86_64", execution_surface = "opencode" }},
  {{ host_id = "main-workstation", platform = "linux", architecture = "aarch64", execution_surface = "terminal" }}
]

links = []
"#,
        receipt = receipt,
    );
    let meta = t.write_metadata("m.toml", &text);
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_22_invalid_host_forms_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let base = standard_metadata("phase-1", &receipt);
    let surface_long = format!("execution_surface = \"{}\"", "y".repeat(257));
    let cases: Vec<(&str, &str, &str)> = vec![
        (
            "host id space",
            "host_id = \"main-workstation\"",
            "host_id = \"main workstation\"",
        ),
        (
            "host id empty",
            "host_id = \"main-workstation\"",
            "host_id = \"\"",
        ),
        (
            "platform slash",
            "platform = \"windows\"",
            "platform = \"win/dows\"",
        ),
        (
            "arch colon",
            "architecture = \"x86_64\"",
            "architecture = \"x86:64\"",
        ),
        (
            "surface leading space",
            "execution_surface = \"opencode\"",
            "execution_surface = \" opencode\"",
        ),
        (
            "surface repeated space",
            "execution_surface = \"opencode\"",
            "execution_surface = \"open  code\"",
        ),
        (
            "surface long",
            "execution_surface = \"opencode\"",
            &surface_long,
        ),
    ];
    for (label, from, to) in &cases {
        let text = base.replace(from, to);
        assert!(text != base, "host case {} did not change text", label);
        let out = t.continuity_record(&t.write_metadata("mh.toml", &text), &[]);
        assert_category(&out, "CONTINUITY_METADATA_INVALID");
    }
    assert_no_temp_files(&t.repo);
}

// ============================================================================
// 27.3 Target completion binding (23-32)
// ============================================================================

#[test]
fn test_obligation_23_closed_phase_exact_receipt_accepted() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let entry = &ledger["entries"][0];
    let manifest = &entry["continuity_manifest"];
    assert_eq!(manifest["phase_id"].as_str().unwrap(), "phase-1");
    assert_eq!(
        manifest["target_completion_receipt_sha256"]
            .as_str()
            .unwrap(),
        receipt
    );
    // The exact Phase 6 receipt object is archived
    let embedded = &manifest["target_completion_receipt"];
    assert_eq!(embedded["phase_id"].as_str().unwrap(), "phase-1");
    assert_eq!(
        embedded["completion_receipt_sha256"],
        serde_json::Value::Null
    );
    assert_eq!(
        embedded["previous_completion_receipt_sha256"],
        serde_json::Value::Null
    );
    assert_eq!(
        manifest["target_final_manifest_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
}

#[test]
fn test_obligation_24_unknown_phase_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let text = standard_metadata("phase-1", &receipt)
        .replace("phase_id = \"phase-1\"", "phase_id = \"phase-99\"");
    let meta = t.write_metadata("m.toml", &text);
    let out = t.continuity_record(&meta, &[]);
    assert_failure(&out);
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_25_active_or_unclosed_phase_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // Unclosed phase-2 (in the plan, not completed)
    let text = standard_metadata("phase-1", &receipt)
        .replace("phase_id = \"phase-1\"", "phase_id = \"phase-2\"");
    let meta = t.write_metadata("m.toml", &text);
    let out = t.continuity_record(&meta, &[]);
    assert_failure(&out);
    // Active phase: select phase-2 (deps satisfied) and try to record it.
    assert_success(&t.select_phase("phase-2"));
    let meta2 = t.write_metadata("m2.toml", &text);
    let out2 = t.continuity_record(&meta2, &[]);
    assert_failure(&out2);
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_26_wrong_completion_receipt_hash_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let wrong = flip_first_hex_char(&receipt);
    let text = standard_metadata("phase-1", &receipt).replace(&receipt, &wrong);
    let meta = t.write_metadata("m.toml", &text);
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_27_malformed_completion_ledger_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["completions"][0]["completion_receipt"]["phase_id"] = serde_json::json!("phase-other");
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CLOSEOUT_LEDGER_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_28_completion_ledger_other_plan_stale() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    ledger["accepted_plan_sha256"] = serde_json::json!(all_zeros_sha());
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CLOSEOUT_LEDGER_STALE");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_29_phase_missing_from_closed_state_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // Make the completion ledger reference phase-1 while state still lists it
    // as the active, not-yet-closed phase (in-progress finalization shape).
    let state_path = t.repo.join(".mrgs/state.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    state["active_phase"] = serde_json::json!("phase-1");
    state["closed_phases"] = serde_json::json!([]);
    std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CLOSEOUT_CONFLICT");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_30_changed_completion_binding_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta, &[]));
    // Same continuity_id and phase, but a changed target completion binding
    let wrong = flip_first_hex_char(&receipt);
    let text = standard_metadata("phase-1", &receipt).replace(&receipt, &wrong);
    let meta2 = t.write_metadata("m2.toml", &text);
    let out = t.continuity_record(&meta2, &[]);
    assert_category(&out, "CONTINUITY_CONFLICT");
    // Ledger bytes untouched
    let ledger = t.get_continuity_ledger().unwrap();
    assert_eq!(
        ledger["entries"][0]["continuity_manifest"]["phase_id"]
            .as_str()
            .unwrap(),
        "phase-1"
    );
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_31_final_manifest_hash_mismatch_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    let orig = ledger["completions"][0]["final_manifest_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let broken = flip_first_hex_char(&orig);
    ledger["completions"][0]["final_manifest_sha256"] = serde_json::json!(broken);
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CLOSEOUT_LEDGER_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_32_completion_ordering_chain_rejects() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let _receipt2 = t.complete_phase2();
    let ledger_path = t.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
    // Break the receipt chain: entry 2's previous hash no longer matches
    ledger["completions"][1]["completion_receipt"]["previous_completion_receipt_sha256"] =
        serde_json::json!(all_zeros_sha());
    std::fs::write(&ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt1));
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CLOSEOUT_LEDGER_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

// ============================================================================
// 27.4 Cross-repository resolution (33-48)
// ============================================================================

#[test]
fn test_obligation_33_empty_links_zero_sources_accepted() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let resolved = ledger["entries"][0]["continuity_manifest"]["resolved_links"]
        .as_array()
        .unwrap();
    assert!(resolved.is_empty());
}

#[test]
fn test_obligation_34_nonempty_links_require_sources() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_35_unreferenced_source_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    let extra = TestRepo::build_source(false);
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(
        &meta,
        &[
            &source.repo.to_string_lossy(),
            &extra.repo.to_string_lossy(),
        ],
    );
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_36_duplicate_source_roots_reject() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(
        &meta,
        &[
            &source.repo.to_string_lossy(),
            &source.repo.to_string_lossy(),
        ],
    );
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_37_source_equal_target_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    // Pass the canonical target root as the source
    let out = t.continuity_record(&meta, &[&t.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_38_invalid_source_plan_authority_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    // Corrupt the source accepted-plan record
    let path = source.repo.join(".mrgs/accepted-plan.json");
    let mut record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    record["sha256"] = serde_json::json!(all_zeros_sha());
    std::fs::write(&path, serde_json::to_string_pretty(&record).unwrap()).unwrap();
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_39_invalid_source_ledger_state_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    // Corrupt the source completion ledger structure
    let path = source.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    ledger["completions"][0]["completion_receipt_sha256"] = serde_json::json!(all_zeros_sha());
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_40_source_repository_id_mismatch_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // Source has its own continuity ledger with repository_id "mrgs"
    let source = TestRepo::build_source(true);
    let cont_sha = source.continuity_receipt_sha("phase-1");
    // Link names a different repository_id than the source ledger
    let link = link_fragment(
        "other-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        Some(&cont_sha),
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_41_source_plan_sha_mismatch_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    let link = link_fragment(
        "source-repository",
        all_zeros_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_42_missing_source_phase_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    // Link names phase-2 which the source never completed
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-2",
        all_zeros_sha(),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_43_source_receipt_mismatch_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    let wrong = all_zeros_sha();
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        wrong,
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_44_source_manifest_chain_mismatch_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(false);
    // Break the source's final-manifest hash so complete Phase 6 hash and
    // chain validation fails on the source authority
    let path = source.repo.join(".mrgs/completion-ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    ledger["completions"][0]["final_manifest_sha256"] = serde_json::json!(all_zeros_sha());
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_45_omitted_source_continuity_accepted() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // Source has completion proof only (no continuity ledger at all)
    let source = TestRepo::build_source(false);
    assert!(!source.repo.join(".mrgs/continuity-ledger.json").exists());
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let resolved = &ledger["entries"][0]["continuity_manifest"]["resolved_links"][0];
    assert_eq!(resolved["relation"].as_str().unwrap(), "continues_from");
    assert_eq!(
        resolved["source_repository_id"].as_str().unwrap(),
        "source-repository"
    );
    assert_eq!(
        resolved["source_completion_receipt_sha256"]
            .as_str()
            .unwrap(),
        source.completion_receipt_sha("phase-1")
    );
    assert!(resolved["source_continuity_receipt_sha256"].is_null());
    assert!(resolved["source_continuity_manifest_sha256"].is_null());
    assert!(resolved["source_continuity_receipt"].is_null());
}

#[test]
fn test_obligation_46_source_continuity_resolved_exactly() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(true);
    let cont_sha = source.continuity_receipt_sha("phase-1");
    // The link names the source ledger's repository ID ("mrgs"), never the
    // target's ("target-repo").
    let link = link_fragment(
        "mrgs",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        Some(&cont_sha),
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links_target_repo("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let resolved = &ledger["entries"][0]["continuity_manifest"]["resolved_links"][0];
    // The resolved link keeps the source ledger's repository ID, never the
    // target's.
    assert_eq!(resolved["source_repository_id"].as_str().unwrap(), "mrgs");
    assert_eq!(
        resolved["source_continuity_receipt_sha256"]
            .as_str()
            .unwrap(),
        cont_sha
    );
    assert_eq!(
        resolved["source_continuity_manifest_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    // The exact source continuity receipt object is archived
    let src_ledger = source.get_continuity_ledger().unwrap();
    let src_entry = &src_ledger["entries"][0];
    assert_eq!(
        resolved["source_continuity_receipt"],
        src_entry["continuity_receipt"]
    );
    assert_eq!(
        resolved["source_continuity_receipt_sha256"]
            .as_str()
            .unwrap(),
        src_entry["continuity_receipt_sha256"].as_str().unwrap()
    );
    // The source continuity manifest hash is the archived manifest hash
    assert_eq!(
        resolved["source_continuity_manifest_sha256"]
            .as_str()
            .unwrap(),
        src_entry["continuity_manifest_sha256"].as_str().unwrap()
    );
    // Completion proof fields are exact
    assert_eq!(
        resolved["source_completion_receipt_sha256"]
            .as_str()
            .unwrap(),
        source.completion_receipt_sha("phase-1")
    );
    assert_eq!(resolved["source_completion_sequence"].as_u64().unwrap(), 1);
    assert_eq!(resolved["source_plan_id"].as_str().unwrap(), "test-plan");
}

#[test]
fn test_obligation_47_missing_stale_mismatched_source_continuity() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();

    // (a) link requires a continuity receipt but the source has no ledger
    let source = TestRepo::build_source(false);
    let ghost = all_zeros_sha();
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        Some(ghost),
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());

    // (b) receipt hash names no entry in the source ledger
    let source = TestRepo::build_source(true);
    let ghost = all_zeros_sha();
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        Some(ghost),
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());

    // (c) entry exists but binds a different phase/receipt
    let source = TestRepo::build_source(true);
    let cont_sha = source.continuity_receipt_sha("phase-1");
    let mut src_ledger_path = source.repo.join(".mrgs/continuity-ledger.json");
    let mut src_ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&src_ledger_path).unwrap()).unwrap();
    src_ledger["entries"][0]["continuity_manifest"]["phase_id"] = serde_json::json!("phase-other");
    std::fs::write(
        &src_ledger_path,
        serde_json::to_string_pretty(&src_ledger).unwrap(),
    )
    .unwrap();
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        Some(&cont_sha),
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());

    let _ = &mut src_ledger_path;
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_48_link_relation_sorting_uniqueness_resolution() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let base = |link: &str| metadata_with_links("phase-1", &receipt, link);

    // Wrong relation
    let text = base("  { relation = \"derived_from\", repository_id = \"x\", accepted_plan_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\", phase_id = \"p\", completion_receipt_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" }");
    let out = t.continuity_record(&t.write_metadata("m1.toml", &text), &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");

    // Unsorted links
    let l2 = "  { relation = \"continues_from\", repository_id = \"b\", accepted_plan_sha256 = \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\", phase_id = \"p1\", completion_receipt_sha256 = \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\" }";
    let l1 = "  { relation = \"continues_from\", repository_id = \"a\", accepted_plan_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\", phase_id = \"p2\", completion_receipt_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" }";
    let text = base(&format!("{},\n{}", l2, l1));
    let out = t.continuity_record(&t.write_metadata("m2.toml", &text), &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");

    // Duplicate (equal tuple) links
    let text = base(&format!("{},\n{}", l1, l1));
    let out = t.continuity_record(&t.write_metadata("m3.toml", &text), &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");

    // Link naming the target repository_id
    let text = base("  { relation = \"continues_from\", repository_id = \"mrgs\", accepted_plan_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\", phase_id = \"p\", completion_receipt_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" }");
    let out = t.continuity_record(&t.write_metadata("m4.toml", &text), &[]);
    assert_category(&out, "CONTINUITY_METADATA_INVALID");

    // One-to-one resolution violation: one source must not satisfy two links.
    // Both links match the same source (same plan/phase/receipt), and only one
    // source is supplied.
    let source = TestRepo::build_source(false);
    let sa = source.completion_receipt_sha("phase-1");
    let link_a = link_fragment("source-a", &source.plan_sha(), "phase-1", &sa, None);
    let link_b = link_fragment("source-b", &source.plan_sha(), "phase-1", &sa, None);
    let text = base(&format!("{},\n{}", link_a, link_b));
    let meta = t.write_metadata("m5.toml", &text);
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_MISMATCH");

    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

// ============================================================================
// 27.5 Manifest, receipt, and ledger (49-62)
// ============================================================================

#[test]
fn test_obligation_49_manifest_exact_fields_and_order() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta, &[]));
    let ledger_text = String::from_utf8(t.continuity_ledger_bytes()).unwrap();

    let manifest_keys = [
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
    let (m_start, m_end) = manifest_region(&ledger_text);
    assert_key_order_in_region(&ledger_text, (m_start, m_end), &manifest_keys);

    let receipt_keys = [
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
    let (r_start, r_end) = receipt_region(&ledger_text);
    assert_key_order_in_region(&ledger_text, (r_start, r_end), &receipt_keys);

    // No unlisted fields: exact key sets
    let manifest_obj: serde_json::Value =
        serde_json::from_str(&ledger_text[m_start..m_end]).unwrap();
    let mut keys: Vec<&str> = manifest_obj
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    keys.sort_unstable();
    let mut expected: Vec<&str> = manifest_keys.to_vec();
    expected.sort_unstable();
    assert_eq!(
        keys, expected,
        "manifest must contain exactly the listed fields"
    );

    let ledger: serde_json::Value = serde_json::from_str(&ledger_text).unwrap();
    let receipt_obj = &ledger["entries"][0]["continuity_receipt"];
    let mut rkeys: Vec<&str> = receipt_obj
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    rkeys.sort_unstable();
    let mut rexpected: Vec<&str> = receipt_keys.to_vec();
    rexpected.sort_unstable();
    assert_eq!(
        rkeys, rexpected,
        "receipt must contain exactly the listed fields"
    );

    // Top-level ledger fields
    let mut tkeys: Vec<&str> = ledger
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    tkeys.sort_unstable();
    assert_eq!(
        tkeys,
        vec![
            "accepted_plan_sha256",
            "entries",
            "plan_id",
            "repository_id",
            "schema_version"
        ],
        "ledger must contain exactly the listed top-level fields"
    );
}

#[test]
fn test_obligation_50_metadata_path_bytes_sha_preserved() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // Nested metadata path to prove normalized '/' separators
    std::fs::create_dir_all(t.repo.join("meta-dir")).unwrap();
    let path = t.repo.join("meta-dir/continuity.toml");
    let content = standard_metadata("phase-1", &receipt);
    write_file(&path, &content);
    let out = t.continuity_record(&path, &[]);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let manifest = &ledger["entries"][0]["continuity_manifest"];
    assert_eq!(
        manifest["metadata_source_path"].as_str().unwrap(),
        "meta-dir/continuity.toml"
    );
    assert_eq!(
        manifest["metadata_sha256"].as_str().unwrap(),
        sha256_hex(content.as_bytes())
    );
    assert_eq!(manifest["metadata_content"].as_str().unwrap(), content);
    // The source file itself is untouched
    assert_eq!(std::fs::read(&path).unwrap(), content.as_bytes());
}

#[test]
fn test_obligation_51_note_models_hosts_preserved() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let content = standard_metadata("phase-1", &receipt);
    let meta = t.write_metadata("continuity.toml", &content);
    assert_success(&t.continuity_record(&meta, &[]));
    let ledger = t.get_continuity_ledger().unwrap();
    let manifest = &ledger["entries"][0]["continuity_manifest"];
    assert_eq!(
        manifest["note"].as_str().unwrap(),
        "Primary governed execution continuity record"
    );
    assert_eq!(manifest["models"].as_array().unwrap().len(), 1);
    assert_eq!(
        manifest["models"][0]["role"].as_str().unwrap(),
        "implementer"
    );
    assert_eq!(
        manifest["models"][0]["provider"].as_str().unwrap(),
        "openai"
    );
    assert_eq!(
        manifest["models"][0]["model_id"].as_str().unwrap(),
        "gpt-5.6"
    );
    assert_eq!(
        manifest["models"][0]["execution_mode"].as_str().unwrap(),
        "hosted"
    );
    assert_eq!(
        manifest["models"][0]["session_label"].as_str().unwrap(),
        "phase-1-implementation"
    );
    assert_eq!(manifest["hosts"].as_array().unwrap().len(), 1);
    assert_eq!(
        manifest["hosts"][0]["host_id"].as_str().unwrap(),
        "main-workstation"
    );
    assert_eq!(
        manifest["hosts"][0]["platform"].as_str().unwrap(),
        "windows"
    );
    assert_eq!(
        manifest["hosts"][0]["architecture"].as_str().unwrap(),
        "x86_64"
    );
    assert_eq!(
        manifest["hosts"][0]["execution_surface"].as_str().unwrap(),
        "opencode"
    );
}

#[test]
fn test_obligation_52_resolved_links_exact_no_source_path() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(true);
    let cont_sha = source.continuity_receipt_sha("phase-1");
    let link = link_fragment(
        "mrgs",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        Some(&cont_sha),
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links_target_repo("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_success(&out);
    let ledger_text = String::from_utf8(t.continuity_ledger_bytes()).unwrap();
    let ledger: serde_json::Value = serde_json::from_str(&ledger_text).unwrap();
    let resolved = &ledger["entries"][0]["continuity_manifest"]["resolved_links"][0];
    let mut keys: Vec<&str> = resolved
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "relation",
            "source_accepted_plan_sha256",
            "source_completion_receipt",
            "source_completion_receipt_sha256",
            "source_completion_sequence",
            "source_continuity_manifest_sha256",
            "source_continuity_receipt",
            "source_continuity_receipt_sha256",
            "source_final_manifest_sha256",
            "source_phase_id",
            "source_plan_id",
            "source_repository_id",
        ],
        "resolved link must contain exactly the proof fields"
    );
    // No source filesystem path may be persisted anywhere in the ledger.
    let source_path_str = source.repo.to_string_lossy().to_string();
    assert!(
        !ledger_text.contains(&source_path_str),
        "source path leaked into ledger"
    );
    // A real backslash inside a JSON value (JSON-escaped as \\) would be a
    // path separator; plain \n-style JSON escapes are legitimate content.
    assert!(
        !ledger_text.contains("\\\\"),
        "backslash path leaked into ledger"
    );
}

#[test]
fn test_obligation_53_manifest_bytes_hash_deterministic() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let out1 = t.continuity_record(&meta, &[]);
    assert_success(&out1);
    let bytes1 = t.continuity_ledger_bytes();
    let out2 = {
        // Remove the ledger and re-record with identical inputs
        std::fs::remove_file(t.repo.join(".mrgs/continuity-ledger.json")).unwrap();
        t.continuity_record(&meta, &[])
    };
    assert_success(&out2);
    let bytes2 = t.continuity_ledger_bytes();
    assert_eq!(out1.stdout, out2.stdout, "output must be deterministic");
    assert_eq!(bytes1, bytes2, "ledger bytes must be deterministic");
    // Recompute the manifest hash from the file's compact JSON
    let text = String::from_utf8(bytes2.clone()).unwrap();
    let region = manifest_region(&text);
    let compact = compact_json_of_range(&text, region);
    let ledger: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        ledger["entries"][0]["continuity_manifest_sha256"]
            .as_str()
            .unwrap(),
        sha256_hex(compact.as_bytes())
    );
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_54_first_receipt_null_previous() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta, &[]));
    let ledger_text = String::from_utf8(t.continuity_ledger_bytes()).unwrap();
    let ledger: serde_json::Value = serde_json::from_str(&ledger_text).unwrap();
    assert!(
        ledger["entries"][0]["continuity_receipt"]["previous_continuity_receipt_sha256"].is_null()
    );
    assert!(ledger_text.contains("\"previous_continuity_receipt_sha256\": null"));
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_55_later_receipt_chains_previous() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let receipt2 = t.complete_phase2();
    let meta1 = t.write_metadata("c1.toml", &standard_metadata("phase-1", &receipt1));
    assert_success(&t.continuity_record(&meta1, &[]));
    let meta2_text = standard_metadata("phase-2", &receipt2).replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"phase-2-primary\"",
    );
    let meta2 = t.write_metadata("c2.toml", &meta2_text);
    assert_success(&t.continuity_record(&meta2, &[]));
    let ledger = t.get_continuity_ledger().unwrap();
    let entries = ledger["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[1]["continuity_receipt"]["previous_continuity_receipt_sha256"]
            .as_str()
            .unwrap(),
        entries[0]["continuity_receipt_sha256"].as_str().unwrap()
    );
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_56_continuity_sequence_contiguous() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let receipt2 = t.complete_phase2();
    let meta1 = t.write_metadata("c1.toml", &standard_metadata("phase-1", &receipt1));
    assert_success(&t.continuity_record(&meta1, &[]));
    let meta2_text = standard_metadata("phase-2", &receipt2).replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"phase-2-primary\"",
    );
    let meta2 = t.write_metadata("c2.toml", &meta2_text);
    assert_success(&t.continuity_record(&meta2, &[]));
    let ledger = t.get_continuity_ledger().unwrap();
    let entries = ledger["entries"].as_array().unwrap();
    assert_eq!(
        entries[0]["continuity_receipt"]["continuity_sequence"]
            .as_u64()
            .unwrap(),
        1
    );
    assert_eq!(
        entries[1]["continuity_receipt"]["continuity_sequence"]
            .as_u64()
            .unwrap(),
        2
    );
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_57_target_sequence_receipt_binding_exact() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let receipt2 = t.complete_phase2();
    assert_eq!(t.completion_sequence("phase-1"), 1);
    assert_eq!(t.completion_sequence("phase-2"), 2);
    let meta1 = t.write_metadata("c1.toml", &standard_metadata("phase-1", &receipt1));
    assert_success(&t.continuity_record(&meta1, &[]));
    let meta2_text = standard_metadata("phase-2", &receipt2).replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"phase-2-primary\"",
    );
    let meta2 = t.write_metadata("c2.toml", &meta2_text);
    assert_success(&t.continuity_record(&meta2, &[]));
    let ledger = t.get_continuity_ledger().unwrap();
    let entries = ledger["entries"].as_array().unwrap();
    assert_eq!(
        entries[0]["continuity_manifest"]["target_completion_sequence"]
            .as_u64()
            .unwrap(),
        1
    );
    assert_eq!(
        entries[1]["continuity_manifest"]["target_completion_sequence"]
            .as_u64()
            .unwrap(),
        2
    );
    assert_eq!(
        entries[1]["continuity_receipt"]["target_completion_receipt_sha256"]
            .as_str()
            .unwrap(),
        receipt2
    );
    assert_eq!(
        entries[1]["continuity_manifest"]["target_completion_receipt_sha256"]
            .as_str()
            .unwrap(),
        receipt2
    );
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_58_receipt_bytes_hash_deterministic() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta, &[]));
    let bytes1 = t.continuity_ledger_bytes();
    std::fs::remove_file(t.repo.join(".mrgs/continuity-ledger.json")).unwrap();
    assert_success(&t.continuity_record(&meta, &[]));
    let bytes2 = t.continuity_ledger_bytes();
    assert_eq!(bytes1, bytes2, "receipt bytes must be deterministic");
    // Recompute the receipt hash from the file's compact JSON
    let text = String::from_utf8(bytes2).unwrap();
    let region = receipt_region(&text);
    let compact = compact_json_of_range(&text, region);
    let ledger: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        ledger["entries"][0]["continuity_receipt_sha256"]
            .as_str()
            .unwrap(),
        sha256_hex(compact.as_bytes())
    );
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_59_ledger_top_level_immutable_repo_id() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let meta1_content = standard_metadata("phase-1", &receipt1);
    let meta1 = t.write_metadata("c1.toml", &meta1_content);
    assert_success(&t.continuity_record(&meta1, &[]));
    // Commit the metadata file so the later Phase 4 phase-2 chain sees a
    // clean tree.
    t.commit_file("c1.toml", &meta1_content);
    let ledger_text = String::from_utf8(t.continuity_ledger_bytes()).unwrap();
    let ledger: serde_json::Value = serde_json::from_str(&ledger_text).unwrap();
    assert_eq!(ledger["schema_version"].as_u64().unwrap(), 1);
    assert_eq!(
        ledger["accepted_plan_sha256"].as_str().unwrap(),
        t.plan_sha()
    );
    assert_eq!(ledger["plan_id"].as_str().unwrap(), "test-plan");
    assert_eq!(ledger["repository_id"].as_str().unwrap(), "mrgs");

    // Immutability: a later record with a different repository_id conflicts
    let receipt2 = t.complete_phase2();
    let text = standard_metadata("phase-2", &receipt2)
        .replace(
            "continuity_id = \"phase-1-primary\"",
            "continuity_id = \"phase-2-primary\"",
        )
        .replace("repository_id = \"mrgs\"", "repository_id = \"other-repo\"");
    let meta2 = t.write_metadata("c2.toml", &text);
    let out = t.continuity_record(&meta2, &[]);
    assert_category(&out, "CONTINUITY_CONFLICT");
    // Same repository_id continues to work
    let text_ok = standard_metadata("phase-2", &receipt2).replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"phase-2-primary\"",
    );
    let meta2ok = t.write_metadata("c2ok.toml", &text_ok);
    assert_success(&t.continuity_record(&meta2ok, &[]));
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_60_reordered_entries_reject() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let receipt2 = t.complete_phase2();
    let meta1 = t.write_metadata("c1.toml", &standard_metadata("phase-1", &receipt1));
    assert_success(&t.continuity_record(&meta1, &[]));
    let meta2_text = standard_metadata("phase-2", &receipt2).replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"phase-2-primary\"",
    );
    let meta2 = t.write_metadata("c2.toml", &meta2_text);
    assert_success(&t.continuity_record(&meta2, &[]));
    let saved = t.continuity_ledger_bytes();
    let path = t.repo.join(".mrgs/continuity-ledger.json");
    let mut ledger: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    let entries = ledger["entries"].as_array_mut().unwrap();
    entries.swap(0, 1);
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.continuity_record(&meta2, &[]);
    assert_category(&out, "CONTINUITY_LEDGER_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_61_duplicate_phase_continuity_id_rejects() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let receipt2 = t.complete_phase2();
    let meta1 = t.write_metadata("c1.toml", &standard_metadata("phase-1", &receipt1));
    assert_success(&t.continuity_record(&meta1, &[]));
    let meta2_text = standard_metadata("phase-2", &receipt2).replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"phase-2-primary\"",
    );
    let meta2 = t.write_metadata("c2.toml", &meta2_text);
    assert_success(&t.continuity_record(&meta2, &[]));
    let saved = t.continuity_ledger_bytes();
    let path = t.repo.join(".mrgs/continuity-ledger.json");

    // Duplicate continuity_id: second entry copies the first entry's id
    let mut ledger: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    ledger["entries"][1]["continuity_receipt"]["continuity_id"] =
        serde_json::json!("phase-1-primary");
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.continuity_record(&meta2, &[]);
    assert_category(&out, "CONTINUITY_LEDGER_INVALID");

    // Duplicate phase: restore, then duplicate the phase in entry 2
    std::fs::write(&path, &saved).unwrap();
    let mut ledger: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    ledger["entries"][1]["continuity_manifest"]["phase_id"] = serde_json::json!("phase-1");
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.continuity_record(&meta2, &[]);
    assert_category(&out, "CONTINUITY_LEDGER_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_62_broken_hash_binding_chain_rejects() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let receipt2 = t.complete_phase2();
    let meta1 = t.write_metadata("c1.toml", &standard_metadata("phase-1", &receipt1));
    assert_success(&t.continuity_record(&meta1, &[]));
    let meta2_text = standard_metadata("phase-2", &receipt2).replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"phase-2-primary\"",
    );
    let meta2 = t.write_metadata("c2.toml", &meta2_text);
    assert_success(&t.continuity_record(&meta2, &[]));
    let saved = t.continuity_ledger_bytes();
    let path = t.repo.join(".mrgs/continuity-ledger.json");
    let zero = all_zeros_sha();

    // (a) broken manifest hash
    std::fs::write(&path, &saved).unwrap();
    let mut ledger: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    ledger["entries"][0]["continuity_manifest_sha256"] = serde_json::json!(zero);
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.continuity_record(&meta1, &[]);
    assert_category(&out, "CONTINUITY_LEDGER_INVALID");

    // (b) broken receipt hash
    std::fs::write(&path, &saved).unwrap();
    let mut ledger: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    ledger["entries"][1]["continuity_receipt_sha256"] = serde_json::json!(zero);
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.continuity_record(&meta1, &[]);
    assert_category(&out, "CONTINUITY_LEDGER_INVALID");

    // (c) broken manifest-to-receipt binding
    std::fs::write(&path, &saved).unwrap();
    let mut ledger: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    ledger["entries"][0]["continuity_receipt"]["continuity_manifest_sha256"] =
        serde_json::json!(zero);
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.continuity_record(&meta1, &[]);
    assert_category(&out, "CONTINUITY_LEDGER_INVALID");

    // (d) broken previous link
    std::fs::write(&path, &saved).unwrap();
    let mut ledger: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    ledger["entries"][1]["continuity_receipt"]["previous_continuity_receipt_sha256"] =
        serde_json::json!(zero);
    std::fs::write(&path, serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let out = t.continuity_record(&meta1, &[]);
    assert_category(&out, "CONTINUITY_LEDGER_INVALID");

    // Restore the valid ledger and confirm the replay still succeeds
    std::fs::write(&path, &saved).unwrap();
    let out = t.continuity_record(&meta1, &[]);
    assert_success(&out);
    assert_no_temp_files(&t.repo);
}

// ============================================================================
// 27.6 Publication, idempotency, and conflicts (63-72)
// ============================================================================

#[test]
fn test_embedded_receipt_content_tamper_rejects() {
    // Contract sections 11 and 20: the continuity manifest must archive the
    // exact target completion receipt object. A fully hash- and
    // chain-consistent ledger whose embedded receipt object was altered (a
    // non-binding field) must be rejected by full-object equality against the
    // authoritative stored completion receipt.
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta, &[]));
    let saved = t.continuity_ledger_bytes();

    // Tamper the embedded receipt's phase_title inside the first manifest.
    let text = String::from_utf8(saved.clone()).unwrap();
    let (m0, m1) = manifest_region(&text);
    let (r0, r1) = receipt_region(&text);
    let tampered_manifest = text[m0..m1].replace(
        "\"phase_title\": \"First phase\"",
        "\"phase_title\": \"EVIL\"",
    );
    assert_ne!(
        tampered_manifest,
        text[m0..m1],
        "embedded receipt content must be altered"
    );

    // Recompute the manifest hash over the order-preserving compact form.
    let new_manifest_hash = sha256_hex(
        compact_json_of_range(&tampered_manifest, (0, tampered_manifest.len())).as_bytes(),
    );

    let ledger: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    let old_manifest_hash = ledger["entries"][0]["continuity_manifest_sha256"]
        .as_str()
        .unwrap();
    let old_receipt_hash = ledger["entries"][0]["continuity_receipt_sha256"]
        .as_str()
        .unwrap();
    let old_mh_field = format!("\"continuity_manifest_sha256\": \"{}\"", old_manifest_hash);
    let new_mh_field = format!("\"continuity_manifest_sha256\": \"{}\"", new_manifest_hash);

    // Fix the entry-level manifest-hash field (between manifest and receipt)
    // and the receipt's manifest-hash binding (inside the receipt).
    let between = text[m1..r0].replace(&old_mh_field, &new_mh_field);
    assert_ne!(
        between,
        text[m1..r0],
        "entry manifest-hash field must be updated"
    );
    let receipt_text = text[r0..r1].replace(&old_mh_field, &new_mh_field);
    assert_ne!(
        receipt_text,
        text[r0..r1],
        "receipt manifest-hash binding must be updated"
    );
    let new_receipt_hash =
        sha256_hex(compact_json_of_range(&receipt_text, (0, receipt_text.len())).as_bytes());

    // Fix the entry-level receipt-hash field after the receipt.
    let old_rh_field = format!("\"continuity_receipt_sha256\": \"{}\"", old_receipt_hash);
    let new_rh_field = format!("\"continuity_receipt_sha256\": \"{}\"", new_receipt_hash);
    let suffix = text[r1..].replacen(&old_rh_field, &new_rh_field, 1);
    assert_ne!(
        suffix,
        text[r1..],
        "entry receipt-hash field must be updated"
    );

    let final_text = format!(
        "{}{}{}{}{}",
        &text[..m0],
        tampered_manifest,
        between,
        receipt_text,
        suffix
    );
    std::fs::write(
        t.repo.join(".mrgs/continuity-ledger.json"),
        final_text.as_bytes(),
    )
    .unwrap();

    // The ledger is now fully hash- and chain-consistent except for the
    // altered embedded receipt object; only the exact-object binding check
    // can reject it.
    let out = t.continuity_record(&meta, &[]);
    assert_category(&out, "CONTINUITY_LEDGER_INVALID");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_63_first_record_exact_output() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let entry = &ledger["entries"][0];
    let expected = format!(
        "CONTINUITY_RECORDED mrgs phase-1 1 {} {}",
        entry["continuity_manifest_sha256"].as_str().unwrap(),
        entry["continuity_receipt_sha256"].as_str().unwrap(),
    );
    assert_eq!(stdout_str(&out), expected);
}

#[test]
fn test_obligation_64_first_publication_only_ledger_file() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let before = mrgs_listing(&t.repo);
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_success(&out);
    let after = mrgs_listing(&t.repo);
    let mut expected = before.clone();
    expected.push("continuity-ledger.json".to_string());
    expected.sort();
    assert_eq!(after, expected, "only continuity-ledger.json may be added");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_65_exact_replay_identical_output_bytes() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let out1 = t.continuity_record(&meta, &[]);
    assert_success(&out1);
    let bytes1 = t.continuity_ledger_bytes();
    let out2 = t.continuity_record(&meta, &[]);
    assert_success(&out2);
    let bytes2 = t.continuity_ledger_bytes();
    assert_eq!(
        out1.stdout, out2.stdout,
        "replay must return identical output"
    );
    assert_eq!(bytes1, bytes2, "replay must preserve every byte");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_66_replay_with_links_without_sources() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(true);
    let cont_sha = source.continuity_receipt_sha("phase-1");
    let link = link_fragment(
        "mrgs",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        Some(&cont_sha),
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links_target_repo("phase-1", &receipt, &link),
    );
    let out1 = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_success(&out1);
    let bytes1 = t.continuity_ledger_bytes();
    // Replay without any source repository must succeed
    let out2 = t.continuity_record(&meta, &[]);
    assert_success(&out2);
    assert_eq!(out1.stdout, out2.stdout);
    assert_eq!(bytes1, t.continuity_ledger_bytes());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_67_same_id_changed_metadata_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta, &[]));
    let changed =
        standard_metadata("phase-1", &receipt).replace("Primary governed", "Different governed");
    let meta2 = t.write_metadata("m2.toml", &changed);
    let out = t.continuity_record(&meta2, &[]);
    assert_category(&out, "CONTINUITY_CONFLICT");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_68_same_phase_different_id_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta, &[]));
    let changed =
        standard_metadata("phase-1", &receipt).replace("phase-1-primary", "phase-1-secondary");
    let meta2 = t.write_metadata("m2.toml", &changed);
    let out = t.continuity_record(&meta2, &[]);
    assert_category(&out, "CONTINUITY_CONFLICT");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_69_earlier_sequence_after_later_rejects() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let receipt2 = t.complete_phase2();
    let meta1 = t.write_metadata("c1.toml", &standard_metadata("phase-1", &receipt1));
    assert_success(&t.continuity_record(&meta1, &[]));
    let meta2_text = standard_metadata("phase-2", &receipt2).replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"phase-2-primary\"",
    );
    let meta2 = t.write_metadata("c2.toml", &meta2_text);
    assert_success(&t.continuity_record(&meta2, &[]));

    // Backfill phase-1 (equal or earlier completion sequence) -> conflict
    let backfill =
        standard_metadata("phase-1", &receipt1).replace("phase-1-primary", "phase-1-backfill");
    let meta3 = t.write_metadata("c3.toml", &backfill);
    let out = t.continuity_record(&meta3, &[]);
    assert_category(&out, "CONTINUITY_CONFLICT");

    // Equal sequence (phase-2 again, new id) -> conflict
    let equal = standard_metadata("phase-2", &receipt2).replace("phase-1-primary", "phase-2-again");
    let meta4 = t.write_metadata("c4.toml", &equal);
    let out = t.continuity_record(&meta4, &[]);
    assert_category(&out, "CONTINUITY_CONFLICT");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_70_temp_collision_no_truncate() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // Pre-create the first candidate temp name; publication must skip it with
    // create-new semantics and leave its bytes untouched.
    let collision_bytes = b"continuity temp collision data that must be preserved";
    let collision_path = t.repo.join(".mrgs/.continuity.0.tmp");
    std::fs::write(&collision_path, collision_bytes).unwrap();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta, &[]);
    assert_success(&out);
    let preserved = std::fs::read(&collision_path).unwrap();
    assert_eq!(
        preserved, collision_bytes,
        "pre-existing temp file bytes must be preserved"
    );
    // Remove the test-created collision file, then verify no command-created
    // temporary file remains.
    std::fs::remove_file(&collision_path).unwrap();
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_71_replacement_failure_preserves_bytes() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    let meta1_content = standard_metadata("phase-1", &receipt1);
    let meta1 = t.write_metadata("c1.toml", &meta1_content);
    assert_success(&t.continuity_record(&meta1, &[]));
    t.commit_file("c1.toml", &meta1_content);
    let ledger_bytes = t.continuity_ledger_bytes();
    let receipt2 = t.complete_phase2();

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Hold the ledger open without FILE_SHARE_DELETE so the atomic
        // replace fails while reads still succeed; the prior bytes must
        // survive.
        let ledger_path = t.repo.join(".mrgs/continuity-ledger.json");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2) // FILE_SHARE_READ | FILE_SHARE_WRITE
            .open(&ledger_path)
            .unwrap();
        let meta2_text = standard_metadata("phase-2", &receipt2).replace(
            "continuity_id = \"phase-1-primary\"",
            "continuity_id = \"phase-2-primary\"",
        );
        let meta2 = t.write_metadata("c2.toml", &meta2_text);
        let out = t.continuity_record(&meta2, &[]);
        assert_category(&out, "PERSISTENCE_FAILED");
        drop(file);
        assert_eq!(
            std::fs::read(&ledger_path).unwrap(),
            ledger_bytes,
            "replacement failure must preserve prior ledger bytes"
        );
        assert_no_temp_files(&t.repo);
    }
    #[cfg(not(windows))]
    {
        // POSIX rename over an open file succeeds; the concrete fallback
        // safety assertion is that the second publication still produces a
        // valid two-entry ledger with the first entry intact.
        assert!(cfg!(windows) == false);
        let meta2_text = standard_metadata("phase-2", &receipt2).replace(
            "continuity_id = \"phase-1-primary\"",
            "continuity_id = \"phase-2-primary\"",
        );
        let meta2 = t.write_metadata("c2.toml", &meta2_text);
        let out = t.continuity_record(&meta2, &[]);
        assert_success(&out);
        let ledger = t.get_continuity_ledger().unwrap();
        assert_eq!(ledger["entries"].as_array().unwrap().len(), 2);
        assert_eq!(
            ledger["entries"][0]["continuity_manifest"]["phase_id"]
                .as_str()
                .unwrap(),
            "phase-1"
        );
        assert_no_temp_files(&t.repo);
    }
}

#[test]
fn test_obligation_72_no_temp_files_after_success_failure() {
    let t = TestRepo::new();
    let receipt1 = t.close_phase1();
    // Success path
    let meta1_content = standard_metadata("phase-1", &receipt1);
    let meta1 = t.write_metadata("c1.toml", &meta1_content);
    assert_success(&t.continuity_record(&meta1, &[]));
    t.commit_file("c1.toml", &meta1_content);
    assert_no_temp_files(&t.repo);
    // Handled failure path (conflict before publication)
    let changed =
        standard_metadata("phase-1", &receipt1).replace("Primary governed", "Different governed");
    let meta2 = t.write_metadata("c2.toml", &changed);
    let out = t.continuity_record(&meta2, &[]);
    assert_failure(&out);
    assert_no_temp_files(&t.repo);
    // Commit both metadata files so the later Phase 4 chain sees a clean tree.
    t.commit_file("c2.toml", &changed);
    // Handled failure during publication (Windows: locked destination)
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let receipt2 = t.complete_phase2();
        let ledger_path = t.repo.join(".mrgs/continuity-ledger.json");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2) // FILE_SHARE_READ | FILE_SHARE_WRITE
            .open(&ledger_path)
            .unwrap();
        let meta3_text = standard_metadata("phase-2", &receipt2).replace(
            "continuity_id = \"phase-1-primary\"",
            "continuity_id = \"phase-2-primary\"",
        );
        let meta3 = t.write_metadata("c3.toml", &meta3_text);
        let out = t.continuity_record(&meta3, &[]);
        assert_failure(&out);
        drop(file);
        assert_no_temp_files(&t.repo);
    }
    #[cfg(not(windows))]
    {
        // Concrete fallback: a rejected record leaves no temp files.
        assert!(cfg!(windows) == false);
        assert_no_temp_files(&t.repo);
    }
}

// ============================================================================
// 27.7 Safety and Phase 1-6 regression (73-80)
// ============================================================================

#[test]
fn test_obligation_73_unsafe_target_ledger_topology_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let link = t.repo.join(".mrgs/continuity-ledger.json");
    let target = t.repo.join(".mrgs");
    let created = make_symlink_or_fallback(&link, &target);
    if created {
        let out = t.continuity_record(&meta, &[]);
        assert_category(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
    } else {
        assert!(!cfg!(windows) || !link_exists_as_symlink(&link));
        let out = t.continuity_record(&meta, &[]);
        assert_category(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
    }
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_74_unsafe_source_topology_rejects() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();

    // Unsafe source completion-ledger topology
    let source = TestRepo::build_source(false);
    let link = link_fragment(
        "source-repository",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        None,
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links("phase-1", &receipt, &link),
    );
    let clink = source.repo.join(".mrgs/completion-ledger.json");
    // Remove the real file first so the unsafe object can occupy the path.
    std::fs::remove_file(&clink).unwrap();
    let created = make_symlink_or_fallback(&clink, &source.repo.join(".mrgs"));
    assert!(created || !cfg!(windows) || !link_exists_as_symlink(&clink));
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());

    // Unsafe source continuity-ledger topology
    let source = TestRepo::build_source(true);
    let cont_sha = source.continuity_receipt_sha("phase-1");
    let link = link_fragment(
        "mrgs",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        Some(&cont_sha),
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links_target_repo("phase-1", &receipt, &link),
    );
    let clink = source.repo.join(".mrgs/continuity-ledger.json");
    std::fs::remove_file(&clink).unwrap();
    let created = make_symlink_or_fallback(&clink, &source.repo.join(".mrgs"));
    assert!(created || !cfg!(windows) || !link_exists_as_symlink(&clink));
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_category(&out, "CONTINUITY_SOURCE_INVALID");
    assert!(!t.repo.join(".mrgs/continuity-ledger.json").exists());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_75_boundaries_exempt_exact_untracked_path() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // Metadata must be committed so later Phase 4/5 commands see a clean tree
    let meta_content = standard_metadata("phase-1", &receipt);
    let meta = t.write_metadata("continuity.toml", &meta_content);
    t.commit_file("continuity.toml", &meta_content);
    assert_success(&t.continuity_record(&meta, &[]));
    let ledger_bytes = t.continuity_ledger_bytes();

    // (a) implementation begin for phase-2 succeeds with the untracked
    //     (ignored) continuity ledger present
    assert_success(&t.select_phase("phase-2"));
    write_file(&t.contract_path, &contract_toml_for_phase("phase-2"));
    git_commit(
        &t.repo,
        "contract.toml",
        contract_toml_for_phase("phase-2").as_bytes(),
    );
    assert_success(&t.draft_contract());
    let sha = t.get_draft_sha();
    assert_success(&t.accept_contract(1, &sha));
    let begin = t.impl_begin(1, &sha);
    assert_success(&begin);

    // (b) a non-exempt sibling .mrgs path is NOT exempt
    std::fs::write(t.repo.join(".mrgs/extra.json"), b"{}").unwrap();
    let begin2 = t.impl_begin(1, &sha);
    assert_category(&begin2, "GIT_DIRTY");
    std::fs::remove_file(t.repo.join(".mrgs/extra.json")).unwrap();

    // (c) audit boundary also exempts the exact path
    assert_success(&t.audit_begin("auditor1"));

    // Ledger untouched by all Phase 4/5 commands
    assert_eq!(t.continuity_ledger_bytes(), ledger_bytes);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_76_no_paths_env_or_host_persisted() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let source = TestRepo::build_source(true);
    let cont_sha = source.continuity_receipt_sha("phase-1");
    let link = link_fragment(
        "mrgs",
        &source.plan_sha(),
        "phase-1",
        &source.completion_receipt_sha("phase-1"),
        Some(&cont_sha),
    );
    let meta = t.write_metadata(
        "continuity.toml",
        &metadata_with_links_target_repo("phase-1", &receipt, &link),
    );
    let out = t.continuity_record(&meta, &[&source.repo.to_string_lossy()]);
    assert_success(&out);
    let ledger_text = String::from_utf8(t.continuity_ledger_bytes()).unwrap();

    // No filesystem paths of any kind
    let target_path = t.repo.to_string_lossy().to_string();
    let source_path = source.repo.to_string_lossy().to_string();
    assert!(!ledger_text.contains(&target_path));
    assert!(!ledger_text.contains(&source_path));
    // A real backslash in a JSON value (\\) would be a path separator.
    assert!(
        !ledger_text.contains("\\\\"),
        "no backslash paths in ledger"
    );
    // No drive/UNC/absolute prefixes
    assert!(!ledger_text.contains(":/"));
    assert!(!ledger_text.contains("://"));
    // No Git remote or URL material
    assert!(!ledger_text.contains("http"));

    // No automatically observed host or environment values
    for var in ["USERNAME", "USER", "COMPUTERNAME", "HOSTNAME", "LOGNAME"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                assert!(
                    !ledger_text.contains(&v),
                    "environment value {} persisted in ledger",
                    var
                );
            }
        }
    }
    // The only path-like field is the normalized repository-relative metadata
    // source path, which contains no drive, backslash, or absolute form.
    let ledger: serde_json::Value = serde_json::from_str(&ledger_text).unwrap();
    let src = ledger["entries"][0]["continuity_manifest"]["metadata_source_path"]
        .as_str()
        .unwrap();
    assert_eq!(src, "continuity.toml");
    assert!(!src.contains('/') || src == "continuity.toml");
}

#[test]
fn test_obligation_77_phase1_6_preserve_safe_ledger() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    let meta_content = standard_metadata("phase-1", &receipt);
    let meta = t.write_metadata("continuity.toml", &meta_content);
    t.commit_file("continuity.toml", &meta_content);
    assert_success(&t.continuity_record(&meta, &[]));
    let ledger_bytes = t.continuity_ledger_bytes();
    let assert_preserved = |t: &TestRepo| {
        assert_eq!(t.continuity_ledger_bytes(), ledger_bytes);
    };

    // phase selection
    assert_success(&t.select_phase("phase-2"));
    assert_preserved(&t);
    // contract lifecycle
    write_file(&t.contract_path, &contract_toml_for_phase("phase-2"));
    git_commit(
        &t.repo,
        "contract.toml",
        contract_toml_for_phase("phase-2").as_bytes(),
    );
    assert_success(&t.draft_contract());
    assert_preserved(&t);
    let sha = t.get_draft_sha();
    assert_success(&t.accept_contract(1, &sha));
    assert_preserved(&t);
    // implementation
    assert_success(&t.impl_begin(1, &sha));
    assert_preserved(&t);
    assert_success(&t.impl_check());
    assert_preserved(&t);
    // audit: FAIL round routes a repair; the ledger survives the repair
    // cycle and the subsequent re-audit and closeout.
    let out = t.audit_begin("auditor1");
    assert_success(&out);
    assert_preserved(&t);
    let parts = split_stdout(&out);
    let fail_report = t.make_fail_report(&parts[1], &parts[3], "auditor1");
    let fail_path = t.write_report(&fail_report);
    let rec = t.audit_record(&fail_path);
    assert_success(&rec);
    assert!(stdout_str(&rec).starts_with("REPAIR_ROUTED "));
    assert_preserved(&t);
    git_commit(
        &t.repo,
        "src/main.rs",
        b"fn main() { println!(\"fixed\"); }\n",
    );
    let rc = t.repair_check();
    assert_success(&rc);
    assert_preserved(&t);
    // re-audit to PASS, then closeout
    let out = t.audit_begin("auditor1");
    assert_success(&out);
    let parts = split_stdout(&out);
    let pass_report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let pass_path = t.write_report(&pass_report);
    let pr = t.audit_record(&pass_path);
    assert_success(&pr);
    assert!(stdout_str(&pr).starts_with("AUDIT_PASS "));
    assert_preserved(&t);
    let co = t.phase_close("phase-2");
    assert_success(&co);
    assert_preserved(&t);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_78_phase1_6_outputs_unchanged() {
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
    assert_eq!(ab_parts[1].len(), 64);
    let report = t.make_pass_report(&ab_parts[1], &ab_parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let ar = t.audit_record(&report_path);
    assert_success(&ar);
    let co = t.phase_close("phase-1");
    assert_success(&co);
    assert!(stdout_str(&co).starts_with("PHASE_CLOSED phase-1 1"));

    // Representative error category unchanged: closing an unknown phase on a
    // completed repo still yields the Phase 1-6 governance-authority category.
    let err_out = t.phase_close("phase-99");
    assert_failure(&err_out);
    assert_eq!(
        stderr_str(&err_out),
        "error: GOVERNANCE_AUTHORITY_INVALID",
        "existing error category must remain unchanged"
    );
}

#[test]
fn test_obligation_79_no_git_mutation_or_observation() {
    let t = TestRepo::new();
    let receipt = t.close_phase1();
    // The metadata file is part of the pre-command worktree state.
    let meta = t.write_metadata("continuity.toml", &standard_metadata("phase-1", &receipt));
    let head_before = git_head(&t.repo);
    let branch_before = git_branch(&t.repo);
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
    let remotes_before = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&t.repo)
            .args(["remote"])
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

    assert_success(&t.continuity_record(&meta, &[]));

    assert_eq!(git_head(&t.repo), head_before, "HEAD must not change");
    assert_eq!(git_branch(&t.repo), branch_before, "branch must not change");
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
    assert_eq!(status_after, status_before, "worktree must not change");
    let remotes_after = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&t.repo)
            .args(["remote"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(remotes_after, remotes_before, "no remote changes");
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
    assert_eq!(config_after, config_before, "no config changes");
    // No hostname or environment enumeration results in the ledger
    let ledger_text = String::from_utf8(t.continuity_ledger_bytes()).unwrap();
    for var in ["USERNAME", "USER", "COMPUTERNAME", "HOSTNAME"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                assert!(!ledger_text.contains(&v));
            }
        }
    }
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_80_no_new_dependency_no_recursive_test() {
    // No new production or dev dependency: the dependency sections must be
    // byte-identical to the frozen Phase 1-6 set.
    let cargo = std::fs::read_to_string("Cargo.toml").unwrap();
    assert!(cargo.contains(
        "[dependencies]\nclap = { version = \"4\", features = [\"derive\"] }\nserde = { version = \"1\", features = [\"derive\"] }\nserde_json = \"1\"\ntoml = \"0.8\"\nsha2 = \"0.10\"\nthiserror = \"1\""
    ));
    assert!(cargo
        .contains("[dev-dependencies]\ntempfile = \"3\"\nassert_cmd = \"2\"\npredicates = \"3\""));

    // This test binary must never invoke cargo recursively. The needles are
    // assembled dynamically so the assertion cannot match its own source or
    // the contract fixture (which legitimately lists "cargo test" as a
    // verification command string).
    let source = std::fs::read_to_string("tests/phase7.rs").unwrap();
    let cmd_needle = format!("Command::new({})", "\"cargo\"");
    assert!(!source.contains(&cmd_needle), "recursive cargo invocation");
    let cword = String::from("cargo");
    let tword = String::from("test");
    let args_needle = format!("\"{}\", \"{}\"", cword, tword);
    assert!(
        !source.contains(&args_needle),
        "recursive cargo test argument pair"
    );
}
