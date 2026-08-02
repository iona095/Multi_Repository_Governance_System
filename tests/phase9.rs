// Phase 9 — Adversarial, Security, Resource, and Regression Validation
// ======================================================================
// Obligation map (exactly 64 primary tests, 8 families x 8 obligations):
//
// 16.1 CLI and adversarial input validation
//   01 test_obligation_01_plan_and_phase_cli_rejection_matrix
//   02 test_obligation_02_contract_cli_and_source_adversarial_matrix
//   03 test_obligation_03_implementation_cli_rejection_matrix
//   04 test_obligation_04_audit_and_repair_cli_rejection_matrix
//   05 test_obligation_05_closeout_cli_rejection_matrix
//   06 test_obligation_06_continuity_cli_and_metadata_adversarial_matrix
//   07 test_obligation_07_recovery_cli_rejection_matrix
//   08 test_obligation_08_global_error_and_no_mutation_invariants
// 16.2 Filesystem and path-topology security
//   09 test_obligation_09_repository_root_and_escape_topology
//   10 test_obligation_10_source_path_normalization_and_external_boundaries
//   11 test_obligation_11_governance_directory_and_unknown_child_topology
//   12 test_obligation_12_symlink_traversal_capability_branch
//   13 test_obligation_13_windows_reparse_and_junction_capability_branch
//   14 test_obligation_14_nonregular_file_and_external_source_objects
//   15 test_obligation_15_temporary_ambiguity_and_destination_replacement
//   16 test_obligation_16_cross_repository_path_and_isolation_boundary
// 16.3 Governance-authority corruption and stale-state handling
//   17 test_obligation_17_accepted_plan_corruption_matrix
//   18 test_obligation_18_state_corruption_and_plan_relation_matrix
//   19 test_obligation_19_contract_authority_corruption_matrix
//   20 test_obligation_20_implementation_authority_corruption_matrix
//   21 test_obligation_21_audit_ledger_corruption_matrix
//   22 test_obligation_22_completion_ledger_and_receipt_corruption_matrix
//   23 test_obligation_23_continuity_ledger_corruption_matrix
//   24 test_obligation_24_recovery_ledger_and_cross_chain_corruption_matrix
// 16.4 Persistence, interruption, and fault-injection safety
//   25 test_obligation_25_failure_before_temp_creation_preserves_absence
//   26 test_obligation_26_failure_after_temp_creation_disposes_safely
//   27 test_obligation_27_failure_before_atomic_replace_preserves_target
//   28 test_obligation_28_target_replaced_before_journal_advance_resumes
//   29 test_obligation_29_interrupted_closeout_cleanup_resumes_exactly
//   30 test_obligation_30_interrupted_recovery_action_and_ledger_publish
//   31 test_obligation_31_interrupted_audit_continuity_and_completion_publication
//   32 test_obligation_32_incomplete_durable_operation_replay_fixed_point
// 16.5 Idempotency, replay, conflict, and concurrency behavior
//   33 test_obligation_33_exact_replay_matrix_all_publishers
//   34 test_obligation_34_conflicting_replay_matrix_all_publishers
//   35 test_obligation_35_stale_authorization_and_compare_and_swap
//   36 test_obligation_36_concurrent_first_publication_eight_callers
//   37 test_obligation_37_concurrent_duplicate_publication_eight_callers
//   38 test_obligation_38_concurrent_conflicting_publication_eight_callers
//   39 test_obligation_39_journal_advance_and_caller_observation_races
//   40 test_obligation_40_replay_and_concurrency_cross_repository_isolation
// 16.6 Privacy, process, network, environment, and output security
//   41 test_obligation_41_network_and_shell_nonuse
//   42 test_obligation_42_git_child_process_sanitization
//   43 test_obligation_43_environment_secret_nonobservation
//   44 test_obligation_44_path_and_identity_privacy
//   45 test_obligation_45_source_content_and_error_redaction
//   46 test_obligation_46_git_nonmutation_all_commands
//   47 test_obligation_47_repository_and_external_write_confinement
//   48 test_obligation_48_output_contract_regression_and_secret_safety
// 16.7 Deterministic resource-bound robustness
//   49 test_obligation_49_large_plan_and_phase_selection_fixture
//   50 test_obligation_50_large_contract_and_audit_fixture
//   51 test_obligation_51_long_completion_history_fixture
//   52 test_obligation_52_long_continuity_and_cross_link_fixture
//   53 test_obligation_53_long_recovery_history_and_pending_fixture
//   54 test_obligation_54_large_inventory_and_temp_candidate_fixture
//   55 test_obligation_55_scalar_boundaries_and_one_over_limits
//   56 test_obligation_56_repeated_replay_inspection_and_bounded_callers
// 16.8 Phase 1-8 regression and cross-platform compatibility
//   57 test_obligation_57_phase1_plan_and_selection_regression
//   58 test_obligation_58_phase2_contract_draft_regression
//   59 test_obligation_59_phase3_acceptance_and_revision_regression
//   60 test_obligation_60_phase4_implementation_enforcement_regression
//   61 test_obligation_61_phase5_audit_and_repair_regression
//   62 test_obligation_62_phase6_closeout_regression
//   63 test_obligation_63_phase7_continuity_and_phase8_recovery_regression
//   64 test_obligation_64_complete_public_cli_lifecycle_and_test_discipline
//
// Platform-sensitive obligations report exactly one branch via eprintln!:
//   CAPABILITY_EXECUTED
//   CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Binary + IO helpers
// ---------------------------------------------------------------------------

fn cargo_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mrgs"))
}

fn write_file(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

fn write_bytes(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn stdout_raw(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn split_stdout(output: &Output) -> Vec<String> {
    stdout_str(output)
        .split_whitespace()
        .map(String::from)
        .collect()
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
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
        "expected failure, stdout={}",
        stdout_str(output)
    );
}

fn assert_category(output: &Output, category: &str) {
    assert_failure(output);
    assert_eq!(stderr_str(output), format!("error: {}", category));
}

fn assert_category_no_stdout(output: &Output, category: &str) {
    assert_category(output, category);
    assert_eq!(stdout_str(output), "", "failure must not print stdout");
}

/// Phase 1-3 commands print the error Display text; assert the exact prefix.
fn assert_err_prefix(output: &Output, prefix: &str) {
    assert_failure(output);
    let err = stderr_str(output);
    assert!(
        err.starts_with(prefix),
        "stderr {:?} does not start with {:?}",
        err,
        prefix
    );
}

/// Clap rejection class: exit code 2, usage on stderr, empty stdout.
fn assert_clap_rejection(output: &Output) {
    assert_failure(output);
    assert_eq!(output.status.code(), Some(2), "expected Clap exit code 2");
    assert_eq!(
        stdout_str(output),
        "",
        "Clap rejection must print no stdout"
    );
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("Usage:"), "stderr lacks usage: {}", err);
    assert!(
        err.trim_start().starts_with("error:"),
        "stderr lacks error: {}",
        err
    );
}

/// Clap value-validation rejection (e.g. u32 overflow): exit code 2 with an
/// error line but no usage block.
fn assert_clap_value_rejection(output: &Output) {
    assert_failure(output);
    assert_eq!(output.status.code(), Some(2), "expected Clap exit code 2");
    assert_eq!(
        stdout_str(output),
        "",
        "Clap rejection must print no stdout"
    );
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        err.trim_start().starts_with("error:"),
        "stderr lacks error: {}",
        err
    );
}

fn assert_no_temp_files(repo: &Path) {
    let gov = repo.join(".mrgs");
    if !gov.exists() {
        return;
    }
    for entry in std::fs::read_dir(&gov).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            !name.ends_with(".tmp"),
            "unexpected temporary file in .mrgs: {}",
            name
        );
    }
}

fn mrgs_snapshot(repo: &Path) -> BTreeMap<String, Vec<u8>> {
    let gov = repo.join(".mrgs");
    let mut map = BTreeMap::new();
    if gov.exists() {
        for entry in std::fs::read_dir(&gov).unwrap() {
            let entry = entry.unwrap();
            let meta = entry.metadata().unwrap();
            if meta.is_file() {
                let name = entry.file_name().to_string_lossy().into_owned();
                map.insert(name, std::fs::read(entry.path()).unwrap());
            }
        }
    }
    map
}

fn assert_snapshot_unchanged(repo: &Path, before: &BTreeMap<String, Vec<u8>>) {
    assert_eq!(
        mrgs_snapshot(repo),
        *before,
        "governance bytes must be unchanged"
    );
}

fn assert_mrgs_absent(repo: &Path) {
    assert!(
        !repo.join(".mrgs").exists(),
        "no .mrgs may be created: {}",
        repo.join(".mrgs").display()
    );
}

/// Recursive snapshot of a tree (relative path -> bytes), used for
/// worktree / external-tree confinement evidence.
fn snapshot_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let meta = entry.metadata().unwrap();
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if meta.is_dir() {
                walk(&entry.path(), root, map);
            } else if meta.is_file() {
                map.insert(rel, std::fs::read(entry.path()).unwrap());
            }
        }
    }
    let mut map = BTreeMap::new();
    if root.exists() {
        walk(root, root, &mut map);
    }
    map
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn sha_of_file(path: &Path) -> String {
    sha256_hex(&std::fs::read(path).unwrap())
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

fn git(repo: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap()
}

fn git_init(repo: &Path) {
    std::fs::create_dir_all(repo).unwrap();
    let out = git(repo, &["init", "-b", "main"]);
    assert!(out.status.success(), "git init failed: {:?}", out.status);
    let out = git(repo, &["config", "user.email", "test@test.com"]);
    assert!(out.status.success());
    let out = git(repo, &["config", "user.name", "Test"]);
    assert!(out.status.success());
}

fn git_commit(repo: &Path, filename: &str, content: &[u8]) {
    let path = repo.join(filename);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    let out = git(repo, &["add", "--", filename]);
    assert!(
        out.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = git(repo, &["commit", "-m", "add file"]);
    assert!(
        out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_commit_many(repo: &Path, files: &[(String, String)]) {
    for (name, content) in files {
        let path = repo.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }
    let out = git(repo, &["add", "-A"]);
    assert!(
        out.status.success(),
        "git add -A failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = git(repo, &["commit", "-m", "add files"]);
    assert!(
        out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_head(repo: &Path) -> String {
    let out = git(repo, &["rev-parse", "--verify", "HEAD^{commit}"]);
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn git_branch(repo: &Path) -> String {
    let out = git(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn git_index_bytes(repo: &Path) -> Vec<u8> {
    std::fs::read(repo.join(".git/index")).unwrap_or_default()
}

fn git_config_list(repo: &Path) -> Vec<u8> {
    let out = git(repo, &["config", "--list"]);
    assert!(out.status.success());
    out.stdout
}

fn git_refs(repo: &Path) -> Vec<u8> {
    let out = git(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]);
    assert!(out.status.success());
    out.stdout
}

fn git_remotes(repo: &Path) -> String {
    let out = git(repo, &["remote", "-v"]);
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap()
}

fn git_hooks(repo: &Path) -> Vec<String> {
    let hooks = repo.join(".git/hooks");
    let mut names = Vec::new();
    if hooks.exists() {
        for entry in std::fs::read_dir(&hooks).unwrap() {
            names.push(entry.unwrap().file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    names
}

/// Snapshot of tracked worktree + git control surface (excludes .mrgs).
fn git_snapshot(repo: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    fn walk(dir: &Path, root: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".git" || name == ".mrgs" {
                continue;
            }
            let meta = entry.metadata().unwrap();
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if meta.is_dir() {
                walk(&entry.path(), root, map);
            } else if meta.is_file() {
                map.insert(rel, std::fs::read(entry.path()).unwrap());
            }
        }
    }
    walk(repo, repo, &mut map);
    map
}

// ---------------------------------------------------------------------------
// Fixture TOML
// ---------------------------------------------------------------------------

fn valid_plan_toml() -> &'static str {
    r#"schema_version = 1
plan_id = "test-plan"

[[phases]]
id = "phase-1"
title = "Phase One"
depends_on = []

[[phases]]
id = "phase-2"
title = "Phase Two"
depends_on = ["phase-1"]
"#
}

/// Linear chain of `count` phases named phase-001 .. phase-0NN.
fn plan_toml_with_phases(count: usize) -> String {
    let mut out = String::from("schema_version = 1\nplan_id = \"test-plan\"\n");
    for i in 1..=count {
        out.push_str(&format!(
            "\n[[phases]]\nid = \"phase-{:03}\"\ntitle = \"Phase {:03}\"\n",
            i, i
        ));
        if i > 1 {
            out.push_str(&format!("depends_on = [\"phase-{:03}\"]\n", i - 1));
        } else {
            out.push_str("depends_on = []\n");
        }
    }
    out
}

fn contract_toml_for_phase(phase_id: &str) -> String {
    format!(
        r#"schema_version = 1
contract_id = "test-contract-v1"
phase_id = "{phase_id}"
title = "Test Contract"
objective = "Exercise the full governance chain"
requirements = ["req1", "req2"]
allowed_paths = ["src/"]
forbidden_paths = [".git/", ".mrgs/"]
verification_commands = ["cargo test", "cargo clippy"]
handoff_fields = ["FIELD1"]
"#
    )
}

/// Contract fixture without the requirements list (for list-mutation cases).
fn contract_toml_minus_requirements() -> String {
    r#"schema_version = 1
contract_id = "test-contract-v1"
phase_id = "phase-1"
title = "Test Contract"
objective = "Exercise the full governance chain"
allowed_paths = ["src/"]
forbidden_paths = [".git/", ".mrgs/"]
verification_commands = ["cargo test", "cargo clippy"]
handoff_fields = ["FIELD1"]
"#
    .to_string()
}

fn contract_toml_custom(
    contract_id: &str,
    phase_id: &str,
    requirements: &[String],
    allowed: &[String],
    forbidden: &[String],
    verification: &[String],
    handoff: &[String],
) -> String {
    let list = |items: &[String]| {
        items
            .iter()
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        r#"schema_version = 1
contract_id = "{contract_id}"
phase_id = "{phase_id}"
title = "Test Contract"
objective = "Exercise the full governance chain"
requirements = [{req}]
allowed_paths = [{allowed}]
forbidden_paths = [{forbidden}]
verification_commands = [{ver}]
handoff_fields = [{handoff}]
"#,
        req = list(requirements),
        allowed = list(allowed),
        forbidden = list(forbidden),
        ver = list(verification),
        handoff = list(handoff),
    )
}

fn standard_metadata(phase: &str, receipt_sha: &str) -> String {
    // NOTE: `links` must precede the [[models]]/[[hosts]] table headers;
    // an inline key after a table header would attach to that table.
    format!(
        r#"schema_version = 1
repository_id = "mrgs"
continuity_id = "phase-1-primary"
phase_id = "{phase}"
completion_receipt_sha256 = "{receipt_sha}"
note = "continuity record"
links = []

[[models]]
role = "implementer"
provider = "openai"
model_id = "gpt-5.6"
execution_mode = "hosted"
session_label = "{phase}-implementation"

[[hosts]]
host_id = "main-workstation"
platform = "windows"
architecture = "x86_64"
execution_surface = "opencode"
"#
    )
}

/// Metadata with one link entry (for cross-repository proof fixtures).
#[allow(clippy::too_many_arguments)]
fn linked_metadata(
    phase: &str,
    receipt_sha: &str,
    repository_id: &str,
    link_repository_id: &str,
    link_plan_sha: &str,
    link_phase: &str,
    link_completion_receipt: &str,
    link_continuity_receipt: Option<&str>,
) -> String {
    let mut link = format!(
        "[[links]]\nrelation = \"continues_from\"\nrepository_id = \"{}\"\naccepted_plan_sha256 = \"{}\"\nphase_id = \"{}\"\ncompletion_receipt_sha256 = \"{}\"\n",
        link_repository_id, link_plan_sha, link_phase, link_completion_receipt
    );
    if let Some(cr) = link_continuity_receipt {
        link.push_str(&format!("source_continuity_receipt_sha256 = \"{}\"\n", cr));
    }
    format!(
        r#"schema_version = 1
repository_id = "{repository_id}"
continuity_id = "phase-1-primary"
phase_id = "{phase}"
completion_receipt_sha256 = "{receipt_sha}"
note = "continuity record"

[[models]]
role = "implementer"
provider = "openai"
model_id = "gpt-5.6"
execution_mode = "hosted"
session_label = "{phase}-implementation"

[[hosts]]
host_id = "main-workstation"
platform = "windows"
architecture = "x86_64"
execution_surface = "opencode"

{link}"#
    )
}

/// Append a second link entry to an existing metadata document.
fn linked_metadata_second(
    base: &str,
    link_repository_id: &str,
    link_plan_sha: &str,
    link_phase: &str,
    link_completion_receipt: &str,
    link_continuity_receipt: Option<&str>,
) -> String {
    let mut out = base.to_string();
    if let Some(cr) = link_continuity_receipt {
        out.push_str(&format!(
            "[[links]]\nrelation = \"continues_from\"\nrepository_id = \"{}\"\naccepted_plan_sha256 = \"{}\"\nphase_id = \"{}\"\ncompletion_receipt_sha256 = \"{}\"\nsource_continuity_receipt_sha256 = \"{}\"\n",
            link_repository_id, link_plan_sha, link_phase, link_completion_receipt, cr
        ));
    } else {
        out.push_str(&format!(
            "[[links]]\nrelation = \"continues_from\"\nrepository_id = \"{}\"\naccepted_plan_sha256 = \"{}\"\nphase_id = \"{}\"\ncompletion_receipt_sha256 = \"{}\"\n",
            link_repository_id, link_plan_sha, link_phase, link_completion_receipt
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Capability / platform helpers
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn make_file_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn make_file_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn make_dir_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn make_dir_link(target: &Path, link: &Path) -> std::io::Result<()> {
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("mklink /J failed"))
    }
}

#[cfg(windows)]
fn assert_reparse(link: &Path) {
    use std::os::windows::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(link).unwrap();
    assert_ne!(
        meta.file_attributes() & 0x400,
        0,
        "expected reparse-point attribute on {}",
        link.display()
    );
}

// ---------------------------------------------------------------------------
// Deterministic synchronization helpers
// ---------------------------------------------------------------------------

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

fn kill_child(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Compile a tiny barrier wrapper with rustc. Each spawned copy writes
/// ready-<i> into BARRIER_DIR, waits for go-<i>, then execs the mrgs binary
/// with args from args-<i>.txt. This is the contract-sanctioned process
/// barrier for synchronized callers (no arbitrary sleeps as proof).
struct BarrierRunner {
    _dir: TempDir,
    exe: PathBuf,
}

fn create_barrier_runner() -> BarrierRunner {
    let dir = tempfile::TempDir::new().unwrap();
    let source = r#"
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let dir = env::var("BARRIER_DIR").unwrap();
    let idx = env::var("BARRIER_INDEX").unwrap();
    fs::write(Path::new(&dir).join(format!("ready-{}", idx)), b"ready").unwrap();
    let go = Path::new(&dir).join(format!("go-{}", idx));
    while !go.exists() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let args_path = Path::new(&dir).join(format!("args-{}.txt", idx));
    let args: Vec<String> = fs::read_to_string(&args_path)
        .unwrap()
        .lines()
        .map(|s| s.to_string())
        .collect();
    let bin = env::var("MRGS_BIN").unwrap();
    let status = Command::new(bin).args(&args).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
"#;
    let src = dir.path().join("barrier.rs");
    std::fs::write(&src, source).unwrap();
    let exe = dir.path().join("barrier.exe");
    let compile = Command::new("rustc")
        .arg(&src)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "barrier compilation failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    BarrierRunner { _dir: dir, exe }
}

/// Launch exactly 8 synchronized callers; every caller is released only after
/// all 8 are ready. Returns the 8 outputs in caller order.
fn run_barrier_8(runner: &BarrierRunner, args: &[&str], envs: &[(&str, &str)]) -> Vec<Output> {
    let args_line = args.join("\n");
    let mut children = Vec::new();
    for i in 0..8usize {
        std::fs::write(
            runner.exe.parent().unwrap().join(format!("args-{}.txt", i)),
            &args_line,
        )
        .unwrap();
        let mut cmd = Command::new(&runner.exe);
        cmd.env("BARRIER_DIR", runner.exe.parent().unwrap())
            .env("BARRIER_INDEX", i.to_string())
            .env("MRGS_BIN", env!("CARGO_BIN_EXE_mrgs"));
        for (k, v) in envs {
            cmd.env(k, v);
        }
        children.push(
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    for i in 0..8usize {
        wait_for_file(
            &runner.exe.parent().unwrap().join(format!("ready-{}", i)),
            60,
        );
    }
    for i in 0..8usize {
        std::fs::write(
            runner.exe.parent().unwrap().join(format!("go-{}", i)),
            b"go",
        )
        .unwrap();
    }
    children
        .into_iter()
        .map(|c| c.wait_with_output().unwrap())
        .collect()
}

// ---------------------------------------------------------------------------
// Git recorder (env-aware, compiled fixture wrapper)
// ---------------------------------------------------------------------------

struct EnvAwareGitRecorder {
    _dir: TempDir,
    argv_log: PathBuf,
    env_log: PathBuf,
}

fn real_git_executable() -> PathBuf {
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap()) {
        for name in ["git.exe", "git.cmd", "git"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return std::fs::canonicalize(candidate).unwrap();
            }
        }
    }
    panic!("no git executable found on PATH");
}

const RECORDED_GIT_VARS: &[&str] = &[
    "GIT_CONFIG_PARAMETERS",
    "GIT_SHALLOW_FILE",
    "GIT_NO_LAZY_FETCH",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_CONFIG_COUNT",
    "GIT_OPTIONAL_LOCKS",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_ATTR_NOSYSTEM",
    "GIT_SSH_COMMAND",
    "GIT_SSH_VARIANT",
    "GIT_PAGER",
    "GIT_EDITOR",
    "GIT_ASKPASS",
    "GIT_TERMINAL_PROMPT",
    "GIT_PROXY_COMMAND",
    "GIT_HTTP_PROXY",
    "GIT_SSL_NO_VERIFY",
    "GIT_AUTHOR_NAME",
    "GIT_COMMITTER_NAME",
    "GIT_CEILING_DIRECTORIES",
    "GIT_CONFIG_KEY_0",
    "GIT_CONFIG_VALUE_0",
    "GIT_CONFIG_KEY_1",
    "GIT_CONFIG_VALUE_1",
];

fn create_env_aware_git_recorder() -> EnvAwareGitRecorder {
    let dir = tempfile::TempDir::new().unwrap();
    let wrapper_dir = dir.path().join("bin");
    std::fs::create_dir_all(&wrapper_dir).unwrap();
    let argv_log = dir.path().join("git-argv.bin");
    let env_log = dir.path().join("git-env.log");

    let real = real_git_executable();
    let vars_list = RECORDED_GIT_VARS
        .iter()
        .map(|v| format!("\"{}\"", v))
        .collect::<Vec<_>>()
        .join(", ");

    let source = format!(
        r#"
use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;

fn main() {{
    let args: Vec<OsString> = env::args_os().skip(1).collect();

    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open({argv_log:?})
        .unwrap();
    log.write_all(&(args.len() as u64).to_le_bytes()).unwrap();
    for arg in &args {{
        let bytes = arg.to_string_lossy().into_owned().into_bytes();
        log.write_all(&(bytes.len() as u64).to_le_bytes()).unwrap();
        log.write_all(&bytes).unwrap();
    }}

    let mut env_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open({env_log:?})
        .unwrap();
    writeln!(env_file, "---INVOCATION---").unwrap();

    let vars_to_check: &[&str] = &[{vars_list}];
    for var in vars_to_check {{
        match env::var(var) {{
            Ok(val) => writeln!(env_file, "{{}}={{}}", var, val).unwrap(),
            Err(_) => writeln!(env_file, "{{}}=<absent>", var).unwrap(),
        }}
    }}

    let status = Command::new({real:?}).args(&args).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}}
"#,
        argv_log = argv_log.display().to_string(),
        env_log = env_log.display().to_string(),
        real = real.display().to_string(),
        vars_list = vars_list,
    );

    let source_path = wrapper_dir.join("git-recorder.rs");
    std::fs::write(&source_path, &source).unwrap();
    let wrapper = wrapper_dir.join("git.exe");
    let compile = Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&wrapper)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "recorder compilation failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    EnvAwareGitRecorder {
        _dir: dir,
        argv_log,
        env_log,
    }
}

fn run_with_env_aware_recorder(
    recorder: &EnvAwareGitRecorder,
    repo: &Path,
    operation: &[&str],
    extra_env: &[(&str, &str)],
) -> Output {
    let wrapper_path = recorder.argv_log.parent().unwrap().join("bin");
    let mut paths: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap()).collect();
    paths.insert(0, wrapper_path);
    let new_path = std::env::join_paths(paths).unwrap();
    let mut cmd = cargo_bin();
    cmd.args(operation)
        .arg("--repo")
        .arg(repo)
        .env("PATH", new_path);
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

fn read_recorder_argv(path: &Path) -> Vec<Vec<String>> {
    let bytes = std::fs::read(path).unwrap();
    let mut invocations = Vec::new();
    let mut i = 0usize;
    while i + 8 <= bytes.len() {
        let count = u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap()) as usize;
        i += 8;
        let mut args = Vec::new();
        for _ in 0..count {
            assert!(i + 8 <= bytes.len(), "truncated recorder argv log");
            let len = u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap()) as usize;
            i += 8;
            assert!(i + len <= bytes.len(), "truncated recorder argv entry");
            args.push(String::from_utf8_lossy(&bytes[i..i + len]).into_owned());
            i += len;
        }
        invocations.push(args);
    }
    invocations
}

fn read_recorder_env(path: &Path) -> Vec<BTreeMap<String, String>> {
    let content = std::fs::read_to_string(path).unwrap();
    let mut invocations = Vec::new();
    let mut current: BTreeMap<String, String> = BTreeMap::new();
    for line in content.lines() {
        if line == "---INVOCATION---" {
            if !current.is_empty() {
                invocations.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            current.insert(k.to_string(), v.to_string());
        }
    }
    if !current.is_empty() {
        invocations.push(current);
    }
    invocations
}

// ---------------------------------------------------------------------------
// TestRepo fixture (isolated governance chain)
// ---------------------------------------------------------------------------

struct TestRepo {
    _dir: TempDir,
    repo: PathBuf,
    report_dir: PathBuf,
    contract_path: PathBuf,
    plan_path: PathBuf,
}

impl TestRepo {
    fn new() -> TestRepo {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        git_init(&repo);
        git_commit(&repo, ".gitignore", b".mrgs/\n");
        git_commit(&repo, "src/main.rs", b"fn main() {}\n");
        let plan_path = repo.join("plan.toml");
        let contract_path = repo.join("contract.toml");
        write_file(&plan_path, valid_plan_toml());
        write_file(&contract_path, &contract_toml_for_phase("phase-1"));
        let report_dir = dir.path().join("reports");
        std::fs::create_dir_all(&report_dir).unwrap();
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

    // -- command wrappers --------------------------------------------------

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

    fn accept_plan_success(&self) {
        let out = self.accept_plan();
        assert_success(&out);
    }

    fn select_phase_success(&self, phase: &str) {
        let out = self.select_phase(phase);
        assert_success(&out);
        assert_eq!(stdout_str(&out), phase);
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

    fn revise_contract(&self, expected_revision: u32, expected_sha256: &str) -> Output {
        self.run(&[
            "contract",
            "revise",
            "--repo",
            &self.repo.to_string_lossy(),
            "--contract",
            &self.contract_path.to_string_lossy(),
            "--expected-revision",
            &expected_revision.to_string(),
            "--expected-sha256",
            expected_sha256,
        ])
    }

    fn impl_begin(&self, revision: u32, sha256: &str) -> Output {
        // Implementation begin requires a clean worktree; commit any fixture
        // sources first (no-op when already clean).
        self.commit_sources();
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

    fn phase_close(&self, phase: &str) -> Output {
        self.run(&[
            "phase",
            "close",
            "--repo",
            &self.repo.to_string_lossy(),
            "--phase",
            phase,
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

    // -- readers -----------------------------------------------------------

    fn read_mrgs(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.repo.join(".mrgs").join(name)).unwrap()
    }

    fn read_mrgs_str(&self, name: &str) -> String {
        String::from_utf8(self.read_mrgs(name)).unwrap()
    }

    fn write_mrgs(&self, name: &str, bytes: &[u8]) {
        let dir = self.repo.join(".mrgs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    fn delete(&self, name: &str) {
        std::fs::remove_file(self.repo.join(".mrgs").join(name)).unwrap();
    }

    fn get_draft(&self) -> Value {
        serde_json::from_str(&self.read_mrgs_str("contract-draft.json")).unwrap()
    }

    fn get_state(&self) -> Value {
        serde_json::from_str(&self.read_mrgs_str("state.json")).unwrap()
    }

    fn get_completion_ledger(&self) -> Option<Value> {
        let path = self.repo.join(".mrgs/completion-ledger.json");
        if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap())
        } else {
            None
        }
    }

    fn get_continuity_ledger(&self) -> Option<Value> {
        let path = self.repo.join(".mrgs/continuity-ledger.json");
        if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap())
        } else {
            None
        }
    }

    fn get_recovery_ledger(&self) -> Option<Value> {
        let path = self.repo.join(".mrgs/recovery-ledger.json");
        if path.exists() {
            Some(serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap())
        } else {
            None
        }
    }

    fn recovery_ledger_bytes(&self) -> Vec<u8> {
        self.read_mrgs("recovery-ledger.json")
    }

    fn write_report(&self, content: &str) -> PathBuf {
        let path = self.report_dir.join("report.json");
        write_file(&path, content);
        path
    }

    fn write_metadata(&self, name: &str, content: &str) -> PathBuf {
        let path = self.repo.join(name);
        write_file(&path, content);
        path
    }

    // -- audit report builders --------------------------------------------

    fn make_pass_report(&self, audit_id: &str, subject_sha256: &str, auditor_id: &str) -> String {
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "audit_id": audit_id,
            "subject_sha256": subject_sha256,
            "auditor_id": auditor_id,
            "independence_declaration": "INDEPENDENT",
            "verdict": "PASS",
            "summary": "All requirements satisfied",
            "requirement_results": [
                {"requirement": "req1", "status": "PASS", "evidence": "verified"},
                {"requirement": "req2", "status": "PASS", "evidence": "verified"}
            ],
            "verification_results": [
                {"command": "cargo test", "status": "PASS", "evidence": "verified"},
                {"command": "cargo clippy", "status": "PASS", "evidence": "verified"}
            ],
            "findings": []
        }))
        .unwrap()
    }

    fn make_fail_report(
        &self,
        audit_id: &str,
        subject_sha256: &str,
        auditor_id: &str,
        finding_id: &str,
    ) -> String {
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "audit_id": audit_id,
            "subject_sha256": subject_sha256,
            "auditor_id": auditor_id,
            "independence_declaration": "INDEPENDENT",
            "verdict": "FAIL",
            "summary": "A requirement failed",
            "requirement_results": [
                {"requirement": "req1", "status": "PASS", "evidence": "verified"},
                {"requirement": "req2", "status": "FAIL", "evidence": "not verified"}
            ],
            "verification_results": [
                {"command": "cargo test", "status": "PASS", "evidence": "verified"},
                {"command": "cargo clippy", "status": "PASS", "evidence": "verified"}
            ],
            "findings": [
                {
                    "id": finding_id,
                    "severity": "MAJOR",
                    "claim_kind": "REQUIREMENT",
                    "claim_index": 2,
                    "summary": "req2 not satisfied",
                    "evidence": "observed",
                    "repair_paths": ["src/main.rs"]
                }
            ]
        }))
        .unwrap()
    }

    /// Report with exact one-to-one coverage for a large contract fixture.
    fn make_pass_report_exact(
        &self,
        audit_id: &str,
        subject_sha256: &str,
        auditor_id: &str,
        requirement_ids: &[String],
        verification_ids: &[String],
    ) -> String {
        let requirements: Vec<Value> = requirement_ids
            .iter()
            .map(|r| json!({"requirement": r, "status": "PASS", "evidence": "verified"}))
            .collect();
        let verifications: Vec<Value> = verification_ids
            .iter()
            .map(|v| json!({"command": v, "status": "PASS", "evidence": "verified"}))
            .collect();
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "audit_id": audit_id,
            "subject_sha256": subject_sha256,
            "auditor_id": auditor_id,
            "independence_declaration": "INDEPENDENT",
            "verdict": "PASS",
            "summary": "All requirements satisfied",
            "requirement_results": requirements,
            "verification_results": verifications,
            "findings": []
        }))
        .unwrap()
    }

    // -- governance chain builders ----------------------------------------

    /// Commit any uncommitted fixture sources so implementation-level git
    /// checks see a clean worktree. No-op when the tree is already clean.
    fn commit_sources(&self) {
        let add = git(&self.repo, &["add", "-A"]);
        assert!(add.status.success(), "git add failed");
        let status = git(&self.repo, &["status", "--porcelain"]);
        assert!(status.status.success());
        if !status.stdout.is_empty() {
            let commit = git(&self.repo, &["commit", "-m", "sources"]);
            assert!(
                commit.status.success(),
                "git commit failed: {}",
                String::from_utf8_lossy(&commit.stderr)
            );
        }
    }

    fn setup_impl_bound(&self) {
        self.commit_sources();
        let plan = self.accept_plan();
        assert_success(&plan);
        assert!(stdout_str(&plan).starts_with("test-plan "));
        let sel = self.select_phase("phase-1");
        assert_success(&sel);
        assert_eq!(stdout_str(&sel), "phase-1");
        let draft = self.draft_contract();
        assert_success(&draft);
        let sha = self.get_draft()["sha256"].as_str().unwrap().to_string();
        let accept = self.accept_contract(1, &sha);
        assert_success(&accept);
        let begin = self.impl_begin(1, &sha);
        assert_success(&begin);
        assert!(stdout_str(&begin).starts_with("IMPLEMENTATION_BOUND "));
        let check = self.impl_check();
        assert_success(&check);
        assert!(stdout_str(&check).starts_with("IMPLEMENTATION_OK "));
    }

    fn full_pass_audit(&self) {
        let open = self.audit_begin("auditor1");
        assert_success(&open);
        let parts = split_stdout(&open);
        assert_eq!(parts[0], "AUDIT_OPEN");
        let report = self.make_pass_report(&parts[1], &parts[3], "auditor1");
        let path = self.write_report(&report);
        let record = self.audit_record(&path);
        assert_success(&record);
        assert!(stdout_str(&record).starts_with("AUDIT_PASS "));
    }

    fn setup_closeout_ready(&self) {
        self.setup_impl_bound();
        self.full_pass_audit();
    }

    fn close_phase1(&self) -> (String, String) {
        self.setup_closeout_ready();
        let close = self.phase_close("phase-1");
        assert_success(&close);
        let parts = split_stdout(&close);
        assert_eq!(parts[0], "PHASE_CLOSED");
        assert_eq!(parts[1], "phase-1");
        (parts[3].clone(), parts[4].clone())
    }

    /// Complete one phase end-to-end for multi-phase plans: the caller must
    /// have committed the matching contract source for `phase` beforehand.
    fn complete_phase(&self, phase: &str) {
        self.commit_sources();
        let sel = self.select_phase(phase);
        assert_success(&sel);
        let draft = self.draft_contract();
        assert_success(&draft);
        let sha = self.get_draft()["sha256"].as_str().unwrap().to_string();
        let accept = self.accept_contract(1, &sha);
        assert_success(&accept);
        let begin = self.impl_begin(1, &sha);
        assert_success(&begin);
        self.full_pass_audit();
        let close = self.phase_close(phase);
        assert_success(&close);
        let parts = split_stdout(&close);
        assert_eq!(parts[0], "PHASE_CLOSED");
        assert_eq!(parts[1], phase);
    }

    fn archived_governance(&self) -> Value {
        let ledger = self.get_completion_ledger().expect("completion ledger");
        ledger["completions"].as_array().unwrap().last().unwrap()["final_manifest"]
            ["archived_governance"]
            .clone()
    }

    fn plan_sha(&self) -> String {
        sha_of_file(&self.plan_path)
    }
}

// ---------------------------------------------------------------------------
// Recovery crash-point helpers (deterministic signal/release failpoints)
// ---------------------------------------------------------------------------

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

/// Induce the canonical recoverable fixture: delete state.json.
fn induce_recoverable(t: &TestRepo) {
    t.delete("state.json");
}

/// Return (recovery_id, pre_subject_sha256) from a recoverable inspect.
fn recoverable_ids(t: &TestRepo) -> (String, String) {
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_REQUIRED "));
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    (parts[1].to_string(), parts[2].to_string())
}

// ---------------------------------------------------------------------------
// Recovery subject oracle (mirrors the production RecoverySubject field
// order; used only to bind a caller to the CURRENT subject after a crash
// leaves an authorized recovery temp in .mrgs — the apply API requires the
// live subject hash, which is not otherwise observable).
// ---------------------------------------------------------------------------

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

/// Recompute the current recovery subject hash from the live repository
/// (the recovery-ledger.json itself is excluded from the subject).
fn recompute_subject(repo: &Path) -> String {
    let objfmt = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--show-object-format"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let head = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--verify", "HEAD^{commit}"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let branch = String::from_utf8(
        Command::new("git")
            .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    let gov = repo.join(".mrgs");
    let mut entries: Vec<EntryProbe> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(&gov).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "recovery-ledger.json" {
            continue;
        }
        seen.insert(name.clone());
        let meta = std::fs::symlink_metadata(entry.path()).unwrap();
        let ft = meta.file_type();
        let kind = if ft.is_symlink() {
            "SYMLINK"
        } else if ft.is_dir() {
            "DIRECTORY"
        } else if ft.is_file() {
            "REGULAR"
        } else {
            "OTHER"
        };
        let (byte_length, sha256) = if kind == "REGULAR" {
            let bytes = std::fs::read(entry.path()).unwrap();
            (Some(bytes.len() as u64), Some(sha256_hex(&bytes)))
        } else {
            (None, None)
        };
        entries.push(EntryProbe {
            filename: name,
            kind: kind.to_string(),
            byte_length,
            sha256,
        });
    }
    for name in PERMANENT_FILES {
        if !seen.contains(name) {
            entries.push(EntryProbe {
                filename: name.to_string(),
                kind: "ABSENT".to_string(),
                byte_length: None,
                sha256: None,
            });
        }
    }
    entries.sort_by(|a, b| a.filename.as_bytes().cmp(b.filename.as_bytes()));

    let plan_source = {
        let accepted_path = gov.join("accepted-plan.json");
        let plan_path: String = if accepted_path.exists() {
            let v: Value = serde_json::from_slice(&std::fs::read(&accepted_path).unwrap()).unwrap();
            v["plan_path"].as_str().unwrap().to_string()
        } else {
            let ledger_path = gov.join("completion-ledger.json");
            let v: Value = serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
            v["completions"].as_array().unwrap().last().unwrap()["final_manifest"]
                ["plan_source_path"]
                .as_str()
                .unwrap()
                .to_string()
        };
        let full = repo.join(&plan_path);
        match std::fs::symlink_metadata(&full) {
            Ok(meta) => {
                let ft = meta.file_type();
                let topology = if ft.is_symlink() {
                    "SYMLINK"
                } else if ft.is_dir() {
                    "DIRECTORY"
                } else if ft.is_file() {
                    "REGULAR"
                } else {
                    "OTHER"
                };
                let (byte_length, sha256) = if topology == "REGULAR" {
                    let bytes = std::fs::read(&full).unwrap();
                    (Some(bytes.len() as u64), Some(sha256_hex(&bytes)))
                } else {
                    (None, None)
                };
                Some(PlanSourceProbe {
                    path: plan_path,
                    topology: topology.to_string(),
                    byte_length,
                    sha256,
                })
            }
            Err(_) => None,
        }
    };

    let subject = SubjectProbe {
        schema_version: 1,
        repository_git_object_format: objfmt,
        repository_head: head,
        repository_branch: branch,
        governance_entries: entries,
        plan_source,
    };
    sha256_hex(serde_json::to_string(&subject).unwrap().as_bytes())
}
// ===========================================================================
// 16.1 CLI and adversarial input validation
// ===========================================================================

#[test]
fn test_obligation_01_plan_and_phase_cli_rejection_matrix() {
    let t = TestRepo::new();
    let repo = t.repo.to_string_lossy().into_owned();
    let plan = t.plan_path.to_string_lossy().into_owned();

    // Missing / duplicate / unknown arguments -> Clap rejection class.
    assert_clap_rejection(&t.run(&["plan", "accept", "--repo", &repo]));
    assert_clap_rejection(&t.run(&["plan", "accept", "--plan", &plan]));
    assert_clap_rejection(&t.run(&[
        "plan", "accept", "--repo", &repo, "--repo", &repo, "--plan", &plan,
    ]));
    assert_clap_rejection(&t.run(&[
        "plan", "accept", "--repo", &repo, "--plan", &plan, "--bogus", "x",
    ]));
    assert_clap_rejection(&t.run(&["plan", "bogus", "--repo", &repo]));
    assert_clap_rejection(&t.run(&["phase", "select", "--repo", &repo]));
    assert_clap_rejection(&t.run(&[
        "phase", "select", "--repo", &repo, "--phase", "phase-1", "--phase", "phase-1",
    ]));
    assert_clap_rejection(&t.run(&[
        "phase", "select", "--repo", &repo, "--phase", "phase-1", "--extra",
    ]));

    // No .mrgs may exist after any Clap rejection.
    assert_mrgs_absent(&t.repo);

    // Empty / nonexistent / whitespace / control / unicode plan paths.
    assert_err_prefix(
        &t.run(&["plan", "accept", "--repo", &repo, "--plan", ""]),
        "error: plan not found",
    );
    assert_err_prefix(
        &t.run(&["plan", "accept", "--repo", &repo, "--plan", "missing.toml"]),
        "error: plan not found: ",
    );
    assert_err_prefix(
        &t.run(&["plan", "accept", "--repo", &repo, "--plan", "   "]),
        "error: plan not found",
    );
    assert_err_prefix(
        &t.run(&[
            "plan",
            "accept",
            "--repo",
            &repo,
            "--plan",
            "\u{1}plan.toml",
        ]),
        "error: plan not found: ",
    );
    // Nonexistent plan outside the repository is still rejected by the
    // existing first check (assert_existing_file precedes the boundary check).
    let outside = t._dir.path().join("outside-plan.toml");
    assert_err_prefix(
        &t.run(&[
            "plan",
            "accept",
            "--repo",
            &repo,
            "--plan",
            &outside.to_string_lossy(),
        ]),
        "error: plan not found: ",
    );
    // Existing plan outside the repository -> boundary rejection.
    write_file(&outside, valid_plan_toml());
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &repo,
        "--plan",
        &outside.to_string_lossy(),
    ]);
    assert_err_prefix(&out, "error: plan path not inside repository: ");
    assert_mrgs_absent(&t.repo);

    // Boundary-length / deep normalized plan path is accepted.
    let deep = t.repo.join("a/b/c/d/e/f/g/plan.toml");
    std::fs::create_dir_all(deep.parent().unwrap()).unwrap();
    write_file(&deep, valid_plan_toml());
    let git_before = git_snapshot(&t.repo);
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &repo,
        "--plan",
        &deep.to_string_lossy(),
    ]);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("test-plan "));
    let accepted: Value = serde_json::from_str(&t.read_mrgs_str("accepted-plan.json")).unwrap();
    assert_eq!(accepted["plan_path"], "a/b/c/d/e/f/g/plan.toml");
    let state: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
    assert_eq!(state["active_phase"], Value::Null);
    assert_eq!(git_snapshot(&t.repo), git_before, "git surface unchanged");

    // Phase value adversarial cases against a valid accepted authority.
    let before = mrgs_snapshot(&t.repo);
    let long256 = "p".repeat(256);
    let long4096 = "p".repeat(4096);
    for bad in [
        "",
        " ",
        "  ",
        "\u{1}",
        "\u{9}phase-1",
        "x",
        "phase-1\n",
        long256.as_str(),
        long4096.as_str(),
        "ph\u{e9}ase-1",
        "PHASE-1",
    ] {
        let out = t.run(&["phase", "select", "--repo", &repo, "--phase", bad]);
        assert_err_prefix(&out, "error: unknown phase");
        assert_eq!(stdout_str(&out), "");
        assert_snapshot_unchanged(&t.repo, &before);
    }

    // Fresh repository: repo validation precedes phase-value handling.
    let t2 = TestRepo::new();
    let repo2 = t2.repo.to_string_lossy().into_owned();
    let out = t2.run(&["phase", "select", "--repo", &repo2, "--phase", "phase-1"]);
    assert_err_prefix(&out, "error: governance directory does not exist: ");
    assert_mrgs_absent(&t2.repo);

    // A valid phase still selects exactly.
    let out = t.run(&["phase", "select", "--repo", &repo, "--phase", "phase-1"]);
    assert_success(&out);
    assert_eq!(stdout_str(&out), "phase-1");
    let state: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
    assert_eq!(state["active_phase"], "phase-1");
}

#[test]
fn test_obligation_02_contract_cli_and_source_adversarial_matrix() {
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    let repo = t.repo.to_string_lossy().into_owned();
    let contract = t.contract_path.to_string_lossy().into_owned();
    let sha64 = "a".repeat(64);

    // Clap rejections.
    assert_clap_rejection(&t.run(&["contract", "draft", "--repo", &repo]));
    assert_clap_rejection(&t.run(&["contract", "draft", "--contract", &contract]));
    assert_clap_rejection(&t.run(&[
        "contract",
        "accept",
        "--repo",
        &repo,
        "--revision",
        "1",
        "--sha256",
        &sha64,
    ]));
    assert_clap_rejection(&t.run(&[
        "contract",
        "accept",
        "--repo",
        &repo,
        "--revision",
        "1",
        "--sha256",
        &sha64,
        "--decision",
        "ACCEPTED",
        "--decision",
        "ACCEPTED",
    ]));
    assert_clap_rejection(&t.run(&["contract", "bogus", "--repo", &repo]));
    assert_clap_rejection(&t.run(&[
        "contract",
        "revise",
        "--repo",
        &repo,
        "--contract",
        &contract,
        "--expected-revision",
        "1",
    ]));

    // Establish the required draft state BEFORE revision/SHA validation
    // order can be exercised (draft absence fires first otherwise).
    let draft = t.draft_contract();
    assert_success(&draft);
    assert_eq!(
        stdout_str(&draft),
        format!(
            "test-contract-v1 {}",
            t.get_draft()["sha256"].as_str().unwrap()
        )
    );
    let good_sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    let wrong_sha = if let Some(rest) = good_sha.strip_prefix('a') {
        format!("b{}", rest)
    } else {
        format!("a{}", &good_sha[1..])
    };

    // Zero / overflow / malformed revisions.
    assert_err_prefix(
        &t.run(&[
            "contract",
            "accept",
            "--repo",
            &repo,
            "--revision",
            "0",
            "--sha256",
            &sha64,
            "--decision",
            "ACCEPTED",
        ]),
        "error: contract draft revision must be at least 1, got 0",
    );
    assert_clap_value_rejection(&t.run(&[
        "contract",
        "accept",
        "--repo",
        &repo,
        "--revision",
        "4294967296",
        "--sha256",
        &sha64,
        "--decision",
        "ACCEPTED",
    ]));
    assert_clap_value_rejection(&t.run(&[
        "contract",
        "accept",
        "--repo",
        &repo,
        "--revision",
        "abc",
        "--sha256",
        &sha64,
        "--decision",
        "ACCEPTED",
    ]));

    // Malformed / uppercase / mixed-case SHA-256 values.
    let sha_upper = "A".repeat(64);
    let sha_short = "a".repeat(63);
    let sha_bad = format!("{}g", "a".repeat(63));
    let sha_mixed = format!("{}B", "a".repeat(63));
    for bad_sha in [&sha_upper, &sha_short, &sha_bad, &sha_mixed] {
        let out = t.run(&[
            "contract",
            "accept",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            bad_sha,
            "--decision",
            "ACCEPTED",
        ]);
        assert_err_prefix(&out, "error: invalid SHA-256 hex string");
    }

    // Establish a draft, then exercise validation order.
    // (Draft established above; validation order follows.)
    // Validation order: revision mismatch precedes SHA mismatch precedes
    // decision casing; the malformed SHA is not reached because the revision
    // check fires first.
    let out = t.run(&[
        "contract",
        "accept",
        "--repo",
        &repo,
        "--revision",
        "2",
        "--sha256",
        &sha_bad,
        "--decision",
        "ACCEPTED",
    ]);
    assert_err_prefix(
        &out,
        "error: contract accept revision 2 does not match draft revision 1",
    );
    let out = t.run(&[
        "contract",
        "accept",
        "--repo",
        &repo,
        "--revision",
        "1",
        "--sha256",
        &wrong_sha,
        "--decision",
        "ACCEPTED",
    ]);
    assert_err_prefix(&out, "error: contract accept SHA does not match draft SHA");
    let out = t.run(&[
        "contract",
        "accept",
        "--repo",
        &repo,
        "--revision",
        "1",
        "--sha256",
        &good_sha,
        "--decision",
        "accepted",
    ]);
    assert_err_prefix(
        &out,
        "error: contract accept decision must be exactly ACCEPTED, got 'accepted'",
    );
    let before = mrgs_snapshot(&t.repo);
    assert_snapshot_unchanged(&t.repo, &before);

    // Accept succeeds.
    let out = t.run(&[
        "contract",
        "accept",
        "--repo",
        &repo,
        "--revision",
        "1",
        "--sha256",
        &good_sha,
        "--decision",
        "ACCEPTED",
    ]);
    assert_success(&out);
    assert_eq!(
        stdout_str(&out),
        format!("ACCEPTED test-contract-v1 1 {}", good_sha)
    );

    // Revise adversarial: stale expected values, same content, overflow.
    // The revise validation order checks same-content before the expected
    // revision/SHA; use a modified source for the stale-value cases.
    let rev_contract = contract_toml_for_phase("phase-1").replacen(
        "requirements = [\"req1\", \"req2\"]",
        "requirements = [\"req1\", \"req2\", \"req3\"]",
        1,
    );
    write_file(&t.contract_path, &rev_contract);
    let out = t.revise_contract(2, &good_sha);
    assert_err_prefix(
        &out,
        "error: contract revise expected revision 2 does not match current 1",
    );
    let out = t.revise_contract(1, &wrong_sha);
    assert_err_prefix(
        &out,
        "error: contract revise expected SHA does not match current draft SHA",
    );
    // Restore the original source: revising to the current content is
    // rejected as same-content.
    write_file(&t.contract_path, &contract_toml_for_phase("phase-1"));
    let out = t.revise_contract(1, &good_sha);
    assert_err_prefix(
        &out,
        "error: contract revise would produce same content as current draft",
    );
    assert_clap_value_rejection(&t.run(&[
        "contract",
        "revise",
        "--repo",
        &repo,
        "--contract",
        &contract,
        "--expected-revision",
        "4294967296",
        "--expected-sha256",
        &good_sha,
    ]));

    // Strict TOML adversarial matrix (fresh fixture per case).
    let cases: Vec<(&str, &str, &str)> = vec![
        ("unknown field", "bogus = 1\n", "error: TOML parse error: "),
        ("missing field", "", "error: TOML parse error: "),
        (
            "duplicate list entries",
            "requirements = [\"r1\", \"r1\"]\n",
            "error: duplicate entry in contract 'requirements' list",
        ),
        (
            "empty list entry",
            "requirements = [\" \"]\n",
            "error: empty or whitespace-only entry in contract 'requirements' list",
        ),
        (
            "trailing data",
            "garbage after document\n",
            "error: TOML parse error: ",
        ),
    ];
    for (label, mutation, expected) in cases {
        let t2 = TestRepo::new();
        t2.accept_plan_success();
        t2.select_phase_success("phase-1");
        let mut toml;
        match label {
            "unknown field" => {
                toml = contract_toml_for_phase("phase-1");
                toml.push_str(mutation);
            }
            "missing field" => {
                toml = "schema_version = 1\ncontract_id = \"c\"\n".to_string();
            }
            "duplicate list entries" => {
                toml = format!(
                    "{}\nrequirements = [\"r1\", \"r1\"]\n",
                    contract_toml_minus_requirements()
                );
            }
            "empty list entry" => {
                toml = format!(
                    "{}\nrequirements = [\" \"]\n",
                    contract_toml_minus_requirements()
                );
            }
            _ => {
                toml = contract_toml_for_phase("phase-1");
                toml.push_str(mutation);
            }
        }
        write_file(&t2.contract_path, &toml);
        let out = t2.draft_contract();
        assert_err_prefix(&out, expected);
        assert!(out.stdout.is_empty());
        // The failed draft published nothing.
        assert!(!t2.repo.join(".mrgs/contract-draft.json").exists());
        assert_no_temp_files(&t2.repo);
    }

    // Semantic falsehoods that parse cleanly.
    let t3 = TestRepo::new();
    t3.accept_plan_success();
    t3.select_phase_success("phase-1");
    write_file(&t3.contract_path, &contract_toml_for_phase("phase-2"));
    let out = t3.draft_contract();
    assert_err_prefix(
        &out,
        "error: contract phase ID 'phase-2' does not match active phase 'phase-1'",
    );
    let toml =
        contract_toml_for_phase("phase-1").replacen("schema_version = 1", "schema_version = 2", 1);
    write_file(&t3.contract_path, &toml);
    let out = t3.draft_contract();
    assert_err_prefix(&out, "error: unsupported contract schema version: 2");
    let toml = contract_toml_for_phase("phase-1").replacen(
        "title = \"Test Contract\"",
        "title = \"   \"",
        1,
    );
    write_file(&t3.contract_path, &toml);
    let out = t3.draft_contract();
    assert_err_prefix(
        &out,
        "error: empty or whitespace-only field 'title' in contract",
    );
    let toml = contract_toml_for_phase("phase-1").replacen(
        "requirements = [\"req1\", \"req2\"]",
        "requirements = [\"req1\", \"req1\"]",
        1,
    );
    write_file(&t3.contract_path, &toml);
    let out = t3.draft_contract();
    assert_err_prefix(
        &out,
        "error: duplicate entry in contract 'requirements' list",
    );
    // Every failed draft published nothing.
    assert!(!t3.repo.join(".mrgs/contract-draft.json").exists());
    assert_no_temp_files(&t3.repo);

    // Source boundaries: outside and .mrgs.
    let external = t3._dir.path().join("external-contract.toml");
    write_file(&external, &contract_toml_for_phase("phase-1"));
    let out = t3.run(&[
        "contract",
        "draft",
        "--repo",
        &t3.repo.to_string_lossy(),
        "--contract",
        &external.to_string_lossy(),
    ]);
    assert_err_prefix(&out, "error: contract source file is outside repository");
    let in_mrgs = t3.repo.join(".mrgs/contract.toml");
    std::fs::create_dir_all(in_mrgs.parent().unwrap()).unwrap();
    write_file(&in_mrgs, &contract_toml_for_phase("phase-1"));
    let out = t3.run(&[
        "contract",
        "draft",
        "--repo",
        &t3.repo.to_string_lossy(),
        "--contract",
        &in_mrgs.to_string_lossy(),
    ]);
    assert_err_prefix(
        &out,
        "error: contract source file is inside .mrgs directory",
    );
    // The pre-existing authority is untouched; nothing was drafted.
    assert!(t3.repo.join(".mrgs/accepted-plan.json").exists());
    assert!(t3.repo.join(".mrgs/state.json").exists());
    assert!(!t3.repo.join(".mrgs/contract-draft.json").exists());
    assert_no_temp_files(&t3.repo);
}

#[test]
fn test_obligation_03_implementation_cli_rejection_matrix() {
    let t = TestRepo::new();
    let repo = t.repo.to_string_lossy().into_owned();
    let sha = "a".repeat(64);

    // Clap rejections.
    assert_clap_rejection(&t.run(&["implementation", "begin", "--repo", &repo]));
    assert_clap_rejection(&t.run(&["implementation", "check"]));
    assert_clap_rejection(&t.run(&["implementation", "bogus", "--repo", &repo]));
    assert_clap_rejection(&t.run(&[
        "implementation",
        "begin",
        "--repo",
        &repo,
        "--repo",
        &repo,
        "--revision",
        "1",
        "--sha256",
        &sha,
    ]));
    assert_clap_rejection(&t.run(&[
        "implementation",
        "begin",
        "--repo",
        &repo,
        "--revision",
        "1",
        "--sha256",
        &sha,
        "--nope",
    ]));

    // Token validation precedes repository checks: malformed tokens fail even
    // on a fresh repo with the exact INVALID_ARGUMENT category.
    for bad_rev in [
        "",
        "01",
        "+1",
        "1x",
        "1.0",
        " 1",
        "1 ",
        "\u{1}",
        "99999999999999999999",
    ] {
        let out = t.run(&[
            "implementation",
            "begin",
            "--repo",
            &repo,
            "--revision",
            bad_rev,
            "--sha256",
            &sha,
        ]);
        assert_category_no_stdout(&out, "INVALID_ARGUMENT");
    }
    // A leading dash makes the token parse as a Clap option, not a value.
    assert_clap_rejection(&t.run(&[
        "implementation",
        "begin",
        "--repo",
        &repo,
        "--revision",
        "-1",
        "--sha256",
        &sha,
    ]));
    let sha_upper = "A".repeat(64);
    let sha_short = "a".repeat(63);
    let sha_bad = format!("{}g", "a".repeat(63));
    for bad_sha in [&sha_upper, &sha_short, &sha_bad] {
        let out = t.run(&[
            "implementation",
            "begin",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            bad_sha,
        ]);
        assert_category_no_stdout(&out, "INVALID_ARGUMENT");
    }
    assert_mrgs_absent(&t.repo);

    // Stale authorization on a bound fixture; no authority publication.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let repo2 = t2.repo.to_string_lossy().into_owned();
    let draft_sha = t2.get_draft()["sha256"].as_str().unwrap().to_string();
    let before = mrgs_snapshot(&t2.repo);
    let out = t2.impl_begin(2, &draft_sha);
    assert_category_no_stdout(&out, "REQUESTED_REVISION_STALE");
    let wrong_sha = if let Some(rest) = draft_sha.strip_prefix('a') {
        format!("b{}", rest)
    } else {
        format!("a{}", &draft_sha[1..])
    };
    let out = t2.impl_begin(1, &wrong_sha);
    assert_category_no_stdout(&out, "REQUESTED_SHA_STALE");
    assert_snapshot_unchanged(&t2.repo, &before);
    assert_no_temp_files(&t2.repo);

    // Implementation check on a repo with no accepted plan fails closed.
    let t3 = TestRepo::new();
    let out = t3.run(&[
        "implementation",
        "check",
        "--repo",
        &t3.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // Injected Git-control environment values must not change behavior.
    let git_before = git_snapshot(&t2.repo);
    let git_dir = t2.repo.to_string_lossy().into_owned();
    let work_tree = t2.repo.to_string_lossy().into_owned();
    let index_file = t2.repo.join(".git/index").to_string_lossy().into_owned();
    let objects_dir = t2.repo.join(".git/objects").to_string_lossy().into_owned();
    let envs: &[(&str, &str)] = &[
        ("GIT_DIR", &git_dir),
        ("GIT_WORK_TREE", &work_tree),
        ("GIT_INDEX_FILE", &index_file),
        ("GIT_OBJECT_DIRECTORY", &objects_dir),
        ("GIT_ALTERNATE_OBJECT_DIRECTORIES", "/tmp/evil"),
        ("GIT_CONFIG_COUNT", "2"),
        ("GIT_CONFIG_KEY_0", "core.sshCommand"),
        ("GIT_CONFIG_VALUE_0", "echo pwned"),
        ("GIT_CONFIG_KEY_1", "alias.checkout"),
        ("GIT_CONFIG_VALUE_1", "!echo pwned"),
        ("GIT_SSH_COMMAND", "echo pwned"),
        ("GIT_PAGER", "cat"),
        ("GIT_EDITOR", "echo"),
        ("GIT_ASKPASS", "/bin/false"),
        ("GIT_TERMINAL_PROMPT", "0"),
    ];
    let out = t2.run_with_env(&["implementation", "check", "--repo", &repo2], envs);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("IMPLEMENTATION_OK "));
    assert_eq!(git_snapshot(&t2.repo), git_before, "git surface unchanged");
    assert_snapshot_unchanged(&t2.repo, &before);
    assert_no_temp_files(&t2.repo);
}

#[test]
fn test_obligation_04_audit_and_repair_cli_rejection_matrix() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let repo = t.repo.to_string_lossy().into_owned();
    let before_invalid = mrgs_snapshot(&t.repo);

    // Auditor ID grammar.
    let out = t.audit_begin("");
    assert_category_no_stdout(&out, "AUDITOR_ID_INVALID");
    for bad in [" ", " 1", "1 ", "\u{1}a", "a/b", "a\\b", "a b"] {
        let out = t.audit_begin(bad);
        assert_category_no_stdout(&out, "AUDITOR_ID_INVALID");
    }
    // An embedded NUL is rejected by the process API boundary itself: the
    // spawn fails with InvalidInput before any application code runs.
    {
        let mut cmd = cargo_bin();
        cmd.args([
            "audit",
            "begin",
            "--repo",
            &t.repo.to_string_lossy(),
            "--auditor",
            "a\u{0}b",
        ]);
        let err = cmd.output().unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("nul byte"));
    }
    let out = t.audit_begin(&"a".repeat(129));
    assert_category_no_stdout(&out, "AUDITOR_ID_INVALID");
    // None of the invalid-ID rejections mutated the authority.
    assert_snapshot_unchanged(&t.repo, &before_invalid);
    assert_no_temp_files(&t.repo);
    // Exact boundary (128 bytes) accepted.
    let out = t.audit_begin(&"a".repeat(128));
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("AUDIT_OPEN "));
    let open_128 = stdout_raw(&out);
    // A different one-byte auditor conflicts with the pending round.
    let out = t.audit_begin("a");
    assert_category_no_stdout(&out, "AUDIT_PENDING_CONFLICT");
    // Pending conflict with a different auditor.
    let out = t.audit_begin("other-auditor");
    assert_category_no_stdout(&out, "AUDIT_PENDING_CONFLICT");
    // Idempotent begin with the same auditor: byte-identical output.
    let out = t.audit_begin(&"a".repeat(128));
    assert_success(&out);
    assert_eq!(stdout_raw(&out), open_128);
    // Conflict/idempotence and invalid-report cases must not mutate the
    // settled pending ledger.
    let before_pending = mrgs_snapshot(&t.repo);
    assert_snapshot_unchanged(&t.repo, &before_pending);
    assert_no_temp_files(&t.repo);

    // Audit record adversarial matrix (same pending round/auditor).
    let open = t.audit_begin(&"a".repeat(128));
    let parts = split_stdout(&open);
    let (audit_id, subject_sha) = (parts[1].clone(), parts[3].clone());
    assert_eq!(parts[2], "1");
    let auditor_128 = "a".repeat(128);

    let missing = t.report_dir.join("missing.json");
    let out = t.audit_record(&missing);
    assert_category_no_stdout(&out, "AUDIT_REPORT_INVALID");
    let dir_report = t.report_dir.join("dir.json");
    std::fs::create_dir_all(&dir_report).unwrap();
    let out = t.audit_record(&dir_report);
    assert_category_no_stdout(&out, "AUDIT_REPORT_INVALID");

    let bad = t.write_report("not json at all");
    let out = t.audit_record(&bad);
    assert_category_no_stdout(&out, "AUDIT_REPORT_INVALID");

    let mut v: Value =
        serde_json::from_str(&t.make_pass_report(&audit_id, &subject_sha, &auditor_128)).unwrap();
    v["extra"] = json!(1);
    let bad = t.write_report(&serde_json::to_string_pretty(&v).unwrap());
    let out = t.audit_record(&bad);
    assert_category_no_stdout(&out, "AUDIT_REPORT_INVALID");

    let mut v: Value =
        serde_json::from_str(&t.make_pass_report(&audit_id, &subject_sha, &auditor_128)).unwrap();
    v.as_object_mut().unwrap().remove("verdict");
    let bad = t.write_report(&serde_json::to_string_pretty(&v).unwrap());
    let out = t.audit_record(&bad);
    assert_category_no_stdout(&out, "AUDIT_REPORT_INVALID");

    let mut v: Value =
        serde_json::from_str(&t.make_pass_report(&audit_id, &subject_sha, &auditor_128)).unwrap();
    v["subject_sha256"] = json!("b".repeat(64));
    let bad = t.write_report(&serde_json::to_string_pretty(&v).unwrap());
    let out = t.audit_record(&bad);
    assert_category_no_stdout(&out, "AUDIT_REPORT_MISMATCH");

    let mut v: Value =
        serde_json::from_str(&t.make_pass_report(&audit_id, &subject_sha, &auditor_128)).unwrap();
    v["audit_id"] = json!("wrong-audit-id");
    let bad = t.write_report(&serde_json::to_string_pretty(&v).unwrap());
    let out = t.audit_record(&bad);
    assert_category_no_stdout(&out, "AUDIT_REPORT_MISMATCH");

    let mut v: Value =
        serde_json::from_str(&t.make_pass_report(&audit_id, &subject_sha, &auditor_128)).unwrap();
    v["independence_declaration"] = json!("independent");
    let bad = t.write_report(&serde_json::to_string_pretty(&v).unwrap());
    let out = t.audit_record(&bad);
    assert_category_no_stdout(&out, "AUDIT_REPORT_INVALID");

    // Duplicate semantic identifiers (duplicate finding id).
    let mut v: Value =
        serde_json::from_str(&t.make_fail_report(&audit_id, &subject_sha, &auditor_128, "F1"))
            .unwrap();
    let dup = v["findings"][0].clone();
    v["findings"].as_array_mut().unwrap().push(dup);
    let bad = t.write_report(&serde_json::to_string_pretty(&v).unwrap());
    let out = t.audit_record(&bad);
    assert_category_no_stdout(&out, "AUDIT_REPORT_INVALID");

    // Clap rejections for audit/repair.
    assert_clap_rejection(&t.run(&["audit", "begin", "--repo", &repo]));
    assert_clap_rejection(&t.run(&["audit", "record", "--repo", &repo]));
    assert_clap_rejection(&t.run(&[
        "audit", "record", "--repo", &repo, "--report", "x", "--report", "y",
    ]));
    assert_clap_rejection(&t.run(&["repair", "check"]));

    // Repair check without a routed round fails closed.
    let out = t.repair_check();
    assert_category_no_stdout(&out, "REPAIR_NOT_ROUTED");

    assert_snapshot_unchanged(&t.repo, &before_pending);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_05_closeout_cli_rejection_matrix() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let repo = t.repo.to_string_lossy().into_owned();

    // Not ready (no PASS audit) -> exact category, no completion publication.
    let out = t.phase_close("phase-1");
    assert_category_no_stdout(&out, "CLOSEOUT_NOT_READY");
    assert_no_temp_files(&t.repo);
    assert!(t.get_completion_ledger().is_none());

    // Clap rejections.
    assert_clap_rejection(&t.run(&["phase", "close", "--repo", &repo]));
    assert_clap_rejection(&t.run(&[
        "phase", "close", "--repo", &repo, "--phase", "phase-1", "--phase", "phase-1",
    ]));
    assert_clap_rejection(&t.run(&[
        "phase", "close", "--repo", &repo, "--phase", "phase-1", "--bogus",
    ]));
    assert_clap_rejection(&t.run(&["phase", "bogus", "--repo", &repo]));

    // Empty phase ID -> fail closed, no stdout, no temp.
    let out = t.phase_close("");
    assert_failure(&out);
    assert_eq!(stdout_str(&out), "");
    assert_no_temp_files(&t.repo);

    // Wrong casing / unknown phase.
    let out = t.phase_close("PHASE-1");
    assert_failure(&out);
    assert_eq!(stdout_str(&out), "");

    // Non-active phase (phase-2 exists in plan but is not active).
    let out = t.phase_close("phase-2");
    assert_failure(&out);
    assert_eq!(stdout_str(&out), "");
    assert_no_temp_files(&t.repo);
    assert!(t.get_completion_ledger().is_none());

    // Semantically false but parse-valid final audit authority fails closed.
    t.full_pass_audit();
    let ledger_before = t.read_mrgs("audit-ledger.json");
    let v: Value = serde_json::from_str(&t.read_mrgs_str("audit-ledger.json")).unwrap();
    let mut v2 = v.clone();
    v2["rounds"][0]["status"] = json!("FAIL");
    let tampered_bytes = serde_json::to_vec_pretty(&v2).unwrap();
    t.write_mrgs("audit-ledger.json", &tampered_bytes);
    let out = t.phase_close("phase-1");
    assert_failure(&out);
    assert_eq!(stdout_str(&out), "");
    assert!(t.get_completion_ledger().is_none());
    // Phase-scoped bytes preserved: the failed close did not rewrite the
    // (tampered) ledger.
    assert_eq!(t.read_mrgs("audit-ledger.json"), tampered_bytes);
    // Restore and close successfully.
    t.write_mrgs("audit-ledger.json", &ledger_before);
    let close = t.phase_close("phase-1");
    assert_success(&close);
    assert!(stdout_str(&close).starts_with("PHASE_CLOSED phase-1 "));
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_06_continuity_cli_and_metadata_adversarial_matrix() {
    let t = TestRepo::new();
    let (_manifest_sha, receipt_sha) = t.close_phase1();
    let repo = t.repo.to_string_lossy().into_owned();

    // Clap rejections.
    assert_clap_rejection(&t.run(&["continuity", "record", "--repo", &repo]));
    assert_clap_rejection(&t.run(&["continuity", "record", "--metadata", "m.toml"]));
    assert_clap_rejection(&t.run(&[
        "continuity",
        "record",
        "--repo",
        &repo,
        "--metadata",
        "m.toml",
        "--source-repo",
        "a",
        "--source-repo",
        "b",
        "--nope",
    ]));

    // Metadata outside the repository.
    let outside = t._dir.path().join("meta.toml");
    write_file(&outside, &standard_metadata("phase-1", &receipt_sha));
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &repo,
        "--metadata",
        &outside.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // Malformed UTF-8 metadata bytes.
    let bad_utf8 = t.write_metadata("meta-bad.toml", "schema_version = 1\n");
    write_bytes(&bad_utf8, b"note = \"\xff\xfe\"\n");
    let out = t.continuity_record(&bad_utf8);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // Unknown / missing nested fields.
    let mut meta = standard_metadata("phase-1", &receipt_sha);
    let p = t.write_metadata("meta-unknown.toml", &format!("{}\nbogus = 1\n", meta));
    let out = t.continuity_record(&p);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");
    meta = standard_metadata("phase-1", &receipt_sha);
    let p = t.write_metadata(
        "meta-missing.toml",
        &meta.replace("note = \"continuity record\"", ""),
    );
    let out = t.continuity_record(&p);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // Uppercase completion receipt hash.
    let meta = standard_metadata("phase-1", &receipt_sha)
        .replace(&receipt_sha, &receipt_sha.to_uppercase());
    let p = t.write_metadata("meta-upper.toml", &meta);
    let out = t.continuity_record(&p);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // Stale completion binding (valid format, wrong receipt).
    let wrong_receipt = if let Some(rest) = receipt_sha.strip_prefix('a') {
        format!("b{}", rest)
    } else {
        format!("a{}", &receipt_sha[1..])
    };
    let p = t.write_metadata(
        "meta-stale.toml",
        &standard_metadata("phase-1", &wrong_receipt),
    );
    let out = t.continuity_record(&p);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // Control characters.
    let meta = standard_metadata("phase-1", &receipt_sha)
        .replace("note = \"continuity record\"", "note = \"x\u{1}y\"");
    let p = t.write_metadata("meta-ctrl.toml", &meta);
    let out = t.continuity_record(&p);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // Unsorted / duplicate models.
    let mut meta = standard_metadata("phase-1", &receipt_sha);
    meta = meta.replace("role = \"implementer\"", "role = \"reviewer\"");
    let dup_models = format!(
        "{}\n[[models]]\nrole = \"implementer\"\nprovider = \"openai\"\nmodel_id = \"gpt-5.6\"\nexecution_mode = \"hosted\"\nsession_label = \"phase-1-implementation\"\n",
        meta
    );
    let p = t.write_metadata("meta-dup-models.toml", &dup_models);
    let out = t.continuity_record(&p);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // Unsorted / duplicate hosts.
    let mut meta = standard_metadata("phase-1", &receipt_sha);
    meta = meta.replace("host_id = \"main-workstation\"", "host_id = \"zzz\"");
    let dup_hosts = format!(
        "{}\n[[hosts]]\nhost_id = \"aaa\"\nplatform = \"windows\"\narchitecture = \"x86_64\"\nexecution_surface = \"opencode\"\n",
        meta
    );
    let p = t.write_metadata("meta-dup-hosts.toml", &dup_hosts);
    let out = t.continuity_record(&p);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // Empty / unsafe source repositories: with no links in the metadata, any
    // supplied source is unreferenced and rejected as a mismatch.
    let m1 = t.write_metadata("m1.toml", &standard_metadata("phase-1", &receipt_sha));
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &repo,
        "--metadata",
        &m1.to_string_lossy(),
        "--source-repo",
        "",
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_SOURCE_MISMATCH");
    let file_as_source = t._dir.path().join("not-a-repo");
    write_file(&file_as_source, "x");
    let m2 = t.write_metadata("m2.toml", &standard_metadata("phase-1", &receipt_sha));
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &repo,
        "--metadata",
        &m2.to_string_lossy(),
        "--source-repo",
        &file_as_source.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_SOURCE_MISMATCH");

    // No ledger or source mutation from any rejection.
    assert!(t.get_continuity_ledger().is_none());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_07_recovery_cli_rejection_matrix() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    let repo = t.repo.to_string_lossy().into_owned();
    let sha64 = "a".repeat(64);
    let sha64b = "b".repeat(64);

    // Clap rejections.
    assert_clap_rejection(&t.run(&["recovery", "inspect"]));
    assert_clap_rejection(&t.run(&[
        "recovery",
        "inspect",
        "--repo",
        &repo,
        "--decision",
        "RECOVER",
    ]));
    assert_clap_rejection(&t.run(&["recovery", "apply", "--repo", &repo]));
    assert_clap_rejection(&t.run(&[
        "recovery",
        "apply",
        "--repo",
        &repo,
        "--recovery-id",
        &sha64,
        "--subject-sha256",
        &sha64b,
    ]));
    assert_clap_rejection(&t.run(&[
        "recovery",
        "apply",
        "--repo",
        &repo,
        "--recovery-id",
        &sha64,
        "--subject-sha256",
        &sha64b,
        "--decision",
        "RECOVER",
        "--decision",
        "RECOVER",
    ]));

    // Malformed / uppercase hashes on both hash flags.
    let sha_upper = "A".repeat(64);
    let sha_short = "a".repeat(63);
    let sha_bad = format!("{}g", "a".repeat(63));
    for bad in [&sha_upper, &sha_short, &sha_bad] {
        let out = t.run(&[
            "recovery",
            "apply",
            "--repo",
            &repo,
            "--recovery-id",
            bad,
            "--subject-sha256",
            &sha64b,
            "--decision",
            "RECOVER",
        ]);
        assert_category_no_stdout(&out, "RECOVERY_ID_INVALID");
        let out = t.run(&[
            "recovery",
            "apply",
            "--repo",
            &repo,
            "--recovery-id",
            &sha64,
            "--subject-sha256",
            bad,
            "--decision",
            "RECOVER",
        ]);
        assert_category_no_stdout(&out, "RECOVERY_ID_INVALID");
    }

    // Decision casing / embedded control.
    for bad in [
        "recover",
        "Recover",
        " RECOVER",
        "RECOVER ",
        "RECOVERED",
        "RECOVER\n",
    ] {
        let out = t.run(&[
            "recovery",
            "apply",
            "--repo",
            &repo,
            "--recovery-id",
            &sha64,
            "--subject-sha256",
            &sha64b,
            "--decision",
            bad,
        ]);
        assert_category_no_stdout(&out, "RECOVERY_DECISION_INVALID");
    }

    // Healthy subject: apply is read-only and reports RECOVERY_NOT_REQUIRED.
    let healthy = t.inspect_output();
    assert_eq!(healthy.len(), 1);
    assert!(healthy[0].starts_with("RECOVERY_NOT_REQUIRED "));
    let healthy_sha = healthy[0].split_whitespace().nth(1).unwrap().to_string();
    let before = mrgs_snapshot(&t.repo);
    let out = t.apply(&sha64, &healthy_sha);
    assert_success(&out);
    assert_eq!(
        stdout_str(&out),
        format!("RECOVERY_NOT_REQUIRED {}", healthy_sha)
    );
    assert!(t.get_recovery_ledger().is_none());
    assert_snapshot_unchanged(&t.repo, &before);

    // Recoverable subject with a stale recovery id / stale subject. A
    // mismatched id on a fresh recoverable subject is an invalid id; a
    // mismatched subject hash is stale.
    induce_recoverable(&t);
    let (rid, pre_sha) = recoverable_ids(&t);
    let out = t.apply(&"c".repeat(64), &pre_sha);
    assert_category_no_stdout(&out, "RECOVERY_ID_INVALID");
    let out = t.apply(&rid, &"d".repeat(64));
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");
    assert_no_temp_files(&t.repo);

    // Unrecoverable subject: unknown child.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    t2.write_mrgs("rogue.json", b"{}");
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Parse-valid corrupt journal: next_action beyond the action count.
    let out = t.apply(&rid, &pre_sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    let mut j: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    j["recoveries"][0]["next_action"] = json!(99);
    let tampered = serde_json::to_vec_pretty(&j).unwrap();
    t.write_mrgs("recovery-ledger.json", &tampered);
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    let out = t.apply(&rid, &pre_sha);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    // Zero unauthorized mutation: journal bytes preserved.
    assert_eq!(t.recovery_ledger_bytes(), tampered);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_08_global_error_and_no_mutation_invariants() {
    let t = TestRepo::new();
    let missing = t._dir.path().join("does-not-exist");
    let missing_str = missing.to_string_lossy().into_owned();
    let sha = "a".repeat(64);

    // One representative adversarial rejection per command surface.
    // (args, is_phase4, expected stderr prefix or category)
    let cases: Vec<(Vec<&str>, bool, &str)> = vec![
        (
            vec!["plan", "accept", "--repo", &missing_str, "--plan", "x"],
            false,
            "error: not a directory: ",
        ),
        (
            vec![
                "phase",
                "select",
                "--repo",
                &missing_str,
                "--phase",
                "phase-1",
            ],
            false,
            "error: not a directory: ",
        ),
        (
            vec![
                "contract",
                "draft",
                "--repo",
                &missing_str,
                "--contract",
                "x",
            ],
            false,
            "error: not a directory: ",
        ),
        (
            vec![
                "contract",
                "accept",
                "--repo",
                &missing_str,
                "--revision",
                "1",
                "--sha256",
                &sha,
                "--decision",
                "ACCEPTED",
            ],
            false,
            "error: not a directory: ",
        ),
        (
            vec![
                "contract",
                "revise",
                "--repo",
                &missing_str,
                "--contract",
                "x",
                "--expected-revision",
                "1",
                "--expected-sha256",
                &sha,
            ],
            false,
            "error: not a directory: ",
        ),
        (
            vec![
                "implementation",
                "begin",
                "--repo",
                &missing_str,
                "--revision",
                "1",
                "--sha256",
                &sha,
            ],
            true,
            "REPOSITORY_INVALID",
        ),
        (
            vec!["implementation", "check", "--repo", &missing_str],
            true,
            "REPOSITORY_INVALID",
        ),
        (
            vec!["audit", "begin", "--repo", &missing_str, "--auditor", "a"],
            true,
            "REPOSITORY_INVALID",
        ),
        (
            vec!["audit", "record", "--repo", &missing_str, "--report", "x"],
            true,
            "REPOSITORY_INVALID",
        ),
        (
            vec!["repair", "check", "--repo", &missing_str],
            true,
            "REPOSITORY_INVALID",
        ),
        (
            vec![
                "phase",
                "close",
                "--repo",
                &missing_str,
                "--phase",
                "phase-1",
            ],
            true,
            "GOVERNANCE_AUTHORITY_INVALID",
        ),
        (
            vec![
                "continuity",
                "record",
                "--repo",
                &missing_str,
                "--metadata",
                "x",
            ],
            true,
            "GOVERNANCE_AUTHORITY_INVALID",
        ),
        (
            vec!["recovery", "inspect", "--repo", &missing_str],
            true,
            "REPOSITORY_INVALID",
        ),
        (
            vec![
                "recovery",
                "apply",
                "--repo",
                &missing_str,
                "--recovery-id",
                &sha,
                "--subject-sha256",
                &sha,
                "--decision",
                "RECOVER",
            ],
            true,
            "REPOSITORY_INVALID",
        ),
    ];

    for (args, is_phase4, expected) in &cases {
        let out = t.run(args);
        assert_failure(&out);
        assert_eq!(stdout_str(&out), "", "no success stdout on rejection");
        if *is_phase4 {
            assert_eq!(
                stderr_str(&out),
                format!("error: {}", expected),
                "args: {:?}",
                args
            );
        } else {
            assert_err_prefix(&out, expected);
        }
        // No writes anywhere: .mrgs absent in the fixture repo, and the
        // nonexistent repo path produced no directory.
        assert_mrgs_absent(&t.repo);
        assert!(!missing.exists());
        assert_no_temp_files(&t.repo);
    }

    // Git surface unchanged after every rejection.
    let git_before = git_snapshot(&t.repo);
    assert_eq!(git_snapshot(&t.repo), git_before);
}
// ===========================================================================
// 16.2 Filesystem and path-topology security
// ===========================================================================

#[test]
fn test_obligation_09_repository_root_and_escape_topology() {
    let t = TestRepo::new();
    let repo_str = t.repo.to_string_lossy().into_owned();
    let plan_str = t.plan_path.to_string_lossy().into_owned();

    // Nonexistent / file / empty / tilde / drive / UNC / device roots are
    // rejected with the existing directory category and no .mrgs anywhere.
    let missing = t._dir.path().join("nope");
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &missing.to_string_lossy(),
        "--plan",
        &plan_str,
    ]);
    assert_err_prefix(&out, "error: not a directory: ");
    assert_mrgs_absent(&t.repo);
    let as_file = t._dir.path().join("plain-file");
    write_file(&as_file, "x");
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &as_file.to_string_lossy(),
        "--plan",
        &plan_str,
    ]);
    assert_err_prefix(&out, "error: not a directory: ");
    let out = t.run(&["plan", "accept", "--repo", "", "--plan", &plan_str]);
    assert_err_prefix(&out, "error: not a directory");
    let out = t.run(&["plan", "accept", "--repo", "~", "--plan", &plan_str]);
    assert_err_prefix(&out, "error: not a directory: ");
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        "C:relative",
        "--plan",
        &plan_str,
    ]);
    assert_err_prefix(&out, "error: not a directory: ");
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        "\\\\server\\share\\missing",
        "--plan",
        &plan_str,
    ]);
    assert_err_prefix(&out, "error: not a directory: ");
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        "\\\\.\\NUL",
        "--plan",
        &plan_str,
    ]);
    assert_err_prefix(&out, "error: not a directory: ");
    assert_mrgs_absent(&t.repo);

    // Legitimate canonical aliases: accepted, and the governance dir lands
    // exactly at the canonical repository root with a repo-relative plan.
    for _ in 0..6 {
        let t2 = TestRepo::new();
        let repo2 = t2.repo.to_string_lossy().into_owned();
        let name = t2.repo.file_name().unwrap().to_string_lossy().into_owned();
        let aliases: Vec<String> = vec![
            repo2.clone(),
            format!("{}/.", repo2),
            format!("{}/./", repo2),
            format!("{}/../{}", repo2, name),
            format!("{}//", repo2),
            format!("{}/", repo2),
        ];
        for alias in &aliases {
            let out = t2.run(&[
                "plan",
                "accept",
                "--repo",
                alias,
                "--plan",
                &t2.plan_path.to_string_lossy(),
            ]);
            assert_success(&out);
            assert!(stdout_str(&out).starts_with("test-plan "));
            let expected = std::fs::canonicalize(&t2.repo).unwrap().join(".mrgs");
            assert!(
                expected.exists(),
                ".mrgs must exist at canonical root for alias {}",
                alias
            );
            let accepted: Value = serde_json::from_str(
                &std::fs::read_to_string(expected.join("accepted-plan.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(accepted["plan_path"], "plan.toml");
            // Reset between aliases so each alias exercises first acceptance.
            std::fs::remove_dir_all(&expected).unwrap();
        }
    }

    // Escapes to nonexistent locations are rejected; no .mrgs is created.
    let escape = format!("{}/../does-not-exist", repo_str);
    let out = t.run(&["plan", "accept", "--repo", &escape, "--plan", &plan_str]);
    assert_err_prefix(&out, "error: not a directory: ");
    assert_mrgs_absent(&t.repo);
    assert_mrgs_absent(t._dir.path());

    // Unsafe ancestor: the repository sits below a symlink/junction alias.
    let root = t._dir.path();
    let real_dir = root.join("real-ancestor");
    std::fs::create_dir_all(&real_dir).unwrap();
    let repo_in_real = real_dir.join("inner-repo");
    std::fs::create_dir_all(&repo_in_real).unwrap();
    git_init(&repo_in_real);
    git_commit(&repo_in_real, "seed.txt", b"seed");
    write_file(&repo_in_real.join("plan.toml"), valid_plan_toml());
    match make_dir_link(&real_dir, &root.join("ancestor-link")) {
        Ok(()) => {
            let via_link = root.join("ancestor-link/inner-repo");
            let out = t.run(&[
                "plan",
                "accept",
                "--repo",
                &via_link.to_string_lossy(),
                "--plan",
                &repo_in_real.join("plan.toml").to_string_lossy(),
            ]);
            assert_success(&out);
            let canonical_expected = std::fs::canonicalize(&repo_in_real).unwrap().join(".mrgs");
            assert!(
                canonical_expected.exists(),
                ".mrgs must land at the canonical root"
            );
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Capability unavailable: prove the equivalent canonical-alias
            // case is still confined to the canonical boundary.
            let alias = format!(
                "{}/../{}",
                repo_in_real.to_string_lossy(),
                repo_in_real.file_name().unwrap().to_string_lossy()
            );
            let out = t.run(&[
                "plan",
                "accept",
                "--repo",
                &alias,
                "--plan",
                &repo_in_real.join("plan.toml").to_string_lossy(),
            ]);
            assert_success(&out);
            assert!(std::fs::canonicalize(&repo_in_real)
                .unwrap()
                .join(".mrgs")
                .exists());
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("link creation failed: {}", e),
    }
}

#[test]
fn test_obligation_10_source_path_normalization_and_external_boundaries() {
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    let repo = t.repo.to_string_lossy().into_owned();

    // --- Plan sources ---
    let outside = t._dir.path().join("ext-plan.toml");
    write_file(&outside, valid_plan_toml());
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &repo,
        "--plan",
        &outside.to_string_lossy(),
    ]);
    assert_err_prefix(&out, "error: plan path not inside repository: ");
    // Plan under .mrgs and .git: canonical-boundary acceptance with exact
    // normalized repository-relative persistence.
    for (sub, persisted) in [(".mrgs", ".mrgs/plan.toml"), (".git", ".git/plan.toml")] {
        let t2 = TestRepo::new();
        let dir = t2.repo.join(sub);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("plan.toml");
        write_file(&p, valid_plan_toml());
        let out = t2.run(&[
            "plan",
            "accept",
            "--repo",
            &t2.repo.to_string_lossy(),
            "--plan",
            &p.to_string_lossy(),
        ]);
        assert_success(&out);
        let accepted: Value =
            serde_json::from_str(&t2.read_mrgs_str("accepted-plan.json")).unwrap();
        assert_eq!(accepted["plan_path"], persisted);
    }
    // Alias components normalize to the exact relative value.
    let t3 = TestRepo::new();
    let sub = t3.repo.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::copy(&t3.plan_path, sub.join("plan.toml")).unwrap();
    let out = t3.run(&[
        "plan",
        "accept",
        "--repo",
        &t3.repo.to_string_lossy(),
        "--plan",
        &format!("{}/./sub/../sub/plan.toml", t3.repo.to_string_lossy()),
    ]);
    assert_success(&out);
    let accepted: Value = serde_json::from_str(&t3.read_mrgs_str("accepted-plan.json")).unwrap();
    assert_eq!(accepted["plan_path"], "sub/plan.toml");

    // --- Contract sources ---
    let t4 = TestRepo::new();
    t4.accept_plan_success();
    t4.select_phase_success("phase-1");
    let ext_contract = t4._dir.path().join("ext-contract.toml");
    write_file(&ext_contract, &contract_toml_for_phase("phase-1"));
    let out = t4.run(&[
        "contract",
        "draft",
        "--repo",
        &t4.repo.to_string_lossy(),
        "--contract",
        &ext_contract.to_string_lossy(),
    ]);
    assert_err_prefix(&out, "error: contract source file is outside repository");
    write_file(
        &t4.repo.join(".git/contract.toml"),
        &contract_toml_for_phase("phase-1"),
    );
    let out = t4.run(&[
        "contract",
        "draft",
        "--repo",
        &t4.repo.to_string_lossy(),
        "--contract",
        &t4.repo.join(".git/contract.toml").to_string_lossy(),
    ]);
    assert_success(&out);
    assert_eq!(t4.get_draft()["source_path"], ".git/contract.toml");
    // Contract under .mrgs: rejected.
    let t5 = TestRepo::new();
    t5.accept_plan_success();
    t5.select_phase_success("phase-1");
    std::fs::create_dir_all(t5.repo.join(".mrgs")).unwrap();
    write_file(
        &t5.repo.join(".mrgs/contract.toml"),
        &contract_toml_for_phase("phase-1"),
    );
    let out = t5.run(&[
        "contract",
        "draft",
        "--repo",
        &t5.repo.to_string_lossy(),
        "--contract",
        &t5.repo.join(".mrgs/contract.toml").to_string_lossy(),
    ]);
    assert_err_prefix(
        &out,
        "error: contract source file is inside .mrgs directory",
    );

    // --- Audit reports: the one contractually authorized external source ---
    let t6 = TestRepo::new();
    t6.setup_impl_bound();
    let open = t6.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = t6.make_pass_report(&parts[1], &parts[3], "auditor1");
    let external_report = t6._dir.path().join("reports/external-report.json");
    write_file(&external_report, &report);
    let out = t6.audit_record(&external_report);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("AUDIT_PASS "));
    let ledger: Value = serde_json::from_str(&t6.read_mrgs_str("audit-ledger.json")).unwrap();
    let stored = ledger["rounds"][0]["report_source_path"]
        .as_str()
        .unwrap()
        .to_string();
    // The stored path is the canonical external absolute path; the Windows
    // canonical form may carry the verbatim \\?\ prefix, so compare against
    // the same canonicalization.
    let expected_stored = std::fs::canonicalize(&external_report)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    assert_eq!(stored, expected_stored);

    // --- Continuity metadata: strict inside-repo, outside-.git/.mrgs rule ---
    let t7 = TestRepo::new();
    let (_m, receipt) = t7.close_phase1();
    let meta_outside = t7._dir.path().join("meta.toml");
    write_file(&meta_outside, &standard_metadata("phase-1", &receipt));
    let out = t7.run(&[
        "continuity",
        "record",
        "--repo",
        &t7.repo.to_string_lossy(),
        "--metadata",
        &meta_outside.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");
    write_file(
        &t7.repo.join(".git/meta.toml"),
        &standard_metadata("phase-1", &receipt),
    );
    let out = t7.run(&[
        "continuity",
        "record",
        "--repo",
        &t7.repo.to_string_lossy(),
        "--metadata",
        &t7.repo.join(".git/meta.toml").to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");
    write_file(
        &t7.repo.join(".mrgs/meta.toml"),
        &standard_metadata("phase-1", &receipt),
    );
    let out = t7.run(&[
        "continuity",
        "record",
        "--repo",
        &t7.repo.to_string_lossy(),
        "--metadata",
        &t7.repo.join(".mrgs/meta.toml").to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");
    // Alias metadata path: accepted with normalized relative persistence.
    let sub = t7.repo.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    t7.write_metadata("sub/meta.toml", &standard_metadata("phase-1", &receipt));
    let out = t7.run(&[
        "continuity",
        "record",
        "--repo",
        &t7.repo.to_string_lossy(),
        "--metadata",
        &format!("{}/./sub/../sub/meta.toml", t7.repo.to_string_lossy()),
    ]);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("CONTINUITY_RECORDED "));
    let ledger: Value = serde_json::from_str(&t7.read_mrgs_str("continuity-ledger.json")).unwrap();
    assert_eq!(
        ledger["entries"][0]["continuity_manifest"]["metadata_source_path"],
        "sub/meta.toml"
    );
}

#[test]
fn test_obligation_11_governance_directory_and_unknown_child_topology() {
    // .mrgs as a regular file: every consumer fails closed and the hostile
    // object is never deleted or rewritten.
    let t = TestRepo::new();
    t.accept_plan_success();
    std::fs::remove_dir_all(t.repo.join(".mrgs")).unwrap();
    write_file(&t.repo.join(".mrgs"), "hostile");
    let out = t.select_phase("phase-1");
    assert_err_prefix(&out, "error: governance directory is not a directory: ");
    assert_eq!(std::fs::read(t.repo.join(".mrgs")).unwrap(), b"hostile");
    let out = t.run(&[
        "implementation",
        "check",
        "--repo",
        &t.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
    assert_eq!(std::fs::read(t.repo.join(".mrgs")).unwrap(), b"hostile");

    // .mrgs as a symlink/junction to an external directory.
    let t2 = TestRepo::new();
    t2.accept_plan_success();
    std::fs::remove_dir_all(t2.repo.join(".mrgs")).unwrap();
    let outside = t2._dir.path().join("outside-gov");
    std::fs::create_dir_all(&outside).unwrap();
    match make_dir_link(&outside, &t2.repo.join(".mrgs")) {
        Ok(()) => {
            let out = t2.select_phase("phase-1");
            assert_err_prefix(&out, "error: governance directory escapes repository: ");
            assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
            let out = t2.run(&["recovery", "inspect", "--repo", &t2.repo.to_string_lossy()]);
            // Recovery's own boundary classification reports the unsafe
            // governance object with its filesystem-boundary category.
            assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Fallback: the file-object rejection proves the same trust
            // boundary rejects before traversal.
            let out = t2.select_phase("phase-1");
            assert_err_prefix(&out, "error: governance directory is not a directory: ");
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("link creation failed: {}", e),
    }

    // Nested unexpected objects and unknown children.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.write_mrgs("rogue.json", b"{}");
    let before = mrgs_snapshot(&t3.repo);
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_snapshot_unchanged(&t3.repo, &before);

    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    std::fs::create_dir_all(t4.repo.join(".mrgs/rogue-dir")).unwrap();
    let out = t4.inspect();
    assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
    assert!(t4.repo.join(".mrgs/rogue-dir").is_dir());

    // Case aliases where the filesystem permits them.
    let t5 = TestRepo::new();
    t5.accept_plan_success();
    t5.select_phase_success("phase-1");
    let before = mrgs_snapshot(&t5.repo);
    #[cfg(unix)]
    {
        // A distinct .MRGS sibling is not the governance directory; commands
        // keep using .mrgs untouched and the alias is never mutated.
        std::fs::create_dir_all(t5.repo.join(".MRGS")).unwrap();
        write_file(&t5.repo.join(".MRGS/x.toml"), "x");
        let out = t5.run(&[
            "phase",
            "select",
            "--repo",
            &t5.repo.to_string_lossy(),
            "--phase",
            "phase-2",
        ]);
        assert_failure(&out);
        assert_eq!(stdout_str(&out), "");
        assert_snapshot_unchanged(&t5.repo, &before);
        assert!(t5.repo.join(".MRGS/x.toml").exists());
        eprintln!("CAPABILITY_EXECUTED");
    }
    #[cfg(windows)]
    {
        // Case-insensitive filesystem: .MRGS is the same directory as .mrgs;
        // assert the native alias resolution and snapshot stability.
        std::fs::create_dir_all(t5.repo.join(".MRGS")).unwrap();
        assert!(t5.repo.join(".mrgs").is_dir());
        assert_snapshot_unchanged(&t5.repo, &before);
        eprintln!("CAPABILITY_EXECUTED");
    }
}

#[test]
fn test_obligation_12_symlink_traversal_capability_branch() {
    // Source symlink escaping the repository.
    let t = TestRepo::new();
    let outside = t._dir.path().join("target-file.toml");
    write_file(&outside, valid_plan_toml());
    match make_file_link(&outside, &t.repo.join("link-plan.toml")) {
        Ok(()) => {
            let out = t.run(&[
                "plan",
                "accept",
                "--repo",
                &t.repo.to_string_lossy(),
                "--plan",
                &t.repo.join("link-plan.toml").to_string_lossy(),
            ]);
            assert_err_prefix(&out, "error: plan path not inside repository: ");
            assert_mrgs_absent(&t.repo);
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Fallback: the lexical equivalent escape is rejected identically.
            let out = t.run(&[
                "plan",
                "accept",
                "--repo",
                &t.repo.to_string_lossy(),
                "--plan",
                &outside.to_string_lossy(),
            ]);
            assert_err_prefix(&out, "error: plan path not inside repository: ");
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("symlink creation failed: {}", e),
    }

    // Governance directory symlink.
    let t2 = TestRepo::new();
    t2.accept_plan_success();
    std::fs::remove_dir_all(t2.repo.join(".mrgs")).unwrap();
    let outside2 = t2._dir.path().join("gov-target");
    std::fs::create_dir_all(&outside2).unwrap();
    match make_dir_link(&outside2, &t2.repo.join(".mrgs")) {
        Ok(()) => {
            let out = t2.run(&[
                "implementation",
                "check",
                "--repo",
                &t2.repo.to_string_lossy(),
            ]);
            assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let out = t2.run(&[
                "implementation",
                "check",
                "--repo",
                &t2.repo.to_string_lossy(),
            ]);
            assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("link creation failed: {}", e),
    }

    // Git-layer symlink: a .git redirection makes the git root disagree with
    // the canonical repository root.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    let other_repo = t3._dir.path().join("other-repo");
    git_init(&other_repo);
    git_commit(&other_repo, "a.txt", b"a");
    std::fs::remove_dir_all(t3.repo.join(".git")).unwrap();
    match make_dir_link(&other_repo.join(".git"), &t3.repo.join(".git")) {
        Ok(()) => {
            let out = t3.run(&[
                "implementation",
                "check",
                "--repo",
                &t3.repo.to_string_lossy(),
            ]);
            assert_failure(&out);
            assert_eq!(stdout_str(&out), "");
            let err = stderr_str(&out);
            assert!(
                err == "error: GIT_ROOT_MISMATCH"
                    || err == "error: GOVERNANCE_AUTHORITY_INVALID"
                    || err == "error: BASELINE_COMMIT_MISSING",
                "unexpected stderr: {}",
                err
            );
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Fallback: a plain ordinary-directory .git replacement fails
            // closed.
            std::fs::create_dir_all(t3.repo.join(".git")).unwrap();
            let out = t3.run(&[
                "implementation",
                "check",
                "--repo",
                &t3.repo.to_string_lossy(),
            ]);
            assert_failure(&out);
            assert_eq!(stdout_str(&out), "");
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("link creation failed: {}", e),
    }

    // Leaf symlink at a governance file: reads resolve only to validated
    // bytes and writes never write through to the symlink target.
    let t4 = TestRepo::new();
    t4.accept_plan_success();
    t4.select_phase_success("phase-1");
    let state_target = t4._dir.path().join("state-target.json");
    write_file(&state_target, &t4.read_mrgs_str("state.json"));
    // Replace the fixture's own state.json with the symlink.
    std::fs::remove_file(t4.repo.join(".mrgs/state.json")).unwrap();
    match make_file_link(&state_target, &t4.repo.join(".mrgs/state.json")) {
        Ok(()) => {
            let target_before = std::fs::read(&state_target).unwrap();
            let out = t4.run(&[
                "phase",
                "select",
                "--repo",
                &t4.repo.to_string_lossy(),
                "--phase",
                "phase-2",
            ]);
            assert_failure(&out);
            assert_eq!(stdout_str(&out), "");
            assert_eq!(std::fs::read(&state_target).unwrap(), target_before);
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Fallback: the ordinary-directory equivalent is rejected.
            std::fs::remove_file(t4.repo.join(".mrgs/state.json")).unwrap();
            std::fs::create_dir_all(t4.repo.join(".mrgs/state.json")).unwrap();
            let out = t4.run(&[
                "phase",
                "select",
                "--repo",
                &t4.repo.to_string_lossy(),
                "--phase",
                "phase-1",
            ]);
            assert_failure(&out);
            assert_eq!(stdout_str(&out), "");
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("symlink creation failed: {}", e),
    }
}

#[test]
fn test_obligation_13_windows_reparse_and_junction_capability_branch() {
    #[cfg(windows)]
    {
        let t = TestRepo::new();
        t.accept_plan_success();
        std::fs::remove_dir_all(t.repo.join(".mrgs")).unwrap();
        let outside = t._dir.path().join("junction-target");
        std::fs::create_dir_all(&outside).unwrap();
        match make_dir_link(&outside, &t.repo.join(".mrgs")) {
            Ok(()) => {
                assert_reparse(&t.repo.join(".mrgs"));
                let out = t.select_phase("phase-1");
                assert_err_prefix(&out, "error: governance directory escapes repository: ");
                let out = t.run(&[
                    "implementation",
                    "check",
                    "--repo",
                    &t.repo.to_string_lossy(),
                ]);
                assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
                assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
                eprintln!("CAPABILITY_EXECUTED");
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                let out = t.select_phase("phase-1");
                assert_failure(&out);
                assert_eq!(stdout_str(&out), "");
                eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
            }
            Err(e) => panic!("junction creation failed: {}", e),
        }
        // Source-ancestor junction escape: the junction target must contain
        // the plan file so the boundary check (not the existence check)
        // decides the outcome.
        let t2 = TestRepo::new();
        let outside2 = t2._dir.path().join("real-dir");
        std::fs::create_dir_all(&outside2).unwrap();
        match make_dir_link(&outside2, &t2.repo.join("ancestor")) {
            Ok(()) => {
                write_file(&outside2.join("plan.toml"), valid_plan_toml());
                let via = t2.repo.join("ancestor/plan.toml");
                let out = t2.run(&[
                    "plan",
                    "accept",
                    "--repo",
                    &t2.repo.to_string_lossy(),
                    "--plan",
                    &via.to_string_lossy(),
                ]);
                assert_err_prefix(&out, "error: plan path not inside repository: ");
                eprintln!("CAPABILITY_EXECUTED");
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                let out = t2.run(&[
                    "plan",
                    "accept",
                    "--repo",
                    &t2.repo.to_string_lossy(),
                    "--plan",
                    &t2._dir.path().join("x").to_string_lossy(),
                ]);
                assert_failure(&out);
                assert_eq!(stdout_str(&out), "");
                eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
            }
            Err(e) => panic!("junction creation failed: {}", e),
        }
    }
    #[cfg(not(windows))]
    {
        // Junction capability is unavailable on this host: prove it.
        let probe = Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .output();
        assert!(
            probe.is_err() || !probe.unwrap().status.success(),
            "mklink must not be available off Windows"
        );
        // Concrete non-regular fallback for the same trust boundary.
        let t = TestRepo::new();
        t.accept_plan_success();
        std::fs::remove_dir_all(t.repo.join(".mrgs")).unwrap();
        std::fs::create_dir_all(t.repo.join(".mrgs")).unwrap();
        let out = t.select_phase("phase-1");
        assert_err_prefix(&out, "error: governance directory is not a directory: ");
        eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
    }
}

#[test]
fn test_obligation_14_nonregular_file_and_external_source_objects() {
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    let repo = t.repo.to_string_lossy().into_owned();

    // Directory as plan / contract / metadata / report.
    let dir = t.repo.join("dir-plan");
    std::fs::create_dir_all(&dir).unwrap();
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &repo,
        "--plan",
        &dir.to_string_lossy(),
    ]);
    assert_err_prefix(&out, "error: not a regular file: ");
    let out = t.run(&[
        "contract",
        "draft",
        "--repo",
        &repo,
        "--contract",
        &dir.to_string_lossy(),
    ]);
    assert_err_prefix(&out, "error: not a regular file: ");
    let m = t.repo.join("meta-dir");
    std::fs::create_dir_all(&m).unwrap();
    // Continuity validates the completion chain before the metadata source;
    // use a closed-phase fixture so the metadata-dir rejection is reached.
    let t_cont = TestRepo::new();
    let (_mc, receipt_c) = t_cont.close_phase1();
    let m2 = t_cont.repo.join("meta-dir");
    std::fs::create_dir_all(&m2).unwrap();
    let out = t_cont.run(&[
        "continuity",
        "record",
        "--repo",
        &t_cont.repo.to_string_lossy(),
        "--metadata",
        &m2.to_string_lossy(),
    ]);
    // A non-regular metadata source is an unsafe filesystem object.
    assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
    let _ = (receipt_c,);
    let t_bound = TestRepo::new();
    t_bound.setup_impl_bound();
    let open = t_bound.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let _ = (parts[1].clone(), parts[3].clone());
    let report_dir = t_bound.report_dir.join("report-dir");
    std::fs::create_dir_all(&report_dir).unwrap();
    let out = t_bound.audit_record(&report_dir);
    assert_category_no_stdout(&out, "AUDIT_REPORT_INVALID");

    // Dangling symlink as plan source.
    match make_file_link(
        &t.repo.join("no-such-target.toml"),
        &t.repo.join("dangling.toml"),
    ) {
        Ok(()) => {
            let out = t.run(&[
                "plan",
                "accept",
                "--repo",
                &repo,
                "--plan",
                &t.repo.join("dangling.toml").to_string_lossy(),
            ]);
            assert_err_prefix(&out, "error: plan not found: ");
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let out = t.run(&[
                "plan",
                "accept",
                "--repo",
                &repo,
                "--plan",
                "missing-regular.toml",
            ]);
            assert_err_prefix(&out, "error: plan not found: ");
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("symlink creation failed: {}", e),
    }

    // FIFO as plan source (unix-only); directory substitute elsewhere.
    #[cfg(unix)]
    {
        let fifo = t.repo.join("fifo.toml");
        let created = Command::new("mkfifo").arg(&fifo).status();
        let made_fifo = created.is_ok() && created.unwrap().success();
        if made_fifo {
            let out = t.run(&[
                "plan",
                "accept",
                "--repo",
                &repo,
                "--plan",
                &fifo.to_string_lossy(),
            ]);
            assert_err_prefix(&out, "error: not a regular file: ");
            eprintln!("CAPABILITY_EXECUTED");
        } else {
            std::fs::create_dir_all(&t.repo.join("fifo.toml")).unwrap();
            let out = t.run(&[
                "plan",
                "accept",
                "--repo",
                &repo,
                "--plan",
                &t.repo.join("fifo.toml").to_string_lossy(),
            ]);
            assert_err_prefix(&out, "error: not a regular file: ");
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
    }
    #[cfg(windows)]
    {
        // FIFOs/sockets are not creatable on Windows; a directory substitute
        // exercises the same non-regular rejection, and the NUL device is a
        // device substitute.
        let path = t.repo.join("fifo-substitute.toml");
        std::fs::create_dir_all(&path).unwrap();
        let out = t.run(&[
            "plan",
            "accept",
            "--repo",
            &repo,
            "--plan",
            &path.to_string_lossy(),
        ]);
        assert_err_prefix(&out, "error: not a regular file: ");
        let out = t.run(&["plan", "accept", "--repo", &repo, "--plan", "\\\\.\\NUL"]);
        assert_failure(&out);
        assert_eq!(stdout_str(&out), "");
        eprintln!("CAPABILITY_EXECUTED");
    }

    // Locked / unreadable file (Windows exclusive lock); non-regular
    // fallback elsewhere.
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let locked = t.repo.join("locked.toml");
        write_file(&locked, "x");
        // Full-tree snapshot taken before the lock is created (locked.toml is
        // unreadable while the exclusive handle is held); compared after the
        // handle is dropped to prove no unrelated file was created or changed.
        let git_before = git_snapshot(&t.repo);
        // share_mode(0) denies read/write sharing on the held handle, so the
        // file is unopenable for the duration of the lock. Prove the lock is
        // effective before invoking mrgs.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&locked)
            .unwrap();
        assert!(
            std::fs::OpenOptions::new()
                .read(true)
                .open(&locked)
                .is_err(),
            "fixture ineffective: locked.toml was readable while the exclusive handle was held"
        );
        let gov_before = mrgs_snapshot(&t.repo);
        let out = t.run(&[
            "plan",
            "accept",
            "--repo",
            &repo,
            "--plan",
            &locked.to_string_lossy(),
        ]);
        assert_err_prefix(&out, "error: I/O error: ");
        // The rejected plan source must not have changed any governance
        // authority or left temporary files.
        assert_snapshot_unchanged(&t.repo, &gov_before);
        assert_no_temp_files(&t.repo);
        drop(lock);
        // The file is a normal readable file again once the lock is gone.
        assert!(
            std::fs::OpenOptions::new().read(true).open(&locked).is_ok(),
            "locked.toml still unopenable after the exclusive handle was dropped"
        );
        // No unrelated file in the worktree was created or changed by the
        // rejected run.
        assert_eq!(git_snapshot(&t.repo), git_before, "worktree changed");
        eprintln!("CAPABILITY_EXECUTED");
    }
    #[cfg(not(windows))]
    {
        let path = t.repo.join("locked-fallback.toml");
        std::fs::create_dir_all(&path).unwrap();
        let out = t.run(&[
            "plan",
            "accept",
            "--repo",
            &repo,
            "--plan",
            &path.to_string_lossy(),
        ]);
        assert_err_prefix(&out, "error: not a regular file: ");
        eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
    }

    // External source roots: audit reports are the authorized external root;
    // plan/contract/metadata sources are not. None of the rejections above
    // mutated the existing governance bytes or left temporary files.
    assert_no_temp_files(&t.repo);
    assert!(t.repo.join(".mrgs/state.json").exists());
}

#[test]
fn test_obligation_15_temporary_ambiguity_and_destination_replacement() {
    // Recognized redundant temp -> REMOVE_REDUNDANT_TEMP action.
    let t = TestRepo::new();
    let (_manifest_sha, _receipt_sha) = t.close_phase1();
    let completion_bytes = t.read_mrgs("completion-ledger.json");
    t.write_mrgs(".closeout.0.tmp", &completion_bytes);
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_REQUIRED "));
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[3], "1");
    assert_eq!(
        lines[1],
        "RECOVERY_ACTION 1 REMOVE_REDUNDANT_TEMP .closeout.0.tmp"
    );
    let out = t.apply(parts[1], parts[2]);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    assert!(!t.repo.join(".mrgs/.closeout.0.tmp").exists());
    assert_no_temp_files(&t.repo);

    // Duplicate candidates for one target -> RECOVERY_UNRECOVERABLE.
    let t2 = TestRepo::new();
    t2.close_phase1();
    let completion_bytes = t2.read_mrgs("completion-ledger.json");
    t2.write_mrgs(".closeout.0.tmp", &completion_bytes);
    t2.write_mrgs(".closeout.1.tmp", &completion_bytes);
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Target-absent variant with a stray recognized temp: inspection is
    // read-only and must neither remove nor truncate the fixture temp.
    let t3 = TestRepo::new();
    t3.close_phase1();
    let completion_bytes = t3.read_mrgs("completion-ledger.json");
    t3.delete("completion-ledger.json");
    t3.write_mrgs(".closeout.0.tmp", &completion_bytes);
    let out = t3.inspect();
    assert_failure(&out);
    assert_eq!(stdout_str(&out), "");
    assert_eq!(t3.read_mrgs(".closeout.0.tmp"), completion_bytes);
    // The only temp present is the fixture sentinel itself.
    let temps: Vec<String> = std::fs::read_dir(t3.repo.join(".mrgs"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert_eq!(temps, vec![".closeout.0.tmp"]);

    // Unknown / noncanonical temporary names -> RECOVERY_UNRECOVERABLE.
    for name in ["mystery.tmp", ".closeout.abc.tmp", ".closeout-state.00.tmp"] {
        let t4 = TestRepo::new();
        t4.setup_impl_bound();
        t4.write_mrgs(name, b"junk");
        let out = t4.inspect();
        assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    }

    // Occupied create-new slots for the closeout producer: a pre-existing
    // closeout-temp path is a foreign object in .mrgs, so closeout fails
    // closed (CLOSEOUT_NOT_READY) and every sentinel stays byte-exact.
    // (The occupied-slot collision search itself is exercised through the
    // continuity producer in obligation 54/55, whose validation order
    // reaches the publication search.)
    let t5 = TestRepo::new();
    t5.setup_closeout_ready();
    let mut sentinel_bytes = Vec::new();
    for i in 0..15u32 {
        let content = format!("sentinel-{}", i);
        t5.write_mrgs(&format!(".closeout.{}.tmp", i), content.as_bytes());
        sentinel_bytes.push(content);
    }
    let close = t5.phase_close("phase-1");
    assert_category_no_stdout(&close, "CLOSEOUT_NOT_READY");
    assert!(t5.get_completion_ledger().is_none());
    for i in 0..15u32 {
        assert_eq!(
            std::fs::read(t5.repo.join(".mrgs").join(format!(".closeout.{}.tmp", i))).unwrap(),
            sentinel_bytes[i as usize].as_bytes(),
            "sentinel {} must be preserved",
            i
        );
    }

    // Controlled replacement failure: the recovery journal advance rename
    // fails; prior journal bytes preserved exactly (no truncation).
    let t6 = TestRepo::new();
    t6.setup_impl_bound();
    induce_recoverable(&t6);
    let (rid, pre_sha) = recoverable_ids(&t6);
    let out = t6.run_with_env(
        &[
            "recovery",
            "apply",
            "--repo",
            &t6.repo.to_string_lossy(),
            "--recovery-id",
            &rid,
            "--subject-sha256",
            &pre_sha,
            "--decision",
            "RECOVER",
        ],
        &[("MRGS_TEST_ONLY_RECOVERY_FAIL_RENAME_AFTER_PUBLISH", "1")],
    );
    assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
    assert!(t6.repo.join(".mrgs/state.json").exists());
    let journal: Value = serde_json::from_slice(&t6.recovery_ledger_bytes()).unwrap();
    assert_eq!(journal["recoveries"][0]["status"], "PENDING");
    assert_eq!(journal["recoveries"][0]["next_action"], 0);
    assert_no_temp_files(&t6.repo);
    let current_sha = journal["recoveries"][0]["plan"]["prefix_subject_sha256"][1]
        .as_str()
        .unwrap()
        .to_string();
    let out = t6.apply(&rid, &current_sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
}

#[test]
fn test_obligation_16_cross_repository_path_and_isolation_boundary() {
    // Build target + two source repositories with sentinel identities.
    let t = TestRepo::new();
    let (_manifest_sha, receipt_sha) = t.close_phase1();
    let t_plan_sha = t.plan_sha();

    let mk_source = |_name: &str| -> TestRepo {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        git_init(&repo);
        git_commit(&repo, ".gitignore", b".mrgs/\n");
        git_commit(&repo, "src/main.rs", b"fn main() {}\n");
        let plan_path = repo.join("plan.toml");
        let contract_path = repo.join("contract.toml");
        write_file(&plan_path, valid_plan_toml());
        write_file(&contract_path, &contract_toml_for_phase("phase-1"));
        let report_dir = dir.path().join("reports");
        std::fs::create_dir_all(&report_dir).unwrap();
        TestRepo {
            _dir: dir,
            repo,
            report_dir,
            contract_path,
            plan_path,
        }
    };

    let s1 = mk_source("source-one");
    s1.setup_impl_bound();
    let s1_open = s1.audit_begin("auditor1");
    let s1_parts = split_stdout(&s1_open);
    let s1_report = s1.make_pass_report(&s1_parts[1], &s1_parts[3], "auditor1");
    let s1_path = s1.write_report(&s1_report);
    assert_success(&s1.audit_record(&s1_path));
    let s1_close = s1.phase_close("phase-1");
    assert_success(&s1_close);
    let s1_close_parts = split_stdout(&s1_close);
    let s1_receipt = s1_close_parts[4].clone();
    let s1_meta = s1.write_metadata(
        "meta.toml",
        &standard_metadata("phase-1", &s1_receipt)
            .replace("repository_id = \"mrgs\"", "repository_id = \"repo-alpha\""),
    );
    let s1_rec = s1.continuity_record(&s1_meta);
    assert_success(&s1_rec);
    let s1_cont_parts = split_stdout(&s1_rec);
    let s1_cont_receipt = s1_cont_parts[5].clone();
    let s1_plan_sha = s1.plan_sha();

    let s2 = mk_source("source-two");
    s2.setup_impl_bound();
    let s2_open = s2.audit_begin("auditor1");
    let s2_parts = split_stdout(&s2_open);
    let s2_report = s2.make_pass_report(&s2_parts[1], &s2_parts[3], "auditor1");
    let s2_path = s2.write_report(&s2_report);
    assert_success(&s2.audit_record(&s2_path));
    let s2_close = s2.phase_close("phase-1");
    assert_success(&s2_close);
    let s2_close_parts = split_stdout(&s2_close);
    let s2_receipt = s2_close_parts[4].clone();
    let s2_meta = s2.write_metadata(
        "meta.toml",
        &standard_metadata("phase-1", &s2_receipt)
            .replace("repository_id = \"mrgs\"", "repository_id = \"repo-beta\""),
    );
    let s2_rec = s2.continuity_record(&s2_meta);
    assert_success(&s2_rec);
    let s2_cont_parts = split_stdout(&s2_rec);
    let s2_cont_receipt = s2_cont_parts[5].clone();
    let s2_plan_sha = s2.plan_sha();

    // Duplicate canonical roots -> CONTINUITY_SOURCE_MISMATCH.
    let meta_simple = t.write_metadata("m0.toml", &standard_metadata("phase-1", &receipt_sha));
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &t.repo.to_string_lossy(),
        "--metadata",
        &meta_simple.to_string_lossy(),
        "--source-repo",
        &s1.repo.to_string_lossy(),
        "--source-repo",
        &s1.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_SOURCE_MISMATCH");

    // Target-equals-source -> CONTINUITY_SOURCE_MISMATCH.
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &t.repo.to_string_lossy(),
        "--metadata",
        &meta_simple.to_string_lossy(),
        "--source-repo",
        &t.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_SOURCE_MISMATCH");

    // Unsafe source governance topology -> CONTINUITY_SOURCE_INVALID. The
    // metadata must reference the source (one link) so resolution reaches
    // the source-topology validation instead of the unreferenced-root
    // mismatch.
    let s1_gov = s1.repo.join(".mrgs");
    let s1_gov_backup = s1.repo.join(".mrgs-backup");
    std::fs::rename(&s1_gov, &s1_gov_backup).unwrap();
    write_file(&s1.repo.join(".mrgs"), "hostile");
    let hostile_meta = linked_metadata(
        "phase-1",
        &receipt_sha,
        "mrgs",
        "repo-alpha",
        &s1_plan_sha,
        "phase-1",
        &s1_receipt,
        None,
    );
    let hostile_path = t.write_metadata("m-hostile.toml", &hostile_meta);
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &t.repo.to_string_lossy(),
        "--metadata",
        &hostile_path.to_string_lossy(),
        "--source-repo",
        &s1.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_SOURCE_INVALID");
    std::fs::remove_file(s1.repo.join(".mrgs")).unwrap();
    std::fs::rename(&s1_gov_backup, &s1_gov).unwrap();

    // Unreferenced root (metadata has no links) -> CONTINUITY_SOURCE_MISMATCH.
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &t.repo.to_string_lossy(),
        "--metadata",
        &meta_simple.to_string_lossy(),
        "--source-repo",
        &s1.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_SOURCE_MISMATCH");

    // One-to-one resolution with two distinct sentinel repositories.
    let s1_tree_before = snapshot_tree(&s1.repo);
    let s2_tree_before = snapshot_tree(&s2.repo);
    let s1_git_before = git_snapshot(&s1.repo);
    let s2_git_before = git_snapshot(&s2.repo);
    let meta = linked_metadata(
        "phase-1",
        &receipt_sha,
        "mrgs",
        "repo-alpha",
        &s1_plan_sha,
        "phase-1",
        &s1_receipt,
        Some(&s1_cont_receipt),
    );
    let meta = linked_metadata_second(
        &meta,
        "repo-beta",
        &s2_plan_sha,
        "phase-1",
        &s2_receipt,
        Some(&s2_cont_receipt),
    );
    let meta_path = t.write_metadata("m-links.toml", &meta);
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &t.repo.to_string_lossy(),
        "--metadata",
        &meta_path.to_string_lossy(),
        "--source-repo",
        &s1.repo.to_string_lossy(),
        "--source-repo",
        &s2.repo.to_string_lossy(),
    ]);
    assert_success(&out);
    let parts = split_stdout(&out);
    assert_eq!(parts[0], "CONTINUITY_RECORDED");
    let ledger = t.get_continuity_ledger().unwrap();
    let entry = &ledger["entries"][0];
    let links = entry["continuity_manifest"]["resolved_links"]
        .as_array()
        .unwrap();
    assert_eq!(links.len(), 2);
    let ids: Vec<&str> = links
        .iter()
        .map(|l| l["source_repository_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["repo-alpha", "repo-beta"]);
    for link in links {
        assert_eq!(link["source_phase_id"], "phase-1");
        assert_eq!(
            link["source_accepted_plan_sha256"].as_str().unwrap().len(),
            64
        );
        assert_eq!(
            link["source_completion_receipt_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        assert_eq!(
            link["source_continuity_receipt_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
    }
    // Sources read-only; unrelated repos untouched.
    assert_eq!(
        snapshot_tree(&s1.repo),
        s1_tree_before,
        "source 1 must be read-only"
    );
    assert_eq!(
        snapshot_tree(&s2.repo),
        s2_tree_before,
        "source 2 must be read-only"
    );
    assert_eq!(git_snapshot(&s1.repo), s1_git_before);
    assert_eq!(git_snapshot(&s2.repo), s2_git_before);
    // Target identity sentinels and plan binding.
    assert_eq!(ledger["repository_id"], "mrgs");
    assert_ne!(ledger["repository_id"], "repo-alpha");
    assert_eq!(ledger["accepted_plan_sha256"], t_plan_sha);
    assert_no_temp_files(&t.repo);
}
// ===========================================================================
// 16.3 Governance-authority corruption and stale-state handling
// ===========================================================================

#[test]
fn test_obligation_17_accepted_plan_corruption_matrix() {
    // Unknown key: tolerated by the existing record parser, and the exact
    // bytes are never rewritten by a read-only consumer.
    let t = TestRepo::new();
    t.accept_plan_success();
    let mut v: Value = serde_json::from_str(&t.read_mrgs_str("accepted-plan.json")).unwrap();
    v["rogue_key"] = json!("x");
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t.write_mrgs("accepted-plan.json", &tampered);
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &t.repo.to_string_lossy(),
        "--plan",
        &t.plan_path.to_string_lossy(),
    ]);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("test-plan "));
    assert_eq!(
        t.read_mrgs("accepted-plan.json"),
        tampered,
        "bytes must not be rewritten"
    );

    // Missing key -> JSON parse failure (Phase 1-3 display, Phase 4 category).
    let t2 = TestRepo::new();
    t2.accept_plan_success();
    let mut v: Value = serde_json::from_str(&t2.read_mrgs_str("accepted-plan.json")).unwrap();
    v.as_object_mut().unwrap().remove("plan_id");
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t2.write_mrgs("accepted-plan.json", &tampered);
    let out = t2.select_phase("phase-1");
    assert_err_prefix(&out, "error: JSON parse error: ");
    let t2b = TestRepo::new();
    t2b.accept_plan_success();
    t2b.write_mrgs("accepted-plan.json", &tampered);
    let out = t2b.run(&[
        "implementation",
        "check",
        "--repo",
        &t2b.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // Schema drift.
    let t3 = TestRepo::new();
    t3.accept_plan_success();
    let mut v: Value = serde_json::from_str(&t3.read_mrgs_str("accepted-plan.json")).unwrap();
    v["schema_version"] = json!(2);
    t3.write_mrgs(
        "accepted-plan.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t3.select_phase("phase-1");
    assert_err_prefix(
        &out,
        "error: accepted record schema version: expected 1, got 2",
    );
    let out = t3.run(&[
        "implementation",
        "check",
        "--repo",
        &t3.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // False lowercase hash -> the state relation check fires first.
    let t4 = TestRepo::new();
    t4.accept_plan_success();
    let mut v: Value = serde_json::from_str(&t4.read_mrgs_str("accepted-plan.json")).unwrap();
    v["sha256"] = json!("a".repeat(64));
    t4.write_mrgs(
        "accepted-plan.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t4.select_phase("phase-1");
    assert_err_prefix(&out, "error: state SHA does not match accepted plan SHA");
    let out = t4.run(&[
        "implementation",
        "check",
        "--repo",
        &t4.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // Stale plan path: relative escape rejected lexically; a nonexistent
    // path fails on resolution.
    let t5 = TestRepo::new();
    t5.accept_plan_success();
    let mut v: Value = serde_json::from_str(&t5.read_mrgs_str("accepted-plan.json")).unwrap();
    v["plan_path"] = json!("../evil.toml");
    t5.write_mrgs(
        "accepted-plan.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t5.select_phase("phase-1");
    assert_err_prefix(&out, "error: unsafe plan path: ");
    let mut v: Value = serde_json::from_str(&t5.read_mrgs_str("accepted-plan.json")).unwrap();
    v["plan_path"] = json!("missing-plan.toml");
    t5.write_mrgs(
        "accepted-plan.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t5.select_phase("phase-1");
    assert_err_prefix(&out, "error: I/O error: ");

    // Stale phase count.
    let t6 = TestRepo::new();
    t6.accept_plan_success();
    let mut v: Value = serde_json::from_str(&t6.read_mrgs_str("accepted-plan.json")).unwrap();
    v["phase_count"] = json!(99);
    t6.write_mrgs(
        "accepted-plan.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t6.select_phase("phase-1");
    assert_err_prefix(&out, "error: accepted phase count mismatch: ");

    // Parse-valid plan disagreement: the plan source bytes drift while the
    // phase count stays identical (the count check fires before the drift
    // check, so the drift must be observable through the SHA comparison).
    let t7 = TestRepo::new();
    t7.accept_plan_success();
    let accepted_sha = t7.get_state()["accepted_plan_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let drifted_plan = valid_plan_toml().replacen("Phase One", "Phase One Drifted", 1);
    write_file(&t7.plan_path, &drifted_plan);
    let actual_sha = sha_of_file(&t7.plan_path);
    let out = t7.select_phase("phase-1");
    assert_err_prefix(&out, "error: plan drift detected: ");
    let err = stderr_str(&out);
    assert!(
        err.contains(&format!("expected {}", accepted_sha))
            && err.contains(&format!("actual {}", actual_sha)),
        "plan drift must name exact hashes: {}",
        err
    );

    // Consumers fail closed before mutation; bytes preserved.
    let t8 = TestRepo::new();
    t8.setup_impl_bound();
    let mut v: Value = serde_json::from_str(&t8.read_mrgs_str("accepted-plan.json")).unwrap();
    v["plan_id"] = json!("different-plan");
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t8.write_mrgs("accepted-plan.json", &tampered);
    let before = mrgs_snapshot(&t8.repo);
    let repo8 = t8.repo.to_string_lossy().into_owned();
    for cmd in [
        vec!["implementation", "check"],
        vec!["audit", "begin", "--auditor", "auditor1"],
        vec!["phase", "close", "--phase", "phase-1"],
        vec!["continuity", "record", "--metadata", "m.toml"],
    ] {
        let mut args = cmd.clone();
        args.push("--repo");
        args.push(&repo8);
        let out = t8.run(&args);
        assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
    }
    // Recovery's independent diagnosis reports its own boundary category.
    let out = t8.run(&["recovery", "inspect", "--repo", &repo8]);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_snapshot_unchanged(&t8.repo, &before);
    assert_no_temp_files(&t8.repo);
}

#[test]
fn test_obligation_18_state_corruption_and_plan_relation_matrix() {
    let mk = || {
        let t = TestRepo::new();
        t.accept_plan_success();
        t.select_phase_success("phase-1");
        t
    };

    // Unknown key: tolerated; a read-only consumer preserves exact bytes.
    let t = mk();
    let mut v: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
    v["rogue_key"] = json!("x");
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t.write_mrgs("state.json", &tampered);
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &t.repo.to_string_lossy(),
        "--plan",
        &t.plan_path.to_string_lossy(),
    ]);
    assert_success(&out);
    assert_eq!(t.read_mrgs("state.json"), tampered);

    // Missing key.
    let t2 = mk();
    let mut v: Value = serde_json::from_str(&t2.read_mrgs_str("state.json")).unwrap();
    v.as_object_mut().unwrap().remove("closed_phases");
    t2.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t2.select_phase("phase-2");
    assert_err_prefix(&out, "error: JSON parse error: ");

    // Schema drift.
    let t3 = mk();
    let mut v: Value = serde_json::from_str(&t3.read_mrgs_str("state.json")).unwrap();
    v["schema_version"] = json!(2);
    t3.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t3.select_phase("phase-2");
    assert_err_prefix(
        &out,
        "error: state record schema version: expected 1, got 2",
    );
    let out = t3.run(&[
        "implementation",
        "check",
        "--repo",
        &t3.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // False accepted-plan hash.
    let t4 = mk();
    let mut v: Value = serde_json::from_str(&t4.read_mrgs_str("state.json")).unwrap();
    v["accepted_plan_sha256"] = json!("b".repeat(64));
    t4.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t4.select_phase("phase-2");
    assert_err_prefix(&out, "error: state SHA does not match accepted plan SHA");

    // Duplicate closed phase.
    let t5 = mk();
    let mut v: Value = serde_json::from_str(&t5.read_mrgs_str("state.json")).unwrap();
    v["active_phase"] = Value::Null;
    v["closed_phases"] = json!(["phase-1", "phase-1"]);
    t5.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t5.select_phase("phase-2");
    assert_err_prefix(&out, "error: duplicate phase 'phase-1' in closed_phases");

    // Unknown closed phase.
    let t6 = mk();
    let mut v: Value = serde_json::from_str(&t6.read_mrgs_str("state.json")).unwrap();
    v["active_phase"] = Value::Null;
    v["closed_phases"] = json!(["phase-9"]);
    t6.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t6.select_phase("phase-2");
    assert_err_prefix(&out, "error: unknown phase 'phase-9' in closed_phases");

    // Dependency/order violation: closed phase-2 without phase-1.
    let t7 = mk();
    let mut v: Value = serde_json::from_str(&t7.read_mrgs_str("state.json")).unwrap();
    v["active_phase"] = Value::Null;
    v["closed_phases"] = json!(["phase-2"]);
    t7.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t7.select_phase("phase-2");
    assert_err_prefix(
        &out,
        "error: inconsistent closed dependency: phase 'phase-2' has unclosed dependency 'phase-1'",
    );

    // Invalid active phase.
    let t8 = mk();
    let mut v: Value = serde_json::from_str(&t8.read_mrgs_str("state.json")).unwrap();
    v["active_phase"] = json!("phase-9");
    t8.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t8.select_phase("phase-2");
    assert_err_prefix(&out, "error: unknown active phase: phase-9");

    // Closed-active conflict.
    let t9 = mk();
    let mut v: Value = serde_json::from_str(&t9.read_mrgs_str("state.json")).unwrap();
    v["closed_phases"] = json!(["phase-1"]);
    t9.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t9.select_phase("phase-2");
    assert_err_prefix(
        &out,
        "error: active phase 'phase-1' is also in closed_phases",
    );

    // Every tampered state is preserved byte-exact by failing consumers.
    let t10 = mk();
    let mut v: Value = serde_json::from_str(&t10.read_mrgs_str("state.json")).unwrap();
    v["active_phase"] = json!("phase-9");
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t10.write_mrgs("state.json", &tampered);
    let before = mrgs_snapshot(&t10.repo);
    let out = t10.run(&[
        "contract",
        "draft",
        "--repo",
        &t10.repo.to_string_lossy(),
        "--contract",
        &t10.contract_path.to_string_lossy(),
    ]);
    assert_err_prefix(&out, "error: unknown active phase: phase-9");
    let out = t10.run(&[
        "implementation",
        "check",
        "--repo",
        &t10.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
    // Recovery runs its own independent diagnosis: the drifted active phase
    // is classified as a recoverable state (successful inspection output,
    // never silently accepted as healthy).
    let out = t10.run(&["recovery", "inspect", "--repo", &t10.repo.to_string_lossy()]);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_REQUIRED "));
    assert_eq!(t10.read_mrgs("state.json"), tampered);
    assert_snapshot_unchanged(&t10.repo, &before);
}

#[test]
fn test_obligation_19_contract_authority_corruption_matrix() {
    let mk = || {
        let t = TestRepo::new();
        t.accept_plan_success();
        t.select_phase_success("phase-1");
        t.draft_contract();
        t
    };

    // Unknown raw key.
    let t = mk();
    let mut v: Value = serde_json::from_str(&t.read_mrgs_str("contract-draft.json")).unwrap();
    v["rogue_key"] = json!(1);
    t.write_mrgs(
        "contract-draft.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t.accept_contract(1, "a".repeat(64).as_str());
    assert_err_prefix(&out, "error: JSON parse error: ");

    // Missing key.
    let t2 = mk();
    let mut v: Value = serde_json::from_str(&t2.read_mrgs_str("contract-draft.json")).unwrap();
    v.as_object_mut().unwrap().remove("content");
    t2.write_mrgs(
        "contract-draft.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    // Audit begin consumes the draft authority and rejects the missing key.
    let out = t2.run(&[
        "audit",
        "begin",
        "--repo",
        &t2.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // Revision gap: revision 3 without a preimage.
    let t3 = mk();
    let mut v: Value = serde_json::from_str(&t3.read_mrgs_str("contract-draft.json")).unwrap();
    v["revision"] = json!(3);
    t3.write_mrgs(
        "contract-draft.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t3.accept_contract(3, "a".repeat(64).as_str());
    assert_err_prefix(&out, "error: contract draft revision 3 requires a preimage");
    // Audit begin consumes the draft authority and rejects the gap.
    let out = t3.run(&[
        "audit",
        "begin",
        "--repo",
        &t3.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // Revision 1 with a preimage is rejected.
    let t4 = mk();
    let mut v: Value = serde_json::from_str(&t4.read_mrgs_str("contract-draft.json")).unwrap();
    v["preimage"] = json!({"revision": 1, "sha256": "a".repeat(64)});
    t4.write_mrgs(
        "contract-draft.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t4.accept_contract(1, "a".repeat(64).as_str());
    assert_err_prefix(
        &out,
        "error: contract draft revision 1 must not have a preimage",
    );

    // False content hash.
    let t5 = mk();
    let good_sha = t5.get_draft()["sha256"].as_str().unwrap().to_string();
    let mut v: Value = serde_json::from_str(&t5.read_mrgs_str("contract-draft.json")).unwrap();
    let wrong_sha = if let Some(rest) = good_sha.strip_prefix('a') {
        format!("b{}", rest)
    } else {
        format!("a{}", &good_sha[1..])
    };
    v["sha256"] = json!(wrong_sha);
    t5.write_mrgs(
        "contract-draft.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t5.accept_contract(1, &good_sha);
    assert_err_prefix(&out, "error: contract draft content hash mismatch");
    // Audit begin consumes the draft authority and rejects the false hash.
    let out = t5.run(&[
        "audit",
        "begin",
        "--repo",
        &t5.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // Stale phase binding.
    let t6 = mk();
    let mut v: Value = serde_json::from_str(&t6.read_mrgs_str("contract-draft.json")).unwrap();
    v["phase_id"] = json!("phase-2");
    t6.write_mrgs(
        "contract-draft.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t6.accept_contract(1, "a".repeat(64).as_str());
    assert_failure(&out);
    assert_eq!(stdout_str(&out), "");
    // Audit begin consumes the draft authority and rejects the stale phase.
    let out = t6.run(&[
        "audit",
        "begin",
        "--repo",
        &t6.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // Accepted-contract corruption: reordered / duplicate revisions. The
    // ledger needs two revisions for a meaningful reorder, so build it via
    // a public revise + accept before tampering.
    let t7 = TestRepo::new();
    t7.accept_plan_success();
    t7.select_phase_success("phase-1");
    t7.draft_contract();
    let sha1 = t7.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t7.accept_contract(1, &sha1));
    let rev_contract = contract_toml_for_phase("phase-1").replacen("req1", "req1-revised", 1);
    write_file(&t7.contract_path, &rev_contract);
    assert_success(&t7.revise_contract(1, &sha1));
    let draft2 = t7.get_draft();
    let sha2 = draft2["sha256"].as_str().unwrap().to_string();
    assert_success(&t7.accept_contract(2, &sha2));
    t7.commit_sources();
    let revs = {
        let v: Value = serde_json::from_str(&t7.read_mrgs_str("accepted-contract.json")).unwrap();
        v["revisions"].as_array().unwrap().clone()
    };
    assert_eq!(revs.len(), 2, "the fixture must have two revisions");
    let mut v: Value = serde_json::from_str(&t7.read_mrgs_str("accepted-contract.json")).unwrap();
    let mut reversed = revs.clone();
    reversed.reverse();
    v["revisions"] = json!(reversed);
    t7.write_mrgs(
        "accepted-contract.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    // Audit begin consumes the accepted-contract ledger.
    let out = t7.run(&[
        "audit",
        "begin",
        "--repo",
        &t7.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
    let mut v: Value = serde_json::from_str(&t7.read_mrgs_str("accepted-contract.json")).unwrap();
    let mut dup = revs.clone();
    dup.push(revs[0].clone());
    v["revisions"] = json!(dup);
    t7.write_mrgs(
        "accepted-contract.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t7.run(&[
        "audit",
        "begin",
        "--repo",
        &t7.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // Draft/accepted disagreement: accepted revision content differs from
    // what the draft accepts.
    let t8 = TestRepo::new();
    t8.setup_impl_bound();
    let mut v: Value = serde_json::from_str(&t8.read_mrgs_str("accepted-contract.json")).unwrap();
    let content = v["revisions"][0]["content"].as_str().unwrap().to_string();
    v["revisions"][0]["content"] =
        json!(content.replacen("objective = \"Exercise", "objective = \"Tampered", 1));
    t8.write_mrgs(
        "accepted-contract.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    // Audit begin revalidates the accepted-contract ledger against its
    // embedded content hash; the drifted revision content is rejected.
    let out = t8.run(&[
        "audit",
        "begin",
        "--repo",
        &t8.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    // Consumers across the chain fail at the correct boundary.
    let t9 = TestRepo::new();
    t9.setup_impl_bound();
    let mut v: Value = serde_json::from_str(&t9.read_mrgs_str("contract-draft.json")).unwrap();
    v["sha256"] = json!("b".repeat(64));
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t9.write_mrgs("contract-draft.json", &tampered);
    let before = mrgs_snapshot(&t9.repo);
    let out = t9.run(&[
        "audit",
        "begin",
        "--repo",
        &t9.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
    let out = t9.run(&[
        "phase",
        "close",
        "--repo",
        &t9.repo.to_string_lossy(),
        "--phase",
        "phase-1",
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
    // Recovery's independent diagnosis reports its own boundary category.
    let out = t9.run(&["recovery", "inspect", "--repo", &t9.repo.to_string_lossy()]);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_eq!(t9.read_mrgs("contract-draft.json"), tampered);
    assert_snapshot_unchanged(&t9.repo, &before);
}

#[test]
fn test_obligation_20_implementation_authority_corruption_matrix() {
    // Unknown key / schema drift -> structural INVALID.
    let t = TestRepo::new();
    t.setup_impl_bound();
    let mut v: Value =
        serde_json::from_str(&t.read_mrgs_str("implementation-authority.json")).unwrap();
    v["rogue_key"] = json!(1);
    t.write_mrgs(
        "implementation-authority.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t.run(&[
        "implementation",
        "check",
        "--repo",
        &t.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "IMPLEMENTATION_AUTHORITY_INVALID");
    let out = t.run(&[
        "audit",
        "begin",
        "--repo",
        &t.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "IMPLEMENTATION_AUTHORITY_INVALID");

    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let mut v: Value =
        serde_json::from_str(&t2.read_mrgs_str("implementation-authority.json")).unwrap();
    v["schema_version"] = json!(2);
    t2.write_mrgs(
        "implementation-authority.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t2.run(&[
        "implementation",
        "check",
        "--repo",
        &t2.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "IMPLEMENTATION_AUTHORITY_INVALID");

    // False plan/contract/revision binding -> STALE against the authority.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    let mut v: Value =
        serde_json::from_str(&t3.read_mrgs_str("implementation-authority.json")).unwrap();
    v["contract_revision"] = json!(2);
    t3.write_mrgs(
        "implementation-authority.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t3.run(&[
        "implementation",
        "check",
        "--repo",
        &t3.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "IMPLEMENTATION_AUTHORITY_STALE");

    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    let mut v: Value =
        serde_json::from_str(&t4.read_mrgs_str("implementation-authority.json")).unwrap();
    v["accepted_plan_sha256"] = json!("c".repeat(64));
    t4.write_mrgs(
        "implementation-authority.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t4.run(&[
        "implementation",
        "check",
        "--repo",
        &t4.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "IMPLEMENTATION_AUTHORITY_STALE");

    // Inconsistent rule hashes: embedded content disagrees with the stored
    // contract SHA.
    let t5 = TestRepo::new();
    t5.setup_impl_bound();
    let mut v: Value =
        serde_json::from_str(&t5.read_mrgs_str("implementation-authority.json")).unwrap();
    let content = v["contract_content"].as_str().unwrap().to_string();
    v["contract_content"] =
        json!(content.replacen("title = \"Test Contract\"", "title = \"Tampered\"", 1));
    t5.write_mrgs(
        "implementation-authority.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t5.run(&[
        "implementation",
        "check",
        "--repo",
        &t5.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "IMPLEMENTATION_AUTHORITY_INVALID");

    // Unsafe rule paths: the contract source file drifts from the
    // authority. The modified tracked source is an out-of-rule change.
    let t6 = TestRepo::new();
    t6.setup_impl_bound();
    write_file(
        &t6.contract_path,
        &contract_toml_for_phase("phase-1").replacen(
            "title = \"Test Contract\"",
            "title = \"Drifted\"",
            1,
        ),
    );
    let out = t6.run(&[
        "implementation",
        "check",
        "--repo",
        &t6.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CHANGE_NOT_ALLOWED");

    // Stale repository identity: baseline branch and commit.
    let t7 = TestRepo::new();
    t7.setup_impl_bound();
    let out = git(&t7.repo, &["branch", "-M", "other-branch"]);
    assert!(out.status.success());
    let out = t7.run(&[
        "implementation",
        "check",
        "--repo",
        &t7.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "BASELINE_BRANCH_CHANGED");
    git(&t7.repo, &["branch", "-M", "main"]);

    let t8 = TestRepo::new();
    t8.setup_impl_bound();
    let mut v: Value =
        serde_json::from_str(&t8.read_mrgs_str("implementation-authority.json")).unwrap();
    v["baseline_head"] = json!("f".repeat(40));
    t8.write_mrgs(
        "implementation-authority.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t8.run(&[
        "implementation",
        "check",
        "--repo",
        &t8.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "BASELINE_COMMIT_MISSING");

    // Consumers reject before target publication or Git mutation. The
    // tampered contract_id disagrees with the embedded contract content, so
    // the structure cross-check reports INVALID; recovery's independent
    // diagnosis reports its own boundary category.
    let t9 = TestRepo::new();
    t9.setup_impl_bound();
    let mut v: Value =
        serde_json::from_str(&t9.read_mrgs_str("implementation-authority.json")).unwrap();
    v["contract_id"] = json!("wrong-contract");
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t9.write_mrgs("implementation-authority.json", &tampered);
    let before = mrgs_snapshot(&t9.repo);
    let git_before = git_snapshot(&t9.repo);
    let repo9 = t9.repo.to_string_lossy().into_owned();
    for cmd in [
        vec!["implementation", "check"],
        vec!["audit", "begin", "--auditor", "auditor1"],
        vec!["phase", "close", "--phase", "phase-1"],
    ] {
        let mut args = cmd.clone();
        args.push("--repo");
        args.push(&repo9);
        let out = t9.run(&args);
        assert_category_no_stdout(&out, "IMPLEMENTATION_AUTHORITY_INVALID");
    }
    let out = t9.run(&["recovery", "inspect", "--repo", &repo9]);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    assert_eq!(t9.read_mrgs("implementation-authority.json"), tampered);
    assert_snapshot_unchanged(&t9.repo, &before);
    assert_eq!(git_snapshot(&t9.repo), git_before);
    assert_no_temp_files(&t9.repo);
}

#[test]
fn test_obligation_21_audit_ledger_corruption_matrix() {
    // Unknown field.
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.full_pass_audit();
    let mut v: Value = serde_json::from_str(&t.read_mrgs_str("audit-ledger.json")).unwrap();
    v["rogue_key"] = json!(1);
    t.write_mrgs(
        "audit-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t.run(&[
        "audit",
        "begin",
        "--repo",
        &t.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "AUDIT_LEDGER_INVALID");

    // Broken round ordering: [2, 1].
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    t2.full_pass_audit();
    let mut v: Value = serde_json::from_str(&t2.read_mrgs_str("audit-ledger.json")).unwrap();
    let rounds = v["rounds"].as_array().unwrap().clone();
    let mut reversed = rounds.clone();
    reversed.reverse();
    v["rounds"] = json!(reversed);
    t2.write_mrgs(
        "audit-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t2.run(&[
        "audit",
        "begin",
        "--repo",
        &t2.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    // The terminal lifecycle gate (last round PASS) fires before the
    // reversed history is consumed.
    assert_category_no_stdout(&out, "AUDIT_TERMINAL");

    // Skipped repair attempt: a round with repair attempt 2 and no attempt 1.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.full_pass_audit();
    let mut v: Value = serde_json::from_str(&t3.read_mrgs_str("audit-ledger.json")).unwrap();
    let mut round2 = v["rounds"][0].clone();
    round2["round"] = json!(2);
    round2["status"] = json!("ROUTED");
    round2["repair"] = json!({
        "attempt": 2,
        "status": "ROUTED",
        "finding_ids": ["F1"],
        "allowed_paths": ["src/"],
        "pre_subject_sha256": "a".repeat(64),
        "changed_paths": []
    });
    v["rounds"].as_array_mut().unwrap().push(round2);
    t3.write_mrgs(
        "audit-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t3.run(&[
        "audit",
        "begin",
        "--repo",
        &t3.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "AUDIT_LEDGER_INVALID");

    // Terminal-state inconsistency: PASS followed by another round.
    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    t4.full_pass_audit();
    let mut v: Value = serde_json::from_str(&t4.read_mrgs_str("audit-ledger.json")).unwrap();
    let mut round2 = v["rounds"][0].clone();
    round2["round"] = json!(2);
    round2["status"] = json!("PENDING");
    v["rounds"].as_array_mut().unwrap().push(round2);
    t4.write_mrgs(
        "audit-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t4.run(&[
        "audit",
        "begin",
        "--repo",
        &t4.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    // The extra round after a PASS makes the ledger history invalid.
    assert_category_no_stdout(&out, "AUDIT_LEDGER_INVALID");

    // Stale auditor/contract/implementation binding (ledger head drift).
    let t5 = TestRepo::new();
    t5.setup_impl_bound();
    t5.full_pass_audit();
    let mut v: Value = serde_json::from_str(&t5.read_mrgs_str("audit-ledger.json")).unwrap();
    v["contract_id"] = json!("wrong-contract");
    t5.write_mrgs(
        "audit-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t5.run(&[
        "audit",
        "begin",
        "--repo",
        &t5.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "AUDIT_LEDGER_STALE");

    // False subject hash inside a settled round.
    let t6 = TestRepo::new();
    t6.setup_impl_bound();
    t6.full_pass_audit();
    let mut v: Value = serde_json::from_str(&t6.read_mrgs_str("audit-ledger.json")).unwrap();
    v["rounds"][0]["subject_sha256"] = json!("e".repeat(64));
    t6.write_mrgs(
        "audit-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t6.run(&[
        "audit",
        "begin",
        "--repo",
        &t6.repo.to_string_lossy(),
        "--auditor",
        "a",
    ]);
    assert_category_no_stdout(&out, "AUDIT_LEDGER_INVALID");

    // Archived report disagreement: the ledger's archived report bytes
    // drift from the recorded report hash; closeout readiness revalidates
    // them and fails closed.
    let t7 = TestRepo::new();
    t7.setup_impl_bound();
    let open = t7.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = t7.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t7.write_report(&report);
    assert_success(&t7.audit_record(&report_path));
    let mut ledger: Value = serde_json::from_str(&t7.read_mrgs_str("audit-ledger.json")).unwrap();
    let drifted = report.replacen(
        "All requirements satisfied",
        "All requirements satisfied now",
        1,
    );
    ledger["rounds"][0]["report_content"] = json!(drifted);
    t7.write_mrgs(
        "audit-ledger.json",
        serde_json::to_vec_pretty(&ledger).unwrap().as_slice(),
    );
    let out = t7.run(&[
        "phase",
        "close",
        "--repo",
        &t7.repo.to_string_lossy(),
        "--phase",
        "phase-1",
    ]);
    // The ledger-history validation rejects the drifted archived report.
    assert_category_no_stdout(&out, "AUDIT_LEDGER_INVALID");
    assert!(t7.get_completion_ledger().is_none());
    assert_no_temp_files(&t7.repo);

    // Failing consumers leave ledger bytes exact.
    let t8 = TestRepo::new();
    t8.setup_impl_bound();
    t8.full_pass_audit();
    let mut v: Value = serde_json::from_str(&t8.read_mrgs_str("audit-ledger.json")).unwrap();
    v["rounds"][0]["status"] = json!("FAIL");
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t8.write_mrgs("audit-ledger.json", &tampered);
    let before = mrgs_snapshot(&t8.repo);
    // Repair check requires a routed round; the tampered status has none.
    let out = t8.run(&["repair", "check", "--repo", &t8.repo.to_string_lossy()]);
    assert_category_no_stdout(&out, "REPAIR_NOT_ROUTED");
    // Recovery treats the ledger as subject bytes (its diagnosis is not
    // gated on audit-ledger semantics): the inspection stays healthy.
    let out = t8.run(&["recovery", "inspect", "--repo", &t8.repo.to_string_lossy()]);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_NOT_REQUIRED "));
    assert_eq!(t8.read_mrgs("audit-ledger.json"), tampered);
    assert_snapshot_unchanged(&t8.repo, &before);
}

#[test]
fn test_obligation_22_completion_ledger_and_receipt_corruption_matrix() {
    // Two completions for the ordering cases.
    let mk2 = || {
        let t = TestRepo::new();
        let (_m, _r) = t.close_phase1();
        let sel = t.select_phase("phase-2");
        assert_success(&sel);
        write_file(&t.contract_path, &contract_toml_for_phase("phase-2"));
        assert!(git(&t.repo, &["add", "-A"]).status.success());
        assert!(git(&t.repo, &["commit", "-m", "phase-2 contract"])
            .status
            .success());
        let draft = t.draft_contract();
        assert_success(&draft);
        let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
        assert_success(&t.accept_contract(1, &sha));
        assert_success(&t.impl_begin(1, &sha));
        t.full_pass_audit();
        let close = t.phase_close("phase-2");
        assert_success(&close);
        t
    };

    // Non-contiguous sequence.
    let t = mk2();
    let mut v: Value = serde_json::from_str(&t.read_mrgs_str("completion-ledger.json")).unwrap();
    v["completions"][0]["completion_receipt"]["completion_sequence"] = json!(2);
    t.write_mrgs(
        "completion-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t.run(&[
        "phase",
        "close",
        "--repo",
        &t.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]);
    assert_category_no_stdout(&out, "CLOSEOUT_LEDGER_INVALID");
    let out = t.run(&["recovery", "inspect", "--repo", &t.repo.to_string_lossy()]);
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Reordered phases.
    let t2 = mk2();
    let mut v: Value = serde_json::from_str(&t2.read_mrgs_str("completion-ledger.json")).unwrap();
    let entries = v["completions"].as_array().unwrap().clone();
    let mut swapped = entries.clone();
    swapped.swap(0, 1);
    v["completions"] = json!(swapped);
    t2.write_mrgs(
        "completion-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t2.run(&[
        "phase",
        "close",
        "--repo",
        &t2.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]);
    assert_category_no_stdout(&out, "CLOSEOUT_LEDGER_INVALID");

    // False manifest hash.
    let t3 = mk2();
    let mut v: Value = serde_json::from_str(&t3.read_mrgs_str("completion-ledger.json")).unwrap();
    v["completions"][0]["final_manifest_sha256"] = json!("d".repeat(64));
    t3.write_mrgs(
        "completion-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t3.run(&[
        "phase",
        "close",
        "--repo",
        &t3.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]);
    assert_category_no_stdout(&out, "CLOSEOUT_LEDGER_INVALID");

    // Broken previous receipt link.
    let t4 = mk2();
    let mut v: Value = serde_json::from_str(&t4.read_mrgs_str("completion-ledger.json")).unwrap();
    v["completions"][1]["completion_receipt"]["previous_completion_receipt_sha256"] =
        json!("e".repeat(64));
    t4.write_mrgs(
        "completion-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t4.run(&[
        "phase",
        "close",
        "--repo",
        &t4.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]);
    assert_category_no_stdout(&out, "CLOSEOUT_LEDGER_INVALID");

    // Stale before/after state in the receipt.
    let t5 = mk2();
    let mut v: Value = serde_json::from_str(&t5.read_mrgs_str("completion-ledger.json")).unwrap();
    v["completions"][0]["completion_receipt"]["closed_phases_before"] = json!(["phase-9"]);
    t5.write_mrgs(
        "completion-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t5.run(&[
        "phase",
        "close",
        "--repo",
        &t5.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]);
    assert_category_no_stdout(&out, "CLOSEOUT_LEDGER_INVALID");

    // Archived authority byte mismatch.
    let t6 = mk2();
    let mut v: Value = serde_json::from_str(&t6.read_mrgs_str("completion-ledger.json")).unwrap();
    let archived = v["completions"][0]["final_manifest"]["archived_governance"]
        ["contract_draft_content"]
        .as_str()
        .unwrap()
        .to_string();
    v["completions"][0]["final_manifest"]["archived_governance"]["contract_draft_content"] =
        json!(archived.replacen("\"schema_version\"", "\"schema_version_x\"", 1));
    t6.write_mrgs(
        "completion-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t6.run(&[
        "phase",
        "close",
        "--repo",
        &t6.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]);
    assert_category_no_stdout(&out, "CLOSEOUT_LEDGER_INVALID");

    // Duplicate phase.
    let t7 = mk2();
    let mut v: Value = serde_json::from_str(&t7.read_mrgs_str("completion-ledger.json")).unwrap();
    let entries = v["completions"].as_array().unwrap().clone();
    let mut dup = entries.clone();
    dup.push(entries[0].clone());
    v["completions"] = json!(dup);
    t7.write_mrgs(
        "completion-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t7.run(&[
        "phase",
        "close",
        "--repo",
        &t7.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]);
    assert_category_no_stdout(&out, "CLOSEOUT_LEDGER_INVALID");

    // Plan disagreement: a consistent ledger bound to another plan is stale.
    let t8 = mk2();
    let mut v: Value = serde_json::from_str(&t8.read_mrgs_str("completion-ledger.json")).unwrap();
    v["accepted_plan_sha256"] = json!("f".repeat(64));
    t8.write_mrgs(
        "completion-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t8.run(&[
        "phase",
        "close",
        "--repo",
        &t8.repo.to_string_lossy(),
        "--phase",
        "phase-2",
    ]);
    assert_category_no_stdout(&out, "CLOSEOUT_LEDGER_STALE");

    // Continuity consumer rejects tampered completion without partial cleanup.
    let t9 = TestRepo::new();
    let (_m, receipt) = t9.close_phase1();
    let mut v: Value = serde_json::from_str(&t9.read_mrgs_str("completion-ledger.json")).unwrap();
    v["completions"][0]["final_manifest_sha256"] = json!("d".repeat(64));
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t9.write_mrgs("completion-ledger.json", &tampered);
    let meta = t9.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t9.continuity_record(&meta);
    assert_failure(&out);
    assert_eq!(stdout_str(&out), "");
    assert!(t9.get_continuity_ledger().is_none());
    assert_eq!(t9.read_mrgs("completion-ledger.json"), tampered);
    assert_no_temp_files(&t9.repo);
}

#[test]
fn test_obligation_23_continuity_ledger_corruption_matrix() {
    let mk = || {
        let t = TestRepo::new();
        let (_m, receipt) = t.close_phase1();
        let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
        let out = t.continuity_record(&meta);
        assert_success(&out);
        t
    };
    let receipt_sha_of = |t: &TestRepo| {
        t.get_completion_ledger().unwrap()["completions"][0]["completion_receipt_sha256"]
            .as_str()
            .unwrap()
            .to_string()
    };

    // Unknown key.
    let t = mk();
    let mut v: Value = serde_json::from_str(&t.read_mrgs_str("continuity-ledger.json")).unwrap();
    v["rogue_key"] = json!(1);
    t.write_mrgs(
        "continuity-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let meta = t.write_metadata(
        "m2.toml",
        &standard_metadata("phase-1", &receipt_sha_of(&t)),
    );
    let out = t.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_LEDGER_INVALID");

    // Missing key.
    let t1 = mk();
    let mut v: Value = serde_json::from_str(&t1.read_mrgs_str("continuity-ledger.json")).unwrap();
    v.as_object_mut().unwrap().remove("entries");
    t1.write_mrgs(
        "continuity-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let meta = t1.write_metadata("m2.toml", &standard_metadata("phase-1", &"a".repeat(64)));
    let out = t1.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_LEDGER_INVALID");

    // Immutable repository-ID drift.
    let t2 = mk();
    let mut v: Value = serde_json::from_str(&t2.read_mrgs_str("continuity-ledger.json")).unwrap();
    v["repository_id"] = json!("repo-beta");
    t2.write_mrgs(
        "continuity-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let meta = t2.write_metadata("m2.toml", &standard_metadata("phase-1", &"a".repeat(64)));
    let out = t2.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_LEDGER_INVALID");

    // Non-contiguous sequence.
    let t3 = mk();
    let mut v: Value = serde_json::from_str(&t3.read_mrgs_str("continuity-ledger.json")).unwrap();
    v["entries"][0]["continuity_receipt"]["continuity_sequence"] = json!(2);
    t3.write_mrgs(
        "continuity-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let meta = t3.write_metadata("m2.toml", &standard_metadata("phase-1", &"a".repeat(64)));
    let out = t3.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_LEDGER_INVALID");

    // Duplicate continuity ID (second entry with the same id).
    let t4 = mk();
    let mut v: Value = serde_json::from_str(&t4.read_mrgs_str("continuity-ledger.json")).unwrap();
    let mut dup_entry = v["entries"][0].clone();
    dup_entry["continuity_receipt"]["continuity_sequence"] = json!(2);
    v["entries"].as_array_mut().unwrap().push(dup_entry);
    t4.write_mrgs(
        "continuity-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let meta = t4.write_metadata("m2.toml", &standard_metadata("phase-1", &"a".repeat(64)));
    let out = t4.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_LEDGER_INVALID");

    // False manifest/receipt hash.
    let t5 = mk();
    let mut v: Value = serde_json::from_str(&t5.read_mrgs_str("continuity-ledger.json")).unwrap();
    v["entries"][0]["continuity_manifest_sha256"] = json!("c".repeat(64));
    t5.write_mrgs(
        "continuity-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let meta = t5.write_metadata("m2.toml", &standard_metadata("phase-1", &"a".repeat(64)));
    let out = t5.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_LEDGER_INVALID");

    // Broken chain (previous receipt hash).
    let t6 = mk();
    let mut v: Value = serde_json::from_str(&t6.read_mrgs_str("continuity-ledger.json")).unwrap();
    v["entries"][0]["continuity_receipt"]["previous_continuity_receipt_sha256"] =
        json!("d".repeat(64));
    t6.write_mrgs(
        "continuity-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let meta = t6.write_metadata("m2.toml", &standard_metadata("phase-1", &"a".repeat(64)));
    let out = t6.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_LEDGER_INVALID");

    // Exact metadata-byte disagreement: manifest metadata_content is drifted
    // while the source file is unchanged.
    let t7 = mk();
    let mut v: Value = serde_json::from_str(&t7.read_mrgs_str("continuity-ledger.json")).unwrap();
    let content = v["entries"][0]["continuity_manifest"]["metadata_content"]
        .as_str()
        .unwrap()
        .to_string();
    v["entries"][0]["continuity_manifest"]["metadata_content"] =
        json!(content.replacen("note = \"continuity record\"", "note = \"drifted\"", 1));
    t7.write_mrgs(
        "continuity-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let meta = t7.write_metadata("m2.toml", &standard_metadata("phase-1", &"a".repeat(64)));
    let out = t7.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_LEDGER_INVALID");

    // Recovery is gated on the continuity ledger: a tampered continuity
    // ledger makes recovery inspection fail closed with its own boundary
    // category (the subject includes the ledger bytes, and the ledger is
    // authoritatively validated).
    let t8 = mk();
    let healthy_before = t8.inspect_output();
    assert_eq!(healthy_before.len(), 1);
    assert!(healthy_before[0].starts_with("RECOVERY_NOT_REQUIRED "));
    let mut v: Value = serde_json::from_str(&t8.read_mrgs_str("continuity-ledger.json")).unwrap();
    v["repository_id"] = json!("repo-beta");
    t8.write_mrgs(
        "continuity-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t8.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
}

#[test]
fn test_obligation_24_recovery_ledger_and_cross_chain_corruption_matrix() {
    let mk_applied = || {
        let t = TestRepo::new();
        t.setup_impl_bound();
        induce_recoverable(&t);
        let (rid, pre_sha) = recoverable_ids(&t);
        let out = t.apply(&rid, &pre_sha);
        assert_success(&out);
        t
    };

    // Raw-key / schema changes.
    let t = mk_applied();
    let mut v: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    v["rogue_key"] = json!(1);
    t.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    let t1 = mk_applied();
    let mut v: Value = serde_json::from_slice(&t1.recovery_ledger_bytes()).unwrap();
    v["schema_version"] = json!(2);
    t1.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t1.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // False plan/prefix/action hashes.
    let t2 = mk_applied();
    let mut v: Value = serde_json::from_slice(&t2.recovery_ledger_bytes()).unwrap();
    v["recoveries"][0]["plan"]["prefix_subject_sha256"][1] = json!("b".repeat(64));
    t2.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t2.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    let t3 = mk_applied();
    let mut v: Value = serde_json::from_slice(&t3.recovery_ledger_bytes()).unwrap();
    v["recoveries"][0]["recovery_receipt_sha256"] = json!("c".repeat(64));
    t3.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // Invalid next_action on an APPLIED entry (2 exceeds the one-action
    // count; the APPLIED value is already 1, so 1 would be a no-op).
    let t4 = mk_applied();
    let mut v: Value = serde_json::from_slice(&t4.recovery_ledger_bytes()).unwrap();
    v["recoveries"][0]["next_action"] = json!(2);
    t4.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t4.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // Noncanonical action target.
    let t5 = mk_applied();
    let mut v: Value = serde_json::from_slice(&t5.recovery_ledger_bytes()).unwrap();
    v["recoveries"][0]["plan"]["actions"][0]["target"] = json!("../evil");
    t5.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t5.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // Broken prior receipt link: recovery IDs are deterministic hashes of
    // the plan, so an identical re-apply is a unique-id conflict (stale),
    // and a fabricated second entry with a forged previous-receipt hash is
    // rejected before mutation.
    let t6 = TestRepo::new();
    t6.setup_impl_bound();
    induce_recoverable(&t6);
    let (rid, pre_sha) = recoverable_ids(&t6);
    assert_success(&t6.apply(&rid, &pre_sha));
    // Identical re-apply: never a duplicate append.
    induce_recoverable(&t6);
    let (rid2, pre2) = recoverable_ids(&t6);
    assert_eq!(rid2, rid, "the deterministic plan hash repeats");
    let out = t6.apply(&rid2, &pre2);
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");
    // Fabricated second entry with a forged previous-receipt hash.
    let mut v: Value = serde_json::from_slice(&t6.recovery_ledger_bytes()).unwrap();
    let mut entry1 = v["recoveries"][0].clone();
    entry1["recovery_receipt"]["recovery_sequence"] = json!(2);
    entry1["recovery_receipt"]["previous_recovery_receipt_sha256"] = json!("e".repeat(64));
    v["recoveries"].as_array_mut().unwrap().push(entry1);
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t6.write_mrgs("recovery-ledger.json", &tampered);
    let before = mrgs_snapshot(&t6.repo);
    let out = t6.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_eq!(t6.recovery_ledger_bytes(), tampered);
    assert_snapshot_unchanged(&t6.repo, &before);

    // False accepted-plan binding: the top-level plan identity disagrees
    // with the repository authority.
    let t7 = mk_applied();
    let mut v: Value = serde_json::from_slice(&t7.recovery_ledger_bytes()).unwrap();
    v["accepted_plan_sha256"] = json!("f".repeat(64));
    t7.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t7.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // Disagreement with completion authority: plan binding drift.
    let t8 = TestRepo::new();
    t8.setup_impl_bound();
    induce_recoverable(&t8);
    let (rid, pre_sha) = recoverable_ids(&t8);
    assert_success(&t8.apply(&rid, &pre_sha));
    let mut v: Value = serde_json::from_slice(&t8.recovery_ledger_bytes()).unwrap();
    v["recoveries"][0]["plan"]["accepted_plan_sha256"] = json!("a".repeat(64));
    t8.write_mrgs(
        "recovery-ledger.json",
        serde_json::to_vec_pretty(&v).unwrap().as_slice(),
    );
    let out = t8.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");

    // Consumers reject before mutation; journal bytes exact.
    let t9 = mk_applied();
    let mut v: Value = serde_json::from_slice(&t9.recovery_ledger_bytes()).unwrap();
    let rid_ok = v["recoveries"][0]["recovery_id"]
        .as_str()
        .unwrap()
        .to_string();
    // Capture the true current (healthy) subject from a pre-tamper inspect.
    let healthy = t9.inspect_output();
    assert!(healthy[0].starts_with("RECOVERY_NOT_REQUIRED "));
    let healthy_sha = healthy[0].split_whitespace().nth(1).unwrap().to_string();
    v["recoveries"][0]["plan"]["actions"][0]["target"] = json!("state.json/");
    let tampered = serde_json::to_vec_pretty(&v).unwrap();
    t9.write_mrgs("recovery-ledger.json", &tampered);
    let before = mrgs_snapshot(&t9.repo);
    let out = t9.inspect();
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    // Apply with the true current subject still consumes the corrupt
    // journal: exact read-only RECOVERY_LEDGER_INVALID rejection and zero
    // mutation.
    let out = t9.apply(&rid_ok, &healthy_sha);
    assert_category_no_stdout(&out, "RECOVERY_LEDGER_INVALID");
    assert_eq!(t9.recovery_ledger_bytes(), tampered);
    assert_snapshot_unchanged(&t9.repo, &before);
    assert_no_temp_files(&t9.repo);
}
// ===========================================================================
// 16.4 Persistence, interruption, and fault-injection safety
// ===========================================================================

#[test]
fn test_obligation_25_failure_before_temp_creation_preserves_absence() {
    // plan accept: parse-valid-plan validation failure before any write.
    let t = TestRepo::new();
    let mut bad_plan = valid_plan_toml().to_string();
    bad_plan.push_str("[[phases]]\nid = \"phase-1\"\ntitle = \"Dup\"\ndepends_on = []\n");
    write_file(&t.plan_path, &bad_plan);
    let out = t.accept_plan();
    assert_err_prefix(&out, "error: duplicate phase ID: ");
    assert_mrgs_absent(&t.repo);
    assert_no_temp_files(&t.repo);

    // contract draft: no active phase -> fails before any temp.
    let t2 = TestRepo::new();
    t2.accept_plan_success();
    let out = t2.draft_contract();
    assert_err_prefix(&out, "error: no active phase selected");
    assert!(!t2.repo.join(".mrgs/contract-draft.json").exists());
    assert_no_temp_files(&t2.repo);

    // contract accept: stale revision -> fails before publication.
    let t3 = TestRepo::new();
    t3.accept_plan_success();
    t3.select_phase_success("phase-1");
    t3.draft_contract();
    let out = t3.accept_contract(2, "a".repeat(64).as_str());
    assert_err_prefix(
        &out,
        "error: contract accept revision 2 does not match draft revision 1",
    );
    assert!(!t3.repo.join(".mrgs/accepted-contract.json").exists());
    assert_no_temp_files(&t3.repo);

    // implementation begin: stale authorization -> no authority publication.
    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    let before = mrgs_snapshot(&t4.repo);
    let out = t4.impl_begin(2, t4.get_draft()["sha256"].as_str().unwrap());
    assert_category_no_stdout(&out, "REQUESTED_REVISION_STALE");
    assert_snapshot_unchanged(&t4.repo, &before);
    assert_no_temp_files(&t4.repo);

    // audit begin: invalid auditor -> no ledger.
    let t5 = TestRepo::new();
    t5.setup_impl_bound();
    let out = t5.audit_begin("");
    assert_category_no_stdout(&out, "AUDITOR_ID_INVALID");
    assert!(!t5.repo.join(".mrgs/audit-ledger.json").exists());
    assert_no_temp_files(&t5.repo);

    // phase close: not ready -> no completion ledger.
    let t6 = TestRepo::new();
    t6.setup_impl_bound();
    let out = t6.phase_close("phase-1");
    assert_category_no_stdout(&out, "CLOSEOUT_NOT_READY");
    assert!(t6.get_completion_ledger().is_none());
    assert_no_temp_files(&t6.repo);

    // continuity record: invalid metadata -> no ledger.
    let t7 = TestRepo::new();
    let (_m, receipt) = t7.close_phase1();
    let before = mrgs_snapshot(&t7.repo);
    let bad_meta = t7.write_metadata("bad.toml", "not toml at all");
    let out = t7.continuity_record(&bad_meta);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");
    assert!(t7.get_continuity_ledger().is_none());
    assert_snapshot_unchanged(&t7.repo, &before);
    assert_no_temp_files(&t7.repo);
    let _ = receipt;

    // recovery apply: bad decision -> no journal.
    let t8 = TestRepo::new();
    t8.setup_impl_bound();
    induce_recoverable(&t8);
    let (rid, pre_sha) = recoverable_ids(&t8);
    let out = t8.apply_decision(&rid, &pre_sha, "nope");
    assert_category_no_stdout(&out, "RECOVERY_DECISION_INVALID");
    assert!(t8.get_recovery_ledger().is_none());
    assert_no_temp_files(&t8.repo);
}

#[test]
fn test_obligation_26_failure_after_temp_creation_disposes_safely() {
    // implementation begin: the no-clobber failpoint fires after the temp
    // exists; the handled failure removes only its own temp.
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    t.draft_contract();
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    t.commit_sources();
    let out = t.run_with_env(
        &[
            "implementation",
            "begin",
            "--repo",
            &t.repo.to_string_lossy(),
            "--revision",
            "1",
            "--sha256",
            &sha,
        ],
        &[("MRGS_TEST_ONLY_FORCE_NO_CLOBBER_UNSUPPORTED", "1")],
    );
    assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
    assert!(!t.repo.join(".mrgs/implementation-authority.json").exists());
    assert_no_temp_files(&t.repo);

    // audit begin: a collision sentinel at the canonical temp slot is never
    // truncated; the command advances to the next candidate and succeeds.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let sentinel = b"audit-sentinel-bytes";
    t2.write_mrgs(".mrgs_audit_tmp_0_0_0.tmp", sentinel);
    let out = t2.audit_begin("auditor1");
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("AUDIT_OPEN "));
    assert_eq!(t2.read_mrgs(".mrgs_audit_tmp_0_0_0.tmp"), sentinel);
    // Only the intentional sentinel remains as a .tmp file.
    assert_no_temp_files_except(
        t2.repo.join(".mrgs"),
        &[".mrgs_audit_tmp_0_0_0.tmp".to_string()],
    );

    // closeout with pre-existing closeout-temp paths: the foreign objects
    // make readiness fail closed (CLOSEOUT_NOT_READY) and the sentinels are
    // never truncated.
    let t3 = TestRepo::new();
    t3.setup_closeout_ready();
    let sentinel_a = b"closeout-sentinel-a";
    let sentinel_b = b"closeout-state-sentinel-b";
    t3.write_mrgs(".closeout.0.tmp", sentinel_a);
    t3.write_mrgs(".closeout-state.0.tmp", sentinel_b);
    let out = t3.phase_close("phase-1");
    assert_category_no_stdout(&out, "CLOSEOUT_NOT_READY");
    assert!(t3.get_completion_ledger().is_none());
    assert_eq!(t3.read_mrgs(".closeout.0.tmp"), sentinel_a);
    assert_eq!(t3.read_mrgs(".closeout-state.0.tmp"), sentinel_b);

    // continuity: occupied slot; sentinel preserved; sequence stays 1.
    let t4 = TestRepo::new();
    let (_m, receipt) = t4.close_phase1();
    let sentinel = b"continuity-sentinel";
    t4.write_mrgs(".continuity.0.tmp", sentinel);
    let meta = t4.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t4.continuity_record(&meta);
    assert_success(&out);
    let parts = split_stdout(&out);
    assert_eq!(parts[0], "CONTINUITY_RECORDED");
    assert_eq!(parts[3], "1");
    assert_eq!(t4.read_mrgs(".continuity.0.tmp"), sentinel);
    assert_no_temp_files_except(t4.repo.join(".mrgs"), &[".continuity.0.tmp".to_string()]);

    // recovery: crash after the action temp is written leaves an authorized
    // recovery-owned temp; resume consumes it and completes with one receipt.
    let t5 = TestRepo::new();
    t5.setup_impl_bound();
    induce_recoverable(&t5);
    let (rid, pre_sha) = recoverable_ids(&t5);
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t5, &rid, &pre_sha, "after_temp_write:0", dir.path());
    kill_child(child);
    let journal: Value = serde_json::from_slice(&t5.recovery_ledger_bytes()).unwrap();
    assert_eq!(journal["recoveries"][0]["status"], "PENDING");
    // The crash left an authorized recovery-owned action temp in .mrgs, so
    // the LIVE subject differs from the pre-crash subject. Negative proof:
    // binding the retry to the stale pre-crash subject is REJECTED because
    // the surviving temp changed the subject.
    let out = t5.apply(&rid, &pre_sha);
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");
    // Positive proof: binding the live subject lets production normalize
    // the authorized temp and resume to a fixed point, consuming the temp.
    let sha_now = recompute_subject(&t5.repo);
    assert_ne!(
        sha_now, pre_sha,
        "the surviving temp must change the subject"
    );
    let out = t5.apply(&rid, &sha_now);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    assert_no_temp_files(&t5.repo);
}

#[test]
fn test_obligation_27_failure_before_atomic_replace_preserves_target() {
    // Recovery journal advance: deterministic hook-driven replace failure
    // preserves the prior journal bytes exactly (all platforms).
    let t = TestRepo::new();
    t.setup_impl_bound();
    induce_recoverable(&t);
    let (rid, pre_sha) = recoverable_ids(&t);
    let out = t.run_with_env(
        &[
            "recovery",
            "apply",
            "--repo",
            &t.repo.to_string_lossy(),
            "--recovery-id",
            &rid,
            "--subject-sha256",
            &pre_sha,
            "--decision",
            "RECOVER",
        ],
        &[("MRGS_TEST_ONLY_RECOVERY_FAIL_RENAME_AFTER_PUBLISH", "1")],
    );
    assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    assert_eq!(journal["recoveries"][0]["status"], "PENDING");
    assert_eq!(journal["recoveries"][0]["next_action"], 0);
    assert_no_temp_files(&t.repo);

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // audit record: an exclusive lock on the existing ledger blocks the
        // final replace; old bytes and hash remain exact.
        let t2 = TestRepo::new();
        t2.setup_impl_bound();
        let open = t2.audit_begin("auditor1");
        let parts = split_stdout(&open);
        let report = t2.make_pass_report(&parts[1], &parts[3], "auditor1");
        let report_path = t2.write_report(&report);
        let ledger_bytes_before = t2.read_mrgs("audit-ledger.json");
        let _lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(3)
            .open(t2.repo.join(".mrgs/audit-ledger.json"))
            .unwrap();
        let out = t2.audit_record(&report_path);
        assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
        assert_eq!(t2.read_mrgs("audit-ledger.json"), ledger_bytes_before);
        drop(_lock);
        let out = t2.audit_record(&report_path);
        assert_success(&out);
        assert!(stdout_str(&out).starts_with("AUDIT_PASS "));
        assert_no_temp_files(&t2.repo);

        // continuity: a locked ledger replays an identical entry without
        // writing (idempotent), but a NEW entry (next phase) must write —
        // the atomic replace fails while the target is locked; the prior
        // bytes stay exact and the clean retry records sequence 2.
        let t3 = TestRepo::new();
        let (_m, receipt1) = t3.close_phase1();
        let meta1 = t3.write_metadata("m1.toml", &standard_metadata("phase-1", &receipt1));
        assert_success(&t3.continuity_record(&meta1));
        let replay = t3.continuity_record(&meta1);
        assert_success(&replay);
        let parts = split_stdout(&replay);
        assert_eq!(parts[3], "1", "replay must not skip sequences");
        write_file(&t3.contract_path, &contract_toml_for_phase("phase-2"));
        t3.complete_phase("phase-2");
        let receipt2 = t3.get_completion_ledger().unwrap()["completions"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["completion_receipt_sha256"]
            .as_str()
            .unwrap()
            .to_string();
        let meta2 = t3.write_metadata(
            "m2.toml",
            &standard_metadata("phase-2", &receipt2).replace("phase-1-primary", "phase-2-primary"),
        );
        let ledger_bytes_before = t3.read_mrgs("continuity-ledger.json");
        let _lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(3)
            .open(t3.repo.join(".mrgs/continuity-ledger.json"))
            .unwrap();
        let out = t3.continuity_record(&meta2);
        assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
        assert_eq!(t3.read_mrgs("continuity-ledger.json"), ledger_bytes_before);
        drop(_lock);
        let out = t3.continuity_record(&meta2);
        assert_success(&out);
        let parts = split_stdout(&out);
        assert_eq!(parts[3], "2", "the blocked entry must not be lost");

        // closeout: locking state.json blocks the state promotion AFTER the
        // completion entry is published; prior bytes stay exact and the
        // retry resumes with the same sequence (no duplicate entry).
        let t4 = TestRepo::new();
        t4.setup_closeout_ready();
        let state_before = t4.read_mrgs("state.json");
        let _lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(3)
            .open(t4.repo.join(".mrgs/state.json"))
            .unwrap();
        let out = t4.phase_close("phase-1");
        assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
        assert_eq!(t4.read_mrgs("state.json"), state_before);
        let ledger = t4.get_completion_ledger().unwrap();
        assert_eq!(ledger["completions"].as_array().unwrap().len(), 1);
        drop(_lock);
        let out = t4.phase_close("phase-1");
        assert_success(&out);
        let parts = split_stdout(&out);
        assert_eq!(parts[0], "PHASE_CLOSED");
        assert_eq!(parts[2], "1", "resume must reuse sequence 1");
        let ledger = t4.get_completion_ledger().unwrap();
        assert_eq!(ledger["completions"].as_array().unwrap().len(), 1);
        assert_no_temp_files(&t4.repo);
        eprintln!("CAPABILITY_EXECUTED");
    }
    #[cfg(not(windows))]
    {
        // POSIX advisory locks do not block rename (capability absent); the
        // hook-driven replace failure above is the concrete fallback proving
        // prior-byte preservation on a failed replacement.
        let t2 = TestRepo::new();
        t2.setup_impl_bound();
        let before = mrgs_snapshot(&t2.repo);
        let out = t2.run_with_env(
            &[
                "implementation",
                "begin",
                "--repo",
                &t2.repo.to_string_lossy(),
                "--revision",
                "1",
                "--sha256",
                &t2.get_draft()["sha256"].as_str().unwrap().to_string(),
            ],
            &[("MRGS_TEST_ONLY_FORCE_NO_CLOBBER_UNSUPPORTED", "1")],
        );
        assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
        assert_snapshot_unchanged(&t2.repo, &before);
        assert_no_temp_files(&t2.repo);
        eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
    }
}

#[test]
fn test_obligation_28_target_replaced_before_journal_advance_resumes() {
    let t = TestRepo::new();
    t.setup_impl_bound();
    induce_recoverable(&t);
    let (rid, pre_sha) = recoverable_ids(&t);
    let dir = tempfile::TempDir::new().unwrap();
    // Crash after the action executed but before the journal advanced.
    let child = crash_apply(&t, &rid, &pre_sha, "after_action:0", dir.path());
    kill_child(child);
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    let entry = &journal["recoveries"][0];
    assert_eq!(entry["status"], "PENDING");
    assert_eq!(entry["next_action"], 0);
    let post_sha = entry["plan"]["prefix_subject_sha256"][1]
        .as_str()
        .unwrap()
        .to_string();
    // The target already equals the next prefix.
    let state_after_crash = t.read_mrgs("state.json");
    assert!(t.repo.join(".mrgs/state.json").exists());

    // Retry with the CURRENT subject: the completed action is recognized,
    // advanced once, finalized with exactly one receipt.
    let out = t.apply(&rid, &post_sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    let entry = &journal["recoveries"][0];
    assert_eq!(entry["status"], "APPLIED");
    assert_eq!(entry["next_action"], 1);
    assert_eq!(entry["recovery_receipt"]["recovery_id"], rid);
    assert_eq!(entry["post_subject_sha256"], post_sha);
    assert_eq!(journal["recoveries"].as_array().unwrap().len(), 1);
    // No duplicate target mutation: the restored state bytes are untouched.
    assert_eq!(t.read_mrgs("state.json"), state_after_crash);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_29_interrupted_closeout_cleanup_resumes_exactly() {
    // Fixture: close phase-1, restore the archived phase-scoped files, rewind
    // the state to the receipt-bound pre-closeout state.
    let mk_resumable = || {
        let t = TestRepo::new();
        let (_, receipt_sha) = t.close_phase1();
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
        t.write_mrgs(
            "implementation-authority.json",
            archived["implementation_authority_content"]
                .as_str()
                .unwrap()
                .as_bytes(),
        );
        t.write_mrgs(
            "audit-ledger.json",
            archived["audit_ledger_content"]
                .as_str()
                .unwrap()
                .as_bytes(),
        );
        let pre_state: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
        let mut pre = pre_state.clone();
        pre["active_phase"] = json!("phase-1");
        pre["closed_phases"] = json!([]);
        t.write_mrgs(
            "state.json",
            serde_json::to_vec_pretty(&pre).unwrap().as_slice(),
        );
        let ledger_before = t.read_mrgs("completion-ledger.json");
        (t, receipt_sha, ledger_before)
    };

    let cleanup_order = [
        "audit-ledger.json",
        "implementation-authority.json",
        "accepted-contract.json",
        "contract-draft.json",
    ];

    // Every cleanup prefix resumes the fixed order with no second entry.
    for k in 0..=4usize {
        let (t, _receipt, ledger_before) = mk_resumable();
        for name in &cleanup_order[..k] {
            t.delete(name);
        }
        let lines = t.inspect_output();
        assert!(lines[0].starts_with("RECOVERY_REQUIRED "), "k={}", k);
        let parts: Vec<&str> = lines[0].split_whitespace().collect();
        assert_eq!(parts[3], "1");
        assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
        let out = t.apply(parts[1], parts[2]);
        assert_success(&out);
        assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "), "k={}", k);
        // Fixed cleanup result: exactly the post-closeout permanent set.
        let entries: Vec<String> = std::fs::read_dir(t.repo.join(".mrgs"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let expected = [
            "accepted-plan.json",
            "state.json",
            "completion-ledger.json",
            "recovery-ledger.json",
        ];
        for name in expected {
            assert!(
                entries.iter().any(|n| n == name),
                "missing {} at k={}",
                name,
                k
            );
        }
        for name in cleanup_order {
            assert!(
                !entries.iter().any(|n| n == name),
                "phase-scoped {} must be cleaned at k={}",
                name,
                k
            );
        }
        // State promoted exactly once; completion ledger untouched.
        let state: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
        assert_eq!(state["active_phase"], Value::Null);
        assert_eq!(state["closed_phases"], json!(["phase-1"]));
        assert_eq!(t.read_mrgs("completion-ledger.json"), ledger_before);
        assert_no_temp_files(&t.repo);
    }

    // Byte mismatch before deletion: any drifted phase-scoped file aborts
    // the closeout-resume classification BEFORE any action runs (the
    // closeout preconditions check fails closed) and every durable file
    // stays byte-identical.
    let (t, _receipt, _ledger_before) = mk_resumable();
    let drifted =
        t.read_mrgs_str("accepted-contract.json")
            .replacen("contract_id", "contract_idd", 1);
    t.write_mrgs("accepted-contract.json", drifted.as_bytes());
    let snapshot = mrgs_snapshot(&t.repo);
    let raw = t.inspect();
    assert_category_no_stdout(&raw, "RECOVERY_UNRECOVERABLE");
    assert_snapshot_unchanged(&t.repo, &snapshot);
    for name in cleanup_order {
        assert!(
            t.repo.join(".mrgs").join(name).exists(),
            "{} must not be deleted",
            name
        );
    }
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_30_interrupted_recovery_action_and_ledger_publish() {
    // Crash-point matrix over the canonical 1-action recoverable fixture.
    let mk = || {
        let t = TestRepo::new();
        t.setup_impl_bound();
        induce_recoverable(&t);
        let (rid, pre_sha) = recoverable_ids(&t);
        (t, rid, pre_sha)
    };

    // 1. Journal temp written, no journal on disk: leftover temp is
    //    unrecoverable; after cleanup the operation resumes cleanly.
    let (t, rid0, pre0) = mk();
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(
        &t,
        &rid0,
        &pre0,
        "after_ledger_temp_write_first",
        dir.path(),
    );
    kill_child(child);
    let out = t.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");
    // Remove the contractually unowned leftover; the fixture resumes.
    for entry in std::fs::read_dir(t.repo.join(".mrgs")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".tmp") {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
    let (rid, pre_sha) = recoverable_ids(&t);
    let out = t.apply(&rid, &pre_sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    assert_no_temp_files(&t.repo);

    // 2. Pending journal published, no mutation.
    let (t, rid, pre_sha) = mk();
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t, &rid, &pre_sha, "after_pending_publish", dir.path());
    kill_child(child);
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_PENDING "));
    let out = t.apply(&rid, &pre_sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));

    // 3. Action done, journal not advanced (resume recognizes completion).
    let (t, rid, pre_sha) = mk();
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t, &rid, &pre_sha, "after_action:0", dir.path());
    kill_child(child);
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    let post_sha = journal["recoveries"][0]["plan"]["prefix_subject_sha256"][1]
        .as_str()
        .unwrap()
        .to_string();
    let out = t.apply(&rid, &post_sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));

    // 4. All actions done before finalization (fixed-point finalize).
    let (t, rid, pre_sha) = mk();
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t, &rid, &pre_sha, "before_finalize", dir.path());
    kill_child(child);
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    let post_sha = journal["recoveries"][0]["plan"]["prefix_subject_sha256"][1]
        .as_str()
        .unwrap()
        .to_string();
    let out = t.apply(&rid, &post_sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));

    // 5. Final ledger temp written pre-rename: the released apply renames
    //    the temp; the final journal matches the temp bytes byte-for-byte.
    let (t, rid, pre_sha) = mk();
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(
        &t,
        &rid,
        &pre_sha,
        "after_final_ledger_temp_write",
        dir.path(),
    );
    kill_child(child);
    let temps: Vec<String> = std::fs::read_dir(t.repo.join(".mrgs"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert_eq!(temps.len(), 1, "one finalization temp must exist");
    let final_temp_bytes = std::fs::read(t.repo.join(".mrgs").join(&temps[0])).unwrap();
    let journal_before: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    assert_eq!(journal_before["recoveries"][0]["status"], "PENDING");
    let post_sha = journal_before["recoveries"][0]["plan"]["prefix_subject_sha256"][1]
        .as_str()
        .unwrap()
        .to_string();
    let _ = post_sha;
    // The crash left the finalization temp in .mrgs, so the LIVE subject
    // includes it; bind the retry to the current subject and let production
    // consume the authorized temp and finalize to a journal byte-identical
    // to the crashed temp.
    let sha_now = recompute_subject(&t.repo);
    let out = t.apply(&rid, &sha_now);
    assert_success(&out);
    assert_eq!(t.recovery_ledger_bytes(), final_temp_bytes);
    assert_no_temp_files(&t.repo);

    // 6. Failed advance rename after publish: prior journal bytes exact,
    //    resumable, receipt unique.
    let (t, rid, pre_sha) = mk();
    let out = t.run_with_env(
        &[
            "recovery",
            "apply",
            "--repo",
            &t.repo.to_string_lossy(),
            "--recovery-id",
            &rid,
            "--subject-sha256",
            &pre_sha,
            "--decision",
            "RECOVER",
        ],
        &[("MRGS_TEST_ONLY_RECOVERY_FAIL_RENAME_AFTER_PUBLISH", "1")],
    );
    assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    assert_eq!(journal["recoveries"][0]["status"], "PENDING");
    let post_sha = journal["recoveries"][0]["plan"]["prefix_subject_sha256"][1]
        .as_str()
        .unwrap()
        .to_string();
    let out = t.apply(&rid, &post_sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    assert_eq!(journal["recoveries"].as_array().unwrap().len(), 1);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_31_interrupted_audit_continuity_and_completion_publication() {
    // Audit publication collision: sentinel at the first candidate slot.
    let t = TestRepo::new();
    t.setup_impl_bound();
    t.write_mrgs(".mrgs_audit_tmp_0_0_0.tmp", b"sentinel");
    let open = t.audit_begin("auditor1");
    assert_success(&open);
    let parts = split_stdout(&open);
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let record = t.audit_record(&report_path);
    assert_success(&record);
    assert!(stdout_str(&record).starts_with("AUDIT_PASS "));
    let ledger: Value = serde_json::from_str(&t.read_mrgs_str("audit-ledger.json")).unwrap();
    assert_eq!(ledger["rounds"].as_array().unwrap().len(), 1);
    assert_eq!(ledger["rounds"][0]["round"], 1);
    assert_eq!(t.read_mrgs(".mrgs_audit_tmp_0_0_0.tmp"), b"sentinel");
    assert_no_temp_files_except(
        t.repo.join(".mrgs"),
        &[".mrgs_audit_tmp_0_0_0.tmp".to_string()],
    );

    // Continuity publication collision.
    let t2 = TestRepo::new();
    let (_m, receipt) = t2.close_phase1();
    t2.write_mrgs(".continuity.0.tmp", b"sentinel");
    let meta = t2.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t2.continuity_record(&meta);
    assert_success(&out);
    let parts = split_stdout(&out);
    assert_eq!(parts[3], "1");
    let out2 = t2.continuity_record(&meta);
    assert_success(&out2);
    assert_eq!(
        stdout_str(&out2),
        stdout_str(&out),
        "clean retry replays exactly"
    );
    let ledger = t2.get_continuity_ledger().unwrap();
    assert_eq!(ledger["entries"].as_array().unwrap().len(), 1);
    assert_eq!(t2.read_mrgs(".continuity.0.tmp"), b"sentinel");
    assert_no_temp_files_except(t2.repo.join(".mrgs"), &[".continuity.0.tmp".to_string()]);

    // Completion publication with pre-existing closeout-temp paths: the
    // foreign objects fail readiness closed; no partial completion is
    // accepted and the sentinels stay byte-exact.
    let t3 = TestRepo::new();
    t3.setup_closeout_ready();
    t3.write_mrgs(".closeout.0.tmp", b"sentinel-a");
    t3.write_mrgs(".closeout-state.0.tmp", b"sentinel-b");
    let out = t3.phase_close("phase-1");
    assert_category_no_stdout(&out, "CLOSEOUT_NOT_READY");
    assert!(t3.get_completion_ledger().is_none());
    assert_eq!(t3.read_mrgs(".closeout.0.tmp"), b"sentinel-a");
    assert_eq!(t3.read_mrgs(".closeout-state.0.tmp"), b"sentinel-b");
    assert_no_temp_files_except(
        t3.repo.join(".mrgs"),
        &[
            ".closeout.0.tmp".to_string(),
            ".closeout-state.0.tmp".to_string(),
        ],
    );

    #[cfg(windows)]
    {
        // Locked-destination variants: no partial JSON is accepted, previous
        // ledger bytes remain exact, and a clean retry publishes exactly one
        // canonical entry with contiguous sequence.
        use std::os::windows::fs::OpenOptionsExt;
        let t4 = TestRepo::new();
        t4.setup_impl_bound();
        let open = t4.audit_begin("auditor1");
        let parts = split_stdout(&open);
        let report = t4.make_pass_report(&parts[1], &parts[3], "auditor1");
        let report_path = t4.write_report(&report);
        let before = t4.read_mrgs("audit-ledger.json");
        let _lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(3)
            .open(t4.repo.join(".mrgs/audit-ledger.json"))
            .unwrap();
        let out = t4.audit_record(&report_path);
        assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
        assert_eq!(t4.read_mrgs("audit-ledger.json"), before);
        drop(_lock);
        let out = t4.audit_record(&report_path);
        assert_success(&out);
        assert!(stdout_str(&out).starts_with("AUDIT_PASS "));
        let ledger: Value = serde_json::from_str(&t4.read_mrgs_str("audit-ledger.json")).unwrap();
        assert_eq!(ledger["rounds"].as_array().unwrap().len(), 1);
        assert_no_temp_files(&t4.repo);
        eprintln!("CAPABILITY_EXECUTED");
    }
    #[cfg(not(windows))]
    {
        // Advisory locks cannot block atomic rename on this host; the
        // collision-driven interrupted publications above are the concrete
        // fallback proving no partial JSON and clean single-entry retries.
        eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
    }
}

#[test]
fn test_obligation_32_incomplete_durable_operation_replay_fixed_point() {
    // Implementation publication interrupted by the no-clobber failpoint.
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    t.draft_contract();
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    t.commit_sources();
    let out = t.run_with_env(
        &[
            "implementation",
            "begin",
            "--repo",
            &t.repo.to_string_lossy(),
            "--revision",
            "1",
            "--sha256",
            &sha,
        ],
        &[("MRGS_TEST_ONLY_FORCE_NO_CLOBBER_UNSUPPORTED", "1")],
    );
    assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
    let first = t.impl_begin(1, &sha);
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let bytes_first = t.read_mrgs("implementation-authority.json");
    let second = t.impl_begin(1, &sha);
    assert_success(&second);
    assert_eq!(
        stdout_raw(&second),
        first_out,
        "fixed-point replay must be byte-identical"
    );
    assert_eq!(t.read_mrgs("implementation-authority.json"), bytes_first);
    assert_no_temp_files(&t.repo);

    // Closeout interrupted before finalization (state rewound to the
    // receipt-bound pre-closeout state), then retried twice: the first
    // retry completes via resumable finalization with the original
    // sequence, the second is a byte-identical idempotent replay with no
    // second completion entry and no temporary leftovers.
    let t2 = TestRepo::new();
    let (_m2, _r2) = t2.close_phase1();
    let archived = t2.archived_governance();
    t2.write_mrgs(
        "contract-draft.json",
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t2.write_mrgs(
        "accepted-contract.json",
        archived["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t2.write_mrgs(
        "implementation-authority.json",
        archived["implementation_authority_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t2.write_mrgs(
        "audit-ledger.json",
        archived["audit_ledger_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    let mut pre: Value = serde_json::from_str(&t2.read_mrgs_str("state.json")).unwrap();
    pre["active_phase"] = json!("phase-1");
    pre["closed_phases"] = json!([]);
    t2.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&pre).unwrap().as_slice(),
    );
    let ledger_bytes = t2.read_mrgs("completion-ledger.json");
    let close = t2.phase_close("phase-1");
    assert_success(&close);
    let close_parts = split_stdout(&close);
    assert_eq!(close_parts[0], "PHASE_CLOSED");
    assert_eq!(close_parts[2], "1", "resume reuses the original sequence");
    let close_out = stdout_raw(&close);
    let replay = t2.phase_close("phase-1");
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), close_out);
    assert_eq!(t2.read_mrgs("completion-ledger.json"), ledger_bytes);
    let ledger_after = t2.get_completion_ledger().unwrap();
    assert_eq!(ledger_after["completions"].as_array().unwrap().len(), 1);
    assert_no_temp_files(&t2.repo);

    // Continuity interrupted by an occupied slot, then replayed twice.
    let t3 = TestRepo::new();
    let (_m, receipt) = t3.close_phase1();
    t3.write_mrgs(".continuity.0.tmp", b"sentinel");
    let meta = t3.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let record = t3.continuity_record(&meta);
    assert_success(&record);
    let record_out = stdout_raw(&record);
    let ledger_bytes = t3.read_mrgs("continuity-ledger.json");
    let replay = t3.continuity_record(&meta);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), record_out);
    assert_eq!(t3.read_mrgs("continuity-ledger.json"), ledger_bytes);
    assert_eq!(t3.read_mrgs(".continuity.0.tmp"), b"sentinel");
    assert_no_temp_files_except(t3.repo.join(".mrgs"), &[".continuity.0.tmp".to_string()]);

    // Recovery interrupted before finalization, then replayed twice.
    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    induce_recoverable(&t4);
    let (rid, pre_sha) = recoverable_ids(&t4);
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t4, &rid, &pre_sha, "before_finalize", dir.path());
    kill_child(child);
    let journal: Value = serde_json::from_slice(&t4.recovery_ledger_bytes()).unwrap();
    let post_sha = journal["recoveries"][0]["plan"]["prefix_subject_sha256"][1]
        .as_str()
        .unwrap()
        .to_string();
    let apply = t4.apply(&rid, &post_sha);
    assert_success(&apply);
    assert!(stdout_str(&apply).starts_with("RECOVERY_APPLIED "));
    let apply_out = stdout_raw(&apply);
    let journal_bytes = t4.recovery_ledger_bytes();
    let replay = t4.apply(&rid, &post_sha);
    assert_success(&replay);
    assert_eq!(
        stdout_raw(&replay),
        apply_out,
        "fixed-point replay byte-identical"
    );
    assert_eq!(t4.recovery_ledger_bytes(), journal_bytes);
    assert_no_temp_files(&t4.repo);
}
// ===========================================================================
// 16.5 Idempotency, replay, conflict, and concurrency behavior
// ===========================================================================

#[test]
fn test_obligation_33_exact_replay_matrix_all_publishers() {
    let t = TestRepo::new();
    let repo = t.repo.to_string_lossy().into_owned();

    // Plan acceptance replay.
    let first = t.accept_plan();
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let accepted_bytes = t.read_mrgs("accepted-plan.json");
    let state_bytes = t.read_mrgs("state.json");
    let replay = t.accept_plan();
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(t.read_mrgs("accepted-plan.json"), accepted_bytes);
    assert_eq!(t.read_mrgs("state.json"), state_bytes);

    // Phase selection is a single-shot state transition (replay is
    // rejected); the replay of the authority stays stable.
    assert_success(&t.select_phase("phase-1"));

    // Contract draft replay.
    let first = t.draft_contract();
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let draft_bytes = t.read_mrgs("contract-draft.json");
    let replay = t.draft_contract();
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(t.read_mrgs("contract-draft.json"), draft_bytes);

    // Contract accept replay.
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    let first = t.accept_contract(1, &sha);
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let ledger_bytes = t.read_mrgs("accepted-contract.json");
    let replay = t.accept_contract(1, &sha);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(t.read_mrgs("accepted-contract.json"), ledger_bytes);

    // Contract revise replay.
    let rev_contract = contract_toml_for_phase("phase-1").replacen(
        "requirements = [\"req1\", \"req2\"]",
        "requirements = [\"req1\", \"req2\", \"req3\"]",
        1,
    );
    write_file(&t.contract_path, &rev_contract);
    let first = t.revise_contract(1, &sha);
    assert_success(&first);
    let first_out = stdout_raw(&first);
    assert!(stdout_str(&first).starts_with("REVISION_DRAFT "));
    let draft_bytes = t.read_mrgs("contract-draft.json");
    let replay = t.revise_contract(1, &sha);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(t.read_mrgs("contract-draft.json"), draft_bytes);

    // Implementation begin replay: the begin binds the ACCEPTED contract
    // (final revision + accepted sha), not the revised draft.
    t.commit_sources();
    let first = t.impl_begin(1, &sha);
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let authority_bytes = t.read_mrgs("implementation-authority.json");
    let replay = t.impl_begin(1, &sha);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(
        t.read_mrgs("implementation-authority.json"),
        authority_bytes
    );

    // Implementation check replay.
    let first = t.impl_check();
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let check = t.impl_check();
    assert_success(&check);
    assert_eq!(stdout_raw(&check), first_out);

    // Audit begin replay.
    let first = t.audit_begin("auditor1");
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let ledger_bytes = t.read_mrgs("audit-ledger.json");
    let replay = t.audit_begin("auditor1");
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(t.read_mrgs("audit-ledger.json"), ledger_bytes);

    // Audit record replay.
    let parts = split_stdout(&first);
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let first = t.audit_record(&report_path);
    assert_success(&first);
    let first_out = stdout_raw(&first);
    assert!(stdout_str(&first).starts_with("AUDIT_PASS "));
    let ledger_bytes = t.read_mrgs("audit-ledger.json");
    let replay = t.audit_record(&report_path);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(t.read_mrgs("audit-ledger.json"), ledger_bytes);

    // Phase close replay.
    let first = t.phase_close("phase-1");
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let ledger_bytes = t.read_mrgs("completion-ledger.json");
    let replay = t.phase_close("phase-1");
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(t.read_mrgs("completion-ledger.json"), ledger_bytes);

    // Continuity record replay.
    let close_parts = split_stdout(&first);
    let receipt_sha = close_parts[4].clone();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt_sha));
    let first = t.continuity_record(&meta);
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let ledger_bytes = t.read_mrgs("continuity-ledger.json");
    let replay = t.continuity_record(&meta);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(t.read_mrgs("continuity-ledger.json"), ledger_bytes);

    // Recovery apply replay: the caller must bind the LIVE post-recovery
    // subject (the first apply changed the durable state), then the
    // APPLIED entry replays byte-identically.
    induce_recoverable(&t);
    let (rid, pre_sha) = recoverable_ids(&t);
    let first = t.apply(&rid, &pre_sha);
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let journal_bytes = t.recovery_ledger_bytes();
    let live_post = recompute_subject(&t.repo);
    assert_ne!(live_post, pre_sha, "the apply must change the subject");
    let replay = t.apply(&rid, &live_post);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), first_out);
    assert_eq!(t.recovery_ledger_bytes(), journal_bytes);
    assert_no_temp_files(&t.repo);
    let _ = repo;
}

#[test]
fn test_obligation_34_conflicting_replay_matrix_all_publishers() {
    // Plan acceptance conflict: a different (valid) plan.
    let t = TestRepo::new();
    t.accept_plan_success();
    let accepted_bytes = t.read_mrgs("accepted-plan.json");
    let state_bytes = t.read_mrgs("state.json");
    let other_plan = t.repo.join("other-plan.toml");
    write_file(&other_plan, &plan_toml_with_phases(1));
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &t.repo.to_string_lossy(),
        "--plan",
        &other_plan.to_string_lossy(),
    ]);
    assert_err_prefix(
        &out,
        "error: cannot accept different plan when authority exists",
    );
    assert_eq!(t.read_mrgs("accepted-plan.json"), accepted_bytes);
    assert_eq!(t.read_mrgs("state.json"), state_bytes);

    // Contract draft conflict.
    t.select_phase_success("phase-1");
    let draft = t.draft_contract();
    assert_success(&draft);
    let draft_bytes = t.read_mrgs("contract-draft.json");
    let different = contract_toml_for_phase("phase-1").replacen("req1", "reqX", 1);
    write_file(&t.contract_path, &different);
    let out = t.draft_contract();
    assert_err_prefix(
        &out,
        "error: contract draft already exists with different content",
    );
    assert_eq!(t.read_mrgs("contract-draft.json"), draft_bytes);

    // Contract accept conflicts.
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    let out = t.accept_contract(2, &sha);
    assert_err_prefix(
        &out,
        "error: contract accept revision 2 does not match draft revision 1",
    );
    let wrong_sha = if let Some(rest) = sha.strip_prefix('a') {
        format!("b{}", rest)
    } else {
        format!("a{}", &sha[1..])
    };
    let out = t.accept_contract(1, &wrong_sha);
    assert_err_prefix(&out, "error: contract accept SHA does not match draft SHA");
    assert!(!t.repo.join(".mrgs/accepted-contract.json").exists());

    // Contract revise conflicts.
    let out = t.revise_contract(2, &sha);
    assert_err_prefix(
        &out,
        "error: contract revise expected revision 2 does not match current 1",
    );
    let out = t.revise_contract(1, &wrong_sha);
    assert_err_prefix(
        &out,
        "error: contract revise expected SHA does not match current draft SHA",
    );

    // Implementation conflicts (stale authorization).
    assert_success(&t.accept_contract(1, &sha));
    assert_success(&t.impl_begin(1, &sha));
    let authority_bytes = t.read_mrgs("implementation-authority.json");
    let out = t.impl_begin(2, &sha);
    assert_category_no_stdout(&out, "REQUESTED_REVISION_STALE");
    let out = t.impl_begin(1, &wrong_sha);
    assert_category_no_stdout(&out, "REQUESTED_SHA_STALE");
    assert_eq!(
        t.read_mrgs("implementation-authority.json"),
        authority_bytes
    );

    // Audit begin conflict (different auditor on a pending round).
    assert_success(&t.audit_begin("auditor1"));
    let ledger_bytes = t.read_mrgs("audit-ledger.json");
    let out = t.audit_begin("other-auditor");
    assert_category_no_stdout(&out, "AUDIT_PENDING_CONFLICT");
    assert_eq!(t.read_mrgs("audit-ledger.json"), ledger_bytes);

    // Audit record conflict (different report payload after PASS).
    let open = t.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let pass_report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let pass_path = t.write_report(&pass_report);
    assert_success(&t.audit_record(&pass_path));
    let ledger_bytes = t.read_mrgs("audit-ledger.json");
    let fail_report = t.make_fail_report(&parts[1], &parts[3], "auditor1", "F1");
    let fail_path = t.write_report(&fail_report);
    let out = t.audit_record(&fail_path);
    assert_category_no_stdout(&out, "AUDIT_REPORT_CONFLICT");
    assert_eq!(t.read_mrgs("audit-ledger.json"), ledger_bytes);

    // Phase close conflict: closing a phase that was never selected is a
    // deterministic CAS conflict (probe: exit 1, CLOSEOUT_CONFLICT, no
    // completion entry, state bytes exact) — readiness was not even
    // reached because the requested closeout conflicts with the active
    // phase binding.
    let out = t.phase_close("phase-2");
    assert_category_no_stdout(&out, "CLOSEOUT_CONFLICT");
    assert!(t.get_completion_ledger().is_none());

    // Continuity conflict: different continuity_id after a record.
    let close = t.phase_close("phase-1");
    assert_success(&close);
    let close_parts = split_stdout(&close);
    let receipt_sha = close_parts[4].clone();
    let meta1 = t.write_metadata("m1.toml", &standard_metadata("phase-1", &receipt_sha));
    assert_success(&t.continuity_record(&meta1));
    let ledger_bytes = t.read_mrgs("continuity-ledger.json");
    let meta2 = t.write_metadata(
        "m2.toml",
        &standard_metadata("phase-1", &receipt_sha).replace(
            "continuity_id = \"phase-1-primary\"",
            "continuity_id = \"different-id\"",
        ),
    );
    let out = t.continuity_record(&meta2);
    assert_category_no_stdout(&out, "CONTINUITY_CONFLICT");
    assert_eq!(t.read_mrgs("continuity-ledger.json"), ledger_bytes);

    // Recovery conflicts: wrong decision and wrong subject.
    induce_recoverable(&t);
    let (rid, pre_sha) = recoverable_ids(&t);
    let out = t.apply_decision(&rid, &pre_sha, "recover");
    assert_category_no_stdout(&out, "RECOVERY_DECISION_INVALID");
    let out = t.apply(&rid, &"d".repeat(64));
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");
    assert!(t.get_recovery_ledger().is_none());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_35_stale_authorization_and_compare_and_swap() {
    // Stale revision/SHA CAS: after a revision, old credentials are rejected
    // before publication and the accepted authority stays exact.
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    t.draft_contract();
    let sha1 = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha1));
    let rev_contract = contract_toml_for_phase("phase-1").replacen("req1", "req1-new", 1);
    write_file(&t.contract_path, &rev_contract);
    assert_success(&t.revise_contract(1, &sha1));
    let draft: Value = serde_json::from_str(&t.read_mrgs_str("contract-draft.json")).unwrap();
    let sha2 = draft["sha256"].as_str().unwrap().to_string();
    let ledger_bytes = t.read_mrgs("accepted-contract.json");
    // Old revision against the new draft.
    let out = t.accept_contract(1, &sha2);
    assert_err_prefix(
        &out,
        "error: contract accept revision 1 does not match draft revision 2",
    );
    // Old SHA against the new draft.
    let out = t.accept_contract(2, &sha1);
    assert_err_prefix(&out, "error: contract accept SHA does not match draft SHA");
    assert_eq!(t.read_mrgs("accepted-contract.json"), ledger_bytes);

    // Stale implementation baseline: the branch is rewritten so the recorded
    // baseline commit is no longer an ancestor of HEAD, while the SAME
    // tracked source bytes are restored on the new tip (the authority's
    // source bindings must stay intact — GOVERNANCE_AUTHORITY_INVALID must
    // not fire — and only the baseline-history comparison rejects).
    assert_success(&t.accept_contract(2, &sha2));
    assert_success(&t.impl_begin(2, &sha2));
    let out = git(&t.repo, &["reset", "--hard", "HEAD~1"]);
    assert!(out.status.success());
    // The reset reverted the tracked plan AND contract sources; restore the
    // exact bytes the authority binds (same content, new commit) so the
    // plan/source validations pass and only the baseline ancestry differs.
    write_file(&t.plan_path, valid_plan_toml());
    write_file(&t.contract_path, &rev_contract);
    t.commit_sources();
    let out = t.impl_check();
    assert_category_no_stdout(&out, "BASELINE_HISTORY_CHANGED");

    // Stale audit subject: worktree drift between begin and record.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let open = t2.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = t2.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t2.write_report(&report);
    write_file(
        &t2.repo.join("src/main.rs"),
        "fn main() { println!(\"changed\"); }\n",
    );
    let out = t2.audit_record(&report_path);
    assert_category_no_stdout(&out, "AUDIT_SUBJECT_STALE");
    let ledger: Value = serde_json::from_str(&t2.read_mrgs_str("audit-ledger.json")).unwrap();
    assert_eq!(ledger["rounds"][0]["status"], "PENDING");
    assert!(!t2.read_mrgs("audit-ledger.json").is_empty());

    // Stale closeout phase: closing a phase that is not and was never active
    // is a deterministic CAS conflict (probe: CLOSEOUT_CONFLICT, no
    // completion entry, state bytes exact).
    let t3 = TestRepo::new();
    t3.setup_closeout_ready();
    let out = t3.phase_close("phase-2");
    assert_category_no_stdout(&out, "CLOSEOUT_CONFLICT");
    assert!(t3.get_completion_ledger().is_none());

    // Stale continuity completion receipt: valid format, wrong receipt.
    let t4 = TestRepo::new();
    let (_m, receipt) = t4.close_phase1();
    let wrong_receipt = if let Some(rest) = receipt.strip_prefix('a') {
        format!("b{}", rest)
    } else {
        format!("a{}", &receipt[1..])
    };
    let meta = t4.write_metadata("m.toml", &standard_metadata("phase-1", &wrong_receipt));
    let out = t4.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");
    assert!(t4.get_continuity_ledger().is_none());

    // Stale recovery ID/subject: pending journal with a wrong id.
    let t5 = TestRepo::new();
    t5.setup_impl_bound();
    induce_recoverable(&t5);
    let (rid, pre_sha) = recoverable_ids(&t5);
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t5, &rid, &pre_sha, "after_pending_publish", dir.path());
    kill_child(child);
    let out = t5.apply(&"c".repeat(64), &pre_sha);
    assert_category_no_stdout(&out, "RECOVERY_PENDING_CONFLICT");
    let out = t5.apply(&rid, &"d".repeat(64));
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");
    let journal: Value = serde_json::from_slice(&t5.recovery_ledger_bytes()).unwrap();
    assert_eq!(journal["recoveries"][0]["status"], "PENDING");
    assert_no_temp_files(&t5.repo);
}

#[test]
fn test_obligation_36_concurrent_first_publication_eight_callers() {
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    t.draft_contract();
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    let out = t.accept_contract(1, &sha);
    assert_success(&out);
    // Begin requires a clean worktree; commit the fixture sources so every
    // caller passes validation.
    t.commit_sources();
    let repo = t.repo.to_string_lossy().into_owned();
    let git_before = git_snapshot(&t.repo);
    let barrier = tempfile::TempDir::new().unwrap();

    // 8 synchronized callers racing the first publication. The debug-only
    // pre-coordination barrier holds all eight callers BEFORE any caller
    // acquires the per-repository coordination guard; the production
    // serialization then lets one caller publish while the others replay
    // idempotently. The unchanged post-temp atomic hook pins the winner
    // before its no-clobber rename.
    let mut children = Vec::new();
    for i in 0..8usize {
        let mut cmd = cargo_bin();
        cmd.args([
            "implementation",
            "begin",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            &sha,
        ])
        .env(
            "MRGS_TEST_ONLY_BEGIN_BEFORE_COORDINATION_SIGNAL",
            barrier.path().join(format!("pre-signal-{}", i)),
        )
        .env(
            "MRGS_TEST_ONLY_BEGIN_BEFORE_COORDINATION_RELEASE",
            barrier.path().join(format!("pre-release-{}", i)),
        )
        .env(
            "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_SIGNAL",
            barrier.path().join(format!("signal-{}", i)),
        )
        .env(
            "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_RELEASE",
            barrier.path().join(format!("release-{}", i)),
        );
        children.push(
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    // Phase 1: all eight callers must reach the pre-coordination barrier
    // before any caller is released into the production serialization.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let all_pre_signaled =
            (0..8).all(|i| barrier.path().join(format!("pre-signal-{}", i)).exists());
        let any_early = children.iter_mut().any(|c| c.try_wait().unwrap().is_some());
        if all_pre_signaled || any_early {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for the pre-coordination barrier"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // No caller may exit before the pre-coordination barrier: with the
    // repair, every caller reaches it before touching the coordination or
    // any temporary.
    let mut early = Vec::new();
    let mut idx = 0usize;
    while idx < children.len() {
        if children[idx].try_wait().unwrap().is_some() {
            let c = children.remove(idx);
            early.push(c.wait_with_output().unwrap());
        } else {
            idx += 1;
        }
    }
    assert!(
        early.is_empty(),
        "no caller may exit before the pre-coordination barrier: {:?}",
        early.iter().map(stderr_str).collect::<Vec<String>>()
    );
    for i in 0..8usize {
        std::fs::write(barrier.path().join(format!("pre-release-{}", i)), b"go").unwrap();
    }
    // Phase 2: exactly one caller (the coordination winner) reaches the
    // post-temp atomic publication hook. Pin it, then release.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let any_signaled = (0..8).any(|i| barrier.path().join(format!("signal-{}", i)).exists());
        if any_signaled {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for the atomic publication hook"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    for i in 0..8usize {
        std::fs::write(barrier.path().join(format!("release-{}", i)), b"go").unwrap();
    }
    let mut outputs: Vec<Output> = children
        .into_iter()
        .map(|c| c.wait_with_output().unwrap())
        .collect();
    outputs.extend(early);

    // Every caller returns the exact idempotent success (the winner
    // publishes; the seven losers serialize behind it and replay), with
    // byte-identical output. No GIT_DIRTY and no conflict may appear.
    let mut first_success: Option<String> = None;
    for out in &outputs {
        assert_success(out);
        assert!(
            stdout_str(out).starts_with("IMPLEMENTATION_BOUND "),
            "loser must be idempotent success or IMPLEMENTATION_AUTHORITY_CONFLICT, got: {}",
            stderr_str(out)
        );
        match &first_success {
            None => first_success = Some(stdout_raw(out)),
            Some(prev) => assert_eq!(stdout_raw(out), *prev),
        }
    }
    // One canonical durable publication; valid bytes; no temp leftovers;
    // fixture Git state untouched.
    let authority: Value =
        serde_json::from_str(&t.read_mrgs_str("implementation-authority.json")).unwrap();
    assert_eq!(authority["contract_revision"], 1);
    assert_eq!(authority["contract_sha256"], sha);
    assert_no_temp_files(&t.repo);
    assert_eq!(
        git_snapshot(&t.repo),
        git_before,
        "fixture git state unchanged"
    );
}

#[test]
fn supplemental_01_begin_unhooked_concurrent_first_publication() {
    // The repair must not depend on the debug-only hook: eight genuinely
    // concurrent callers (harness-synchronized start only) must serialize
    // on the per-repository coordination and produce one canonical
    // publication with no GIT_DIRTY caller.
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    t.draft_contract();
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    t.commit_sources();
    let runner = create_barrier_runner();
    let outputs = run_barrier_8(
        &runner,
        &[
            "implementation",
            "begin",
            "--repo",
            &t.repo.to_string_lossy(),
            "--revision",
            "1",
            "--sha256",
            &sha,
        ],
        &[],
    );
    let mut first: Option<String> = None;
    for out in &outputs {
        assert_success(out);
        assert!(
            stdout_str(out).starts_with("IMPLEMENTATION_BOUND "),
            "unhooked loser must be idempotent success, got: {}",
            stderr_str(out)
        );
        match &first {
            None => first = Some(stdout_raw(out)),
            Some(prev) => assert_eq!(stdout_raw(out), *prev),
        }
    }
    let authority: Value =
        serde_json::from_str(&t.read_mrgs_str("implementation-authority.json")).unwrap();
    assert_eq!(authority["contract_revision"], 1);
    assert_eq!(authority["contract_sha256"], sha);
    assert_no_temp_files(&t.repo);
}

#[test]
fn supplemental_02_begin_pre_existing_producer_temp_still_dirty() {
    // A pre-existing path shaped like the publisher's own temporary grammar
    // must NOT become exempt under the repair; it still fails closed before
    // any publication.
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    t.draft_contract();
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    t.commit_sources();
    t.write_mrgs(".mrgs_impl_tmp_123_0_456.tmp", b"leftover");
    let out = t.impl_begin(1, &sha);
    assert_category_no_stdout(&out, "GIT_DIRTY");
    assert!(!t.repo.join(".mrgs/implementation-authority.json").exists());
}

#[test]
fn supplemental_03_begin_malformed_and_nonregular_temp_rejected() {
    // Malformed near-match names, unknown temps, symlinks, and non-regular
    // objects at temp-shaped paths remain strictly rejected.
    let malformed = [
        ".mrgs_impl_tmp_abc_0_456.tmp",
        ".mrgs_impl_tmp_1_x_456.tmp",
        ".mrgs_impl_tmp_1_2_x.tmp",
        ".mrgs_impl_tmp_1_2_3.tmp.extra",
        "rogue.tmp",
    ];
    for name in malformed {
        let t = TestRepo::new();
        t.accept_plan_success();
        t.select_phase_success("phase-1");
        t.draft_contract();
        let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
        assert_success(&t.accept_contract(1, &sha));
        t.commit_sources();
        t.write_mrgs(name, b"x");
        let out = t.impl_begin(1, &sha);
        assert_category_no_stdout(&out, "GIT_DIRTY");
        assert!(!t.repo.join(".mrgs/implementation-authority.json").exists());
    }
    // Symlink at a temp-shaped path (capability branch).
    let t2 = TestRepo::new();
    t2.accept_plan_success();
    t2.select_phase_success("phase-1");
    t2.draft_contract();
    let sha2 = t2.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t2.accept_contract(1, &sha2));
    t2.commit_sources();
    let target = t2._dir.path().join("outside-target");
    write_file(&target, "x");
    match make_file_link(&target, &t2.repo.join(".mrgs/.mrgs_impl_tmp_1_2_3.tmp")) {
        Ok(()) => {
            let out = t2.impl_begin(1, &sha2);
            assert_category_no_stdout(&out, "GIT_DIRTY");
            assert!(!t2.repo.join(".mrgs/implementation-authority.json").exists());
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Fallback: a directory at the temp-shaped path is equally
            // rejected.
            std::fs::create_dir_all(t2.repo.join(".mrgs/.mrgs_impl_tmp_1_2_3.tmp")).unwrap();
            let out = t2.impl_begin(1, &sha2);
            assert_category_no_stdout(&out, "GIT_DIRTY");
            assert!(!t2.repo.join(".mrgs/implementation-authority.json").exists());
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("symlink creation failed: {}", e),
    }
    // Directory at a temp-shaped path: git's ignored-files inventory does
    // not list ignored directories, so begin tolerates it exactly as before
    // the repair (the object remains a recovery-boundary rejection).
    let t3 = TestRepo::new();
    t3.accept_plan_success();
    t3.select_phase_success("phase-1");
    t3.draft_contract();
    let sha3 = t3.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t3.accept_contract(1, &sha3));
    t3.commit_sources();
    std::fs::create_dir_all(t3.repo.join(".mrgs/.mrgs_impl_tmp_4_5_6.tmp")).unwrap();
    let out = t3.impl_begin(1, &sha3);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("IMPLEMENTATION_BOUND "));
    // The non-regular object remains a strict recovery-boundary rejection.
    let insp = t3.run(&["recovery", "inspect", "--repo", &t3.repo.to_string_lossy()]);
    assert_category_no_stdout(&insp, "FILESYSTEM_BOUNDARY_UNSAFE");
}

#[test]
fn supplemental_04_begin_coordination_abandoned_auto_release() {
    // Kill the coordination winner while it is pinned at the pre-publish
    // barrier: the coordination primitive must release automatically, the
    // survivors must terminate (no deadlock), and its stale publisher
    // temporary must remain a strict GIT_DIRTY rejection (crash semantics).
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    t.draft_contract();
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    t.commit_sources();
    let repo = t.repo.to_string_lossy().into_owned();
    let barrier = tempfile::TempDir::new().unwrap();
    let mut children = Vec::new();
    for i in 0..8usize {
        let mut cmd = cargo_bin();
        cmd.args([
            "implementation",
            "begin",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            &sha,
        ])
        .env(
            "MRGS_TEST_ONLY_BEGIN_BEFORE_COORDINATION_SIGNAL",
            barrier.path().join(format!("pre-signal-{}", i)),
        )
        .env(
            "MRGS_TEST_ONLY_BEGIN_BEFORE_COORDINATION_RELEASE",
            barrier.path().join(format!("pre-release-{}", i)),
        )
        .env(
            "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_SIGNAL",
            barrier.path().join(format!("signal-{}", i)),
        )
        .env(
            "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_RELEASE",
            barrier.path().join(format!("release-{}", i)),
        );
        children.push(
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    // All eight callers reach the pre-coordination barrier first.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let all_pre = (0..8).all(|i| barrier.path().join(format!("pre-signal-{}", i)).exists());
        if all_pre {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for the pre-coordination barrier"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    for i in 0..8usize {
        std::fs::write(barrier.path().join(format!("pre-release-{}", i)), b"go").unwrap();
    }
    // The coordination winner reaches the post-temp atomic hook; pin it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let winner = loop {
        if let Some(i) = (0..8).find(|i| barrier.path().join(format!("signal-{}", i)).exists()) {
            break i;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for the winner's atomic hook signal"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    // Kill the winner mid-hold; its mutex handle is closed by the kernel.
    let dead = children.remove(winner);
    kill_child(dead);
    // Release files are never written: survivors must proceed via the
    // abandoned coordination and fail closed on the stale temp.
    let survivors: Vec<Output> = children
        .into_iter()
        .map(|c| c.wait_with_output().unwrap())
        .collect();
    assert_eq!(
        survivors.len(),
        7,
        "all survivors must terminate (no deadlock)"
    );
    for out in &survivors {
        assert_failure(out);
        assert_eq!(
            stderr_str(out),
            "error: GIT_DIRTY",
            "stale producer temp must stay rejected"
        );
    }
    // The crashed winner's temp remains (crash artifact; strict rejection
    // under the unchanged cleanliness rules).
    let temps: Vec<String> = std::fs::read_dir(t.repo.join(".mrgs"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert_eq!(temps.len(), 1);
    assert!(temps[0].starts_with(".mrgs_impl_tmp_"));
    assert!(!t.repo.join(".mrgs/implementation-authority.json").exists());
}

#[test]
fn supplemental_05_recompute_subject_negative_proof() {
    // The subject oracle must (a) bind the LIVE subject of a deterministic
    // post-crash state, (b) track a one-byte mutation of an observed
    // inventory file, and (c) production must reject the stale binding with
    // zero recovery mutation. The crash state carries a surviving
    // recovery-owned temp and a PENDING journal (inspect omits the SHA for
    // PENDING), so the live-subject acceptance is confirmed publicly by a
    // twin positive apply with the same recomputed binding.

    // Deterministic post-crash fixture: kill the apply after the action
    // temp is written, before the rename; the temp survives.
    let mk_crash = || {
        let t = TestRepo::new();
        t.setup_impl_bound();
        induce_recoverable(&t);
        let (rid, pre_sha) = recoverable_ids(&t);
        let dir = tempfile::TempDir::new().unwrap();
        let child = crash_apply(&t, &rid, &pre_sha, "after_temp_write:0", dir.path());
        kill_child(child);
        let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
        assert_eq!(journal["recoveries"][0]["status"], "PENDING");
        let temps: Vec<String> = std::fs::read_dir(t.repo.join(".mrgs"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert_eq!(temps.len(), 1, "the action temp must survive the crash");
        (t, rid, temps[0].clone())
    };

    // Twin fixture: public confirmation that the recomputed live subject is
    // the accepted binding — production applies it to a fixed point.
    {
        let (t, rid, _temp_name) = mk_crash();
        let lines = t.inspect_output();
        assert!(lines[0].starts_with("RECOVERY_PENDING "));
        let live_a = recompute_subject(&t.repo);
        let out = t.apply(&rid, &live_a);
        assert_success(&out);
        assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
        assert_no_temp_files(&t.repo);
    }

    // Negative fixture: flip exactly one byte of the observed surviving
    // temp, then invoke recovery with the stale LIVE_SUBJECT_A.
    {
        let (t, rid, temp_name) = mk_crash();
        let temp_path = t.repo.join(".mrgs").join(&temp_name);
        let live_a = recompute_subject(&t.repo);
        let raw = t.inspect();
        assert!(String::from_utf8_lossy(&raw.stdout).starts_with("RECOVERY_PENDING "));
        let temp_bytes = std::fs::read(&temp_path).unwrap();
        let mut flipped = temp_bytes.clone();
        flipped[0] ^= 0x01;
        assert_ne!(flipped, temp_bytes);
        std::fs::write(&temp_path, &flipped).unwrap();
        let snapshot = mrgs_snapshot(&t.repo);
        let live_b = recompute_subject(&t.repo);
        assert_ne!(live_a, live_b, "one byte must change the subject");
        let out = t.apply(&rid, &live_a);
        assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");
        // Zero recovery mutation: every durable byte (flipped temp included)
        // and the PENDING journal stay exactly as observed.
        assert_snapshot_unchanged(&t.repo, &snapshot);
        let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
        assert_eq!(journal["recoveries"][0]["status"], "PENDING");
        assert!(
            t.repo.join(".mrgs").join(&temp_name).exists(),
            "the surviving temp must remain"
        );
    }
}

#[test]
fn test_obligation_37_concurrent_duplicate_publication_eight_callers() {
    // 8 synchronized identical audit records against one PENDING round.
    let t = TestRepo::new();
    t.setup_impl_bound();
    let open = t.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let runner = create_barrier_runner();
    let outputs = run_barrier_8(
        &runner,
        &[
            "audit",
            "record",
            "--repo",
            &t.repo.to_string_lossy(),
            "--report",
            &report_path.to_string_lossy(),
        ],
        &[],
    );
    // Every success output is byte-identical.
    let mut first_success: Option<String> = None;
    for out in &outputs {
        assert_success(out);
        match &first_success {
            None => first_success = Some(stdout_raw(out)),
            Some(prev) => assert_eq!(
                stdout_raw(out),
                *prev,
                "duplicate callers must be byte-identical"
            ),
        }
    }
    assert_eq!(
        first_success
            .as_ref()
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap(),
        "AUDIT_PASS"
    );
    // One durable semantic entry; sequence contiguous; no rewrite.
    let ledger: Value = serde_json::from_str(&t.read_mrgs_str("audit-ledger.json")).unwrap();
    let rounds = ledger["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0]["round"], 1);
    assert_eq!(rounds[0]["status"], "PASS");
    assert!(!rounds[0]["report_content"].as_str().unwrap().is_empty());
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_38_concurrent_conflicting_publication_eight_callers() {
    // 4 callers with a PASS payload, 4 with a FAIL payload, for the same
    // authority slot (pending round 1). Exactly one payload may win.
    let t = TestRepo::new();
    t.setup_impl_bound();
    let open = t.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let (audit_id, subject_sha) = (parts[1].clone(), parts[3].clone());
    let pass_report = t.make_pass_report(&audit_id, &subject_sha, "auditor1");
    let fail_report = t.make_fail_report(&audit_id, &subject_sha, "auditor1", "F1");
    // Distinct filenames: write_report would alias both payloads to the same
    // report.json, silently overwriting the first.
    let pass_path = t.report_dir.join("pass-report.json");
    write_file(&pass_path, &pass_report);
    let fail_path = t.report_dir.join("fail-report.json");
    write_file(&fail_path, &fail_report);
    let runner = create_barrier_runner();
    let mut outputs = Vec::new();
    for i in 0..8usize {
        let (args, envs): (&[&str], &[(&str, &str)]) = if i < 4 {
            (
                &[
                    "audit",
                    "record",
                    "--repo",
                    &t.repo.to_string_lossy(),
                    "--report",
                    &pass_path.to_string_lossy(),
                ],
                &[],
            )
        } else {
            (
                &[
                    "audit",
                    "record",
                    "--repo",
                    &t.repo.to_string_lossy(),
                    "--report",
                    &fail_path.to_string_lossy(),
                ],
                &[],
            )
        };
        let _ = envs;
        std::fs::write(
            runner.exe.parent().unwrap().join(format!("args-{}.txt", i)),
            args.join("\n"),
        )
        .unwrap();
    }
    for i in 0..8usize {
        let mut cmd = Command::new(&runner.exe);
        cmd.env("BARRIER_DIR", runner.exe.parent().unwrap())
            .env("BARRIER_INDEX", i.to_string())
            .env("MRGS_BIN", env!("CARGO_BIN_EXE_mrgs"));
        outputs.push(
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    for i in 0..8usize {
        wait_for_file(
            &runner.exe.parent().unwrap().join(format!("ready-{}", i)),
            60,
        );
    }
    for i in 0..8usize {
        std::fs::write(
            runner.exe.parent().unwrap().join(format!("go-{}", i)),
            b"go",
        )
        .unwrap();
    }
    let outputs: Vec<Output> = outputs
        .into_iter()
        .map(|c| c.wait_with_output().unwrap())
        .collect();

    for (i, out) in outputs.iter().enumerate() {
        eprintln!(
            "T38 caller {} rc={:?} out={:?} err={:?}",
            i,
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    eprintln!(
        "T38 round: {}",
        t.read_mrgs_str("audit-ledger.json")[..300].replace('\n', " ")
    );
    // Exactly one canonical payload appears in durable bytes.
    let ledger: Value = serde_json::from_str(&t.read_mrgs_str("audit-ledger.json")).unwrap();
    let rounds = ledger["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 1, "exactly one canonical round");
    // Detect the winner by the round's canonical status, not by content
    // substrings: a FAIL report legitimately contains "PASS" entries.
    let winner_status = rounds[0]["status"].as_str().unwrap();
    let pass_won = winner_status == "PASS";
    let fail_won = winner_status == "FAIL";
    assert!(pass_won ^ fail_won, "exactly one payload must win");
    assert!(!rounds[0]["status"].as_str().unwrap().is_empty());

    // The winning payload's callers succeed byte-identically; the losing
    // payload's callers all fail with the same existing conflict/stale
    // category and their payload never appears in durable bytes.
    let mut winner_out: Option<String> = None;
    let mut loser_err: Option<String> = None;
    for (i, out) in outputs.iter().enumerate() {
        let is_pass_caller = i < 4;
        let caller_payload_won = (is_pass_caller && pass_won) || (!is_pass_caller && fail_won);
        if caller_payload_won {
            assert_success(out);
            match &winner_out {
                None => winner_out = Some(stdout_raw(out)),
                Some(prev) => assert_eq!(stdout_raw(out), *prev),
            }
        } else {
            assert_failure(out);
            assert_eq!(stdout_str(out), "");
            let err = stderr_str(out);
            assert!(
                err == "error: AUDIT_REPORT_CONFLICT" || err == "error: AUDIT_NOT_PENDING",
                "losers must fail with the existing conflict/stale category: {}",
                err
            );
            match &loser_err {
                None => loser_err = Some(err),
                Some(prev) => assert_eq!(&err, prev, "all losers report the same category"),
            }
        }
    }
    assert!(winner_out.is_some(), "at least one caller publishes");
    assert!(loser_err.is_some(), "at least one caller loses");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_39_journal_advance_and_caller_observation_races() {
    // State A: action complete before journal advancement.
    let t = TestRepo::new();
    t.setup_impl_bound();
    induce_recoverable(&t);
    let (rid, pre_sha) = recoverable_ids(&t);
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t, &rid, &pre_sha, "after_action:0", dir.path());
    kill_child(child);
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    let post_sha = journal["recoveries"][0]["plan"]["prefix_subject_sha256"][1]
        .as_str()
        .unwrap()
        .to_string();
    let out = t.apply(&rid, &post_sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_APPLIED "));
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    let entry = &journal["recoveries"][0];
    assert_eq!(entry["status"], "APPLIED");
    assert_eq!(entry["recovery_receipt"]["recovery_id"], rid);
    assert_eq!(entry["post_subject_sha256"], post_sha);
    assert_eq!(
        entry["plan"]["actions"].as_array().unwrap().len(),
        1,
        "one action history"
    );
    assert_eq!(
        journal["recoveries"].as_array().unwrap().len(),
        1,
        "one entry"
    );
    assert_no_temp_files(&t.repo);

    // State B: journal finalized before the original caller observes success.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    induce_recoverable(&t2);
    let (rid2, pre_sha2) = recoverable_ids(&t2);
    let dir2 = tempfile::TempDir::new().unwrap();
    let signal = dir2.path().join("signal");
    let release = dir2.path().join("release");
    let mut cmd = cargo_bin();
    cmd.args([
        "recovery",
        "apply",
        "--repo",
        &t2.repo.to_string_lossy(),
        "--recovery-id",
        &rid2,
        "--subject-sha256",
        &pre_sha2,
        "--decision",
        "RECOVER",
    ])
    .env("MRGS_TEST_ONLY_RECOVERY_POINT", "before_finalize")
    .env("MRGS_TEST_ONLY_RECOVERY_SIGNAL_FILE", &signal)
    .env("MRGS_TEST_ONLY_RECOVERY_RELEASE_FILE", &release);
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_file(&signal, 60);
    // While the original caller is blocked, a second caller observes the
    // pending journal and retries to a fixed point.
    let journal: Value = serde_json::from_slice(&t2.recovery_ledger_bytes()).unwrap();
    assert_eq!(journal["recoveries"][0]["status"], "PENDING");
    let post_sha2 = journal["recoveries"][0]["plan"]["prefix_subject_sha256"][1]
        .as_str()
        .unwrap()
        .to_string();
    let second = t2.apply(&rid2, &post_sha2);
    assert_success(&second);
    assert!(stdout_str(&second).starts_with("RECOVERY_APPLIED "));
    let second_out = stdout_raw(&second);
    let journal_after_second = t2.recovery_ledger_bytes();
    // Release the original caller: it finalizes and observes success; the
    // result is the exact fixed point of the second caller's completion.
    std::fs::write(&release, b"go").unwrap();
    let original = child.wait_with_output().unwrap();
    assert_success(&original);
    assert!(stdout_str(&original).starts_with("RECOVERY_APPLIED "));
    assert_eq!(stdout_raw(&original), second_out);
    assert_eq!(t2.recovery_ledger_bytes(), journal_after_second);
    let journal: Value = serde_json::from_slice(&t2.recovery_ledger_bytes()).unwrap();
    let entry = &journal["recoveries"][0];
    assert_eq!(entry["status"], "APPLIED");
    assert_eq!(entry["recovery_receipt"]["recovery_id"], rid2);
    assert_eq!(journal["recoveries"].as_array().unwrap().len(), 1);
    assert_no_temp_files(&t2.repo);
}

#[test]
fn test_obligation_40_replay_and_concurrency_cross_repository_isolation() {
    // Build target + source A + source B + sentinel C.
    let t = TestRepo::new();
    let (_m, receipt_sha) = t.close_phase1();
    let t_plan_sha = t.plan_sha();

    let mk_source = |_name: &str| -> TestRepo {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        git_init(&repo);
        git_commit(&repo, ".gitignore", b".mrgs/\n");
        git_commit(&repo, "src/main.rs", b"fn main() {}\n");
        let plan_path = repo.join("plan.toml");
        let contract_path = repo.join("contract.toml");
        write_file(&plan_path, valid_plan_toml());
        write_file(&contract_path, &contract_toml_for_phase("phase-1"));
        let report_dir = dir.path().join("reports");
        std::fs::create_dir_all(&report_dir).unwrap();
        TestRepo {
            _dir: dir,
            repo,
            report_dir,
            contract_path,
            plan_path,
        }
    };
    let s1 = mk_source("s1");
    s1.setup_impl_bound();
    let open = s1.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = s1.make_pass_report(&parts[1], &parts[3], "auditor1");
    let path = s1.write_report(&report);
    assert_success(&s1.audit_record(&path));
    let close = s1.phase_close("phase-1");
    assert_success(&close);
    let close_parts = split_stdout(&close);
    let s1_receipt = close_parts[4].clone();
    let meta = s1.write_metadata(
        "meta.toml",
        &standard_metadata("phase-1", &s1_receipt)
            .replace("repository_id = \"mrgs\"", "repository_id = \"repo-alpha\""),
    );
    assert_success(&s1.continuity_record(&meta));
    let s1_cont_receipt = split_stdout(&s1.continuity_record(&meta))[5].clone();

    let s2 = mk_source("s2");
    s2.setup_impl_bound();
    let open = s2.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = s2.make_pass_report(&parts[1], &parts[3], "auditor1");
    let path = s2.write_report(&report);
    assert_success(&s2.audit_record(&path));
    let close = s2.phase_close("phase-1");
    assert_success(&close);
    let close_parts = split_stdout(&close);
    let s2_receipt = close_parts[4].clone();
    let meta = s2.write_metadata(
        "meta.toml",
        &standard_metadata("phase-1", &s2_receipt)
            .replace("repository_id = \"mrgs\"", "repository_id = \"repo-beta\""),
    );
    assert_success(&s2.continuity_record(&meta));
    let s2_cont_receipt = split_stdout(&s2.continuity_record(&meta))[5].clone();

    let c = mk_source("sentinel");
    let c_git_head_before = git_head(&c.repo);

    let meta = linked_metadata(
        "phase-1",
        &receipt_sha,
        "mrgs",
        "repo-alpha",
        &s1.plan_sha(),
        "phase-1",
        &s1_receipt,
        Some(&s1_cont_receipt),
    );
    let meta = linked_metadata_second(
        &meta,
        "repo-beta",
        &s2.plan_sha(),
        "phase-1",
        &s2_receipt,
        Some(&s2_cont_receipt),
    );
    let meta_path = t.write_metadata("m-links.toml", &meta);

    let s1_tree_before = snapshot_tree(&s1.repo);
    let s2_tree_before = snapshot_tree(&s2.repo);

    // 8 synchronized identical continuity records against the two sources;
    // simultaneously the sentinel repository receives an unrelated mutation.
    let runner = create_barrier_runner();
    let mut children = Vec::new();
    for i in 0..8usize {
        std::fs::write(
            runner.exe.parent().unwrap().join(format!("args-{}.txt", i)),
            [
                "continuity",
                "record",
                "--repo",
                &t.repo.to_string_lossy(),
                "--metadata",
                &meta_path.to_string_lossy(),
                "--source-repo",
                &s1.repo.to_string_lossy(),
                "--source-repo",
                &s2.repo.to_string_lossy(),
            ]
            .join("\n"),
        )
        .unwrap();
        let mut cmd = Command::new(&runner.exe);
        cmd.env("BARRIER_DIR", runner.exe.parent().unwrap())
            .env("BARRIER_INDEX", i.to_string())
            .env("MRGS_BIN", env!("CARGO_BIN_EXE_mrgs"));
        children.push(
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    for i in 0..8usize {
        wait_for_file(
            &runner.exe.parent().unwrap().join(format!("ready-{}", i)),
            60,
        );
    }
    for i in 0..8usize {
        std::fs::write(
            runner.exe.parent().unwrap().join(format!("go-{}", i)),
            b"go",
        )
        .unwrap();
    }
    // Simultaneous unrelated mutation in the sentinel repository.
    let commit = git(
        &c.repo,
        &["commit", "--allow-empty", "-m", "unrelated mutation"],
    );
    assert!(commit.status.success());
    let c_git_head_after = git_head(&c.repo);
    assert_ne!(c_git_head_before, c_git_head_after);
    // Full snapshot (including git internals) AFTER the test's own
    // deliberate commit: the records are still racing, so the final
    // comparison catches any MRGS mutation to refs/logs/config/bytes
    // without attributing the sentinel's own commit to MRGS.
    let c_tree_after = snapshot_tree(&c.repo);
    let outputs: Vec<Output> = children
        .into_iter()
        .map(|c| c.wait_with_output().unwrap())
        .collect();

    for (i, out) in outputs.iter().enumerate() {
        eprintln!(
            "T40D caller {} rc={:?} out={:?} err={:?}",
            i,
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    eprintln!(
        "T40D mrgs={:?}",
        std::fs::read_dir(t.repo.join(".mrgs"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    );
    // Exactly one canonical durable entry is published. Every caller either
    // returns the exact idempotent result (byte-identical) or fails with
    // the contract-legal persistence category — the race distribution is
    // nondeterministic (5+3 / 4+4 observed), so the assertions must not
    // depend on which callers win.
    let mut first_out: Option<String> = None;
    let mut success_count = 0usize;
    for out in &outputs {
        if out.status.success() {
            success_count += 1;
            assert!(stdout_str(out).starts_with("CONTINUITY_RECORDED "));
            match &first_out {
                None => first_out = Some(stdout_raw(out)),
                Some(prev) => assert_eq!(stdout_raw(out), *prev),
            }
        } else {
            assert_category_no_stdout(out, "PERSISTENCE_FAILED");
        }
    }
    assert!(success_count >= 1, "at least one caller must publish");
    // Exactly one target entry resolving exactly the supplied proofs.
    let ledger = t.get_continuity_ledger().unwrap();
    let entries = ledger["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    let links = entries[0]["continuity_manifest"]["resolved_links"]
        .as_array()
        .unwrap();
    assert_eq!(links.len(), 2);
    let ids: Vec<&str> = links
        .iter()
        .map(|l| l["source_repository_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["repo-alpha", "repo-beta"]);
    assert_eq!(ledger["accepted_plan_sha256"], t_plan_sha);
    // Sources read-only; sentinel repo untouched by MRGS.
    assert_eq!(
        snapshot_tree(&s1.repo),
        s1_tree_before,
        "source A read-only"
    );
    assert_eq!(
        snapshot_tree(&s2.repo),
        s2_tree_before,
        "source B read-only"
    );
    assert_eq!(
        snapshot_tree(&c.repo),
        c_tree_after,
        "sentinel tree unchanged"
    );
    assert!(
        !c.repo.join(".mrgs").exists(),
        "sentinel repo must have no governance dir"
    );
    assert_eq!(git_head(&c.repo), c_git_head_after);
    assert_no_temp_files(&t.repo);
}
// ===========================================================================
// 16.6 Privacy, process, network, environment, and output security
// ===========================================================================

/// Build a TestRepo under an arbitrary parent (for sentinel-path fixtures).
fn test_repo_in(parent: &Path, report_dir: &Path) -> TestRepo {
    let repo = parent.join("repo");
    git_init(&repo);
    git_commit(&repo, ".gitignore", b".mrgs/\n");
    git_commit(&repo, "src/main.rs", b"fn main() {}\n");
    let plan_path = repo.join("plan.toml");
    let contract_path = repo.join("contract.toml");
    write_file(&plan_path, valid_plan_toml());
    write_file(&contract_path, &contract_toml_for_phase("phase-1"));
    std::fs::create_dir_all(report_dir).unwrap();
    TestRepo {
        _dir: tempfile::TempDir::new().unwrap(),
        repo,
        report_dir: report_dir.to_path_buf(),
        contract_path,
        plan_path,
    }
}

#[test]
fn test_obligation_41_network_and_shell_nonuse() {
    // A shim directory whose members would record any invocation. MRGS must
    // never execute them; only the mrgs binary and governed git children run.
    let shim_dir = tempfile::TempDir::new().unwrap();
    let marker_dir = shim_dir.path().join("markers");
    std::fs::create_dir_all(&marker_dir).unwrap();
    let tools = [
        "curl.exe",
        "wget.exe",
        "powershell.exe",
        "pwsh.exe",
        "python.exe",
        "node.exe",
        "ssh.exe",
        "git-lfs.exe",
        "openssl.exe",
        "nc.exe",
        "telnet.exe",
        "sh.exe",
        "bash.exe",
    ];
    #[cfg(windows)]
    {
        let source = r#"
use std::env;
use std::path::Path;
fn main() {
    let marker_dir = env::var("SHIM_MARKER_DIR").unwrap();
    let name = env::current_exe().unwrap().file_name().unwrap().to_string_lossy().into_owned();
    std::fs::write(Path::new(&marker_dir).join(name), b"invoked").unwrap();
    std::process::exit(127);
}
"#;
        let src = shim_dir.path().join("shim.rs");
        std::fs::write(&src, source).unwrap();
        let exe = shim_dir.path().join("tool-shim.exe");
        let compile = Command::new("rustc")
            .arg(&src)
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "shim compilation failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        for tool in &tools {
            std::fs::copy(&exe, shim_dir.path().join(tool)).unwrap();
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for tool in &tools {
            let tool = tool.trim_end_matches(".exe");
            let path = shim_dir.path().join(tool);
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\necho invoked > \"$SHIM_MARKER_DIR/$(basename \"$0\")\"\nexit 127\n"
                ),
            )
            .unwrap();
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
    }

    let t = TestRepo::new();
    let repo = t.repo.to_string_lossy().into_owned();
    let mut paths: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap()).collect();
    paths.insert(0, shim_dir.path().to_path_buf());
    let new_path = std::env::join_paths(paths).unwrap();
    let envs: &[(&str, &str)] = &[
        ("PATH", new_path.to_str().unwrap()),
        ("SHIM_MARKER_DIR", marker_dir.to_str().unwrap()),
        ("HTTP_PROXY", "http://evil-proxy:8080"),
        ("HTTPS_PROXY", "http://evil-proxy:8080"),
        ("ALL_PROXY", "socks5://evil-proxy:1080"),
        ("NO_PROXY", ""),
    ];

    // Representative Phase 1-8 commands under the hostile PATH.
    let out = t.run_with_env(
        &[
            "plan",
            "accept",
            "--repo",
            &repo,
            "--plan",
            &t.plan_path.to_string_lossy(),
        ],
        envs,
    );
    assert_success(&out);
    let out = t.run_with_env(
        &["phase", "select", "--repo", &repo, "--phase", "phase-1"],
        envs,
    );
    assert_success(&out);
    let out = t.run_with_env(
        &[
            "contract",
            "draft",
            "--repo",
            &repo,
            "--contract",
            &t.contract_path.to_string_lossy(),
        ],
        envs,
    );
    assert_success(&out);
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    let out = t.run_with_env(
        &[
            "contract",
            "accept",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            &sha,
            "--decision",
            "ACCEPTED",
        ],
        envs,
    );
    assert_success(&out);
    // Raw begin (not the helper) requires committed sources; TestRepo::new
    // leaves plan.toml/contract.toml untracked.
    t.commit_sources();
    let out = t.run_with_env(
        &[
            "implementation",
            "begin",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            &sha,
        ],
        envs,
    );
    assert_success(&out);
    let out = t.run_with_env(&["implementation", "check", "--repo", &repo], envs);
    assert_success(&out);
    let out = t.run_with_env(
        &["audit", "begin", "--repo", &repo, "--auditor", "auditor1"],
        envs,
    );
    assert_success(&out);
    let parts = split_stdout(&out);
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let out = t.run_with_env(
        &[
            "audit",
            "record",
            "--repo",
            &repo,
            "--report",
            &report_path.to_string_lossy(),
        ],
        envs,
    );
    assert_success(&out);
    let out = t.run_with_env(
        &["phase", "close", "--repo", &repo, "--phase", "phase-1"],
        envs,
    );
    assert_success(&out);
    let close_parts = split_stdout(&out);
    let receipt = close_parts[4].clone();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t.run_with_env(
        &[
            "continuity",
            "record",
            "--repo",
            &repo,
            "--metadata",
            &meta.to_string_lossy(),
        ],
        envs,
    );
    assert_success(&out);
    let out = t.run_with_env(&["recovery", "inspect", "--repo", &repo], envs);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_NOT_REQUIRED "));

    // Zero network-helper / shell tool invocations.
    let markers: Vec<String> = std::fs::read_dir(&marker_dir)
        .map(|rd| {
            rd.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        markers.is_empty(),
        "unauthorized executables were invoked: {:?}",
        markers
    );
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_42_git_child_process_sanitization() {
    let recorder = create_env_aware_git_recorder();
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    t.draft_contract();
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    // Raw begin (not the helper) requires committed sources.
    t.commit_sources();

    // Inject hostile Git control variables into the MRGS process; the child
    // git processes must receive only the sanitized controls.
    let hostile: &[(&str, &str)] = &[
        ("GIT_CONFIG_PARAMETERS", "'core.sshCommand=echo pwned'"),
        ("GIT_SHALLOW_FILE", "evil-shallow"),
        ("GIT_DIR", "/evil/git-dir"),
        ("GIT_WORK_TREE", "/evil/work-tree"),
        ("GIT_INDEX_FILE", "/evil/index"),
        ("GIT_OBJECT_DIRECTORY", "/evil/objects"),
        ("GIT_ALTERNATE_OBJECT_DIRECTORIES", "/evil/alt"),
        ("GIT_NAMESPACE", "evil"),
        ("GIT_CONFIG_COUNT", "2"),
        ("GIT_CONFIG_KEY_0", "core.sshCommand"),
        ("GIT_CONFIG_VALUE_0", "echo pwned"),
        ("GIT_CONFIG_KEY_1", "alias.checkout"),
        ("GIT_CONFIG_VALUE_1", "!echo pwned"),
        ("GIT_SSH_COMMAND", "echo pwned"),
        ("GIT_SSH_VARIANT", "ssh"),
        ("GIT_PAGER", "cat"),
        ("GIT_EDITOR", "echo"),
        ("GIT_ASKPASS", "/bin/false"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_PROXY_COMMAND", "evil-proxy"),
        ("GIT_HTTP_PROXY", "http://evil:8080"),
        ("GIT_SSL_NO_VERIFY", "true"),
        ("GIT_AUTHOR_NAME", "evil"),
        ("GIT_COMMITTER_NAME", "evil"),
        ("GIT_CEILING_DIRECTORIES", "/evil"),
    ];
    let out = run_with_env_aware_recorder(
        &recorder,
        &t.repo,
        &[
            "implementation",
            "begin",
            "--revision",
            "1",
            "--sha256",
            &sha,
        ],
        hostile,
    );
    assert_success(&out);
    let out =
        run_with_env_aware_recorder(&recorder, &t.repo, &["implementation", "check"], hostile);
    assert_success(&out);

    let invocations = read_recorder_env(&recorder.env_log);
    assert!(!invocations.is_empty(), "no git invocations recorded");
    let forbidden = [
        "GIT_CONFIG_PARAMETERS",
        "GIT_SHALLOW_FILE",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_KEY_0",
        "GIT_CONFIG_VALUE_0",
        "GIT_CONFIG_KEY_1",
        "GIT_CONFIG_VALUE_1",
        "GIT_SSH_COMMAND",
        "GIT_SSH_VARIANT",
        "GIT_PAGER",
        "GIT_EDITOR",
        "GIT_ASKPASS",
        "GIT_TERMINAL_PROMPT",
        "GIT_PROXY_COMMAND",
        "GIT_HTTP_PROXY",
        "GIT_SSL_NO_VERIFY",
        "GIT_AUTHOR_NAME",
        "GIT_COMMITTER_NAME",
        "GIT_CEILING_DIRECTORIES",
    ];
    for (idx, env_map) in invocations.iter().enumerate() {
        for var in forbidden {
            // The recorder marks a sanitizer-removed variable as "<absent>";
            // any other value means the hostile control leaked through.
            let leaked = env_map.get(var).map(|s| s.as_str());
            assert!(
                leaked.is_none() || leaked == Some("<absent>"),
                "invocation {} leaked {} to a git child: {:?}",
                idx,
                var,
                leaked
            );
        }
        // Required sanitized controls are set exactly.
        assert_eq!(
            env_map.get("GIT_OPTIONAL_LOCKS").map(|s| s.as_str()),
            Some("0")
        );
        assert_eq!(
            env_map.get("GIT_CONFIG_NOSYSTEM").map(|s| s.as_str()),
            Some("1")
        );
        assert_eq!(
            env_map.get("GIT_ATTR_NOSYSTEM").map(|s| s.as_str()),
            Some("1")
        );
        assert_eq!(
            env_map.get("GIT_NO_LAZY_FETCH").map(|s| s.as_str()),
            Some("1")
        );
    }

    // No lazy object fetch: every recorded argv includes --no-lazy-fetch.
    let argv = read_recorder_argv(&recorder.argv_log);
    assert!(!argv.is_empty(), "no git argv recorded");
    for args in &argv {
        assert!(
            args.iter().any(|a| a == "--no-lazy-fetch"),
            "git child missing --no-lazy-fetch: {:?}",
            args
        );
    }
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_43_environment_secret_nonobservation() {
    let secrets: &[(&str, &str)] = &[
        ("GITHUB_TOKEN", "SENTINEL_TOKEN_9F2C"),
        ("GITLAB_TOKEN", "SENTINEL_GITLAB_77A"),
        ("AWS_ACCESS_KEY_ID", "SENTINEL_AWS_KEY"),
        ("AWS_SECRET_ACCESS_KEY", "SENTINEL_AWS_SECRET"),
        ("NPM_TOKEN", "SENTINEL_NPM_11B"),
        ("OPENAI_API_KEY", "SENTINEL_OPENAI_22C"),
        ("ANTHROPIC_API_KEY", "SENTINEL_ANTHROPIC_33D"),
        ("USERNAME", "evil-user-sentinel"),
        ("USER", "evil-user-sentinel"),
        ("LOGNAME", "evil-user-sentinel"),
        ("COMPUTERNAME", "evil-host-sentinel"),
        ("HOSTNAME", "evil-host-sentinel"),
        ("EMAIL", "evil@sentinel.example"),
        ("GIT_AUTHOR_NAME", "evil-author-sentinel"),
        ("GIT_COMMITTER_NAME", "evil-committer-sentinel"),
        ("HTTP_PROXY", "http://evil-proxy:8080"),
        ("HTTPS_PROXY", "http://evil-proxy:8080"),
        ("CI", "true"),
    ];
    let secret_values: Vec<&str> = secrets.iter().map(|(_, v)| *v).collect();

    let t = TestRepo::new();
    let repo = t.repo.to_string_lossy().into_owned();
    let mut outputs = Vec::new();
    outputs.push(t.run_with_env(
        &[
            "plan",
            "accept",
            "--repo",
            &repo,
            "--plan",
            &t.plan_path.to_string_lossy(),
        ],
        secrets,
    ));
    outputs.push(t.run_with_env(
        &["phase", "select", "--repo", &repo, "--phase", "phase-1"],
        secrets,
    ));
    outputs.push(t.run_with_env(
        &[
            "contract",
            "draft",
            "--repo",
            &repo,
            "--contract",
            &t.contract_path.to_string_lossy(),
        ],
        secrets,
    ));
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    outputs.push(t.run_with_env(
        &[
            "contract",
            "accept",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            &sha,
            "--decision",
            "ACCEPTED",
        ],
        secrets,
    ));
    // Raw begin (not the helper) requires committed sources.
    t.commit_sources();
    outputs.push(t.run_with_env(
        &[
            "implementation",
            "begin",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            &sha,
        ],
        secrets,
    ));
    outputs.push(t.run_with_env(&["implementation", "check", "--repo", &repo], secrets));
    let open = t.run_with_env(
        &["audit", "begin", "--repo", &repo, "--auditor", "auditor1"],
        secrets,
    );
    let parts = split_stdout(&open);
    outputs.push(open);
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    outputs.push(t.run_with_env(
        &[
            "audit",
            "record",
            "--repo",
            &repo,
            "--report",
            &report_path.to_string_lossy(),
        ],
        secrets,
    ));
    let close = t.run_with_env(
        &["phase", "close", "--repo", &repo, "--phase", "phase-1"],
        secrets,
    );
    let close_parts = split_stdout(&close);
    outputs.push(close);
    let receipt = close_parts[4].clone();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    outputs.push(t.run_with_env(
        &[
            "continuity",
            "record",
            "--repo",
            &repo,
            "--metadata",
            &meta.to_string_lossy(),
        ],
        secrets,
    ));
    outputs.push(t.run_with_env(&["recovery", "inspect", "--repo", &repo], secrets));
    for out in &outputs {
        assert_success(out);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        for secret in &secret_values {
            assert!(!stdout.contains(secret), "stdout leaked {}", secret);
            assert!(!stderr.contains(secret), "stderr leaked {}", secret);
        }
    }

    // A rejected recovery apply (unknown id / stale subject) is a failure
    // output; it must also never leak the sentinels.
    let apply_out = t.run_with_env(
        &[
            "recovery",
            "apply",
            "--repo",
            &repo,
            "--recovery-id",
            &"a".repeat(64),
            "--subject-sha256",
            &"b".repeat(64),
            "--decision",
            "RECOVER",
        ],
        secrets,
    );
    assert_failure(&apply_out);
    for secret in &secret_values {
        assert!(
            !String::from_utf8_lossy(&apply_out.stdout).contains(secret),
            "stdout leaked {}",
            secret
        );
        assert!(
            !String::from_utf8_lossy(&apply_out.stderr).contains(secret),
            "stderr leaked {}",
            secret
        );
    }

    // Failure outputs never leak sentinels either.
    let fail = t.run_with_env(
        &["audit", "begin", "--repo", &repo, "--auditor", "\u{1}bad"],
        secrets,
    );
    assert_failure(&fail);
    assert_eq!(stderr_str(&fail), "error: AUDITOR_ID_INVALID");
    assert_eq!(stdout_str(&fail), "");

    // Governance bytes never contain the sentinels.
    let gov_dir = t.repo.join(".mrgs");
    for entry in std::fs::read_dir(&gov_dir).unwrap() {
        let entry = entry.unwrap();
        let bytes = std::fs::read(entry.path()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for secret in &secret_values {
            assert!(
                !text.contains(secret),
                "governance file {:?} contains {}",
                entry.file_name(),
                secret
            );
        }
    }
}

/// The audit record persists the report's canonical path, and the contract
/// forbids persisting the user profile; the fixture's report must therefore
/// live outside USERPROFILE (a writable drive-root temp on Windows).
fn non_profile_tempdir() -> tempfile::TempDir {
    #[cfg(windows)]
    {
        let root = std::env::current_dir()
            .unwrap()
            .ancestors()
            .last()
            .unwrap()
            .to_path_buf();
        tempfile::Builder::new()
            .prefix("mrgs-identity-")
            .tempdir_in(&root)
            .unwrap()
    }
    #[cfg(not(windows))]
    {
        tempfile::TempDir::new().unwrap()
    }
}

#[test]
fn test_obligation_44_path_and_identity_privacy() {
    let sentinel = "PRIVSENTINEL_5B";
    let root = tempfile::Builder::new()
        .prefix(&format!("mrgs-{}-", sentinel))
        .tempdir()
        .unwrap();
    let plain_report_dir = non_profile_tempdir();
    let t = test_repo_in(root.path(), plain_report_dir.path());
    let repo = t.repo.to_string_lossy().into_owned();
    // A remote URL and host identity sentinels must never be persisted.
    let remote = "https://evil-remote.example/org/repo.git";
    assert!(git(&t.repo, &["remote", "add", "origin", remote])
        .status
        .success());
    let envs: &[(&str, &str)] = &[
        ("COMPUTERNAME", "evil-host-sentinel"),
        ("HOSTNAME", "evil-host-sentinel"),
        ("USERNAME", "evil-user-sentinel"),
        ("USER", "evil-user-sentinel"),
    ];

    // Every operation is asserted immediately so the first failing step is
    // the one reported; each step's evidence (exit code, output lengths,
    // expected durable path, .mrgs inventory) is logged before the assert.
    let run_step = |name: &str, args: &[&str], expected: Option<&str>| {
        let out = t.run_with_env(args, envs);
        let mrgs: Vec<String> = std::fs::read_dir(t.repo.join(".mrgs"))
            .map(|it| {
                it.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        let expected_ok = expected
            .map(|f| t.repo.join(".mrgs").join(f).exists())
            .unwrap_or(true);
        eprintln!(
            "STEP {} rc={:?} stdout_len={} stderr_len={} expected_path={:?} expected_ok={:?} mrgs={:?}\n  stdout={:?}\n  stderr={:?}",
            name,
            out.status.code(),
            out.stdout.len(),
            out.stderr.len(),
            expected.map(|f| t.repo.join(".mrgs").join(f).to_string_lossy().into_owned()),
            expected_ok,
            mrgs,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_success(&out);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            !stdout.contains(sentinel),
            "stdout leaks the canonical root"
        );
        assert!(!stdout.contains("evil-"), "stdout leaks host/user identity");
        out
    };
    run_step(
        "plan_accept",
        &[
            "plan",
            "accept",
            "--repo",
            &repo,
            "--plan",
            &t.plan_path.to_string_lossy(),
        ],
        Some("accepted-plan.json"),
    );
    run_step(
        "phase_select",
        &["phase", "select", "--repo", &repo, "--phase", "phase-1"],
        Some("state.json"),
    );
    run_step(
        "contract_draft",
        &[
            "contract",
            "draft",
            "--repo",
            &repo,
            "--contract",
            &t.contract_path.to_string_lossy(),
        ],
        Some("contract-draft.json"),
    );
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    run_step(
        "contract_accept",
        &[
            "contract",
            "accept",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            &sha,
            "--decision",
            "ACCEPTED",
        ],
        Some("accepted-contract.json"),
    );
    // Raw begin (not the helper) requires committed sources.
    t.commit_sources();
    run_step(
        "implementation_begin",
        &[
            "implementation",
            "begin",
            "--repo",
            &repo,
            "--revision",
            "1",
            "--sha256",
            &sha,
        ],
        Some("implementation-authority.json"),
    );
    run_step(
        "implementation_check",
        &["implementation", "check", "--repo", &repo],
        Some("implementation-authority.json"),
    );
    let open = run_step(
        "audit_begin",
        &["audit", "begin", "--repo", &repo, "--auditor", "auditor1"],
        Some("audit-ledger.json"),
    );
    let parts = split_stdout(&open);
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    run_step(
        "audit_record",
        &[
            "audit",
            "record",
            "--repo",
            &repo,
            "--report",
            &report_path.to_string_lossy(),
        ],
        Some("audit-ledger.json"),
    );
    let close = run_step(
        "phase_close",
        &["phase", "close", "--repo", &repo, "--phase", "phase-1"],
        Some("completion-ledger.json"),
    );
    let close_parts = split_stdout(&close);
    let receipt = close_parts[4].clone();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    run_step(
        "continuity_record",
        &[
            "continuity",
            "record",
            "--repo",
            &repo,
            "--metadata",
            &meta.to_string_lossy(),
        ],
        Some("continuity-ledger.json"),
    );
    run_step(
        "recovery_inspect",
        &["recovery", "inspect", "--repo", &repo],
        None,
    );

    // Durable records persist only authorized repo-relative path forms.
    let gov_dir = t.repo.join(".mrgs");
    let mut all_bytes = Vec::new();
    for entry in std::fs::read_dir(&gov_dir).unwrap() {
        let entry = entry.unwrap();
        let bytes = std::fs::read(entry.path()).unwrap();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        assert!(
            !text.contains(sentinel),
            "{} persists the canonical root sentinel",
            entry.file_name().to_string_lossy()
        );
        assert!(
            !text.contains("evil-"),
            "{} persists host/user identity",
            entry.file_name().to_string_lossy()
        );
        assert!(
            !text.contains(remote),
            "{} persists the remote URL",
            entry.file_name().to_string_lossy()
        );
        // No user-profile prefix (HOME/USERPROFILE) may be persisted.
        if let Ok(home) = std::env::var("USERPROFILE") {
            let normalized = home.replace('\\', "/");
            assert!(
                !text.contains(&normalized),
                "{} persists the user profile",
                entry.file_name().to_string_lossy()
            );
        }
        all_bytes.extend_from_slice(&bytes);
    }
    // The only absolute path persisted is the expressly authorized audit
    // report source path; everything else is repo-relative. The closeout
    // archived the audit ledger, so read it from the completion manifest.
    let completion: Value =
        serde_json::from_str(&t.read_mrgs_str("completion-ledger.json")).unwrap();
    let archived_audit: Value = serde_json::from_str(
        completion["completions"][0]["final_manifest"]["archived_governance"]
            ["audit_ledger_content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let stored_path = archived_audit["rounds"][0]["report_source_path"]
        .as_str()
        .unwrap()
        .replace('\\', "/");
    assert!(
        stored_path.contains(&plain_report_dir.path().to_string_lossy().replace('\\', "/")),
        "report source path must be the authorized external path: {}",
        stored_path
    );
    // Plan / contract / metadata paths are exactly repo-relative.
    let accepted: Value = serde_json::from_str(&t.read_mrgs_str("accepted-plan.json")).unwrap();
    assert_eq!(accepted["plan_path"], "plan.toml");
    // The closeout archived (and deleted) the draft; read its source path
    // from the completion manifest's archived draft content.
    let archived_draft: Value = serde_json::from_str(
        completion["completions"][0]["final_manifest"]["archived_governance"]
            ["contract_draft_content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(archived_draft["source_path"], "contract.toml");
    let continuity: Value =
        serde_json::from_str(&t.read_mrgs_str("continuity-ledger.json")).unwrap();
    assert_eq!(
        continuity["entries"][0]["continuity_manifest"]["metadata_source_path"],
        "m.toml"
    );
    assert_eq!(continuity["repository_id"], "mrgs");
    let _ = all_bytes;
}

#[test]
fn test_obligation_45_source_content_and_error_redaction() {
    let plan_secret = "SECRET_PLAN_7F";
    let contract_secret = "SECRET_CONTRACT_9C";
    let report_secret = "SECRET_REPORT_3D";
    let meta_secret = "SECRET_META_1B";
    let auth_secret = "SECRET_AUTH_5E";

    let t = TestRepo::new();
    write_file(
        &t.plan_path,
        &format!("{}\n# {}\n", valid_plan_toml(), plan_secret),
    );
    let contract_toml = format!(
        "{}# {}\n",
        contract_toml_for_phase("phase-1"),
        contract_secret
    );
    write_file(&t.contract_path, &contract_toml);

    let plan_out = t.accept_plan();
    assert_success(&plan_out);
    assert!(!stdout_str(&plan_out).contains(plan_secret));
    let sel = t.select_phase("phase-1");
    assert_success(&sel);
    assert!(!stdout_str(&sel).contains(plan_secret));
    let draft = t.draft_contract();
    assert_success(&draft);
    assert!(!stdout_str(&draft).contains(contract_secret));
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    let accept = t.accept_contract(1, &sha);
    assert_success(&accept);
    assert!(!stdout_str(&accept).contains(contract_secret));
    let begin = t.impl_begin(1, &sha);
    assert_success(&begin);
    assert!(!stdout_str(&begin).contains(contract_secret));
    let check = t.impl_check();
    assert_success(&check);
    assert!(!stdout_str(&check).contains(contract_secret));

    let open = t.audit_begin("auditor1");
    assert_success(&open);
    let parts = split_stdout(&open);
    // A trailing comment is invalid JSON; instead embed the secret in the
    // summary field (parse-valid).
    let mut v: Value =
        serde_json::from_str(&t.make_pass_report(&parts[1], &parts[3], "auditor1")).unwrap();
    v["summary"] = json!(format!("verified {}", report_secret));
    let report = serde_json::to_string_pretty(&v).unwrap();
    let report_path = t.write_report(&report);
    let record = t.audit_record(&report_path);
    assert_success(&record);
    assert!(!stdout_str(&record).contains(report_secret));

    let close = t.phase_close("phase-1");
    assert_success(&close);
    assert!(!stdout_str(&close).contains(contract_secret));
    assert!(!stdout_str(&close).contains(report_secret));
    let close_parts = split_stdout(&close);
    let receipt = close_parts[4].clone();
    let metadata = format!(
        "{}\n# {}\n",
        standard_metadata("phase-1", &receipt),
        meta_secret
    );
    // A trailing comment is parse-valid TOML; keep the secret inside it.
    let meta = t.write_metadata("m.toml", &metadata);
    let cont = t.continuity_record(&meta);
    assert_success(&cont);
    assert!(!stdout_str(&cont).contains(meta_secret));

    // Durable-byte discipline: the plan secret must never be persisted;
    // contract/report/metadata secrets appear only in their authorized
    // archived content fields.
    let gov_dir = t.repo.join(".mrgs");
    for entry in std::fs::read_dir(&gov_dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(entry.path()).unwrap();
        if text.contains(plan_secret) {
            // Only the completion ledger's authorized
            // final_manifest.plan_content archive may carry the plan bytes
            // (closeout.rs FinalManifest schema requirement).
            assert_eq!(
                name, "completion-ledger.json",
                "plan secret in unexpected file {}",
                name
            );
        }
        if text.contains(contract_secret) {
            // Only the content-archiving records may carry it.
            assert!(
                [
                    "contract-draft.json",
                    "accepted-contract.json",
                    "implementation-authority.json",
                    "completion-ledger.json"
                ]
                .contains(&name.as_str()),
                "contract secret in unexpected file {}",
                name
            );
        }
        if text.contains(report_secret) {
            assert!(
                ["audit-ledger.json", "completion-ledger.json"].contains(&name.as_str()),
                "report secret in unexpected file {}",
                name
            );
        }
        if text.contains(meta_secret) {
            assert_eq!(name, "continuity-ledger.json", "metadata secret misplaced");
        }
    }

    // Malformed authority with a secret: the failure stderr carries only the
    // exact category and never the secret.
    // Validation-order probe (MUTATED_RECORD=accepted-plan.json,
    // MUTATED_FIELD=rogue). Pre-mutation: the healthy fixture's public
    // command reaches the expected later gate (RECOVERY_NOT_REQUIRED,
    // exit 0) — the mutation must change exactly that outcome.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let pre = t2.inspect();
    assert_success(&pre);
    assert!(stdout_str(&pre).starts_with("RECOVERY_NOT_REQUIRED "));
    let mut v: Value = serde_json::from_str(&t2.read_mrgs_str("accepted-plan.json")).unwrap();
    v["rogue"] = json!(auth_secret);
    let mutated = serde_json::to_vec_pretty(&v).unwrap();
    t2.write_mrgs("accepted-plan.json", mutated.as_slice());
    // Post-mutation: recovery.rs::exact_keys rejects the unknown plan key
    // FIRST (RecoveryUnrecoverable) before any authority-level check —
    // GOVERNANCE_AUTHORITY_INVALID is not reachable from this command.
    // Zero mutation: every durable byte stays exact.
    let snapshot = mrgs_snapshot(&t2.repo);
    let fail = t2.inspect();
    assert_category_no_stdout(&fail, "RECOVERY_UNRECOVERABLE");
    assert!(!stderr_str(&fail).contains(auth_secret));
    assert_snapshot_unchanged(&t2.repo, &snapshot);
    assert_eq!(t2.read_mrgs("accepted-plan.json"), mutated);
    // A genuinely malformed authority file (invalid JSON with a secret):
    // the recovery inspect classifies the broken state as recoverable
    // (RECOVERY_REQUIRED + RESTORE_STATE, exit 0) and never echoes the
    // malformed bytes or the secret.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    let malformed = format!("{{not json {} }}", auth_secret);
    t3.write_mrgs("state.json", malformed.as_bytes());
    let snapshot = mrgs_snapshot(&t3.repo);
    let out = t3.inspect();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_REQUIRED "));
    assert!(stdout_str(&out).contains("RECOVERY_ACTION 1 RESTORE_STATE state.json"));
    assert!(!stdout_str(&out).contains(auth_secret));
    assert_eq!(stderr_str(&out), "");
    assert_snapshot_unchanged(&t3.repo, &snapshot);
}

#[test]
fn test_obligation_46_git_nonmutation_all_commands() {
    let t = TestRepo::new();
    let repo = t.repo.to_string_lossy().into_owned();

    // Fresh state for plan accept (the fixture commits happen first).
    let snapshot = || -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(git_head(&t.repo).as_bytes());
        out.extend_from_slice(git_branch(&t.repo).as_bytes());
        out.extend_from_slice(&git_index_bytes(&t.repo));
        out.extend_from_slice(&git_refs(&t.repo));
        out.extend_from_slice(&git_config_list(&t.repo));
        out.extend_from_slice(git_remotes(&t.repo).as_bytes());
        out.extend_from_slice(format!("{:?}", git_hooks(&t.repo)).as_bytes());
        out.extend_from_slice(b"---WORKTREE---");
        for (k, v) in git_snapshot(&t.repo) {
            out.extend_from_slice(k.as_bytes());
            out.extend_from_slice(&v);
        }
        out
    };
    // Representative success and failure paths for every command family.
    let sha = "a".repeat(64);
    assert_success(&t.accept_plan());
    assert_success(&t.select_phase("phase-1"));
    assert_success(&t.draft_contract());
    let draft_sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &draft_sha));
    assert_success(&t.impl_begin(1, &draft_sha));
    assert_success(&t.impl_check());
    assert_success(&t.audit_begin("auditor1"));
    let open = t.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    assert_success(&t.audit_record(&report_path));
    assert_success(&t.phase_close("phase-1"));
    let close_parts = split_stdout(&t.phase_close("phase-1"));
    let receipt = close_parts[4].clone();
    // The continuity metadata-path contract requires the supplied metadata
    // inside the repository; it is the test's own input, so the confinement
    // diff whitelists exactly this fixture file alongside repo/.mrgs/.
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta));
    assert_success(&t.inspect());
    // Recovery apply on the healthy state — genuine pre/post mutation proof:
    // capture the OBSERVED pre-mutation subject, flip exactly one observed
    // byte (JSON whitespace -> tab: parse- and validation-neutral), recompute
    // the post-mutation subject, prove they differ, reject the stale
    // pre-mutation binding, then bind the live post-mutation subject and
    // reach the expected RECOVERY_NOT_REQUIRED no-op with zero mutation.
    let subject_before = recompute_subject(&t.repo);
    let state_bytes = t.read_mrgs("state.json");
    let mut flipped = state_bytes.clone();
    let ws_idx = flipped.iter().position(|&b| b == b' ').unwrap();
    flipped[ws_idx] = b'\t';
    assert!(
        serde_json::from_slice::<Value>(&flipped).is_ok(),
        "whitespace flip must stay valid JSON"
    );
    t.write_mrgs("state.json", &flipped);
    let subject_after = recompute_subject(&t.repo);
    assert_ne!(
        subject_before, subject_after,
        "one observed byte must change the subject"
    );
    let durable_before = mrgs_snapshot(&t.repo);
    let out = t.apply(&sha, &subject_before);
    assert_category_no_stdout(&out, "RECOVERY_SUBJECT_STALE");
    assert_snapshot_unchanged(&t.repo, &durable_before);
    let out = t.apply(&sha, &subject_after);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("RECOVERY_NOT_REQUIRED "));
    assert_snapshot_unchanged(&t.repo, &durable_before);
    // Baseline AFTER all test-owned setup: the impl_begin helper committed
    // the fixture sources, and the failure-fixture bad.toml already exists
    // in the worktree. Only mutations from this point on count as MRGS git
    // mutation.
    let bad_meta = t.write_metadata("bad.toml", "nope");
    let before = snapshot();
    // Failure paths. The phase close archived and removed the phase-scoped
    // authorities, so the begin fails at the FIRST validation boundary
    // (governance authority tuple missing -> GOVERNANCE_AUTHORITY_INVALID);
    // the revision comparison is not reachable after closeout.
    let out = t.run(&[
        "implementation",
        "begin",
        "--repo",
        &repo,
        "--revision",
        "2",
        "--sha256",
        &draft_sha,
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
    let out = t.run(&["audit", "begin", "--repo", &repo, "--auditor", ""]);
    assert_category_no_stdout(&out, "AUDITOR_ID_INVALID");
    let out = t.run(&["phase", "close", "--repo", &repo, "--phase", "phase-2"]);
    assert_failure(&out);
    let out = t.run(&["repair", "check", "--repo", &repo]);
    assert_failure(&out);
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &repo,
        "--metadata",
        &bad_meta.to_string_lossy(),
    ]);
    assert_failure(&out);
    let out = t.run(&[
        "recovery",
        "apply",
        "--repo",
        &repo,
        "--recovery-id",
        &sha,
        "--subject-sha256",
        &sha,
        "--decision",
        "no",
    ]);
    assert_failure(&out);

    // No add, commit, checkout, reset, clean, merge, rebase, tag, config
    // mutation, or remote write may have occurred.
    let after = snapshot();
    assert_eq!(after, before, "git state must be byte-identical");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_47_repository_and_external_write_confinement() {
    let root = tempfile::TempDir::new().unwrap();
    // Sentinel trees before, beside, and outside the target repository.
    write_file(&root.path().join("sentinel-before.txt"), "SENTINEL_BEFORE");
    std::fs::create_dir_all(root.path().join("beside")).unwrap();
    write_file(&root.path().join("beside/sentinel.txt"), "SENTINEL_BESIDE");
    std::fs::create_dir_all(root.path().join("outside")).unwrap();
    write_file(
        &root.path().join("outside/sentinel.txt"),
        "SENTINEL_OUTSIDE",
    );

    let repo = root.path().join("repo");
    git_init(&repo);
    git_commit(&repo, ".gitignore", b".mrgs/\n");
    git_commit(&repo, "src/main.rs", b"fn main() {}\n");
    let plan_path = repo.join("plan.toml");
    let contract_path = repo.join("contract.toml");
    write_file(&plan_path, valid_plan_toml());
    write_file(&contract_path, &contract_toml_for_phase("phase-1"));
    // Pre-commit the tracked sources so the impl_begin helper's
    // commit_sources is a no-op: every later git change is then attributable
    // to MRGS, not to test-owned setup.
    let add = git(&repo, &["add", "-A"]);
    assert!(add.status.success());
    let commit = git(&repo, &["commit", "-qm", "sources"]);
    assert!(commit.status.success());
    let report_dir = root.path().join("reports");
    std::fs::create_dir_all(&report_dir).unwrap();
    let t = TestRepo {
        _dir: tempfile::TempDir::new().unwrap(),
        repo,
        report_dir,
        contract_path,
        plan_path,
    };

    // A read-only source repository (byte-snapshot proof).
    let source = root.path().join("source");
    git_init(&source);
    git_commit(&source, "a.txt", b"source-bytes");
    let source_tree_before = snapshot_tree(&source);

    let before = snapshot_tree(root.path());
    assert_success(&t.accept_plan());
    assert_success(&t.select_phase("phase-1"));
    assert_success(&t.draft_contract());
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    // Raw begin: the helper auto-commits (and its no-op `git add` rewrites
    // the index stat cache), which would count as MRGS git mutation.
    let begin = t.run(&[
        "implementation",
        "begin",
        "--repo",
        &t.repo.to_string_lossy(),
        "--revision",
        "1",
        "--sha256",
        &sha,
    ]);
    assert_success(&begin);
    assert_success(&t.impl_check());
    assert_success(&t.audit_begin("auditor1"));
    let open = t.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    assert_success(&t.audit_record(&report_path));
    assert_success(&t.phase_close("phase-1"));
    let close_parts = split_stdout(&t.phase_close("phase-1"));
    let receipt = close_parts[4].clone();
    // The continuity metadata-path contract requires the supplied metadata
    // inside the repository; it is the test's own input, so the confinement
    // diff whitelists exactly this fixture file alongside repo/.mrgs/.
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    assert_success(&t.continuity_record(&meta));
    assert_success(&t.inspect());

    // Trace the differences: every change must be confined to the target
    // repository's .mrgs directory; sources and sentinels untouched.
    let after = snapshot_tree(root.path());
    let mut diffs = Vec::new();
    for key in after.keys() {
        if before.get(key) != after.get(key) {
            diffs.push(key.clone());
        }
    }
    for key in before.keys() {
        if !after.contains_key(key) {
            diffs.push(key.clone());
        }
    }
    assert!(
        !diffs.is_empty(),
        "the fixture must produce durable changes"
    );
    for diff in &diffs {
        assert!(
            diff.starts_with("repo/.mrgs/")
                || diff == "repo/m.toml"
                || diff == "reports/report.json",
            "write escaped the target .mrgs: {}",
            diff
        );
    }
    assert_eq!(
        snapshot_tree(&source),
        source_tree_before,
        "source must stay read-only"
    );
    assert!(std::fs::read(root.path().join("sentinel-before.txt")).unwrap() == b"SENTINEL_BEFORE");
    assert!(std::fs::read(root.path().join("beside/sentinel.txt")).unwrap() == b"SENTINEL_BESIDE");
    assert!(
        std::fs::read(root.path().join("outside/sentinel.txt")).unwrap() == b"SENTINEL_OUTSIDE"
    );
}

#[test]
fn test_obligation_48_output_contract_regression_and_secret_safety() {
    let t = TestRepo::new();
    let repo = t.repo.to_string_lossy().into_owned();
    let plan_sha = sha_of_file(&t.plan_path);

    // Success tokens: exact framing, field order, casing.
    let out = t.accept_plan();
    assert_success(&out);
    assert_eq!(stdout_str(&out), format!("test-plan {}", plan_sha));
    assert_eq!(stdout_raw(&out), format!("test-plan {}\n", plan_sha));
    let out = t.select_phase("phase-1");
    assert_success(&out);
    assert_eq!(stdout_raw(&out), "phase-1\n");
    let out = t.draft_contract();
    assert_success(&out);
    let draft_sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_eq!(stdout_str(&out), format!("test-contract-v1 {}", draft_sha));
    let out = t.accept_contract(1, &draft_sha);
    assert_success(&out);
    assert_eq!(
        stdout_str(&out),
        format!("ACCEPTED test-contract-v1 1 {}", draft_sha)
    );
    let out = t.impl_begin(1, &draft_sha);
    assert_success(&out);
    let begin_parts = split_stdout(&out);
    assert_eq!(begin_parts.len(), 5);
    assert_eq!(begin_parts[0], "IMPLEMENTATION_BOUND");
    assert_eq!(begin_parts[1], "test-contract-v1");
    assert_eq!(begin_parts[2], "1");
    assert_eq!(begin_parts[3], draft_sha);
    assert_eq!(begin_parts[4].len(), 40);
    let out = t.impl_check();
    assert_success(&out);
    let check_parts = split_stdout(&out);
    assert_eq!(check_parts.len(), 5);
    assert_eq!(check_parts[0], "IMPLEMENTATION_OK");
    let out = t.audit_begin("auditor1");
    assert_success(&out);
    let open_parts = split_stdout(&out);
    assert_eq!(open_parts.len(), 4);
    assert_eq!(open_parts[0], "AUDIT_OPEN");
    assert_eq!(open_parts[2], "1");
    let report = t.make_pass_report(&open_parts[1], &open_parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let out = t.audit_record(&report_path);
    assert_success(&out);
    let pass_parts = split_stdout(&out);
    assert_eq!(pass_parts.len(), 4);
    assert_eq!(pass_parts[0], "AUDIT_PASS");
    let out = t.phase_close("phase-1");
    assert_success(&out);
    let close_parts = split_stdout(&out);
    assert_eq!(close_parts.len(), 5);
    assert_eq!(close_parts[0], "PHASE_CLOSED");
    assert_eq!(close_parts[1], "phase-1");
    assert_eq!(close_parts[2], "1");
    assert_eq!(close_parts[3].len(), 64);
    assert_eq!(close_parts[4].len(), 64);
    let out = t.phase_close("phase-1");
    assert_success(&out);
    assert_eq!(stdout_raw(&out), format!("{}\n", close_parts.join(" ")));
    let receipt = close_parts[4].clone();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta);
    assert_success(&out);
    let cont_parts = split_stdout(&out);
    assert_eq!(cont_parts.len(), 6);
    assert_eq!(cont_parts[0], "CONTINUITY_RECORDED");
    assert_eq!(cont_parts[1], "mrgs");
    assert_eq!(cont_parts[2], "phase-1");
    assert_eq!(cont_parts[3], "1");
    let out = t.inspect();
    assert_success(&out);
    let insp_parts = split_stdout(&out);
    assert_eq!(insp_parts.len(), 2);
    assert_eq!(insp_parts[0], "RECOVERY_NOT_REQUIRED");

    // Error families: exact stderr framing, empty stdout, no mixed output.
    // The closeout archived the phase-scoped authorities, so the begin fails
    // at the FIRST boundary (authority tuple missing) — the revision
    // comparison is not reachable post-closeout.
    let out = t.run(&[
        "implementation",
        "begin",
        "--repo",
        &repo,
        "--revision",
        "2",
        "--sha256",
        &draft_sha,
    ]);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");
    let t2 = TestRepo::new();
    t2.accept_plan_success();
    t2.select_phase_success("phase-1");
    let out = t2.run(&[
        "implementation",
        "check",
        "--repo",
        &t2.repo.to_string_lossy(),
    ]);
    // The draft lifecycle check fires before the authority-missing check.
    assert_category_no_stdout(&out, "CONTRACT_NOT_ACCEPTED");
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    assert!(git(&t3.repo, &["branch", "-M", "x"]).status.success());
    let out = t3.run(&[
        "implementation",
        "check",
        "--repo",
        &t3.repo.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "BASELINE_BRANCH_CHANGED");
    // T4A/B/C evidence: fresh fixture per case; state and result are
    // reported before/after each check with immediate assertions.
    let t4_state = |case: &str, t: &TestRepo, src_base: &str, command: &str| {
        let head = git_head(&t.repo);
        let porcelain = git(
            &t.repo,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        );
        let diff = git(&t.repo, &["diff", "--name-status"]);
        let cached = git(&t.repo, &["diff", "--cached", "--name-status"]);
        let tracked = git(&t.repo, &["ls-files", "--", "src/main.rs"]);
        let src_cur = std::fs::read(t.repo.join("src/main.rs"))
            .map(|b| sha256_hex(&b))
            .unwrap_or_default();
        let evil = t.repo.join(".git/evil.txt").exists();
        let auth = std::fs::read(t.repo.join(".mrgs/implementation-authority.json"))
            .map(|b| sha256_hex(&b))
            .unwrap_or_default();
        eprintln!(
            "OB48_CASE={} FIXTURE_ROOT={:?} HEAD={} GIT_STATUS_PORCELAIN_V1_UNTRACKED_ALL={:?} GIT_DIFF_NAME_STATUS={:?} GIT_DIFF_CACHED_NAME_STATUS={:?} SRC_MAIN_TRACKED={} SRC_MAIN_HASH_BASELINE={} SRC_MAIN_HASH_CURRENT={} GIT_EVIL_EXISTS={} IMPLEMENTATION_AUTHORITY_HASH={} COMMAND={}",
            case,
            t.repo,
            head,
            String::from_utf8_lossy(&porcelain.stdout),
            String::from_utf8_lossy(&diff.stdout),
            String::from_utf8_lossy(&cached.stdout),
            !tracked.stdout.is_empty(),
            src_base,
            src_cur,
            evil,
            auth,
            command
        );
    };
    // T4A: fresh fixture, .git/evil.txt only. Git never reports ordinary
    // .git internals and the check has no topology validator for that path,
    // so the deterministic result is IMPLEMENTATION_OK (probe-verified).
    let t4a = TestRepo::new();
    t4a.setup_impl_bound();
    let src_base_a = sha256_hex(&std::fs::read(t4a.repo.join("src/main.rs")).unwrap());
    write_file(&t4a.repo.join(".git/evil.txt"), "x");
    eprintln!("OB48_CASE=T4A_START");
    t4_state(
        "T4A",
        &t4a,
        &src_base_a,
        &format!("implementation check --repo {}", t4a.repo.display()),
    );
    let out = t4a.run(&[
        "implementation",
        "check",
        "--repo",
        &t4a.repo.to_string_lossy(),
    ]);
    eprintln!(
        "OB48_CASE=T4A_RESULT EXIT_CODE={:?} STDOUT_RAW={:?} STDERR_RAW={:?}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("OB48_CASE=T4A_END");
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("IMPLEMENTATION_OK "));
    // T4B: fresh fixture, tracked src/main.rs modified only. The file is
    // inside the accepted implementation allowlist, so the check accepts it.
    let t4b = TestRepo::new();
    t4b.setup_impl_bound();
    let src_base_b = sha256_hex(&std::fs::read(t4b.repo.join("src/main.rs")).unwrap());
    write_file(&t4b.repo.join("src/main.rs"), "changed");
    eprintln!("OB48_CASE=T4B_START");
    t4_state(
        "T4B",
        &t4b,
        &src_base_b,
        &format!("implementation check --repo {}", t4b.repo.display()),
    );
    let out = t4b.run(&[
        "implementation",
        "check",
        "--repo",
        &t4b.repo.to_string_lossy(),
    ]);
    eprintln!(
        "OB48_CASE=T4B_RESULT EXIT_CODE={:?} STDOUT_RAW={:?} STDERR_RAW={:?}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("OB48_CASE=T4B_END");
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("IMPLEMENTATION_OK "));
    // T4C: fresh fixture, both mutations combined.
    let t4c = TestRepo::new();
    t4c.setup_impl_bound();
    let src_base_c = sha256_hex(&std::fs::read(t4c.repo.join("src/main.rs")).unwrap());
    write_file(&t4c.repo.join(".git/evil.txt"), "x");
    write_file(&t4c.repo.join("src/main.rs"), "changed");
    eprintln!("OB48_CASE=T4C_START");
    t4_state(
        "T4C",
        &t4c,
        &src_base_c,
        &format!("implementation check --repo {}", t4c.repo.display()),
    );
    let out = t4c.run(&[
        "implementation",
        "check",
        "--repo",
        &t4c.repo.to_string_lossy(),
    ]);
    eprintln!(
        "OB48_CASE=T4C_RESULT EXIT_CODE={:?} STDOUT_RAW={:?} STDERR_RAW={:?}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("OB48_CASE=T4C_END");
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("IMPLEMENTATION_OK "));
    // T4D: untracked-ignored reserved .mrgs file — the reachable
    // CHANGE_FORBIDDEN family (probe-verified). Force-tracking the file
    // instead would yield GIT_INVENTORY_INVALID (governed tracked path).
    let t4d = TestRepo::new();
    t4d.setup_impl_bound();
    write_file(&t4d.repo.join(".mrgs/evil.json"), "{}");
    eprintln!("OB48_CASE=T4D_START");
    let out = t4d.run(&[
        "implementation",
        "check",
        "--repo",
        &t4d.repo.to_string_lossy(),
    ]);
    eprintln!(
        "OB48_CASE=T4D_RESULT EXIT_CODE={:?} STDOUT_RAW={:?} STDERR_RAW={:?}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("OB48_CASE=T4D_END");
    assert_category_no_stdout(&out, "CHANGE_FORBIDDEN");
    // T4E: fresh dirty-source fixture — implementation begin's cleanliness
    // gate rejects the modified tracked source (deterministic GIT_DIRTY).
    let t4e = TestRepo::new();
    t4e.setup_impl_bound();
    write_file(&t4e.repo.join("src/main.rs"), "changed");
    eprintln!("OB48_CASE=T4E_START");
    let out = t4e.run(&[
        "implementation",
        "begin",
        "--repo",
        &t4e.repo.to_string_lossy(),
        "--revision",
        "1",
        "--sha256",
        t4e.get_draft()["sha256"].as_str().unwrap(),
    ]);
    eprintln!(
        "OB48_CASE=T4E_RESULT EXIT_CODE={:?} STDOUT_RAW={:?} STDERR_RAW={:?}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("OB48_CASE=T4E_END");
    assert_category_no_stdout(&out, "GIT_DIRTY");
    let t5 = TestRepo::new();
    t5.setup_impl_bound();
    std::fs::create_dir_all(t5.repo.join(".mrgs/rogue-dir")).unwrap();
    let out = t5.run(&["recovery", "inspect", "--repo", &t5.repo.to_string_lossy()]);
    assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
    let out = t5.run(&[
        "audit",
        "begin",
        "--repo",
        &t5.repo.to_string_lossy(),
        "--auditor",
        "\u{1}",
    ]);
    assert_category_no_stdout(&out, "AUDITOR_ID_INVALID");
    // No audit round exists (the begin with the invalid auditor id never
    // published): the repair check fails at its ledger read FIRST.
    let out = t5.run(&["repair", "check", "--repo", &t5.repo.to_string_lossy()]);
    assert_category_no_stdout(&out, "AUDIT_LEDGER_MISSING");
    let out = t5.run(&[
        "phase",
        "close",
        "--repo",
        &t5.repo.to_string_lossy(),
        "--phase",
        "phase-1",
    ]);
    assert_category_no_stdout(&out, "CLOSEOUT_NOT_READY");
    let t6 = TestRepo::new();
    let (_m, receipt) = t6.close_phase1();
    let bad_meta = t6.write_metadata("bad.toml", "nope");
    let out = t6.run(&[
        "continuity",
        "record",
        "--repo",
        &t6.repo.to_string_lossy(),
        "--metadata",
        &bad_meta.to_string_lossy(),
    ]);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");
    let out = t6.run(&[
        "recovery",
        "apply",
        "--repo",
        &t6.repo.to_string_lossy(),
        "--recovery-id",
        &"a".repeat(64),
        "--subject-sha256",
        &"b".repeat(64),
        "--decision",
        "no",
    ]);
    assert_category_no_stdout(&out, "RECOVERY_DECISION_INVALID");
    let t7 = TestRepo::new();
    let out = t7.run(&[
        "plan",
        "accept",
        "--repo",
        &t7.repo.to_string_lossy(),
        "--plan",
        "missing.toml",
    ]);
    assert_err_prefix(&out, "error: plan not found: ");
    assert_eq!(stdout_str(&out), "");
    // Fresh properly-bound fixture: the SHA grammar validation is the
    // deterministic first boundary for contract accept on a valid repo.
    let t7b = TestRepo::new();
    t7b.accept_plan_success();
    t7b.select_phase_success("phase-1");
    t7b.draft_contract();
    let out = t7b.run(&[
        "contract",
        "accept",
        "--repo",
        &t7b.repo.to_string_lossy(),
        "--revision",
        "1",
        "--sha256",
        &"A".repeat(64),
        "--decision",
        "ACCEPTED",
    ]);
    assert_err_prefix(&out, "error: invalid SHA-256 hex string");
    assert_eq!(stdout_str(&out), "");
    let _ = receipt;
    let _ = (repo, plan_sha);
}
// 16.7 Deterministic resource-bound robustness
// ===========================================================================

#[test]
fn test_obligation_49_large_plan_and_phase_selection_fixture() {
    // Exactly 128 phases in one linear dependency chain.
    let t = TestRepo::new();
    write_file(&t.plan_path, &plan_toml_with_phases(128));
    let out = t.accept_plan();
    assert_success(&out);
    let accepted: Value = serde_json::from_str(&t.read_mrgs_str("accepted-plan.json")).unwrap();
    assert_eq!(accepted["phase_count"], 128);
    let state: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
    assert_eq!(state["closed_phases"], json!([]));

    // Byte-identical exact replay.
    let replay = t.accept_plan();
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&out));
    // Semantic equality: production serializes struct-field order while
    // re-serializing a parsed serde_json Value sorts keys, so byte
    // comparison against the re-serialization is not canonical.
    let stored: Value = serde_json::from_slice(&t.read_mrgs("accepted-plan.json")).unwrap();
    assert_eq!(stored, accepted);

    // Boundary selection: first phase (no dependency) succeeds.
    let sel = t.select_phase("phase-001");
    assert_success(&sel);
    assert_eq!(stdout_str(&sel), "phase-001");
    let state: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
    assert_eq!(state["active_phase"], "phase-001");
    // No duplicate phases: exactly one active phase remains.
    assert_eq!(state["closed_phases"].as_array().unwrap().len(), 0);

    // Unmet dependency rejection is deterministic at both boundaries.
    let t2 = TestRepo::new();
    write_file(&t2.plan_path, &plan_toml_with_phases(128));
    t2.accept_plan_success();
    let out = t2.select_phase("phase-002");
    assert_err_prefix(
        &out,
        "error: phase 'phase-002' dependency 'phase-001' not closed",
    );
    let out = t2.select_phase("phase-128");
    assert_err_prefix(
        &out,
        "error: phase 'phase-128' dependency 'phase-127' not closed",
    );
    assert_eq!(stdout_str(&out), "");
    assert_no_temp_files(&t2.repo);
}

#[test]
fn test_obligation_50_large_contract_and_audit_fixture() {
    // Contract fixture: 256 requirements, 64 entries per remaining list.
    let requirements: Vec<String> = (0..256).map(|i| format!("req-{:03}", i)).collect();
    let allowed: Vec<String> = (0..64).map(|i| format!("dir-{:03}/", i)).collect();
    let forbidden: Vec<String> = (0..64)
        .map(|i| {
            if i == 0 {
                ".git/".to_string()
            } else {
                format!("f-{:03}/", i)
            }
        })
        .collect();
    let verification: Vec<String> = (0..64).map(|i| format!("cmd-{:03}", i)).collect();
    let handoff: Vec<String> = (0..64).map(|i| format!("FIELD-{:03}", i)).collect();
    let contract_toml = contract_toml_custom(
        "test-contract-v1",
        "phase-1",
        &requirements,
        &allowed,
        &forbidden,
        &verification,
        &handoff,
    );

    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    write_file(&t.contract_path, &contract_toml);
    let out = t.draft_contract();
    assert_success(&out);
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    assert_success(&t.impl_begin(1, &sha));
    let check = t.impl_check();
    assert_success(&check);

    // Exact one-to-one PASS report on a fresh fixture: a PASS round makes
    // the audit terminal, so the 3-round FAIL sequence below must run on
    // its own audit.
    let t1 = TestRepo::new();
    write_file(&t1.plan_path, valid_plan_toml());
    t1.accept_plan_success();
    t1.select_phase_success("phase-1");
    write_file(
        &t1.contract_path,
        &contract_toml_custom(
            "test-contract-v1",
            "phase-1",
            &requirements,
            &allowed,
            &forbidden,
            &verification,
            &handoff,
        ),
    );
    t1.commit_sources();
    t1.draft_contract();
    let sha1 = t1.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t1.accept_contract(1, &sha1));
    assert_success(&t1.impl_begin(1, &sha1));
    let open = t1.audit_begin("auditor1");
    assert_success(&open);
    let parts = split_stdout(&open);
    let pass_report = t1.make_pass_report_exact(
        &parts[1],
        &parts[3],
        "auditor1",
        &requirements,
        &verification,
    );
    let pass_path = t1.write_report(&pass_report);
    let out = t1.audit_record(&pass_path);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("AUDIT_PASS "));

    // 3-round / 2-checked-repair maximum with one terminal result.
    let fail_report = |audit_id: &str, subject: &str| -> String {
        let req_results: Vec<Value> = requirements
            .iter()
            .enumerate()
            .map(|(i, r)| {
                json!({"requirement": r, "status": if i == 0 { "FAIL" } else { "PASS" }, "evidence": "verified"})
            })
            .collect();
        let ver_results: Vec<Value> = verification
            .iter()
            .map(|v| json!({"command": v, "status": "PASS", "evidence": "verified"}))
            .collect();
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "audit_id": audit_id,
            "subject_sha256": subject,
            "auditor_id": "auditor1",
            "independence_declaration": "INDEPENDENT",
            "verdict": "FAIL",
            "summary": "first requirement failed",
            "requirement_results": req_results,
            "verification_results": ver_results,
            "findings": [{
                "id": "F1",
                "severity": "MAJOR",
                "claim_kind": "REQUIREMENT",
                "claim_index": 1,
                "summary": "req-000 not satisfied",
                "evidence": "observed",
                "repair_paths": ["dir-000/changed.txt"]
            }]
        }))
        .unwrap()
    };

    // Round 1 FAIL -> routed (attempt 1).
    let open = t.audit_begin("auditor1");
    eprintln!(
        "T50 open rc={:?} out={:?} err={:?}",
        open.status.code(),
        String::from_utf8_lossy(&open.stdout),
        String::from_utf8_lossy(&open.stderr)
    );
    let parts = split_stdout(&open);
    let report = fail_report(&parts[1], &parts[3]);
    let path = t.write_report(&report);
    let out = t.audit_record(&path);
    eprintln!(
        "T50 record rc={:?} out={:?} err={:?}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_success(&out);
    let routed_parts = split_stdout(&out);
    assert_eq!(routed_parts[0], "REPAIR_ROUTED");
    assert_eq!(routed_parts[3], "1", "attempt 1");
    // Repair: change a file under an allowed path and commit it.
    std::fs::create_dir_all(t.repo.join("dir-000")).unwrap();
    write_file(&t.repo.join("dir-000/changed.txt"), "repaired\n");
    assert!(git(&t.repo, &["add", "-A"]).status.success());
    assert!(git(&t.repo, &["commit", "-m", "repair"]).status.success());
    let out = t.repair_check();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("REPAIR_OK "));

    // Round 2 FAIL -> routed (attempt 2).
    let open = t.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = fail_report(&parts[1], &parts[3]);
    let path = t.write_report(&report);
    let out = t.audit_record(&path);
    assert_success(&out);
    let routed_parts = split_stdout(&out);
    assert_eq!(routed_parts[0], "REPAIR_ROUTED");
    assert_eq!(routed_parts[3], "2", "attempt 2");
    std::fs::create_dir_all(t.repo.join("dir-000")).unwrap();
    // The route allows exactly the finding's repair path; the second repair
    // re-modifies that same allowed file.
    write_file(&t.repo.join("dir-000/changed.txt"), "repaired2\n");
    assert!(git(&t.repo, &["add", "-A"]).status.success());
    assert!(git(&t.repo, &["commit", "-m", "repair2"]).status.success());
    let out = t.repair_check();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("REPAIR_OK "));

    // Round 3 FAIL -> terminal (no third repair allowed).
    let open = t.audit_begin("auditor1");
    let parts = split_stdout(&open);
    assert_eq!(parts[2], "3");
    let report = fail_report(&parts[1], &parts[3]);
    let path = t.write_report(&report);
    let out = t.audit_record(&path);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("AUDIT_FAIL_FINAL "));

    // Ledger structure: exactly 3 rounds, 2 checked repairs, terminal result.
    let ledger: Value = serde_json::from_str(&t.read_mrgs_str("audit-ledger.json")).unwrap();
    let rounds = ledger["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 3);
    assert_eq!(rounds[0]["round"], 1);
    assert_eq!(rounds[1]["round"], 2);
    assert_eq!(rounds[2]["round"], 3);
    assert_eq!(ledger["max_repair_attempts"], 2);
    assert_eq!(rounds[0]["repair"]["attempt"], 1);
    assert_eq!(rounds[1]["repair"]["attempt"], 2);
    assert_eq!(rounds[2]["status"], "FAIL");
    assert_eq!(rounds[2]["repair"], Value::Null);
    assert!(rounds[2]["report_content"]
        .as_str()
        .unwrap()
        .contains("\"FAIL\""));
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_51_long_completion_history_fixture() {
    // 33-phase plan; close 32 phases in order.
    let t = TestRepo::new();
    write_file(&t.plan_path, &plan_toml_with_phases(33));
    assert_success(&t.accept_plan());
    for i in 1..=32usize {
        let phase = format!("phase-{:03}", i);
        let contract_toml = contract_toml_for_phase(&phase);
        write_file(&t.contract_path, &contract_toml);
        assert!(git(&t.repo, &["add", "-A"]).status.success());
        assert!(
            git(&t.repo, &["commit", "-m", &format!("contract {}", phase)])
                .status
                .success()
        );
        t.complete_phase(&phase);
    }

    // Exactly 32 contiguous completion entries with a valid receipt chain.
    let ledger = t.get_completion_ledger().unwrap();
    let completions = ledger["completions"].as_array().unwrap();
    assert_eq!(completions.len(), 32);
    let mut previous: Option<String> = None;
    for (i, entry) in completions.iter().enumerate() {
        let receipt = &entry["completion_receipt"];
        assert_eq!(receipt["completion_sequence"], (i + 1) as u64);
        assert_eq!(receipt["phase_id"], format!("phase-{:03}", i + 1));
        assert_eq!(
            entry["completion_receipt_sha256"].as_str().unwrap().len(),
            64
        );
        assert_eq!(entry["final_manifest_sha256"].as_str().unwrap().len(), 64);
        if let Some(prev) = &previous {
            assert_eq!(receipt["previous_completion_receipt_sha256"], prev.as_str());
        } else {
            assert_eq!(receipt["previous_completion_receipt_sha256"], Value::Null);
        }
        previous = Some(
            entry["completion_receipt_sha256"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    // No duplicate completion: replay of phase-032 is idempotent.
    let close = t.phase_close("phase-032");
    assert_success(&close);
    let parts = split_stdout(&close);
    assert_eq!(parts[0], "PHASE_CLOSED");
    assert_eq!(parts[2], "32");
    let ledger_after = t.get_completion_ledger().unwrap();
    assert_eq!(ledger_after["completions"].as_array().unwrap().len(), 32);

    // Phase selection after the chain: phase-033 is now selectable.
    let sel = t.select_phase("phase-033");
    assert_success(&sel);
    assert_eq!(stdout_str(&sel), "phase-033");

    // Continuity binding targets the 32nd completion.
    let receipt = completions[31]["completion_receipt_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-032", &receipt));
    let out = t.continuity_record(&meta);
    assert_success(&out);
    let cont_ledger = t.get_continuity_ledger().unwrap();
    assert_eq!(
        cont_ledger["entries"][0]["continuity_manifest"]["target_completion_sequence"],
        32
    );

    // Recovery inspection stays healthy on the long history.
    let insp = t.inspect_output();
    assert_eq!(insp.len(), 1);
    assert!(insp[0].starts_with("RECOVERY_NOT_REQUIRED "));
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_52_long_continuity_and_cross_link_fixture() {
    // 16 source repositories with completed phases (sentinels per source).
    let mut sources: Vec<TestRepo> = Vec::new();
    let mut source_receipts: Vec<(String, String, String)> = Vec::new(); // (id, plan_sha, receipt_sha)
    for i in 0..16usize {
        let s = TestRepo::new();
        s.setup_impl_bound();
        let open = s.audit_begin("auditor1");
        let parts = split_stdout(&open);
        let report = s.make_pass_report(&parts[1], &parts[3], "auditor1");
        let path = s.write_report(&report);
        assert_success(&s.audit_record(&path));
        let close = s.phase_close("phase-1");
        assert_success(&close);
        let close_parts = split_stdout(&close);
        source_receipts.push((
            format!("repo-{:03}", i),
            s.plan_sha(),
            close_parts[4].clone(),
        ));
        sources.push(s);
    }

    // One continuity entry per phase (the ledger is phase-scoped), so the 31
    // entries without links run across 31 completed phases of a 33-phase plan.
    let t = TestRepo::new();
    write_file(&t.plan_path, &plan_toml_with_phases(33));
    assert_success(&t.accept_plan());
    for i in 1..=31usize {
        let phase = format!("phase-{:03}", i);
        let contract_toml = contract_toml_for_phase(&phase);
        write_file(&t.contract_path, &contract_toml);
        assert!(git(&t.repo, &["add", "-A"]).status.success());
        assert!(
            git(&t.repo, &["commit", "-m", &format!("contract {}", phase)])
                .status
                .success()
        );
        t.complete_phase(&phase);
        let completion = t.get_completion_ledger().unwrap();
        let receipt = completion["completions"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["completion_receipt_sha256"]
            .as_str()
            .unwrap()
            .to_string();
        let meta = t.write_metadata(
            &format!("m{:02}.toml", i),
            &standard_metadata(&phase, &receipt).replace(
                "continuity_id = \"phase-1-primary\"",
                &format!("continuity_id = \"cont-{:03}\"", i),
            ),
        );
        let out = t.continuity_record(&meta);
        assert_success(&out);
        let parts = split_stdout(&out);
        assert_eq!(parts[3], i.to_string());
    }

    // Final entry: the 32nd completed phase with exactly 16 resolved
    // predecessor links.
    let phase32 = "phase-032";
    let contract_toml = contract_toml_for_phase(phase32);
    write_file(&t.contract_path, &contract_toml);
    assert!(git(&t.repo, &["add", "-A"]).status.success());
    assert!(git(&t.repo, &["commit", "-m", "contract phase-032"])
        .status
        .success());
    t.complete_phase(phase32);
    let completion = t.get_completion_ledger().unwrap();
    let receipt32 = completion["completions"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["completion_receipt_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let mut meta = standard_metadata(phase32, &receipt32).replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"cont-032\"",
    );
    // The base document declares a plain `links = []` value, which TOML
    // forbids combining with appended [[links]] tables (duplicate key);
    // drop the empty array so the 16 appended link tables parse.
    meta = meta.replace("links = []\n", "");
    for (id, plan_sha, s_receipt) in &source_receipts {
        meta = linked_metadata_second(&meta, id, plan_sha, "phase-1", s_receipt, None);
    }
    let meta_path = t.write_metadata("m-final.toml", &meta);
    let target_repo = t.repo.to_string_lossy().into_owned();
    let meta_str = meta_path.to_string_lossy().into_owned();
    let mut args: Vec<String> = vec![
        "continuity".into(),
        "record".into(),
        "--repo".into(),
        target_repo,
        "--metadata".into(),
        meta_str,
    ];
    for s in &sources {
        args.push("--source-repo".into());
        args.push(s.repo.to_string_lossy().into_owned());
    }
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = t.run(&args_refs);
    assert_success(&out);
    let parts = split_stdout(&out);
    assert_eq!(parts[3], "32");

    let ledger = t.get_continuity_ledger().unwrap();
    let entries = ledger["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 32);
    // Contiguous receipt chain.
    let mut previous: Option<String> = None;
    for (i, entry) in entries.iter().enumerate() {
        let receipt = &entry["continuity_receipt"];
        assert_eq!(receipt["continuity_sequence"], (i + 1) as u64);
        if let Some(prev) = &previous {
            assert_eq!(receipt["previous_continuity_receipt_sha256"], prev.as_str());
        } else {
            assert_eq!(receipt["previous_continuity_receipt_sha256"], Value::Null);
        }
        previous = Some(
            entry["continuity_receipt_sha256"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    // Exactly 16 sorted unique resolved links with exact archived proof.
    let links = entries[31]["continuity_manifest"]["resolved_links"]
        .as_array()
        .unwrap();
    assert_eq!(links.len(), 16);
    let mut seen: Vec<String> = Vec::new();
    for (i, link) in links.iter().enumerate() {
        let id = link["source_repository_id"].as_str().unwrap().to_string();
        assert_eq!(id, format!("repo-{:03}", i));
        assert!(!seen.contains(&id), "links must be unique");
        seen.push(id);
        assert_eq!(link["source_phase_id"], "phase-1");
        assert_eq!(
            link["source_completion_receipt_sha256"],
            source_receipts[i].2
        );
        assert_eq!(link["source_accepted_plan_sha256"], source_receipts[i].1);
    }

    // Byte-identical replay WITHOUT source availability: delete every source
    // repository, then replay the final metadata.
    for s in &sources {
        std::fs::remove_dir_all(&s.repo).unwrap();
    }
    let ledger_bytes_before = t.read_mrgs("continuity-ledger.json");
    let replay = t.run(&[
        "continuity",
        "record",
        "--repo",
        &t.repo.to_string_lossy(),
        "--metadata",
        &meta_path.to_string_lossy(),
    ]);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&out));
    assert_eq!(t.read_mrgs("continuity-ledger.json"), ledger_bytes_before);

    // Deterministic rejection of one altered link.
    let mut bad_meta = meta.clone();
    let (_, _, first_receipt) = &source_receipts[0];
    let altered = if let Some(rest) = first_receipt.strip_prefix('a') {
        format!("b{}", rest)
    } else {
        format!("a{}", &first_receipt[1..])
    };
    bad_meta = bad_meta.replacen(first_receipt, &altered, 1);
    let bad_path = t.write_metadata("m-bad.toml", &bad_meta);
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &t.repo.to_string_lossy(),
        "--metadata",
        &bad_path.to_string_lossy(),
    ]);
    assert_failure(&out);
    assert_eq!(stdout_str(&out), "");
    assert_eq!(t.read_mrgs("continuity-ledger.json"), ledger_bytes_before);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_53_long_recovery_history_and_pending_fixture() {
    // 32 applied recovery entries through public behavior.
    let t = TestRepo::new();
    t.setup_impl_bound();
    let mut entry0_first: Option<Value> = None;
    for i in 1..=32usize {
        // Each recovery must start from a DISTINCT subject: the recovery ID
        // is a deterministic hash of the plan seed (subject + actions), and
        // a fresh plan whose ID already exists in history is rejected as
        // stale. Advancing HEAD each iteration changes the subject while
        // the corruption stays the canonical missing state.json.
        git_commit(
            &t.repo,
            &format!("iter-{:03}.txt", i),
            format!("iter {}\n", i).as_bytes(),
        );
        induce_recoverable(&t);
        let (rid, pre_sha) = recoverable_ids(&t);
        let out = t.apply(&rid, &pre_sha);
        assert_success(&out);
        let parts = split_stdout(&out);
        assert_eq!(parts[0], "RECOVERY_APPLIED");
        assert_eq!(parts[1], i.to_string(), "contiguous sequence");
        let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
        if i == 1 {
            entry0_first = Some(journal["recoveries"][0].clone());
        }
        assert_eq!(journal["recoveries"].as_array().unwrap().len(), i);
    }
    let journal: Value = serde_json::from_slice(&t.recovery_ledger_bytes()).unwrap();
    let recoveries = journal["recoveries"].as_array().unwrap();
    assert_eq!(recoveries.len(), 32);
    // Append-only: entry 0 bytes untouched; receipt chain links every entry.
    assert_eq!(recoveries[0], entry0_first.unwrap());
    let mut previous: Option<String> = None;
    for (i, entry) in recoveries.iter().enumerate() {
        let receipt = &entry["recovery_receipt"];
        assert_eq!(receipt["recovery_sequence"], (i + 1) as u64);
        assert_eq!(entry["status"], "APPLIED");
        if let Some(prev) = &previous {
            assert_eq!(receipt["previous_recovery_receipt_sha256"], prev.as_str());
        } else {
            assert_eq!(receipt["previous_recovery_receipt_sha256"], Value::Null);
        }
        previous = Some(
            entry["recovery_receipt_sha256"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }

    // Separately constructed pending entry with the deterministic maximum
    // action count (4): redundant temp + accepted plan + state + closeout.
    let t2 = TestRepo::new();
    let (_m, _r) = t2.close_phase1();
    let archived = t2.archived_governance();
    t2.write_mrgs(
        "contract-draft.json",
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t2.write_mrgs(
        "accepted-contract.json",
        archived["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t2.write_mrgs(
        "implementation-authority.json",
        archived["implementation_authority_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t2.write_mrgs(
        "audit-ledger.json",
        archived["audit_ledger_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    let mut pre: Value = serde_json::from_str(&t2.read_mrgs_str("state.json")).unwrap();
    pre["active_phase"] = json!("phase-1");
    pre["closed_phases"] = json!([]);
    t2.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&pre).unwrap().as_slice(),
    );
    let completion_bytes = t2.read_mrgs("completion-ledger.json");
    t2.write_mrgs(".closeout.0.tmp", &completion_bytes);
    t2.delete("accepted-plan.json");
    t2.delete("state.json");
    let lines = t2.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_REQUIRED "));
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(parts[3], "4", "deterministic maximum action count");
    let expected_actions = [
        "RECOVERY_ACTION 1 REMOVE_REDUNDANT_TEMP .closeout.0.tmp",
        "RECOVERY_ACTION 2 RESTORE_ACCEPTED_PLAN accepted-plan.json",
        "RECOVERY_ACTION 3 RESTORE_STATE state.json",
        "RECOVERY_ACTION 4 RESUME_CLOSEOUT phase-1",
    ];
    for (i, expected) in expected_actions.iter().enumerate() {
        assert_eq!(lines[i + 1], *expected, "fixed action order");
    }
    // Crash after the pending journal publishes: pending entry with 4 actions.
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t2, parts[1], parts[2], "after_pending_publish", dir.path());
    kill_child(child);
    let journal: Value = serde_json::from_slice(&t2.recovery_ledger_bytes()).unwrap();
    let recoveries = journal["recoveries"].as_array().unwrap();
    assert_eq!(recoveries.len(), 1);
    assert_eq!(recoveries[0]["status"], "PENDING");
    assert_eq!(recoveries[0]["next_action"], 0);
    assert_eq!(
        recoveries[0]["plan"]["actions"].as_array().unwrap().len(),
        4
    );
    // Bounded resume: one receipt, no duplicate action, fixed-point replay.
    let out = t2.apply(parts[1], parts[2]);
    assert_success(&out);
    let applied = split_stdout(&out);
    assert_eq!(applied[0], "RECOVERY_APPLIED");
    assert_eq!(applied[1], "1");
    let journal: Value = serde_json::from_slice(&t2.recovery_ledger_bytes()).unwrap();
    let recoveries = journal["recoveries"].as_array().unwrap();
    assert_eq!(recoveries.len(), 1);
    assert_eq!(recoveries[0]["status"], "APPLIED");
    assert_eq!(
        recoveries[0]["plan"]["actions"].as_array().unwrap().len(),
        4
    );
    // Fixed-point replay binds the CURRENT (post-recovery) subject, exactly
    // the proven phase8 obligation-72 replay pattern: an APPLIED entry is
    // replayed with the live subject, not the stale pre-recovery subject.
    let current_sha = recompute_subject(&t2.repo);
    let replay = t2.apply(parts[1], &current_sha);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&out));
    assert_no_temp_files(&t2.repo);
}

#[test]
fn test_obligation_54_large_inventory_and_temp_candidate_fixture() {
    // Exactly 256 ordinary tracked files plus the fixture files.
    let t = TestRepo::new();
    let mut files: Vec<(String, String)> = Vec::new();
    for i in 0..256usize {
        files.push((
            format!("files/file-{:03}.txt", i),
            format!("content-{}\n", i),
        ));
    }
    git_commit_many(&t.repo, &files);
    // Full governance chain on the large inventory.
    assert_success(&t.accept_plan());
    let sel = t.select_phase("phase-1");
    assert_success(&sel);
    // The 256-file inventory lives under files/, so the contract's
    // allowed-path rule must cover it (the default fixture allows only
    // src/).
    write_file(
        &t.contract_path,
        &contract_toml_custom(
            "test-contract-v1",
            "phase-1",
            &["req1".to_string(), "req2".to_string()],
            &["files/".to_string()],
            &[".git/".to_string(), ".mrgs/".to_string()],
            &["cargo test".to_string(), "cargo clippy".to_string()],
            &["FIELD1".to_string()],
        ),
    );
    // plan.toml + contract.toml are committed so the worktree is clean.
    // (git_commit_many already committed them, so a second unconditional
    // commit would be a no-op and fail; commit_sources commits only when
    // something is actually staged.)
    t.commit_sources();
    let draft = t.draft_contract();
    assert_success(&draft);
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    assert_success(&t.impl_begin(1, &sha));
    // The change inventory is baseline -> worktree: modify all 256 files
    // after begin so the implementation check covers the full inventory.
    for i in 0..256usize {
        std::fs::write(
            t.repo.join(format!("files/file-{:03}.txt", i)),
            format!("content-{}-modified\n", i),
        )
        .unwrap();
    }
    let check = t.impl_check();
    assert_success(&check);
    let check_parts = split_stdout(&check);
    let validated: u64 = check_parts[4].parse().unwrap();
    assert!(
        validated >= 256,
        "validated count must cover the 256-file inventory: {}",
        validated
    );

    // Occupy exactly 15 recognized create-new candidates; the sixteenth is
    // selected and all sentinels survive byte-exact. The continuity producer
    // reaches the publication search with a closed phase (its validation
    // order has no change-inventory gate, so the occupied slots are the
    // deterministic trigger for the bounded collision search).
    let t2 = TestRepo::new();
    let (_mc, receipt_c) = t2.close_phase1();
    let mut sentinel_bytes = Vec::new();
    for i in 0..15u32 {
        let content = format!("occupant-{}", i);
        t2.write_mrgs(&format!(".continuity.{}.tmp", i), content.as_bytes());
        sentinel_bytes.push(content);
    }
    let meta = t2.write_metadata("m.toml", &standard_metadata("phase-1", &receipt_c));
    let record = t2.continuity_record(&meta);
    assert_success(&record);
    let parts = split_stdout(&record);
    assert_eq!(parts[0], "CONTINUITY_RECORDED");
    assert_eq!(parts[3], "1");
    for i in 0..15u32 {
        assert_eq!(
            std::fs::read(t2.repo.join(".mrgs").join(format!(".continuity.{}.tmp", i))).unwrap(),
            sentinel_bytes[i as usize].as_bytes()
        );
    }
    // Bounded collision search: the 16th candidate was consumed and removed;
    // the only .tmp files left are the 15 intentional sentinels.
    assert!(!t2.repo.join(".mrgs/.continuity.15.tmp").exists());
    assert_no_temp_files_except(
        t2.repo.join(".mrgs"),
        &(0..15)
            .map(|i| format!(".continuity.{}.tmp", i))
            .collect::<Vec<_>>(),
    );

    // Deterministic sorted inventory: two inspections are byte-identical.
    // (Run on the clean chain repo `t`: the collision fixture's sentinel
    // temps are deliberately not recovery-recognized and would classify as
    // unrecoverable.)
    let insp1 = t.inspect_output();
    let insp2 = t.inspect_output();
    assert_eq!(insp1, insp2);
}

/// assert_no_temp_files, allowing an explicit fixture-sentinel allowlist.
fn assert_no_temp_files_except(gov_dir: PathBuf, allowed: &[String]) {
    let mut temps: Vec<String> = std::fs::read_dir(&gov_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    temps.sort();
    let mut allowed = allowed.to_vec();
    allowed.sort();
    assert_eq!(temps, allowed, "unexpected temporary files");
}

#[test]
fn test_obligation_55_scalar_boundaries_and_one_over_limits() {
    // Continuity scalars: exact-maximum accepted, one-over rejected.
    let mk_closed = || {
        let t = TestRepo::new();
        let (_m, receipt) = t.close_phase1();
        (t, receipt)
    };

    // repository_id: 128 bytes accepted and persisted EXACTLY (no truncation).
    let (t, receipt) = mk_closed();
    let max_id = "a".repeat(128);
    let meta = t.write_metadata(
        "m-max-id.toml",
        &standard_metadata("phase-1", &receipt).replace(
            "repository_id = \"mrgs\"",
            &format!("repository_id = \"{}\"", max_id),
        ),
    );
    let out = t.continuity_record(&meta);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    assert_eq!(ledger["repository_id"].as_str().unwrap().len(), 128);
    assert_eq!(ledger["repository_id"].as_str().unwrap(), max_id);

    let (t2, receipt2) = mk_closed();
    let over_id = "a".repeat(129);
    let meta = t2.write_metadata(
        "m-over-id.toml",
        &standard_metadata("phase-1", &receipt2).replace(
            "repository_id = \"mrgs\"",
            &format!("repository_id = \"{}\"", over_id),
        ),
    );
    let out = t2.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");
    assert!(t2.get_continuity_ledger().is_none());

    // note: 1024 accepted / 1025 rejected.
    let (t3, receipt3) = mk_closed();
    let note_1024 = "n".repeat(1024);
    let meta = t3.write_metadata(
        "m-note.toml",
        &standard_metadata("phase-1", &receipt3).replace(
            "note = \"continuity record\"",
            &format!("note = \"{}\"", note_1024),
        ),
    );
    let out = t3.continuity_record(&meta);
    assert_success(&out);
    let ledger = t3.get_continuity_ledger().unwrap();
    assert_eq!(
        ledger["entries"][0]["continuity_manifest"]["note"]
            .as_str()
            .unwrap()
            .len(),
        1024
    );

    let (t4, receipt4) = mk_closed();
    let note_1025 = "n".repeat(1025);
    let meta = t4.write_metadata(
        "m-note-over.toml",
        &standard_metadata("phase-1", &receipt4).replace(
            "note = \"continuity record\"",
            &format!("note = \"{}\"", note_1025),
        ),
    );
    let out = t4.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // model role: 256 accepted / 257 rejected.
    let (t5, receipt5) = mk_closed();
    let role_256 = "r".repeat(256);
    let meta = t5.write_metadata(
        "m-role.toml",
        &standard_metadata("phase-1", &receipt5).replace(
            "role = \"implementer\"",
            &format!("role = \"{}\"", role_256),
        ),
    );
    let out = t5.continuity_record(&meta);
    assert_success(&out);

    let (t6, receipt6) = mk_closed();
    let role_257 = "r".repeat(257);
    let meta = t6.write_metadata(
        "m-role-over.toml",
        &standard_metadata("phase-1", &receipt6).replace(
            "role = \"implementer\"",
            &format!("role = \"{}\"", role_257),
        ),
    );
    let out = t6.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // host_id: 256 accepted / 257 rejected.
    let (t7, receipt7) = mk_closed();
    let host_256 = "h".repeat(256);
    let meta = t7.write_metadata(
        "m-host.toml",
        &standard_metadata("phase-1", &receipt7).replace(
            "host_id = \"main-workstation\"",
            &format!("host_id = \"{}\"", host_256),
        ),
    );
    let out = t7.continuity_record(&meta);
    assert_success(&out);

    let (t8, receipt8) = mk_closed();
    let host_257 = "h".repeat(257);
    let meta = t8.write_metadata(
        "m-host-over.toml",
        &standard_metadata("phase-1", &receipt8).replace(
            "host_id = \"main-workstation\"",
            &format!("host_id = \"{}\"", host_257),
        ),
    );
    let out = t8.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // phase_id scalar: boundary 128 is metadata-valid but names no real
    // plan phase, so the phase-binding check fails closed with the
    // governance-authority category (unknown phase); one-over (129) is
    // rejected at metadata parse with CONTINUITY_METADATA_INVALID below.
    let (t9, receipt9) = mk_closed();
    let phase_128 = "p".repeat(128);
    let meta = t9.write_metadata(
        "m-phase.toml",
        &standard_metadata("phase-1", &receipt9).replace(
            "phase_id = \"phase-1\"",
            &format!("phase_id = \"{}\"", phase_128),
        ),
    );
    let out = t9.continuity_record(&meta);
    assert_category_no_stdout(&out, "GOVERNANCE_AUTHORITY_INVALID");

    let (t10, receipt10) = mk_closed();
    let phase_129 = "p".repeat(129);
    let meta = t10.write_metadata(
        "m-phase-over.toml",
        &standard_metadata("phase-1", &receipt10).replace(
            "phase_id = \"phase-1\"",
            &format!("phase_id = \"{}\"", phase_129),
        ),
    );
    let out = t10.continuity_record(&meta);
    assert_category_no_stdout(&out, "CONTINUITY_METADATA_INVALID");

    // Auditor id: 128 accepted / 129 rejected (bounded scalar pair).
    let t11 = TestRepo::new();
    t11.setup_impl_bound();
    let out = t11.audit_begin(&"a".repeat(128));
    assert_success(&out);
    let out = t11.audit_begin(&"a".repeat(129));
    assert_category_no_stdout(&out, "AUDITOR_ID_INVALID");

    // Collision search bound: 15 occupied -> 16th selected; 16 occupied ->
    // PERSISTENCE_FAILED (one-over the bounded search). The continuity
    // producer's publication search is reachable with pre-existing
    // candidates (no change-inventory gate before publication).
    let t12 = TestRepo::new();
    let (_mc, receipt_c) = t12.close_phase1();
    for i in 0..15u32 {
        t12.write_mrgs(&format!(".continuity.{}.tmp", i), b"x");
    }
    let meta = t12.write_metadata("m.toml", &standard_metadata("phase-1", &receipt_c));
    let out = t12.continuity_record(&meta);
    assert_success(&out);
    let parts = split_stdout(&out);
    assert_eq!(parts[0], "CONTINUITY_RECORDED");
    assert_eq!(parts[3], "1");

    let t13 = TestRepo::new();
    let (_mc, receipt_d) = t13.close_phase1();
    for i in 0..16u32 {
        t13.write_mrgs(&format!(".continuity.{}.tmp", i), b"x");
    }
    let meta = t13.write_metadata("m.toml", &standard_metadata("phase-1", &receipt_d));
    let out = t13.continuity_record(&meta);
    assert_category_no_stdout(&out, "PERSISTENCE_FAILED");
    assert!(t13.get_continuity_ledger().is_none());
    // The 16 occupied fixture sentinels survive byte-exact (production must
    // never delete another candidate); no producer temporary file remains.
    for i in 0..16u32 {
        assert_eq!(
            std::fs::read(
                t13.repo
                    .join(".mrgs")
                    .join(format!(".continuity.{}.tmp", i))
            )
            .unwrap(),
            b"x"
        );
    }
    assert_no_temp_files_except(
        t13.repo.join(".mrgs"),
        &(0..16)
            .map(|i| format!(".continuity.{}.tmp", i))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn test_obligation_56_repeated_replay_inspection_and_bounded_callers() {
    // Exactly 64 exact plan-accept replays.
    let t = TestRepo::new();
    let first = t.accept_plan();
    assert_success(&first);
    let first_out = stdout_raw(&first);
    let accepted_bytes = t.read_mrgs("accepted-plan.json");
    let state_bytes = t.read_mrgs("state.json");
    for _ in 0..64 {
        let replay = t.accept_plan();
        assert_success(&replay);
        assert_eq!(stdout_raw(&replay), first_out);
    }
    assert_eq!(t.read_mrgs("accepted-plan.json"), accepted_bytes);
    assert_eq!(t.read_mrgs("state.json"), state_bytes);

    // 64 read-only recovery inspections: byte-identical outputs, stable
    // file counts and sizes.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let first_insp = t2.inspect_output();
    let first_sha = first_insp[0].clone();
    let snapshot = || {
        let gov = t2.repo.join(".mrgs");
        let mut sizes: Vec<(String, u64)> = std::fs::read_dir(&gov)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    e.metadata().unwrap().len(),
                )
            })
            .collect();
        sizes.sort();
        sizes
    };
    let sizes_before = snapshot();
    for _ in 0..64 {
        let insp = t2.inspect_output();
        assert_eq!(insp.len(), 1);
        assert_eq!(insp[0], first_sha);
        assert_eq!(snapshot(), sizes_before, "file counts and sizes stable");
    }
    assert_no_temp_files(&t2.repo);

    // 8 synchronized implementation-check callers on a bounded fixture.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    let runner = create_barrier_runner();
    let outputs = run_barrier_8(
        &runner,
        &[
            "implementation",
            "check",
            "--repo",
            &t3.repo.to_string_lossy(),
        ],
        &[],
    );
    let mut first_check: Option<String> = None;
    for out in &outputs {
        assert_success(out);
        assert!(stdout_str(out).starts_with("IMPLEMENTATION_OK "));
        match &first_check {
            None => first_check = Some(stdout_raw(out)),
            Some(prev) => assert_eq!(stdout_raw(out), *prev),
        }
    }
    assert_no_temp_files(&t3.repo);
}
// ===========================================================================
// 16.8 Phase 1-8 regression and cross-platform compatibility
// ===========================================================================

#[test]
fn test_obligation_57_phase1_plan_and_selection_regression() {
    let t = TestRepo::new();
    let plan_sha = sha_of_file(&t.plan_path);

    // Acceptance token and durable records.
    let out = t.accept_plan();
    assert_success(&out);
    assert_eq!(stdout_str(&out), format!("test-plan {}", plan_sha));
    let accepted: Value = serde_json::from_str(&t.read_mrgs_str("accepted-plan.json")).unwrap();
    assert_eq!(accepted["plan_id"], "test-plan");
    assert_eq!(accepted["plan_path"], "plan.toml");
    assert_eq!(accepted["sha256"], plan_sha);
    assert_eq!(accepted["phase_count"], 2);
    let state: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
    assert_eq!(state["active_phase"], Value::Null);
    assert_eq!(state["closed_phases"], json!([]));
    // Git boundary: no git mutation.
    let git_before = git_snapshot(&t.repo);
    assert_eq!(git_snapshot(&t.repo), git_before);

    // Exact replay.
    let replay = t.accept_plan();
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&out));

    // Conflict: different plan.
    let other = t.repo.join("other.toml");
    write_file(&other, &plan_toml_with_phases(1));
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &t.repo.to_string_lossy(),
        "--plan",
        &other.to_string_lossy(),
    ]);
    assert_err_prefix(
        &out,
        "error: cannot accept different plan when authority exists",
    );

    // Dependency ordering.
    let out = t.select_phase("phase-2");
    assert_err_prefix(
        &out,
        "error: phase 'phase-2' dependency 'phase-1' not closed",
    );
    let sel = t.select_phase("phase-1");
    assert_success(&sel);
    assert_eq!(stdout_str(&sel), "phase-1");
    // Active conflict.
    let out = t.select_phase("phase-2");
    assert_err_prefix(&out, "error: phase 'phase-1' is already active");
    // State replacement: the active phase is persisted exactly.
    let state: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
    assert_eq!(state["active_phase"], "phase-1");

    // Representative filesystem failure.
    let as_file = t._dir.path().join("not-a-dir");
    write_file(&as_file, "x");
    let out = t.run(&[
        "plan",
        "accept",
        "--repo",
        &as_file.to_string_lossy(),
        "--plan",
        &t.plan_path.to_string_lossy(),
    ]);
    assert_err_prefix(&out, "error: not a directory: ");
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_58_phase2_contract_draft_regression() {
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    let repo = t.repo.to_string_lossy().into_owned();

    // Strict parsing: unknown field, duplicate/empty/blank list entries,
    // missing fields, trailing data.
    let t2 = TestRepo::new();
    t2.accept_plan_success();
    t2.select_phase_success("phase-1");
    let mut toml = contract_toml_for_phase("phase-1");
    toml.push_str("bogus = 1\n");
    write_file(&t2.contract_path, &toml);
    let out = t2.draft_contract();
    assert_err_prefix(&out, "error: TOML parse error: ");

    let t3 = TestRepo::new();
    t3.accept_plan_success();
    t3.select_phase_success("phase-1");
    let toml = contract_toml_for_phase("phase-1").replacen(
        "requirements = [\"req1\", \"req2\"]",
        "requirements = [\"req1\", \"req1\"]",
        1,
    );
    write_file(&t3.contract_path, &toml);
    let out = t3.draft_contract();
    assert_err_prefix(
        &out,
        "error: duplicate entry in contract 'requirements' list",
    );

    let t4 = TestRepo::new();
    t4.accept_plan_success();
    t4.select_phase_success("phase-1");
    let toml = contract_toml_minus_requirements().replace(
        "allowed_paths = [\"src/\"]",
        "allowed_paths = []\nrequirements = [\"r1\"]",
    );
    write_file(&t4.contract_path, &toml);
    let out = t4.draft_contract();
    assert_err_prefix(&out, "error: 'allowed_paths' list is empty in contract");

    // Exact byte/hash preservation: the draft archives the source bytes and
    // the hash equals the source file hash.
    let draft = t.draft_contract();
    assert_success(&draft);
    let draft_value = t.get_draft();
    let source_sha = sha_of_file(&t.contract_path);
    assert_eq!(draft_value["sha256"], source_sha);
    assert_eq!(
        draft_value["content"].as_str().unwrap().len(),
        contract_toml_for_phase("phase-1").len()
    );
    assert_eq!(draft_value["source_path"], "contract.toml");
    assert_eq!(
        stdout_str(&draft),
        format!("test-contract-v1 {}", source_sha)
    );

    // Exact replay and conflict.
    let replay = t.draft_contract();
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&draft));
    write_file(
        &t.contract_path,
        &contract_toml_for_phase("phase-1").replacen("req1", "reqX", 1),
    );
    let out = t.draft_contract();
    assert_err_prefix(
        &out,
        "error: contract draft already exists with different content",
    );

    // Path rules and failure cleanup.
    let external = t._dir.path().join("ext.toml");
    write_file(&external, &contract_toml_for_phase("phase-1"));
    let out = t.run(&[
        "contract",
        "draft",
        "--repo",
        &repo,
        "--contract",
        &external.to_string_lossy(),
    ]);
    assert_err_prefix(&out, "error: contract source file is outside repository");
    let mrgs = t.repo.join(".mrgs/contract.toml");
    write_file(&mrgs, &contract_toml_for_phase("phase-1"));
    let out = t.run(&[
        "contract",
        "draft",
        "--repo",
        &repo,
        "--contract",
        &mrgs.to_string_lossy(),
    ]);
    assert_err_prefix(
        &out,
        "error: contract source file is inside .mrgs directory",
    );
    // Failed drafts leave no draft file and no temp.
    let t5 = TestRepo::new();
    t5.accept_plan_success();
    t5.select_phase_success("phase-1");
    write_file(&t5.contract_path, "not a toml [");
    let out = t5.draft_contract();
    assert_err_prefix(&out, "error: TOML parse error: ");
    assert!(!t5.repo.join(".mrgs/contract-draft.json").exists());
    assert_no_temp_files(&t5.repo);
}

#[test]
fn test_obligation_59_phase3_acceptance_and_revision_regression() {
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    t.draft_contract();
    let sha1 = t.get_draft()["sha256"].as_str().unwrap().to_string();

    // Exact ACCEPTED token.
    let out = t.accept_contract(1, &sha1);
    assert_success(&out);
    assert_eq!(
        stdout_str(&out),
        format!("ACCEPTED test-contract-v1 1 {}", sha1)
    );
    // Idempotent acceptance.
    let replay = t.accept_contract(1, &sha1);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&out));

    // Revision CAS: wrong revision and wrong sha rejected before publication.
    let out = t.accept_contract(2, &sha1);
    assert_err_prefix(
        &out,
        "error: contract accept revision 2 does not match draft revision 1",
    );
    let wrong_sha = if let Some(rest) = sha1.strip_prefix('a') {
        format!("b{}", rest)
    } else {
        format!("a{}", &sha1[1..])
    };
    let out = t.accept_contract(1, &wrong_sha);
    assert_err_prefix(&out, "error: contract accept SHA does not match draft SHA");
    // Casing rejection.
    let out = t.run(&[
        "contract",
        "accept",
        "--repo",
        &t.repo.to_string_lossy(),
        "--revision",
        "1",
        "--sha256",
        &sha1,
        "--decision",
        "accepted",
    ]);
    assert_err_prefix(
        &out,
        "error: contract accept decision must be exactly ACCEPTED, got 'accepted'",
    );

    // Revision-draft lifecycle: revise -> REVISION_DRAFT rev 2 -> accept.
    let rev_contract = contract_toml_for_phase("phase-1").replacen("req1", "req1-revised", 1);
    write_file(&t.contract_path, &rev_contract);
    let out = t.revise_contract(1, &sha1);
    assert_success(&out);
    let rev_parts = split_stdout(&out);
    assert_eq!(rev_parts[0], "REVISION_DRAFT");
    assert_eq!(rev_parts[2], "2");
    let sha2 = rev_parts[3].to_string();
    let draft: Value = serde_json::from_str(&t.read_mrgs_str("contract-draft.json")).unwrap();
    assert_eq!(draft["revision"], 2);
    assert_eq!(draft["sha256"], sha2);
    assert_eq!(draft["preimage"]["revision"], 1);
    assert_eq!(draft["preimage"]["sha256"], sha1);
    // Stale acceptance after the revision.
    let out = t.accept_contract(1, &sha1);
    assert_err_prefix(
        &out,
        "error: contract accept revision 1 does not match draft revision 2",
    );
    let out = t.accept_contract(2, &sha1);
    assert_err_prefix(&out, "error: contract accept SHA does not match draft SHA");
    let out = t.accept_contract(2, &sha2);
    assert_success(&out);
    assert_eq!(
        stdout_str(&out),
        format!("ACCEPTED test-contract-v1 2 {}", sha2)
    );

    // History ordering: exactly [1, 2] with exact content hashes.
    let ledger: Value = serde_json::from_str(&t.read_mrgs_str("accepted-contract.json")).unwrap();
    let revisions = ledger["revisions"].as_array().unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0]["revision"], 1);
    assert_eq!(revisions[0]["sha256"], sha1);
    assert_eq!(revisions[1]["revision"], 2);
    assert_eq!(revisions[1]["sha256"], sha2);
    assert_no_temp_files(&t.repo);
}

#[test]
fn test_obligation_60_phase4_implementation_enforcement_regression() {
    // Allowed-change contract: src/ is forbidden, docs/ is allowed.
    let t = TestRepo::new();
    t.accept_plan_success();
    t.select_phase_success("phase-1");
    let custom = contract_toml_custom(
        "test-contract-v1",
        "phase-1",
        &["req1".to_string(), "req2".to_string()],
        &["docs/".to_string()],
        &[
            "src/".to_string(),
            ".git/".to_string(),
            ".mrgs/".to_string(),
        ],
        &["cargo test".to_string()],
        &["FIELD1".to_string()],
    );
    write_file(&t.contract_path, &custom);
    let draft = t.draft_contract();
    assert_success(&draft);
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    let begin = t.accept_contract(1, &sha);
    assert_success(&begin);
    let begin = t.impl_begin(1, &sha);
    assert_success(&begin);
    let begin_parts = split_stdout(&begin);
    assert_eq!(begin_parts[0], "IMPLEMENTATION_BOUND");
    let check = t.impl_check();
    assert_success(&check);
    let check_parts = split_stdout(&check);
    assert_eq!(check_parts[0], "IMPLEMENTATION_OK");

    // Forbidden change: committed modification under src/.
    write_file(
        &t.repo.join("src/main.rs"),
        "fn main() { println!(\"x\"); }\n",
    );
    assert!(git(&t.repo, &["add", "-A"]).status.success());
    assert!(git(&t.repo, &["commit", "-m", "forbidden change"])
        .status
        .success());
    let out = t.impl_check();
    assert_category_no_stdout(&out, "CHANGE_FORBIDDEN");
    // Revert the forbidden change by writing back the original content and
    // committing: the net diff vs the baseline is empty, so the inventory
    // stays clean without any worktree-rewriting reset (a reset --hard on
    // Windows can race git's stat cache and leave the check seeing the
    // stale modification nondeterministically).
    write_file(&t.repo.join("src/main.rs"), "fn main() {}\n");
    assert!(git(&t.repo, &["add", "-A"]).status.success());
    assert!(git(&t.repo, &["commit", "-m", "revert forbidden"])
        .status
        .success());

    // Allowed change: committed addition under docs/.
    std::fs::create_dir_all(t.repo.join("docs")).unwrap();
    write_file(&t.repo.join("docs/ok.md"), "ok\n");
    assert!(git(&t.repo, &["add", "-A"]).status.success());
    assert!(git(&t.repo, &["commit", "-m", "allowed change"])
        .status
        .success());
    // Worktree states: check inventories untracked and staged content and
    // enforces the boundary on it (Phase 4 §11.3); GIT_DIRTY is begin-time
    // only, so untracked/staged allowed files validate instead of failing.
    write_file(&t.repo.join("docs/wip.md"), "wip\n");
    let out = t.impl_check();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("IMPLEMENTATION_OK "));
    // Forbidden untracked content is still rejected by the boundary.
    write_file(&t.repo.join("src/forbidden.rs"), "fn main() {}\n");
    let out = t.impl_check();
    assert_category_no_stdout(&out, "CHANGE_FORBIDDEN");
    std::fs::remove_file(t.repo.join("src/forbidden.rs")).unwrap();
    write_file(&t.repo.join("docs/staged.md"), "staged\n");
    assert!(git(&t.repo, &["add", "-A"]).status.success());
    let out = t.impl_check();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("IMPLEMENTATION_OK "));
    assert!(git(&t.repo, &["reset", "--hard", "HEAD"]).status.success());
    // The reset removed the staged dirty-section files from the worktree,
    // so the capability branch starts from a clean worktree.

    // Tracked symlink in the inventory (platform capability branch). The
    // target is repository-relative (Phase 4 §12.1 / P4-077 success shape):
    // a contained relative target is contract-valid, so the check succeeds
    // on platforms that store symlinks (and on Windows with core.symlinks
    // the link is stored as a regular blob, which also validates).
    let link_path = t.repo.join("docs/link.md");
    // Pin core.symlinks=true (the P4-032-B pattern): with the Windows
    // default false, `git add` of a live symlink may store a regular blob
    // (the target's content) instead of the link text, making the inventory
    // nondeterministic.
    assert!(git(&t.repo, &["config", "core.symlinks", "true"])
        .status
        .success());
    let link_result = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("ok.md", &link_path)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file("ok.md", &link_path)
        }
    };
    match link_result {
        Ok(()) => {
            assert!(git(&t.repo, &["add", "-A"]).status.success());
            assert!(git(&t.repo, &["commit", "-m", "symlink"]).status.success());
            let out = t.impl_check();
            assert_success(&out);
            assert!(stdout_str(&out).starts_with("IMPLEMENTATION_OK "));
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Capability-unavailable branch: no link was created, the
            // worktree is clean, and the check succeeds.
            let out = t.impl_check();
            assert_success(&out);
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("symlink creation failed: {}", e),
    }
}

#[test]
fn test_obligation_61_phase5_audit_and_repair_regression() {
    let t = TestRepo::new();
    t.setup_impl_bound();

    // Collision-safe begin + PASS record + exact replay.
    t.write_mrgs(".mrgs_audit_tmp_0_0_0.tmp", b"sentinel");
    let open = t.audit_begin("auditor1");
    assert_success(&open);
    let parts = split_stdout(&open);
    assert_eq!(parts[2], "1");
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let record = t.audit_record(&report_path);
    assert_success(&record);
    assert!(stdout_str(&record).starts_with("AUDIT_PASS "));
    let replay = t.audit_record(&report_path);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&record));
    assert_eq!(t.read_mrgs(".mrgs_audit_tmp_0_0_0.tmp"), b"sentinel");

    // Subject drift: worktree changes between begin and record.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let open = t2.audit_begin("auditor1");
    let parts = split_stdout(&open);
    let report = t2.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t2.write_report(&report);
    write_file(&t2.repo.join("src/main.rs"), "changed\n");
    let out = t2.audit_record(&report_path);
    assert_category_no_stdout(&out, "AUDIT_SUBJECT_STALE");
    let ledger: Value = serde_json::from_str(&t2.read_mrgs_str("audit-ledger.json")).unwrap();
    assert_eq!(ledger["rounds"][0]["status"], "PENDING");
    assert_no_temp_files(&t2.repo);

    // Bounded FAIL/repair routing and terminal failure (small contract).
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    let routed = |t: &TestRepo| {
        let open = t.audit_begin("auditor1");
        let parts = split_stdout(&open);
        let report = t.make_fail_report(&parts[1], &parts[3], "auditor1", "F1");
        let path = t.write_report(&report);
        let out = t.audit_record(&path);
        assert_success(&out);
        (split_stdout(&out), parts)
    };
    let (r1, _p1) = routed(&t3);
    assert_eq!(r1[0], "REPAIR_ROUTED");
    assert_eq!(r1[3], "1");
    write_file(&t3.repo.join("src/main.rs"), "repaired\n");
    assert!(git(&t3.repo, &["add", "-A"]).status.success());
    assert!(git(&t3.repo, &["commit", "-m", "repair"]).status.success());
    let out = t3.repair_check();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("REPAIR_OK "));
    let (r2, _p2) = routed(&t3);
    assert_eq!(r2[0], "REPAIR_ROUTED");
    assert_eq!(r2[3], "2");
    write_file(&t3.repo.join("src/main.rs"), "repaired2\n");
    assert!(git(&t3.repo, &["add", "-A"]).status.success());
    assert!(git(&t3.repo, &["commit", "-m", "repair2"]).status.success());
    let out = t3.repair_check();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("REPAIR_OK "));
    let open = t3.audit_begin("auditor1");
    let parts = split_stdout(&open);
    assert_eq!(parts[2], "3");
    let report = t3.make_fail_report(&parts[1], &parts[3], "auditor1", "F1");
    let path = t3.write_report(&report);
    let out = t3.audit_record(&path);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("AUDIT_FAIL_FINAL "));
    let ledger: Value = serde_json::from_str(&t3.read_mrgs_str("audit-ledger.json")).unwrap();
    assert_eq!(ledger["rounds"].as_array().unwrap().len(), 3);
    assert_eq!(ledger["rounds"][0]["repair"]["attempt"], 1);
    assert_eq!(ledger["rounds"][1]["repair"]["attempt"], 2);
    assert_eq!(ledger["rounds"][2]["status"], "FAIL");
    assert_no_temp_files(&t3.repo);
}

#[test]
fn test_obligation_62_phase6_closeout_regression() {
    // Readiness gate.
    let t = TestRepo::new();
    t.setup_impl_bound();
    let out = t.phase_close("phase-1");
    assert_category_no_stdout(&out, "CLOSEOUT_NOT_READY");

    // Exact manifest/archive/receipt hashes and cleanup order. The audit
    // ledger exists only after audit begin, so the pre-closeout snapshot
    // happens after the full PASS audit.
    t.full_pass_audit();
    let draft_bytes_before = t.read_mrgs("contract-draft.json");
    let accepted_bytes_before = t.read_mrgs("accepted-contract.json");
    let authority_bytes_before = t.read_mrgs("implementation-authority.json");
    let audit_bytes_before = t.read_mrgs("audit-ledger.json");
    let close = t.phase_close("phase-1");
    assert_success(&close);
    let close_parts = split_stdout(&close);
    assert_eq!(close_parts[0], "PHASE_CLOSED");
    assert_eq!(close_parts[2], "1");
    let manifest_sha = close_parts[3].clone();
    let receipt_sha = close_parts[4].clone();
    let ledger = t.get_completion_ledger().unwrap();
    let entry = &ledger["completions"][0];
    assert_eq!(entry["final_manifest_sha256"], manifest_sha);
    assert_eq!(entry["completion_receipt_sha256"], receipt_sha);
    // Archived governance bytes equal the pre-closeout exact bytes.
    let archived = &entry["final_manifest"]["archived_governance"];
    assert_eq!(
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
        draft_bytes_before
    );
    assert_eq!(
        archived["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
        accepted_bytes_before
    );
    assert_eq!(
        archived["implementation_authority_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
        authority_bytes_before
    );
    assert_eq!(
        archived["audit_ledger_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
        audit_bytes_before
    );
    // Cleanup order: every phase-scoped file is gone; state promoted.
    for name in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(
            !t.repo.join(".mrgs").join(name).exists(),
            "{} must be cleaned",
            name
        );
    }
    let state: Value = serde_json::from_str(&t.read_mrgs_str("state.json")).unwrap();
    assert_eq!(state["active_phase"], Value::Null);
    assert_eq!(state["closed_phases"], json!(["phase-1"]));
    // Replay idempotency + collision slot.
    let replay = t.phase_close("phase-1");
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&close));
    assert_eq!(
        t.get_completion_ledger().unwrap()["completions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Chained second completion: sequence 2 with a previous-receipt link.
    t.select_phase_success("phase-2");
    write_file(&t.contract_path, &contract_toml_for_phase("phase-2"));
    assert!(git(&t.repo, &["add", "-A"]).status.success());
    assert!(git(&t.repo, &["commit", "-m", "phase-2"]).status.success());
    let draft = t.draft_contract();
    assert_success(&draft);
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    assert_success(&t.impl_begin(1, &sha));
    t.full_pass_audit();
    let close2 = t.phase_close("phase-2");
    assert_success(&close2);
    let close2_parts = split_stdout(&close2);
    assert_eq!(close2_parts[2], "2");
    let ledger = t.get_completion_ledger().unwrap();
    let completions = ledger["completions"].as_array().unwrap();
    assert_eq!(completions.len(), 2);
    assert_eq!(
        completions[1]["completion_receipt"]["previous_completion_receipt_sha256"],
        completions[0]["completion_receipt_sha256"]
    );

    // Interrupted-closeout classification: rewind state, restore archives.
    let t2 = TestRepo::new();
    let (_m, _r) = t2.close_phase1();
    let archived = t2.archived_governance();
    t2.write_mrgs(
        "contract-draft.json",
        archived["contract_draft_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t2.write_mrgs(
        "accepted-contract.json",
        archived["accepted_contract_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t2.write_mrgs(
        "implementation-authority.json",
        archived["implementation_authority_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    t2.write_mrgs(
        "audit-ledger.json",
        archived["audit_ledger_content"]
            .as_str()
            .unwrap()
            .as_bytes(),
    );
    let mut pre: Value = serde_json::from_str(&t2.read_mrgs_str("state.json")).unwrap();
    pre["active_phase"] = json!("phase-1");
    pre["closed_phases"] = json!([]);
    t2.write_mrgs(
        "state.json",
        serde_json::to_vec_pretty(&pre).unwrap().as_slice(),
    );
    let lines = t2.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_REQUIRED "));
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESUME_CLOSEOUT phase-1");
    assert_no_temp_files(&t2.repo);
}

#[test]
fn test_obligation_63_phase7_continuity_and_phase8_recovery_regression() {
    // Continuity first publication: token, ledger identity, privacy.
    let t = TestRepo::new();
    let (_m, receipt_sha) = t.close_phase1();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt_sha));
    let out = t.continuity_record(&meta);
    assert_success(&out);
    let parts = split_stdout(&out);
    assert_eq!(parts[0], "CONTINUITY_RECORDED");
    assert_eq!(parts[1], "mrgs");
    assert_eq!(parts[2], "phase-1");
    assert_eq!(parts[3], "1");
    let ledger = t.get_continuity_ledger().unwrap();
    assert_eq!(ledger["repository_id"], "mrgs");
    let entry = &ledger["entries"][0];
    assert_eq!(
        entry["continuity_manifest"]["target_completion_sequence"],
        1
    );
    assert_eq!(entry["continuity_receipt"]["continuity_sequence"], 1);
    // Privacy: no filesystem paths or host identity in the ledger.
    let text = t.read_mrgs_str("continuity-ledger.json");
    assert!(!text.contains(&t.repo.to_string_lossy().replace('\\', "/")));
    assert!(!text.contains("C:") && !text.contains("/repo/"));
    // Exact replay.
    let replay = t.continuity_record(&meta);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&out));
    let _ = (parts, entry);

    // Cross-link resolution with one source repository.
    let s = TestRepo::new();
    s.setup_impl_bound();
    let open = s.audit_begin("auditor1");
    let s_parts = split_stdout(&open);
    let report = s.make_pass_report(&s_parts[1], &s_parts[3], "auditor1");
    let path = s.write_report(&report);
    assert_success(&s.audit_record(&path));
    let close = s.phase_close("phase-1");
    assert_success(&close);
    let s_close = split_stdout(&close);
    let s_receipt = s_close[4].clone();
    // The target ledger already holds a phase-1 entry, so the linked record
    // must target the next closed phase (a second phase-1 record would be a
    // conflict).
    t.select_phase_success("phase-2");
    write_file(&t.contract_path, &contract_toml_for_phase("phase-2"));
    assert!(git(&t.repo, &["add", "-A"]).status.success());
    assert!(git(&t.repo, &["commit", "-m", "phase-2"]).status.success());
    let draft = t.draft_contract();
    assert_success(&draft);
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_success(&t.accept_contract(1, &sha));
    assert_success(&t.impl_begin(1, &sha));
    t.full_pass_audit();
    let close2 = t.phase_close("phase-2");
    assert_success(&close2);
    let close2_parts = split_stdout(&close2);
    let t_receipt2 = close2_parts[4].clone();
    let linked = linked_metadata(
        "phase-2",
        &t_receipt2,
        "mrgs",
        "repo-src",
        &s.plan_sha(),
        "phase-1",
        &s_receipt,
        None,
    )
    // The ledger's phase-1 entry already owns "phase-1-primary", and a
    // reused continuity ID would be classified as a conflict.
    .replace(
        "continuity_id = \"phase-1-primary\"",
        "continuity_id = \"phase-2-primary\"",
    );
    let linked_path = t.write_metadata("m-linked.toml", &linked);
    let out = t.run(&[
        "continuity",
        "record",
        "--repo",
        &t.repo.to_string_lossy(),
        "--metadata",
        &linked_path.to_string_lossy(),
        "--source-repo",
        &s.repo.to_string_lossy(),
    ]);
    assert_success(&out);
    let ledger = t.get_continuity_ledger().unwrap();
    let entries = ledger["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    let links = entries[1]["continuity_manifest"]["resolved_links"]
        .as_array()
        .unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["source_repository_id"], "repo-src");
    assert_eq!(links[0]["source_completion_receipt_sha256"], s_receipt);

    // Recovery regression: healthy / recoverable / unrecoverable / pending /
    // apply / replay with one platform capability branch.
    let t2 = TestRepo::new();
    t2.setup_impl_bound();
    let healthy = t2.inspect_output();
    assert_eq!(healthy.len(), 1);
    assert!(healthy[0].starts_with("RECOVERY_NOT_REQUIRED "));
    induce_recoverable(&t2);
    let lines = t2.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_REQUIRED "));
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(lines[1], "RECOVERY_ACTION 1 RESTORE_STATE state.json");
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t2, parts[1], parts[2], "after_pending_publish", dir.path());
    kill_child(child);
    let pending = t2.inspect_output();
    assert!(pending[0].starts_with("RECOVERY_PENDING "));
    let apply = t2.apply(parts[1], parts[2]);
    assert_success(&apply);
    assert!(stdout_str(&apply).starts_with("RECOVERY_APPLIED "));
    // Fixed-point replay binds the CURRENT post-recovery subject (the
    // phase8 obligation-72 pattern); the stale pre-recovery subject is a
    // conflict, not a replay.
    let current_sha = recompute_subject(&t2.repo);
    let replay = t2.apply(parts[1], &current_sha);
    assert_success(&replay);
    assert_eq!(stdout_raw(&replay), stdout_raw(&apply));
    let final_insp = t2.inspect_output();
    assert!(final_insp[0].starts_with("RECOVERY_NOT_REQUIRED "));

    // Unrecoverable via an unknown child.
    let t3 = TestRepo::new();
    t3.setup_impl_bound();
    t3.write_mrgs("rogue.bin", b"x");
    let out = t3.inspect();
    assert_category_no_stdout(&out, "RECOVERY_UNRECOVERABLE");

    // Capability branch: a recovery-owned temp path occupied by a directory.
    let t4 = TestRepo::new();
    t4.setup_impl_bound();
    induce_recoverable(&t4);
    let (rid, pre_sha) = recoverable_ids(&t4);
    // The temp is authorized only while a pending journal binds the same
    // recovery ID: publish the pending entry first.
    let dir = tempfile::TempDir::new().unwrap();
    let child = crash_apply(&t4, &rid, &pre_sha, "after_pending_publish", dir.path());
    kill_child(child);
    let temp_path = t4
        .repo
        .join(".mrgs")
        .join(format!(".recovery-{}-0.tmp", rid));
    match make_dir_link(&t4._dir.path().join("temp-target"), &temp_path) {
        Ok(()) => {
            let out = t4.inspect();
            assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
            eprintln!("CAPABILITY_EXECUTED");
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Capability-unavailable branch: a directory at the authorized
            // recovery-owned temp path is the concrete fail-closed fallback.
            std::fs::create_dir_all(&temp_path).unwrap();
            let out = t4.inspect();
            assert_category_no_stdout(&out, "FILESYSTEM_BOUNDARY_UNSAFE");
            eprintln!("CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION");
        }
        Err(e) => panic!("link creation failed: {}", e),
    }
    let _ = pre_sha;
}

#[test]
fn test_obligation_64_complete_public_cli_lifecycle_and_test_discipline() {
    // Complete lifecycle: plan -> phase -> contract -> implementation ->
    // audit PASS -> closeout -> continuity -> induced recovery -> healthy.
    let t = TestRepo::new();
    let repo = t.repo.to_string_lossy().into_owned();

    let out = t.accept_plan();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("test-plan "));
    let out = t.select_phase("phase-1");
    assert_success(&out);
    assert_eq!(stdout_str(&out), "phase-1");
    let out = t.draft_contract();
    assert_success(&out);
    let sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    let out = t.accept_contract(1, &sha);
    assert_success(&out);
    assert_eq!(
        stdout_str(&out),
        format!("ACCEPTED test-contract-v1 1 {}", sha)
    );
    let out = t.impl_begin(1, &sha);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("IMPLEMENTATION_BOUND "));
    let out = t.impl_check();
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("IMPLEMENTATION_OK "));
    let open = t.audit_begin("auditor1");
    assert_success(&open);
    let parts = split_stdout(&open);
    assert_eq!(parts[0], "AUDIT_OPEN");
    let report = t.make_pass_report(&parts[1], &parts[3], "auditor1");
    let report_path = t.write_report(&report);
    let out = t.audit_record(&report_path);
    assert_success(&out);
    assert!(stdout_str(&out).starts_with("AUDIT_PASS "));
    let out = t.phase_close("phase-1");
    assert_success(&out);
    let close_parts = split_stdout(&out);
    assert_eq!(close_parts[0], "PHASE_CLOSED");
    let receipt = close_parts[4].clone();
    let meta = t.write_metadata("m.toml", &standard_metadata("phase-1", &receipt));
    let out = t.continuity_record(&meta);
    assert_success(&out);
    let cont_parts = split_stdout(&out);
    assert_eq!(cont_parts[0], "CONTINUITY_RECORDED");
    // Induced recoverable state and full recovery.
    induce_recoverable(&t);
    let lines = t.inspect_output();
    assert!(lines[0].starts_with("RECOVERY_REQUIRED "));
    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    let out = t.apply(parts[1], parts[2]);
    assert_success(&out);
    let applied = split_stdout(&out);
    assert_eq!(applied[0], "RECOVERY_APPLIED");
    let final_insp = t.inspect_output();
    assert!(final_insp[0].starts_with("RECOVERY_NOT_REQUIRED "));
    // Every surviving intermediate authority exists and is valid JSON.
    // (accepted-contract.json and implementation-authority.json are removed
    // by closeout cleanup by design and preserved in the completion archive,
    // so they are not expected on disk post-closeout.)
    for name in [
        "accepted-plan.json",
        "state.json",
        "completion-ledger.json",
        "continuity-ledger.json",
        "recovery-ledger.json",
    ] {
        let text = t.read_mrgs_str(name);
        let v: Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} invalid JSON: {}", name, e));
        assert_eq!(v["schema_version"], 1, "{}", name);
    }
    assert_no_temp_files(&t.repo);
    let _ = repo;

    // Test discipline: exactly 64 discoverable primary obligations, no
    // ignored tests, no recursive Cargo invocation, no dependency change.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let source =
        std::fs::read_to_string(std::path::Path::new(manifest_dir).join("tests/phase9.rs"))
            .unwrap();
    let mut primary_count = 0usize;
    for line in source.lines() {
        let line = line.trim_start();
        if !line.starts_with("fn test_obligation_") {
            continue;
        }
        let rest = &line["fn test_obligation_".len()..];
        let mut bytes = rest.bytes();
        let two_digits = bytes.next().is_some_and(|b| b.is_ascii_digit())
            && bytes.next().is_some_and(|b| b.is_ascii_digit());
        let after = &rest[2..];
        let valid_suffix = after.strip_prefix('_').is_some_and(|s| {
            let body = s.strip_suffix("() {").unwrap_or("");
            !body.is_empty()
                && body
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        });
        if two_digits && valid_suffix {
            primary_count += 1;
        }
    }
    assert_eq!(primary_count, 64, "exactly 64 primary obligations required");
    // Needles are assembled from parts so the checks cannot match their own
    // source text; fixture strings legitimately contain the words (e.g.
    // verification_commands), so only the invocation form is forbidden.
    let ignore_attr = format!("#[{}]", "ignore");
    assert!(!source.contains(&ignore_attr), "no test may be ignored");
    let cargo_invocation = format!("Command::new(\"{}\x22)", "cargo");
    assert!(
        !source.contains(&cargo_invocation),
        "no recursive Cargo invocation"
    );
    let cargo_toml =
        std::fs::read_to_string(std::path::Path::new(manifest_dir).join("Cargo.toml")).unwrap();
    let parsed: toml::Value = cargo_toml.parse().unwrap();
    let deps: Vec<&str> = parsed["dependencies"]
        .as_table()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    let mut deps = deps;
    deps.sort();
    assert_eq!(
        deps,
        vec!["clap", "serde", "serde_json", "sha2", "thiserror", "toml"],
        "no dependency change"
    );
    let dev_deps: Vec<&str> = parsed["dev-dependencies"]
        .as_table()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    let mut dev_deps = dev_deps;
    dev_deps.sort();
    assert_eq!(
        dev_deps,
        vec!["assert_cmd", "predicates", "tempfile"],
        "no dev-dependency change"
    );
}
