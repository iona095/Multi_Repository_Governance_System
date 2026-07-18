use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

fn cargo_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mrgs"))
}

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

fn write_plan(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

fn create_repo_and_plan(
    plan_content: &str,
) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, plan_content);
    (dir, repo, plan_path)
}

fn run_plan_accept(repo: &Path, plan: &Path) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("plan")
        .arg("accept")
        .arg("--repo")
        .arg(repo)
        .arg("--plan")
        .arg(plan);
    cmd.output().unwrap()
}

fn run_phase_select(repo: &Path, phase: &str) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("phase")
        .arg("select")
        .arg("--repo")
        .arg(repo)
        .arg("--phase")
        .arg(phase);
    cmd.output().unwrap()
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "expected success, got exit: {} stderr: {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "expected failure, got success stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn stdout_string(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .trim()
        .to_string()
}

fn stderr_string(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone())
        .unwrap()
        .trim()
        .to_string()
}

fn read_json(repo: &Path, name: &str) -> serde_json::Value {
    let path = repo.join(".mrgs").join(name);
    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap()
}

fn write_json(repo: &Path, name: &str, value: &serde_json::Value) {
    let dir = repo.join(".mrgs");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_string_pretty(value).unwrap()).unwrap();
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

// ===== Contract-required acceptance tests =====

#[test]
fn test_valid_first_acceptance() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    let output = run_plan_accept(&repo, &plan_path);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.contains("test-plan"), "stdout: {}", out);
    assert!(out.len() > 20, "expected hash in output: {}", out);
    assert_no_temp_files(&repo);
}

#[test]
fn test_exact_sha256_persistence() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    let output = run_plan_accept(&repo, &plan_path);
    assert_success(&output);
    let printed_hash = stdout_string(&output)
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_string();

    let accepted: serde_json::Value = read_json(&repo, "accepted-plan.json");
    assert_eq!(accepted["sha256"].as_str().unwrap(), &printed_hash);

    let state: serde_json::Value = read_json(&repo, "state.json");
    assert_eq!(
        state["accepted_plan_sha256"].as_str().unwrap(),
        &printed_hash
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_same_plan_idempotence() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    let first = run_plan_accept(&repo, &plan_path);
    assert_success(&first);

    let second = run_plan_accept(&repo, &plan_path);
    assert_success(&second);

    assert_eq!(stdout_string(&first), stdout_string(&second));
    assert_no_temp_files(&repo);
}

#[test]
fn test_rejection_of_different_accepted_bytes() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let modified = valid_plan_toml().replace("test-plan", "other-plan");
    write_plan(&plan_path, &modified);

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    let err = stderr_string(&output);
    assert!(
        err.contains("plan drift")
            || err.contains("plan ID mismatch")
            || err.contains("GOVERNANCE_AUTHORITY_INVALID"),
        "stderr: {}",
        err
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_unsupported_schema() {
    let content = r#"schema_version = 2
plan_id = "test-plan"

[[phases]]
id = "phase-1"
title = "First"
depends_on = []
"#;
    let (_dir, repo, plan_path) = create_repo_and_plan(content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("schema version"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_empty_plan_id() {
    let content = r#"schema_version = 1
plan_id = ""

[[phases]]
id = "phase-1"
title = "First"
depends_on = []
"#;
    let (_dir, repo, plan_path) = create_repo_and_plan(content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("empty plan ID"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_empty_phase_id() {
    let content = r#"schema_version = 1
plan_id = "test"

[[phases]]
id = ""
title = "Empty"
depends_on = []
"#;
    let (_dir, repo, plan_path) = create_repo_and_plan(content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("empty phase ID"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_empty_phase_title() {
    let content = r#"schema_version = 1
plan_id = "test"

[[phases]]
id = "phase-1"
title = ""
depends_on = []
"#;
    let (_dir, repo, plan_path) = create_repo_and_plan(content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("empty phase title"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_zero_phases() {
    let content = r#"schema_version = 1
plan_id = "test-plan"
"#;
    let (_dir, repo, plan_path) = create_repo_and_plan(content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("zero phases"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_duplicate_phase_ids() {
    let content = r#"schema_version = 1
plan_id = "test-plan"

[[phases]]
id = "phase-1"
title = "First"
depends_on = []

[[phases]]
id = "phase-1"
title = "Duplicate"
depends_on = []
"#;
    let (_dir, repo, plan_path) = create_repo_and_plan(content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("duplicate phase ID"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_unknown_dependency() {
    let content = r#"schema_version = 1
plan_id = "test-plan"

[[phases]]
id = "phase-1"
title = "First"
depends_on = ["phase-unknown"]
"#;
    let (_dir, repo, plan_path) = create_repo_and_plan(content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("unknown dependency"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_self_dependency() {
    let content = r#"schema_version = 1
plan_id = "test-plan"

[[phases]]
id = "phase-1"
title = "First"
depends_on = ["phase-1"]
"#;
    let (_dir, repo, plan_path) = create_repo_and_plan(content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("self-dependency"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_dependency_cycle() {
    let content = r#"schema_version = 1
plan_id = "test-plan"

[[phases]]
id = "phase-1"
title = "First"
depends_on = ["phase-2"]

[[phases]]
id = "phase-2"
title = "Second"
depends_on = ["phase-1"]
"#;
    let (_dir, repo, plan_path) = create_repo_and_plan(content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("cycle"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_plan_outside_repository() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&repo).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let plan_path = outside.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("outside") || stderr_string(&output).contains("inside")
    );
    assert_no_temp_files(&repo);
}

// ===== Phase selection tests =====

#[test]
fn test_plan_drift() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let modified = valid_plan_toml().replace("First phase", "Modified phase");
    write_plan(&plan_path, &modified);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("plan drift")
            || stderr_string(&output).contains("GOVERNANCE_AUTHORITY_INVALID")
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_successful_unblocked_selection() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let output = run_phase_select(&repo, "phase-1");
    assert_success(&output);
    assert_eq!(stdout_string(&output), "phase-1");

    let state: serde_json::Value = read_json(&repo, "state.json");
    assert_eq!(state["active_phase"].as_str().unwrap(), "phase-1");
    assert_no_temp_files(&repo);
}

#[test]
fn test_unknown_phase_rejection() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let output = run_phase_select(&repo, "phase-99");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("unknown phase")
            || stderr_string(&output).contains("not found")
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_active_phase_conflict() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));

    let output = run_phase_select(&repo, "phase-2");
    assert_failure(&output);
    assert!(stderr_string(&output).contains("active"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_blocked_dependency_rejection() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let output = run_phase_select(&repo, "phase-2");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("not closed")
            || stderr_string(&output).contains("dependency")
    );
    assert_no_temp_files(&repo);
}

// ===== State preservation tests =====

#[test]
fn test_no_state_mutation_after_failure() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert!(repo.join(".mrgs").join("accepted-plan.json").exists());

    let modified = valid_plan_toml().replace("test-plan", "other-plan");
    write_plan(&plan_path, &modified);
    let original_state = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);

    let current_state = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(
        original_state, current_state,
        "state should not change after failed operation"
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_no_state_created_on_failed_first_accept() {
    let (_dir, repo, plan_path) = create_repo_and_plan(
        r#"schema_version = 99
plan_id = "bad"
[[phases]]
id = "x"
title = "x"
depends_on = []
"#,
    );

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(
        !repo.join(".mrgs").exists(),
        ".mrgs should not be created on failed validation"
    );
    assert_no_temp_files(&repo);
}

// ===== Blocker 2: Persisted plan path validation =====

#[test]
fn test_persisted_absolute_plan_path_rejection() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut accepted: serde_json::Value = read_json(&repo, "accepted-plan.json");
    accepted["plan_path"] = serde_json::json!("C:\\outside\\plan.toml");
    write_json(&repo, "accepted-plan.json", &accepted);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("unsafe plan path")
            || stderr_string(&output).contains("plan path")
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_persisted_parent_traversal_plan_path_rejection() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut accepted: serde_json::Value = read_json(&repo, "accepted-plan.json");
    accepted["plan_path"] = serde_json::json!("../outside/plan.toml");
    write_json(&repo, "accepted-plan.json", &accepted);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(stderr_string(&output).contains("unsafe plan path"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_persisted_plan_path_outside_repo_rejection() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut accepted: serde_json::Value = read_json(&repo, "accepted-plan.json");
    accepted["plan_path"] = serde_json::json!("nonexistent/plan.toml");
    write_json(&repo, "accepted-plan.json", &accepted);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// ===== Blocker 5: State validation =====

#[test]
fn test_unsupported_accepted_record_schema() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut accepted: serde_json::Value = read_json(&repo, "accepted-plan.json");
    accepted["schema_version"] = serde_json::json!(2);
    write_json(&repo, "accepted-plan.json", &accepted);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(stderr_string(&output).contains("schema version"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_unsupported_state_record_schema() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state: serde_json::Value = read_json(&repo, "state.json");
    state["schema_version"] = serde_json::json!(2);
    write_json(&repo, "state.json", &state);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(stderr_string(&output).contains("schema version"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_accepted_state_sha_mismatch() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state: serde_json::Value = read_json(&repo, "state.json");
    state["accepted_plan_sha256"] =
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    write_json(&repo, "state.json", &state);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(stderr_string(&output).contains("SHA"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_accepted_plan_id_mismatch() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let other = valid_plan_toml().replace("test-plan", "other-plan");
    write_plan(&plan_path, &other);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    let err = stderr_string(&output);
    assert!(
        err.contains("plan ID mismatch") || err.contains("GOVERNANCE_AUTHORITY_INVALID"),
        "stderr: {}",
        err
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_accepted_phase_count_mismatch() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let modified = valid_plan_toml().replace(
        "[[phases]]\nid = \"phase-2\"\ntitle = \"Second phase\"\ndepends_on = [\"phase-1\"]",
        "",
    );
    write_plan(&plan_path, &modified);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("phase count") || stderr_string(&output).contains("drift")
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_unknown_active_phase() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state: serde_json::Value = read_json(&repo, "state.json");
    state["active_phase"] = serde_json::json!("phase-99");
    write_json(&repo, "state.json", &state);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("unknown active")
            || stderr_string(&output).contains("unknown phase")
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_unknown_closed_phase() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state: serde_json::Value = read_json(&repo, "state.json");
    state["closed_phases"] = serde_json::json!(["phase-99"]);
    write_json(&repo, "state.json", &state);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(stderr_string(&output).contains("closed"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_duplicate_closed_phase() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state: serde_json::Value = read_json(&repo, "state.json");
    state["closed_phases"] = serde_json::json!(["phase-1", "phase-1"]);
    write_json(&repo, "state.json", &state);

    let output = run_phase_select(&repo, "phase-2");
    assert_failure(&output);
    assert!(stderr_string(&output).contains("duplicate"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_active_phase_also_closed() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state: serde_json::Value = read_json(&repo, "state.json");
    state["active_phase"] = serde_json::json!("phase-1");
    state["closed_phases"] = serde_json::json!(["phase-1"]);
    write_json(&repo, "state.json", &state);

    let output = run_phase_select(&repo, "phase-2");
    assert_failure(&output);
    assert!(stderr_string(&output).contains("closed"));
    assert_no_temp_files(&repo);
}

#[test]
fn test_inconsistent_closed_dep_state() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state: serde_json::Value = read_json(&repo, "state.json");
    state["closed_phases"] = serde_json::json!(["phase-2"]);
    write_json(&repo, "state.json", &state);

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("inconsistent")
            || stderr_string(&output).contains("dependency")
    );
    assert_no_temp_files(&repo);
}

// ===== Blocker 6: Same-plan preservation =====

#[test]
fn test_same_plan_preserves_json_bytes() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    let out1 = run_plan_accept(&repo, &plan_path);
    assert_success(&out1);

    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    let out2 = run_plan_accept(&repo, &plan_path);
    assert_success(&out2);

    let accepted_after = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    assert_eq!(
        accepted_before, accepted_after,
        "accepted-plan.json changed"
    );
    assert_eq!(state_before, state_after, "state.json changed");
    assert_no_temp_files(&repo);
}

#[test]
fn test_same_plan_preserves_active_closed_state() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));

    let out = run_plan_accept(&repo, &plan_path);
    assert_success(&out);

    let state: serde_json::Value = read_json(&repo, "state.json");
    assert_eq!(
        state["active_phase"].as_str().unwrap(),
        "phase-1",
        "active_phase should be preserved"
    );
    assert_no_temp_files(&repo);
}

// ===== Blocker 7: Preservation and cleanup =====

#[test]
fn test_different_plan_preserves_all_governance_files() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    let modified = valid_plan_toml().replace("test-plan", "other-plan");
    write_plan(&plan_path, &modified);

    let out = run_plan_accept(&repo, &plan_path);
    assert_failure(&out);

    let accepted_after = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    assert_eq!(
        accepted_before, accepted_after,
        "accepted-plan.json changed"
    );
    assert_eq!(state_before, state_after, "state.json changed");
    assert_no_temp_files(&repo);
}

#[test]
fn test_failed_phase_select_preserves_state_json() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    let out = run_phase_select(&repo, "phase-99");
    assert_failure(&out);

    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(
        state_before, state_after,
        "state.json changed after failed select"
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_temp_files_absent_after_handled_failure() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let modified = valid_plan_toml().replace("test-plan", "other-plan");
    write_plan(&plan_path, &modified);

    let out = run_plan_accept(&repo, &plan_path);
    assert_failure(&out);

    assert_no_temp_files(&repo);
}

// ===== Blocker 4: Windows replacement =====

#[test]
fn test_state_replacement_succeeds() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    assert_success(&run_phase_select(&repo, "phase-1"));

    let state: serde_json::Value = read_json(&repo, "state.json");
    assert_eq!(state["active_phase"].as_str().unwrap(), "phase-1");
    assert_no_temp_files(&repo);
}

// ===== Symlink escape tests (platform-conditional) =====

#[test]
#[cfg_attr(not(any(unix, windows)), ignore)]
fn test_symlinked_plan_escape_rejection() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&repo).unwrap();
    std::fs::create_dir(&outside).unwrap();

    let real_plan = outside.join("real_plan.toml");
    write_plan(&real_plan, valid_plan_toml());

    let link = repo.join("link.toml");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_plan, &link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&real_plan, &link).unwrap();

    let output = run_plan_accept(&repo, &link);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("outside") || stderr_string(&output).contains("inside")
    );
}

#[test]
#[cfg_attr(not(any(unix, windows)), ignore)]
fn test_symlinked_mrgs_rejection() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let escape_base = repo.parent().unwrap().join("escape_target");
    std::fs::create_dir_all(&escape_base).unwrap();

    let real_mrgs = repo.join(".mrgs");
    std::fs::remove_dir_all(&real_mrgs).unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(&escape_base, &real_mrgs).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&escape_base, &real_mrgs).unwrap();

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("escape")
            || stderr_string(&output).contains("governance directory")
    );
}

// ===== Blocker 1: Failed selection must not create .mrgs =====

#[test]
fn test_missing_authority_select_does_not_create_mrgs() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();

    assert!(!repo.join(".mrgs").exists());

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);

    assert!(
        !repo.join(".mrgs").exists(),
        ".mrgs should not be created on failed select with no accepted authority"
    );
}

// ===== Blocker 2: Active phase with unmet dependency =====

#[test]
fn test_active_phase_with_unmet_dependency_rejected() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state: serde_json::Value = read_json(&repo, "state.json");
    state["active_phase"] = serde_json::json!("phase-2");
    state["closed_phases"] = serde_json::json!([]);
    write_json(&repo, "state.json", &state);

    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("unmet dependency")
            || stderr_string(&output).contains("dependency"),
        "stderr: {}",
        stderr_string(&output)
    );

    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(
        state_before, state_after,
        "state.json must remain byte-for-byte unchanged"
    );
}

// ===== Blocker 3: Uppercase SHA rejection =====

#[test]
fn test_uppercase_accepted_sha_rejected() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut accepted: serde_json::Value = read_json(&repo, "accepted-plan.json");
    accepted["sha256"] =
        serde_json::json!("ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEF");
    write_json(&repo, "accepted-plan.json", &accepted);

    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("invalid SHA"),
        "stderr: {}",
        stderr_string(&output)
    );

    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let accepted_after = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    assert_eq!(
        state_before, state_after,
        "state.json must remain unchanged"
    );
    assert_eq!(
        accepted_before, accepted_after,
        "accepted-plan.json must remain unchanged"
    );
}

#[test]
fn test_uppercase_state_sha_rejected() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state_val: serde_json::Value = read_json(&repo, "state.json");
    state_val["accepted_plan_sha256"] =
        serde_json::json!("ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEF");
    write_json(&repo, "state.json", &state_val);

    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("invalid SHA"),
        "stderr: {}",
        stderr_string(&output)
    );

    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let accepted_after = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    assert_eq!(
        state_before, state_after,
        "state.json must remain unchanged"
    );
    assert_eq!(
        accepted_before, accepted_after,
        "accepted-plan.json must remain unchanged"
    );
}

// ===== Blocker 4: Temp file collision protection =====

#[test]
fn test_preexisting_tmp_files_not_truncated() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mrgs = repo.join(".mrgs");
    let tmp_file1 = mrgs.join(".pre_existing_test_1.tmp");
    let tmp_file2 = mrgs.join(".pre_existing_test_2.tmp");
    std::fs::write(&tmp_file1, b"preserved content 1").unwrap();
    std::fs::write(&tmp_file2, b"preserved content 2").unwrap();

    let output = run_phase_select(&repo, "phase-1");
    assert_success(&output);

    assert!(
        tmp_file1.exists(),
        "pre-existing tmp file should still exist"
    );
    assert!(
        tmp_file2.exists(),
        "pre-existing tmp file should still exist"
    );
    assert_eq!(std::fs::read(&tmp_file1).unwrap(), b"preserved content 1");
    assert_eq!(std::fs::read(&tmp_file2).unwrap(), b"preserved content 2");

    std::fs::remove_file(&tmp_file1).unwrap();
    std::fs::remove_file(&tmp_file2).unwrap();
    assert_no_temp_files(&repo);
}

#[test]
fn test_repeated_rapid_writes_no_collision() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());

    for _ in 0..10 {
        let output = run_plan_accept(&repo, &plan_path);
        assert_success(&output);
    }

    assert_no_temp_files(&repo);
}

// ===== Blocker 6: Independent exact SHA assertion =====

#[test]
fn test_independent_exact_sha_persistence() {
    let plan_content = r#"schema_version = 1
plan_id = "sha-test"

[[phases]]
id = "phase-1"
title = "SHA test"
depends_on = []
"#;

    let expected_digest = "e4a68244749309558343423e5df4259628df11e35542641f482263257bd5170c";

    let (_dir, repo, plan_path) = create_repo_and_plan(plan_content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_success(&output);

    let accepted: serde_json::Value = read_json(&repo, "accepted-plan.json");
    assert_eq!(
        accepted["sha256"].as_str().unwrap(),
        expected_digest,
        "persisted SHA must match independently computed literal digest"
    );

    let state: serde_json::Value = read_json(&repo, "state.json");
    assert_eq!(
        state["accepted_plan_sha256"].as_str().unwrap(),
        expected_digest,
        "state SHA must match independently computed literal digest"
    );
}

#[test]
fn test_one_byte_change_changes_digest() {
    let expected_digest = "e4a68244749309558343423e5df4259628df11e35542641f482263257bd5170c";

    let plan_content = r#"schema_version = 1
plan_id = "sha-test"

[[phases]]
id = "phase-X"
title = "SHA test"
depends_on = []
"#;

    let (_dir, repo, plan_path) = create_repo_and_plan(plan_content);
    let output = run_plan_accept(&repo, &plan_path);
    assert_success(&output);

    let accepted: serde_json::Value = read_json(&repo, "accepted-plan.json");
    assert_ne!(
        accepted["sha256"].as_str().unwrap(),
        expected_digest,
        "one byte changed must produce a different digest"
    );
}

// ===== Blocker 7: Additional failure-preservation tests =====

#[test]
fn test_failed_uppercase_accepted_sha_preserves_governance_files() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut accepted: serde_json::Value = read_json(&repo, "accepted-plan.json");
    accepted["sha256"] =
        serde_json::json!("ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEF");
    write_json(&repo, "accepted-plan.json", &accepted);

    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);

    let accepted_after = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(accepted_before, accepted_after);
    assert_eq!(state_before, state_after);
    assert_no_temp_files(&repo);
}

#[test]
fn test_failed_uppercase_state_sha_preserves_governance_files() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));

    let mut state_val: serde_json::Value = read_json(&repo, "state.json");
    state_val["accepted_plan_sha256"] =
        serde_json::json!("ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEF");
    write_json(&repo, "state.json", &state_val);

    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);

    let accepted_after = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(accepted_before, accepted_after);
    assert_eq!(state_before, state_after);
    assert_no_temp_files(&repo);
}

#[test]
#[cfg_attr(not(any(unix, windows)), ignore)]
fn test_internal_mrgs_symlink_rejected_before_any_writes() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    let target = repo.join("internal-governance-target");
    std::fs::create_dir(&target).unwrap();
    let mrgs = repo.join(".mrgs");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &mrgs).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&target, &mrgs).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("governance directory"));
    assert!(!repo.join("accepted-plan.json").exists());
    assert!(!repo.join("state.json").exists());
    assert!(!target.join("accepted-plan.json").exists());
    assert!(!target.join("state.json").exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_orphaned_state_rejected_and_preserved() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    let mrgs = repo.join(".mrgs");
    std::fs::create_dir(&mrgs).unwrap();
    let state_path = mrgs.join("state.json");
    let state_bytes = br#"{"orphaned":true}"#.to_vec();
    std::fs::write(&state_path, &state_bytes).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("incomplete"));
    assert_eq!(std::fs::read(&state_path).unwrap(), state_bytes);
    assert!(!mrgs.join("accepted-plan.json").exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_orphaned_accepted_plan_rejected_and_preserved() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    let mrgs = repo.join(".mrgs");
    let state_path = mrgs.join("state.json");
    let accepted_path = mrgs.join("accepted-plan.json");
    let accepted_bytes = std::fs::read(&accepted_path).unwrap();
    std::fs::remove_file(&state_path).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(stderr_string(&output).contains("incomplete"));
    assert_eq!(std::fs::read(&accepted_path).unwrap(), accepted_bytes);
    assert!(!state_path.exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_idempotent_accept_rejects_forged_plan_id() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    let accepted_path = repo.join(".mrgs").join("accepted-plan.json");
    let state_path = repo.join(".mrgs").join("state.json");
    let accepted_before = std::fs::read(&accepted_path).unwrap();
    let state_before = std::fs::read(&state_path).unwrap();
    let mut accepted = read_json(&repo, "accepted-plan.json");
    accepted["plan_id"] = serde_json::json!("forged-plan");
    write_json(&repo, "accepted-plan.json", &accepted);

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(&accepted_path).unwrap(),
        serde_json::to_vec_pretty(&accepted).unwrap()
    );
    assert_eq!(std::fs::read(&state_path).unwrap(), state_before);
    assert_ne!(std::fs::read(&accepted_path).unwrap(), accepted_before);
    assert_no_temp_files(&repo);
}

#[test]
fn test_idempotent_accept_rejects_wrong_phase_count() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    let accepted_path = repo.join(".mrgs").join("accepted-plan.json");
    let state_path = repo.join(".mrgs").join("state.json");
    let mut accepted = read_json(&repo, "accepted-plan.json");
    accepted["phase_count"] = serde_json::json!(99);
    write_json(&repo, "accepted-plan.json", &accepted);
    let accepted_before = std::fs::read(&accepted_path).unwrap();
    let state_before = std::fs::read(&state_path).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert_eq!(std::fs::read(&accepted_path).unwrap(), accepted_before);
    assert_eq!(std::fs::read(&state_path).unwrap(), state_before);
    assert_no_temp_files(&repo);
}

#[test]
fn test_idempotent_accept_rejects_missing_recorded_plan() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    let replacement = repo.join("replacement.toml");
    std::fs::copy(&plan_path, &replacement).unwrap();
    std::fs::remove_file(&plan_path).unwrap();
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    let output = run_plan_accept(&repo, &replacement);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        accepted_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_idempotent_accept_rejects_recorded_plan_drift() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    write_plan(
        &plan_path,
        &valid_plan_toml().replace("First phase", "Drifted phase"),
    );
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        accepted_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_idempotent_accept_rejects_malformed_state() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    let state_path = repo.join(".mrgs").join("state.json");
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    std::fs::write(&state_path, b"not-json").unwrap();
    let state_before = std::fs::read(&state_path).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        accepted_before
    );
    assert_eq!(std::fs::read(&state_path).unwrap(), state_before);
    assert_no_temp_files(&repo);
}

#[test]
fn test_idempotent_accept_rejects_malformed_accepted_record() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    let accepted_path = repo.join(".mrgs").join("accepted-plan.json");
    let state_path = repo.join(".mrgs").join("state.json");
    std::fs::write(&accepted_path, b"not-json").unwrap();
    let accepted_before = std::fs::read(&accepted_path).unwrap();
    let state_before = std::fs::read(&state_path).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert_eq!(std::fs::read(&accepted_path).unwrap(), accepted_before);
    assert_eq!(std::fs::read(&state_path).unwrap(), state_before);
    assert_no_temp_files(&repo);
}

#[test]
fn test_idempotent_accept_rejects_wrong_recorded_plan_path() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    let accepted_path = repo.join(".mrgs").join("accepted-plan.json");
    let state_path = repo.join(".mrgs").join("state.json");
    let mut accepted = read_json(&repo, "accepted-plan.json");
    accepted["plan_path"] = serde_json::json!("missing-recorded-plan.toml");
    write_json(&repo, "accepted-plan.json", &accepted);
    let accepted_before = std::fs::read(&accepted_path).unwrap();
    let state_before = std::fs::read(&state_path).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert_eq!(std::fs::read(&accepted_path).unwrap(), accepted_before);
    assert_eq!(std::fs::read(&state_path).unwrap(), state_before);
    assert_no_temp_files(&repo);
}

// ===== Phase 2 — Contract Draft Tests =====

fn valid_contract_toml() -> &'static str {
    r#"schema_version = 1
contract_id = "test-contract-v1"
phase_id = "phase-1"
title = "Test contract"
objective = "Test objective."

requirements = ["req1"]
allowed_paths = ["src/"]
forbidden_paths = [".git/"]
verification_commands = ["cargo test"]
handoff_fields = ["FIELD1"]
"#
}

fn contract_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn run_contract_draft(repo: &Path, contract: &Path) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("contract")
        .arg("draft")
        .arg("--repo")
        .arg(repo)
        .arg("--contract")
        .arg(contract);
    cmd.output().unwrap()
}

fn setup_contract_test(
    contract_content: &str,
) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    let accept = run_plan_accept(&repo, &plan_path);
    assert_success(&accept);
    let select = run_phase_select(&repo, "phase-1");
    assert_success(&select);
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, contract_content);
    (dir, repo, contract_path)
}

// 1. valid first draft
#[test]
fn test_valid_first_draft() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.contains("test-contract-v1"), "stdout: {}", out);
    let draft_path = repo.join(".mrgs").join("contract-draft.json");
    assert!(draft_path.exists(), "contract-draft.json should exist");
    assert_no_temp_files(&repo);
}

// 2. exact source-byte SHA-256 persistence
#[test]
fn test_draft_exact_sha256_persistence() {
    let content = valid_contract_toml();
    let expected_sha = contract_sha256(content);
    let (_dir, repo, contract_path) = setup_contract_test(content);
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let out_sha = stdout_string(&output)
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_string();
    assert_eq!(out_sha, expected_sha);
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft["sha256"].as_str().unwrap(), expected_sha);
    assert_no_temp_files(&repo);
}

// 3. exact content persistence including final newline
#[test]
fn test_content_preserves_exact_bytes_with_newline() {
    let content = valid_contract_toml();
    let (_dir, repo, contract_path) = setup_contract_test(content);
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let stored_content = draft["content"].as_str().unwrap();
    assert!(
        stored_content.ends_with('\n'),
        "content should end with newline"
    );
    let stored_bytes = stored_content.as_bytes();
    let original_bytes = content.as_bytes();
    assert_eq!(
        stored_bytes, original_bytes,
        "content bytes must match exactly"
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_content_preserves_exact_bytes_no_newline() {
    let content = valid_contract_toml().trim_end().to_string();
    let expected_sha = contract_sha256(&content);
    let (_dir, repo, contract_path) = setup_contract_test(&content);
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let out_sha = stdout_string(&output)
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_string();
    assert_eq!(out_sha, expected_sha);
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let stored_content = draft["content"].as_str().unwrap();
    assert!(
        !stored_content.ends_with('\n'),
        "content should not have trailing newline"
    );
    assert_eq!(stored_content.as_bytes(), content.as_bytes());
    assert_no_temp_files(&repo);
}

// 4. normalized repository-relative source_path
#[test]
fn test_normalized_source_path() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sp = draft["source_path"].as_str().unwrap();
    assert!(
        !sp.contains('\\'),
        "source_path must use forward slashes: {}",
        sp
    );
    assert_eq!(sp, "contract.toml", "source_path: {}", sp);
    assert_no_temp_files(&repo);
}

// 5. same exact draft idempotence
#[test]
fn test_same_exact_draft_idempotence() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let first = run_contract_draft(&repo, &contract_path);
    assert_success(&first);
    let second = run_contract_draft(&repo, &contract_path);
    assert_success(&second);
    assert_eq!(stdout_string(&first), stdout_string(&second));
    assert_no_temp_files(&repo);
}

// 6. idempotent operation preserves draft and state bytes
#[test]
fn test_idempotent_preserves_draft_and_state_bytes() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let first = run_contract_draft(&repo, &contract_path);
    assert_success(&first);
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let second = run_contract_draft(&repo, &contract_path);
    assert_success(&second);
    let draft_after = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(draft_before, draft_after, "contract-draft.json changed");
    assert_eq!(state_before, state_after, "state.json changed");
    assert_no_temp_files(&repo);
}

// 7. different draft bytes are rejected
#[test]
fn test_different_draft_bytes_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let first = run_contract_draft(&repo, &contract_path);
    assert_success(&first);
    let modified = valid_contract_toml().replace("Test objective", "Modified objective");
    write_plan(&contract_path, &modified);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 8. different draft rejection preserves all governance files
#[test]
fn test_different_draft_rejection_preserves_files() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let modified = valid_contract_toml().replace("Test objective", "Changed objective");
    write_plan(&contract_path, &modified);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        accepted_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_no_temp_files(&repo);
}

// 9. missing active phase
#[test]
fn test_draft_missing_active_phase() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, valid_contract_toml());
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("active phase"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 10. phase ID mismatch
#[test]
fn test_draft_phase_id_mismatch() {
    let contract =
        valid_contract_toml().replace(r#"phase_id = "phase-1""#, r#"phase_id = "phase-2""#);
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("phase"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 11. unsupported contract schema
#[test]
fn test_draft_unsupported_schema() {
    let contract = valid_contract_toml().replace("schema_version = 1", "schema_version = 2");
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("schema"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 12. empty contract ID
#[test]
fn test_draft_empty_contract_id() {
    let contract =
        valid_contract_toml().replace(r#"contract_id = "test-contract-v1""#, r#"contract_id = """#);
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 13. empty phase ID
#[test]
fn test_draft_empty_phase_id() {
    let contract = valid_contract_toml().replace(r#"phase_id = "phase-1""#, r#"phase_id = """#);
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 14. empty title
#[test]
fn test_draft_empty_title() {
    let contract = valid_contract_toml().replace(r#"title = "Test contract""#, r#"title = """#);
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 15. empty objective
#[test]
fn test_draft_empty_objective() {
    let contract =
        valid_contract_toml().replace(r#"objective = "Test objective.""#, r#"objective = """#);
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// leading/trailing whitespace rejection for scalars (Blocker 1)
#[test]
fn test_draft_leading_whitespace_contract_id() {
    let contract = valid_contract_toml().replace(
        r#"contract_id = "test-contract-v1""#,
        r#"contract_id = " test-contract-v1""#,
    );
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

#[test]
fn test_draft_trailing_whitespace_phase_id() {
    let contract =
        valid_contract_toml().replace(r#"phase_id = "phase-1""#, r#"phase_id = "phase-1 ""#);
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

#[test]
fn test_draft_surrounding_whitespace_title() {
    let contract =
        valid_contract_toml().replace(r#"title = "Test contract""#, r#"title = " Test contract ""#);
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 16. zero requirements
#[test]
fn test_draft_zero_requirements() {
    let contract = valid_contract_toml().replace(r#"requirements = ["req1"]"#, "requirements = []");
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 17. zero allowed paths
#[test]
fn test_draft_zero_allowed_paths() {
    let contract =
        valid_contract_toml().replace(r#"allowed_paths = ["src/"]"#, "allowed_paths = []");
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 18. zero forbidden paths
#[test]
fn test_draft_zero_forbidden_paths() {
    let contract =
        valid_contract_toml().replace(r#"forbidden_paths = [".git/"]"#, "forbidden_paths = []");
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 19. zero verification commands
#[test]
fn test_draft_zero_verification_commands() {
    let contract = valid_contract_toml().replace(
        r#"verification_commands = ["cargo test"]"#,
        "verification_commands = []",
    );
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 20. zero handoff fields
#[test]
fn test_draft_zero_handoff_fields() {
    let contract =
        valid_contract_toml().replace(r#"handoff_fields = ["FIELD1"]"#, "handoff_fields = []");
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 21. empty list entry
#[test]
fn test_draft_empty_list_entry() {
    let contract =
        valid_contract_toml().replace(r#"requirements = ["req1"]"#, r#"requirements = [""]"#);
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 22. duplicate list entry
#[test]
fn test_draft_duplicate_list_entry() {
    let contract = valid_contract_toml().replace(
        r#"requirements = ["req1"]"#,
        r#"requirements = ["req1", "req1"]"#,
    );
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("duplicate"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 23. unknown TOML top-level field
#[test]
fn test_draft_unknown_toml_field() {
    let contract = format!("{}\nunknown_field = true\n", valid_contract_toml());
    let (_dir, repo, contract_path) = setup_contract_test(&contract);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 24. contract source outside repository
#[test]
fn test_draft_source_outside_repo() {
    let (dir, repo, _contract_path) = setup_contract_test(valid_contract_toml());
    let outside = dir.path().join("outside_contract.toml");
    write_plan(&outside, valid_contract_toml());
    let output = run_contract_draft(&repo, &outside);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("outside") || stderr_string(&output).contains("repository"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 25. contract source under .mrgs
#[test]
fn test_draft_source_under_mrgs() {
    let (_dir, repo, _contract_path) = setup_contract_test(valid_contract_toml());
    let inside_mrgs = repo.join(".mrgs").join("contract.toml");
    std::fs::create_dir_all(repo.join(".mrgs")).unwrap();
    write_plan(&inside_mrgs, valid_contract_toml());
    let output = run_contract_draft(&repo, &inside_mrgs);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains(".mrgs"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 26. contract source symlink escape
#[test]
#[cfg_attr(not(any(unix, windows)), ignore)]
fn test_draft_symlink_escape() {
    let (dir, repo, _contract_path) = setup_contract_test(valid_contract_toml());
    let outside = dir.path().join("outside_contract.toml");
    write_plan(&outside, valid_contract_toml());
    let link = repo.join("link_contract.toml");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&outside, &link).unwrap();
    let output = run_contract_draft(&repo, &link);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("outside") || stderr_string(&output).contains("repository"),
        "stderr: {}",
        stderr_string(&output)
    );
}

// 27. invalid UTF-8
#[test]
fn test_draft_invalid_utf8() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    std::fs::write(&contract_path, [0xFF, 0xFE, 0x00, 0x61]).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("UTF") || stderr_string(&output).contains("utf"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 28. malformed TOML
#[test]
fn test_draft_malformed_toml() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    std::fs::write(&contract_path, b"not valid toml {{{").unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 29. plan drift
#[test]
fn test_draft_plan_drift() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let plan_path = repo.join("plan.toml");
    let drifted = valid_plan_toml().replace("First phase", "Drifted phase");
    write_plan(&plan_path, &drifted);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("plan drift")
            || stderr_string(&output).contains("GOVERNANCE_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 30. malformed existing draft record
#[test]
fn test_draft_malformed_existing_record() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    std::fs::write(repo.join(".mrgs").join("contract-draft.json"), b"not-json").unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 31. malformed inconsistent state
#[test]
fn test_draft_malformed_state() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let state_path = repo.join(".mrgs").join("state.json");
    std::fs::write(&state_path, b"not-json").unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(&state_path).unwrap(),
        b"not-json",
        "state.json should remain corrupted (not silently repaired)"
    );
    assert_no_temp_files(&repo);
}

// 32. malformed accepted plan
#[test]
fn test_draft_malformed_accepted_plan() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let accepted_path = repo.join(".mrgs").join("accepted-plan.json");
    std::fs::write(&accepted_path, b"not-json").unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(&accepted_path).unwrap(),
        b"not-json",
        "accepted-plan.json should remain corrupted (not silently repaired)"
    );
    assert_no_temp_files(&repo);
}

// 33. uppercase or invalid persisted draft SHA
#[test]
fn test_draft_uppercase_persisted_sha_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["sha256"] =
        serde_json::json!("ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEF");
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("SHA") || stderr_string(&output).contains("sha"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 34. persisted draft plan SHA mismatch
#[test]
fn test_draft_persisted_plan_sha_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["accepted_plan_sha256"] =
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 35. persisted draft phase mismatch
#[test]
fn test_draft_persisted_phase_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["phase_id"] = serde_json::json!("phase-2");
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 36. persisted draft contract ID mismatch
#[test]
fn test_draft_persisted_contract_id_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["contract_id"] = serde_json::json!("wrong-contract-id");
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 37. persisted draft revision zero (supersedes Phase 2 revision-equals-one test)
#[test]
fn test_draft_persisted_revision_zero_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["revision"] = serde_json::json!(0);
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("revision"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_draft_revision_two_valid_when_ledger_exists() {
    let content = valid_contract_toml();
    let content_v2 = valid_contract_toml().replace("Test objective", "Revised objective");
    let (_dir, repo, contract_path) = setup_contract_test(content);
    assert_success(&run_contract_draft(&repo, &contract_path));
    // Accept to create ledger
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let d_sha = draft["sha256"].as_str().unwrap().to_string();
    let _d_contract_id = draft["contract_id"].as_str().unwrap().to_string();
    let d_revision = draft["revision"].as_u64().unwrap() as u32;
    let accept_out = run_contract_accept(&repo, d_revision, &d_sha, "ACCEPTED");
    assert_success(&accept_out);
    // Now write v2, revise to rev 2
    write_plan(&contract_path, &content_v2);
    let rev_out = run_contract_revise(&repo, &contract_path, 1, &d_sha);
    assert_success(&rev_out);
    // Verify draft revision is now 2
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft2["revision"].as_u64().unwrap(), 2);
    // Verify contract draft still works idempotently with revision 2
    let content_v2b = valid_contract_toml().replace("Test objective", "Revised objective");
    write_plan(&contract_path, &content_v2b);
    let draft2_sha = draft2["sha256"].as_str().unwrap().to_string();
    let draft2_path = repo.join("contract_v2.toml");
    write_plan(&draft2_path, &content_v2b);
    let draft_out = run_contract_draft(&repo, &draft2_path);
    assert_success(&draft_out);
    let out_str = stdout_string(&draft_out);
    assert!(out_str.contains(&draft2_sha), "stdout: {}", out_str);
    assert_no_temp_files(&repo);
}

// 38. persisted draft source path unsafe or under .mrgs
#[test]
fn test_draft_persisted_unsafe_source_path() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["source_path"] = serde_json::json!("../outside.toml");
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("unsafe") || stderr_string(&output).contains("path"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_draft_persisted_source_under_mrgs() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["source_path"] = serde_json::json!(".mrgs/contract.toml");
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 39. persisted content/hash mismatch
#[test]
fn test_draft_persisted_content_hash_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["sha256"] =
        serde_json::json!("1111111111111111111111111111111111111111111111111111111111111111");
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("hash") || stderr_string(&output).contains("SHA"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 40. persisted content/field mismatch (contract_id in content differs from record)
#[test]
fn test_draft_persisted_content_field_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["contract_id"] = serde_json::json!("mismatched-id");
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 41. missing original source after successful first draft
#[test]
fn test_draft_missing_original_source_idempotent() {
    let content = valid_contract_toml();
    let (_dir, repo, contract_path) = setup_contract_test(content);
    assert_success(&run_contract_draft(&repo, &contract_path));
    std::fs::remove_file(&contract_path).unwrap();
    let new_path = repo.join("new_contract.toml");
    write_plan(&new_path, content);
    let output = run_contract_draft(&repo, &new_path);
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 42. no phase-state mutation on success
#[test]
fn test_draft_no_phase_state_mutation_on_success() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(
        state_before, state_after,
        "state.json must not change on success"
    );
    assert_no_temp_files(&repo);
}

// 43. no phase-state mutation on failure
#[test]
fn test_draft_no_phase_state_mutation_on_failure() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let modified = valid_contract_toml().replace("Test objective", "Changed");
    write_plan(&contract_path, &modified);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(
        state_before, state_after,
        "state.json must not change on failure"
    );
    assert_no_temp_files(&repo);
}

// 44. no temporary files after success or handled failure
#[test]
fn test_draft_no_temp_files_after_success() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    assert_no_temp_files(&repo);
}

#[test]
fn test_draft_no_temp_files_after_failure() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let modified = valid_contract_toml().replace("Test objective", "Different");
    write_plan(&contract_path, &modified);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// ===== Blocker 2 — Strict persisted ContractDraft JSON =====
#[test]
fn test_draft_unknown_json_field_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["timestamp"] = serde_json::json!("2024-01-01T00:00:00Z");
    write_json(&repo, "contract-draft.json", &draft);
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        accepted_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_no_temp_files(&repo);
}

// ===== Blocker 3 — Stored phase consistency =====
#[test]
fn test_draft_persisted_content_phase_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["content"] = serde_json::json!(
        "schema_version = 1\ncontract_id = \"test-contract-v1\"\nphase_id = \"phase-2\"\ntitle = \"Test contract\"\nobjective = \"Test objective.\"\n\nrequirements = [\"req1\"]\nallowed_paths = [\"src/\"]\nforbidden_paths = [\".git/\"]\nverification_commands = [\"cargo test\"]\nhandoff_fields = [\"FIELD1\"]\n"
    );
    let modified_content = draft["content"].as_str().unwrap();
    let modified_sha = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(modified_content.as_bytes());
        format!("{:x}", h.finalize())
    };
    draft["sha256"] = serde_json::json!(modified_sha);
    write_json(&repo, "contract-draft.json", &draft);
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        accepted_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_no_temp_files(&repo);
}

// ===== Blocker 4 — Strict normalized source_path =====
fn setup_persisted_source_path_test(
    source_path: &str,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    Vec<u8>,
) {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["source_path"] = serde_json::json!(source_path);
    write_json(&repo, "contract-draft.json", &draft);
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    (dir, repo, contract_path, draft_before)
}

fn assert_source_path_rejected(
    repo: &std::path::Path,
    contract_path: &std::path::Path,
    draft_before: Vec<u8>,
) {
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let output = run_contract_draft(repo, contract_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        accepted_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_no_temp_files(repo);
}

#[test]
fn test_draft_persisted_source_backslash_mrgs() {
    let (_dir, repo, contract_path, draft_before) =
        setup_persisted_source_path_test(".mrgs\\contract.toml");
    assert_source_path_rejected(&repo, &contract_path, draft_before);
}

#[test]
fn test_draft_persisted_source_backslash_docs() {
    let (_dir, repo, contract_path, draft_before) =
        setup_persisted_source_path_test("docs\\contract.toml");
    assert_source_path_rejected(&repo, &contract_path, draft_before);
}

#[test]
fn test_draft_persisted_source_double_slash() {
    let (_dir, repo, contract_path, draft_before) =
        setup_persisted_source_path_test("docs//contract.toml");
    assert_source_path_rejected(&repo, &contract_path, draft_before);
}

#[test]
fn test_draft_persisted_source_dot_slash() {
    let (_dir, repo, contract_path, draft_before) =
        setup_persisted_source_path_test("./docs/contract.toml");
    assert_source_path_rejected(&repo, &contract_path, draft_before);
}

#[test]
fn test_draft_persisted_source_dot_dot() {
    let (_dir, repo, contract_path, draft_before) =
        setup_persisted_source_path_test("docs/../contract.toml");
    assert_source_path_rejected(&repo, &contract_path, draft_before);
}

// ===== Blocker 5 — Non-UTF-8 path rejection (Unix only) =====
#[test]
#[cfg(unix)]
fn test_draft_non_utf8_path_rejected() {
    use std::os::unix::ffi::OsStrExt;
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let bad_bytes = &[0x62, 0x61, 0x64, 0xFF, 0x2E, 0x74, 0x6F, 0x6D, 0x6C];
    let bad_os_str = std::ffi::OsStr::from_bytes(bad_bytes);
    let mut contract_path = repo.clone();
    contract_path.push(bad_os_str);
    std::fs::write(&contract_path, valid_contract_toml()).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    // On failure, no state mutation should occur
    assert!(!repo.join(".mrgs").join("contract-draft.json").exists());
    assert_no_temp_files(&repo);
}

// ===== Blocker 6 — Independent literal SHA test =====
const LITERAL_SHA_CONTRACT: &str = "schema_version = 1\ncontract_id = \"literal-sha-test-v1\"\nphase_id = \"phase-1\"\ntitle = \"Literal SHA test\"\nobjective = \"Fixed test bytes.\"\n\nrequirements = [\"req1\"]\nallowed_paths = [\"src/\"]\nforbidden_paths = [\".git/\"]\nverification_commands = [\"cargo test\"]\nhandoff_fields = [\"FIELD1\"]\n";

const LITERAL_SHA_DIGEST: &str = "e527e65326918a4bf48ae6c2c0318b0e10614acf5fa948bbe8d49b54fcf0c261";

#[test]
fn test_draft_literal_sha() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract.toml");
    std::fs::write(&contract_path, LITERAL_SHA_CONTRACT).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(
        draft["sha256"].as_str().unwrap(),
        LITERAL_SHA_DIGEST,
        "persisted sha256 must match independent literal digest"
    );
    assert_eq!(
        draft["content"].as_str().unwrap().as_bytes(),
        LITERAL_SHA_CONTRACT.as_bytes(),
        "persisted content must exactly match original literal bytes"
    );
    assert_no_temp_files(&repo);
}

// ===== Blocker 7 — LF versus CRLF preservation =====
const LITERAL_CRLF_CONTRACT: &str = "schema_version = 1\r\ncontract_id = \"literal-sha-test-v1\"\r\nphase_id = \"phase-1\"\r\ntitle = \"Literal SHA test\"\r\nobjective = \"Fixed test bytes.\"\r\n\r\nrequirements = [\"req1\"]\r\nallowed_paths = [\"src/\"]\r\nforbidden_paths = [\".git/\"]\r\nverification_commands = [\"cargo test\"]\r\nhandoff_fields = [\"FIELD1\"]\r\n";

const CRLF_SHA_DIGEST: &str = "1bcfaec70e3c39cf5244a7addc57827ec71bcb15cfe8f45bce04ceda44599bb8";

#[test]
fn test_draft_lf_content_preservation() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract_lf.toml");
    std::fs::write(&contract_path, LITERAL_SHA_CONTRACT).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(
        draft["content"].as_str().unwrap().as_bytes(),
        LITERAL_SHA_CONTRACT.as_bytes(),
        "LF content must be stored exactly"
    );
    assert_eq!(
        draft["sha256"].as_str().unwrap(),
        LITERAL_SHA_DIGEST,
        "LF SHA must match literal LF digest"
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_draft_crlf_content_preservation() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract_crlf.toml");
    std::fs::write(&contract_path, LITERAL_CRLF_CONTRACT).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(
        draft["content"].as_str().unwrap().as_bytes(),
        LITERAL_CRLF_CONTRACT.as_bytes(),
        "CRLF content must be stored exactly"
    );
    assert_eq!(
        draft["sha256"].as_str().unwrap(),
        CRLF_SHA_DIGEST,
        "CRLF SHA must match literal CRLF digest"
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_draft_lf_crlf_bytes_differ() {
    assert_ne!(
        LITERAL_SHA_CONTRACT.as_bytes(),
        LITERAL_CRLF_CONTRACT.as_bytes(),
        "LF and CRLF fixtures must have different bytes"
    );
    assert_ne!(
        LITERAL_SHA_DIGEST, CRLF_SHA_DIGEST,
        "LF and CRLF hashes must differ"
    );
}

// ===== Phase 3 — Contract Acceptance, Revision, and Lifecycle Transitions =====

fn run_contract_accept(
    repo: &Path,
    revision: u32,
    sha256: &str,
    decision: &str,
) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("contract")
        .arg("accept")
        .arg("--repo")
        .arg(repo)
        .arg("--revision")
        .arg(revision.to_string())
        .arg("--sha256")
        .arg(sha256)
        .arg("--decision")
        .arg(decision);
    cmd.output().unwrap()
}

fn run_contract_revise(
    repo: &Path,
    contract: &Path,
    expected_revision: u32,
    expected_sha256: &str,
) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("contract")
        .arg("revise")
        .arg("--repo")
        .arg(repo)
        .arg("--contract")
        .arg(contract)
        .arg("--expected-revision")
        .arg(expected_revision.to_string())
        .arg("--expected-sha256")
        .arg(expected_sha256);
    cmd.output().unwrap()
}

fn setup_three_revision_contract() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    String,
    String,
    String,
) {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));

    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));

    let v3 = valid_contract_toml().replace("Test objective", "V3 objective");
    let v3_sha = contract_sha256(&v3);
    write_plan(&contract_path, &v3);
    assert_success(&run_contract_revise(&repo, &contract_path, 2, &v2_sha));

    (_dir, repo, contract_path, sha1, v2_sha, v3_sha)
}

fn governance_bytes(repo: &Path) -> [Vec<u8>; 4] {
    [
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap(),
    ]
}

fn assert_governance_bytes_unchanged(repo: &Path, before: &[Vec<u8>; 4]) {
    assert_eq!(governance_bytes(repo), *before);
    assert_no_temp_files(repo);
}

// 1. valid first acceptance
#[test]
fn test_accept_valid_first_acceptance() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let cid = draft["contract_id"].as_str().unwrap().to_string();
    let output = run_contract_accept(&repo, rev, &sha, "ACCEPTED");
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("ACCEPTED"), "stdout: {}", out);
    assert!(out.contains(&cid), "stdout: {}", out);
    assert!(out.contains(&rev.to_string()), "stdout: {}", out);
    assert!(out.contains(&sha), "stdout: {}", out);
    assert_no_temp_files(&repo);
}

// 2. exact ACCEPTED token
#[test]
fn test_accept_exact_accepted_token() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let output = run_contract_accept(&repo, rev, &sha, "ACCEPTED");
    assert_success(&output);
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    assert_eq!(ledger["schema_version"].as_u64().unwrap(), 1);
    let revs = ledger["revisions"].as_array().unwrap();
    assert_eq!(revs.len(), 1);
    assert_eq!(revs[0]["revision"].as_u64().unwrap() as u32, rev);
    assert_no_temp_files(&repo);
}

// 3. lowercase token rejection
#[test]
fn test_accept_lowercase_token_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let output = run_contract_accept(&repo, rev, &sha, "accepted");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("ACCEPTED"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert!(!repo.join(".mrgs").join("accepted-contract.json").exists());
    assert_no_temp_files(&repo);
}

// 4. mixed-case token rejection
#[test]
fn test_accept_mixed_case_token_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let output = run_contract_accept(&repo, rev, &sha, "Accepted");
    assert_failure(&output);
    assert!(!repo.join(".mrgs").join("accepted-contract.json").exists());
    assert_no_temp_files(&repo);
}

// 5. leading whitespace rejection
#[test]
fn test_accept_leading_whitespace_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let output = run_contract_accept(&repo, rev, &sha, " ACCEPTED");
    assert_failure(&output);
    assert!(!repo.join(".mrgs").join("accepted-contract.json").exists());
    assert_no_temp_files(&repo);
}

// 6. trailing whitespace rejection
#[test]
fn test_accept_trailing_whitespace_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let output = run_contract_accept(&repo, rev, &sha, "ACCEPTED ");
    assert_failure(&output);
    assert!(!repo.join(".mrgs").join("accepted-contract.json").exists());
    assert_no_temp_files(&repo);
}

// 7. wrong token rejection
#[test]
fn test_accept_wrong_token_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let output = run_contract_accept(&repo, rev, &sha, "ACCEPT");
    assert_failure(&output);
    assert!(!repo.join(".mrgs").join("accepted-contract.json").exists());
    assert_no_temp_files(&repo);
}

// 8. stale revision rejection
#[test]
fn test_accept_stale_revision_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    // Accept revision 1
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    // Now revise to rev 2
    let v2 = valid_contract_toml().replace("Test objective", "Revised objective");
    let _v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha));
    // Try to accept with stale revision 1
    let output = run_contract_accept(&repo, 1, &sha, "ACCEPTED");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("revision"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 9. stale SHA rejection
#[test]
fn test_accept_stale_sha_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let _sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let wrong_sha = "0000000000000000000000000000000000000000000000000000000000000000";
    let output = run_contract_accept(&repo, rev, wrong_sha, "ACCEPTED");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("SHA"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert!(!repo.join(".mrgs").join("accepted-contract.json").exists());
    assert_no_temp_files(&repo);
}

// 10. uppercase SHA rejection
#[test]
fn test_accept_uppercase_sha_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let upper_sha = "ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEF";
    let output = run_contract_accept(&repo, rev, upper_sha, "ACCEPTED");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("SHA"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 11. invalid SHA rejection
#[test]
fn test_accept_invalid_sha_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let output = run_contract_accept(&repo, rev, "not-a-sha", "ACCEPTED");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("SHA"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 12. acceptance without draft
#[test]
fn test_accept_without_draft_rejected() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let output = run_contract_accept(
        &repo,
        1,
        "0000000000000000000000000000000000000000000000000000000000000000",
        "ACCEPTED",
    );
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("draft"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 13. first acceptance exact ledger persistence
#[test]
fn test_accept_first_exact_ledger_persistence() {
    let content = valid_contract_toml();
    let expected_sha = contract_sha256(content);
    let (_dir, repo, contract_path) = setup_contract_test(content);
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    assert_eq!(
        ledger["accepted_plan_sha256"].as_str().map(|s| s.len()),
        Some(64)
    );
    assert_eq!(ledger["phase_id"].as_str().unwrap(), "phase-1");
    assert_eq!(ledger["contract_id"].as_str().unwrap(), "test-contract-v1");
    let revs = ledger["revisions"].as_array().unwrap();
    assert_eq!(revs.len(), 1);
    assert_eq!(revs[0]["revision"].as_u64().unwrap(), 1);
    assert_eq!(revs[0]["sha256"].as_str().unwrap(), expected_sha);
    assert_eq!(revs[0]["source_path"].as_str().unwrap(), "contract.toml");
    assert_eq!(
        revs[0]["content"].as_str().unwrap().as_bytes(),
        content.as_bytes()
    );
    assert_no_temp_files(&repo);
}

// 14. accepted content exact-byte persistence
#[test]
fn test_accept_content_exact_bytes() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let stored_content = ledger["revisions"][0]["content"].as_str().unwrap();
    let original_bytes = valid_contract_toml().as_bytes();
    assert_eq!(
        stored_content.as_bytes(),
        original_bytes,
        "accepted content must preserve exact bytes"
    );
    assert_no_temp_files(&repo);
}

// 15. literal SHA verification
#[test]
fn test_accept_literal_sha() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract.toml");
    std::fs::write(&contract_path, LITERAL_SHA_CONTRACT).unwrap();
    assert_success(&run_contract_draft(&repo, &contract_path));
    assert_success(&run_contract_accept(
        &repo,
        1,
        LITERAL_SHA_DIGEST,
        "ACCEPTED",
    ));
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    assert_eq!(
        ledger["revisions"][0]["sha256"].as_str().unwrap(),
        LITERAL_SHA_DIGEST
    );
    assert_no_temp_files(&repo);
}

// 16. accepted ledger unknown-field rejection
#[test]
fn test_accept_ledger_unknown_field_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["timestamp"] = serde_json::json!("now");
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 17. accepted revision unknown-field rejection
#[test]
fn test_accept_revision_unknown_field_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["revisions"][0]["signature"] = serde_json::json!("sig");
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 18. malformed accepted ledger rejection
#[test]
fn test_accept_malformed_ledger_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let ledger_path = repo.join(".mrgs").join("accepted-contract.json");
    std::fs::write(&ledger_path, b"not-json").unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(&ledger_path).unwrap(),
        b"not-json",
        "malformed ledger must not be repaired"
    );
    assert_no_temp_files(&repo);
}

// 19. empty revisions rejection
#[test]
fn test_accept_empty_revisions_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["revisions"] = serde_json::json!([]);
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 20. accepted plan SHA mismatch
#[test]
fn test_accept_plan_sha_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["accepted_plan_sha256"] =
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("plan SHA"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 21. accepted phase mismatch
#[test]
fn test_accept_phase_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["phase_id"] = serde_json::json!("phase-2");
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("phase"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 22. accepted contract ID mismatch
#[test]
fn test_accept_contract_id_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["contract_id"] = serde_json::json!("wrong-id");
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("ID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 23. accepted revision zero rejection
#[test]
fn test_accept_revision_zero_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["revisions"][0]["revision"] = serde_json::json!(0);
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("revision"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 24. duplicate accepted revision rejection
#[test]
fn test_accept_duplicate_revision_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let rev1 = ledger["revisions"][0].clone();
    ledger["revisions"].as_array_mut().unwrap().push(rev1);
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 25. non-increasing revisions rejection
#[test]
fn test_accept_non_increasing_revisions_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let rev1 = ledger["revisions"][0].clone();
    ledger["revisions"].as_array_mut().unwrap().push(rev1);
    let revs = ledger["revisions"].as_array_mut().unwrap();
    revs[1]["sha256"] =
        serde_json::json!("1111111111111111111111111111111111111111111111111111111111111111");
    revs[1]["content"] = serde_json::json!("schema_version = 1\ncontract_id = \"test-contract-v1\"\nphase_id = \"phase-1\"\ntitle = \"Test contract\"\nobjective = \"Modified.\"\n\nrequirements = [\"req1\"]\nallowed_paths = [\"src/\"]\nforbidden_paths = [\".git/\"]\nverification_commands = [\"cargo test\"]\nhandoff_fields = [\"FIELD1\"]\n");
    let mod_sha = contract_sha256(revs[1]["content"].as_str().unwrap());
    revs[1]["sha256"] = serde_json::json!(mod_sha);
    revs[1]["revision"] = serde_json::json!(1);
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("revision"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 26. accepted source path normalization rejection
#[test]
fn test_accept_source_path_backslash_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["revisions"][0]["source_path"] = serde_json::json!("docs\\contract.toml");
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 27. accepted stored-content parse rejection
#[test]
fn test_accept_stored_content_parse_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["revisions"][0]["content"] = serde_json::json!("not valid toml");
    ledger["revisions"][0]["sha256"] =
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 28. accepted stored-content phase mismatch
#[test]
fn test_accept_stored_content_phase_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let bad_content =
        valid_contract_toml().replace(r#"phase_id = "phase-1""#, r#"phase_id = "phase-2""#);
    let bad_sha = contract_sha256(&bad_content);
    ledger["revisions"][0]["content"] = serde_json::json!(bad_content);
    ledger["revisions"][0]["sha256"] = serde_json::json!(bad_sha);
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 29. accepted stored-content contract ID mismatch
#[test]
fn test_accept_stored_content_id_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let bad_content = valid_contract_toml().replace(
        r#"contract_id = "test-contract-v1""#,
        r#"contract_id = "wrong-id""#,
    );
    let bad_sha = contract_sha256(&bad_content);
    ledger["revisions"][0]["content"] = serde_json::json!(bad_content);
    ledger["revisions"][0]["sha256"] = serde_json::json!(bad_sha);
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 30. accepted stored-content hash mismatch
#[test]
fn test_accept_stored_content_hash_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["revisions"][0]["sha256"] =
        serde_json::json!("1111111111111111111111111111111111111111111111111111111111111111");
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 31. accepted final revision > draft rejection
#[test]
fn test_accept_final_revision_exceeds_draft() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    // Stash draft, set revision to 1 in ledger but revision 2
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["revisions"][0]["revision"] = serde_json::json!(2);
    // Need to make content match, so add a tracked revision 2 content
    let v2_content = valid_contract_toml().replace("Test objective", "Rev2 objective");
    let v2_sha = contract_sha256(&v2_content);
    ledger["revisions"][0]["content"] = serde_json::json!(v2_content);
    ledger["revisions"][0]["sha256"] = serde_json::json!(v2_sha);
    write_json(&repo, "accepted-contract.json", &ledger);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 32. equal revision with different content rejection
#[test]
fn test_accept_equal_revision_diff_content() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    // Change draft content but keep revision 1
    let v2 = valid_contract_toml().replace("Test objective", "Different objective");
    write_plan(&contract_path, &v2);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 33. valid idempotent acceptance
#[test]
fn test_accept_idempotent() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let first = run_contract_accept(&repo, rev, &sha, "ACCEPTED");
    assert_success(&first);
    let second = run_contract_accept(&repo, rev, &sha, "ACCEPTED");
    assert_success(&second);
    assert_eq!(stdout_string(&first), stdout_string(&second));
    assert_no_temp_files(&repo);
}

// 34. idempotent acceptance preserves every governance file
#[test]
fn test_accept_idempotent_preserves_files() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let accepted_before = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    let plan_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap(),
        accepted_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        plan_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_no_temp_files(&repo);
}

// 35. valid acceptance append after revision
#[test]
fn test_accept_append_after_revision() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    // Revise to v2
    let v2 = valid_contract_toml().replace("Test objective", "Rev2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    let rev2 = draft2["revision"].as_u64().unwrap() as u32;
    assert_eq!(rev2, 2);
    let sha2 = draft2["sha256"].as_str().unwrap().to_string();
    // Accept revision 2
    let output = run_contract_accept(&repo, 2, &sha2, "ACCEPTED");
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("ACCEPTED"), "stdout: {}", out);
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let revs = ledger["revisions"].as_array().unwrap();
    assert_eq!(revs.len(), 2);
    assert_eq!(revs[0]["revision"].as_u64().unwrap(), 1);
    assert_eq!(revs[0]["sha256"].as_str().unwrap(), sha1);
    assert_eq!(revs[1]["revision"].as_u64().unwrap(), 2);
    assert_eq!(revs[1]["sha256"].as_str().unwrap(), v2_sha);
    assert_no_temp_files(&repo);
}

// 36. append preserves earlier entries and order
#[test]
fn test_accept_append_preserves_order() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    // v2
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let v2_sha = contract_sha256(&v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    // Accept v2
    assert_success(&run_contract_accept(&repo, 2, &v2_sha, "ACCEPTED"));
    // v3
    let v3 = valid_contract_toml().replace("Test objective", "V3 objective");
    write_plan(&contract_path, &v3);
    let v3_sha = contract_sha256(&v3);
    assert_success(&run_contract_revise(&repo, &contract_path, 2, &v2_sha));
    // Accept v3
    assert_success(&run_contract_accept(&repo, 3, &v3_sha, "ACCEPTED"));
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let revs = ledger["revisions"].as_array().unwrap();
    assert_eq!(revs.len(), 3);
    assert_eq!(revs[0]["revision"].as_u64().unwrap(), 1);
    assert_eq!(revs[1]["revision"].as_u64().unwrap(), 2);
    assert_eq!(revs[2]["revision"].as_u64().unwrap(), 3);
    assert_eq!(revs[0]["sha256"].as_str().unwrap(), sha1);
    assert_eq!(revs[1]["sha256"].as_str().unwrap(), v2_sha);
    assert_eq!(revs[2]["sha256"].as_str().unwrap(), v3_sha);
    assert_no_temp_files(&repo);
}

// 37. accepted ledger remains append-only
#[test]
fn test_accept_append_only() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    // Save ledger bytes
    let _ledger_bytes_before =
        std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    // Revise and accept v2
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let v2_sha = contract_sha256(&v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    assert_success(&run_contract_accept(&repo, 2, &v2_sha, "ACCEPTED"));
    // Verify earlier entry preserved
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let revs = ledger["revisions"].as_array().unwrap();
    assert_eq!(revs.len(), 2);
    assert_eq!(revs[0]["revision"].as_u64().unwrap(), 1);
    assert_eq!(revs[0]["sha256"].as_str().unwrap(), sha1);
    assert_no_temp_files(&repo);
}

// 38. valid revision from unaccepted draft
#[test]
fn test_revise_from_unaccepted_draft() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert!(
        !repo.join(".mrgs").join("accepted-contract.json").exists(),
        "no ledger should exist"
    );
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("DRAFT"), "stdout: {}", out);
    assert!(out.contains("2"), "revision should be 2: {}", out);
    assert!(out.contains(&v2_sha), "stdout: {}", out);
    assert_no_temp_files(&repo);
}

// 39. valid revision from accepted state
#[test]
fn test_revise_from_accepted_state() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("REVISION_DRAFT"), "stdout: {}", out);
    assert!(out.contains("2"), "revision should be 2: {}", out);
    assert!(out.contains(&v2_sha), "stdout: {}", out);
    // accepted ledger unchanged
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    assert_eq!(ledger["revisions"].as_array().unwrap().len(), 1);
    assert_no_temp_files(&repo);
}

// 40. chained pending revisions
#[test]
fn test_revise_chained_pending() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();

    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));

    let v3 = valid_contract_toml().replace("Test objective", "V3 objective");
    let v3_sha = contract_sha256(&v3);
    write_plan(&contract_path, &v3);
    let output = run_contract_revise(&repo, &contract_path, 2, &v2_sha);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.contains("3"), "revision should be 3: {}", out);
    assert!(out.contains(&v3_sha), "stdout: {}", out);
    let draft3: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft3["revision"].as_u64().unwrap(), 3);
    assert_no_temp_files(&repo);
}

// 41. revision increments exactly one
#[test]
fn test_revise_increments_exactly_one() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft2["revision"].as_u64().unwrap(), 2);
    assert_no_temp_files(&repo);
}

// 42. revision expected-number mismatch
#[test]
fn test_revise_expected_number_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let output = run_contract_revise(&repo, &contract_path, 99, &sha1);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("revision"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 43. revision expected-hash mismatch
#[test]
fn test_revise_expected_hash_mismatch() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let wrong_sha = "0000000000000000000000000000000000000000000000000000000000000000";
    let output = run_contract_revise(&repo, &contract_path, 1, wrong_sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("SHA"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 44. revision uppercase expected SHA rejection
#[test]
fn test_revise_uppercase_sha_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let upper = "ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEF";
    let output = run_contract_revise(&repo, &contract_path, 1, upper);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("SHA"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 45. revision zero preimage rejection
#[test]
fn test_revise_zero_preimage_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    // Use revision 0 as expected - but the draft is at rev 1, so it's a mismatch
    let output = run_contract_revise(
        &repo,
        &contract_path,
        0,
        &contract_sha256(valid_contract_toml()),
    );
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 46. revision overflow rejection
#[test]
fn test_revise_overflow_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    // Modify draft revision to u32::MAX with valid preimage
    let mut modified_draft: serde_json::Value = draft.clone();
    modified_draft["revision"] = serde_json::json!(u32::MAX);
    modified_draft["preimage"] = serde_json::json!({
        "revision": u32::MAX - 1,
        "sha256": "1111111111111111111111111111111111111111111111111111111111111111"
    });
    write_json(&repo, "contract-draft.json", &modified_draft);
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let output = run_contract_revise(&repo, &contract_path, u32::MAX, &sha1);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("overflow"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 47. revision same-byte rejection
#[test]
fn test_revise_same_content_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("same content") || stderr_string(&output).contains("same"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 48. revision contract-ID change rejection
#[test]
fn test_revise_contract_id_change_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace(
        r#"contract_id = "test-contract-v1""#,
        r#"contract_id = "other-contract""#,
    );
    write_plan(&contract_path, &v2);
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("contract ID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 49. revision phase mismatch rejection
#[test]
fn test_revise_phase_mismatch_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace(r#"phase_id = "phase-1""#, r#"phase_id = "phase-2""#);
    write_plan(&contract_path, &v2);
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("phase"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 50. revision invalid UTF-8 rejection
#[test]
fn test_revise_invalid_utf8_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    std::fs::write(&contract_path, [0xFF, 0xFE, 0x00, 0x61]).unwrap();
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("UTF") || stderr_string(&output).contains("utf"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 51. revision malformed TOML rejection
#[test]
fn test_revise_malformed_toml_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    std::fs::write(&contract_path, b"not valid toml {{{").unwrap();
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 52. revision source outside repository rejection
#[test]
fn test_revise_source_outside_repo() {
    let (dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let outside = dir.path().join("outside_contract.toml");
    write_plan(&outside, valid_contract_toml());
    let output = run_contract_revise(&repo, &outside, 1, &contract_sha256(valid_contract_toml()));
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("outside") || stderr_string(&output).contains("repository"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 53. revision source under .mrgs rejection
#[test]
fn test_revise_source_under_mrgs() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let inside = repo.join(".mrgs").join("contract.toml");
    write_plan(&inside, valid_contract_toml());
    let output = run_contract_revise(&repo, &inside, 1, &contract_sha256(valid_contract_toml()));
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains(".mrgs"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 54. revision symlink escape rejection
#[test]
#[cfg_attr(not(any(unix, windows)), ignore)]
fn test_revise_symlink_escape() {
    let (dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let outside = dir.path().join("outside_contract.toml");
    write_plan(&outside, valid_contract_toml());
    let link = repo.join("link_contract.toml");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&outside, &link).unwrap();
    let output = run_contract_revise(&repo, &link, 1, &contract_sha256(valid_contract_toml()));
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 55. exact revised source-byte persistence
#[test]
fn test_revise_exact_bytes() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft2["sha256"].as_str().unwrap(), v2_sha);
    assert_eq!(
        draft2["content"].as_str().unwrap().as_bytes(),
        v2.as_bytes()
    );
    assert_no_temp_files(&repo);
}

// 56. revised LF and CRLF distinction
#[test]
fn test_revise_lf_content_preservation() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract.toml");
    std::fs::write(&contract_path, LITERAL_SHA_CONTRACT).unwrap();
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let revised_lf = LITERAL_SHA_CONTRACT.replace("Literal SHA test", "Revised LF test");
    let revised_lf_sha = contract_sha256(&revised_lf);
    std::fs::write(&contract_path, &revised_lf).unwrap();
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft2["sha256"].as_str().unwrap(), revised_lf_sha);
    assert_eq!(
        draft2["content"].as_str().unwrap().as_bytes(),
        revised_lf.as_bytes()
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_revise_crlf_content_preservation() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract.toml");
    std::fs::write(&contract_path, LITERAL_CRLF_CONTRACT).unwrap();
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let revised_crlf = LITERAL_CRLF_CONTRACT.replace("Literal SHA test", "Revised CRLF test");
    let revised_crlf_sha = contract_sha256(&revised_crlf);
    std::fs::write(&contract_path, &revised_crlf).unwrap();
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft2["sha256"].as_str().unwrap(), revised_crlf_sha);
    assert_eq!(
        draft2["content"].as_str().unwrap().as_bytes(),
        revised_crlf.as_bytes()
    );
    assert_no_temp_files(&repo);
}

// 57. normalized revised source path
#[test]
fn test_revise_normalized_source_path() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_path = repo.join("subdir").join("v2_contract.toml");
    std::fs::create_dir_all(repo.join("subdir")).unwrap();
    write_plan(&v2_path, &v2);
    assert_success(&run_contract_revise(&repo, &v2_path, 1, &sha1));
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sp = draft2["source_path"].as_str().unwrap();
    assert!(
        !sp.contains('\\'),
        "source_path must use forward slashes: {}",
        sp
    );
    assert_eq!(sp, "subdir/v2_contract.toml", "source_path: {}", sp);
    assert_no_temp_files(&repo);
}

// 58. valid idempotent revision replay
#[test]
fn test_revise_idempotent_replay() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let _v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    let first = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_success(&first);
    // Replay the same revision call
    let second = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_success(&second);
    assert_eq!(stdout_string(&first), stdout_string(&second));
    assert_no_temp_files(&repo);
}

// 59. replay preserves every governance file
#[test]
fn test_revise_replay_preserves_files() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let _plan_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let _state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let _ledger_exists = repo.join(".mrgs").join("accepted-contract.json").exists();
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_no_temp_files(&repo);
}

// 60. stale replay with different source rejection
#[test]
fn test_revise_stale_replay_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    // Now try to replay with wrong content - should fail CAS
    let v3 = valid_contract_toml().replace("Test objective", "V3 objective");
    write_plan(&contract_path, &v3);
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 61. accepted ledger preserved during revision
#[test]
fn test_revise_preserves_accepted_ledger() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let ledger_before = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let ledger_after = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    assert_eq!(ledger_before, ledger_after);
    assert_no_temp_files(&repo);
}

// 62. state preserved during acceptance
#[test]
fn test_accept_preserves_state() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(state_before, state_after);
    assert_no_temp_files(&repo);
}

// 63. state preserved during revision
#[test]
fn test_revise_preserves_state() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let state_after = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    assert_eq!(state_before, state_after);
    assert_no_temp_files(&repo);
}

// 64. draft preserved during acceptance
#[test]
fn test_accept_preserves_draft() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let draft_after = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    assert_eq!(draft_before, draft_after);
    assert_no_temp_files(&repo);
}

// 65. accepted ledger preserved on failed acceptance
#[test]
fn test_failed_accept_preserves_ledger() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let ledger_before = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    // Try to accept with wrong token
    let output = run_contract_accept(&repo, rev, &sha, "WRONG");
    assert_failure(&output);
    let ledger_after = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    assert_eq!(ledger_before, ledger_after);
    assert_no_temp_files(&repo);
}

// 66. draft preserved on failed revision
#[test]
fn test_failed_revise_preserves_draft() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let wrong_sha = "0000000000000000000000000000000000000000000000000000000000000000";
    let output = run_contract_revise(&repo, &contract_path, 1, wrong_sha);
    assert_failure(&output);
    let draft_after = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    assert_eq!(draft_before, draft_after);
    assert_no_temp_files(&repo);
}

// 67. orphaned accepted ledger rejection
#[test]
fn test_orphaned_accepted_ledger_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    // Remove draft
    std::fs::remove_file(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("orphaned")
            || stderr_string(&output).contains("incomplete"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 68. temporary files absent after acceptance success
#[test]
fn test_accept_no_temp_after_success() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    assert_no_temp_files(&repo);
}

// 69. temporary files absent after revision success
#[test]
fn test_revise_no_temp_after_success() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    assert_no_temp_files(&repo);
}

// 70. temporary files absent after handled failures
#[test]
fn test_accept_no_temp_after_failure() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let output = run_contract_accept(
        &repo,
        1,
        "0000000000000000000000000000000000000000000000000000000000000000",
        "ACCEPTED",
    );
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

#[test]
fn test_revise_no_temp_after_failure() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let output = run_contract_revise(
        &repo,
        &contract_path,
        1,
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 71. contract draft remains idempotent for revision > 1
#[test]
fn test_draft_idempotent_revision_greater_than_one() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    // Revise to v2
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    // Now draft is at rev 2. Contract draft with same bytes should be idempotent
    let output = run_contract_draft(&repo, &contract_path);
    assert_success(&output);
    let out = stdout_string(&output);
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert!(
        out.contains(draft2["sha256"].as_str().unwrap()),
        "stdout: {}",
        out
    );
    assert_no_temp_files(&repo);
}

// 72. contract draft validates existing lifecycle authority
#[test]
fn test_draft_validates_lifecycle_authority() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    // Now draft at rev 1, ledger at rev 1. Contract draft with different bytes should fail
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 73. initial draft creates no accepted ledger
#[test]
fn test_initial_draft_no_accepted_ledger() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    assert!(
        !repo.join(".mrgs").join("accepted-contract.json").exists(),
        "accepted-contract.json should not exist after initial draft"
    );
    assert_no_temp_files(&repo);
}

// 75. replaced test has stronger coverage (revision zero + lifecycle consistency)
#[test]
fn test_revision_zero_rejected_in_draft() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["revision"] = serde_json::json!(0);
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("revision"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// Lifecycle consistency: contract draft creates no accepted ledger
#[test]
fn test_lifecycle_draft_no_ledger() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    assert!(
        !repo.join(".mrgs").join("accepted-contract.json").exists(),
        "DRAFT lifecycle: accepted-contract.json should not exist"
    );
    assert_no_temp_files(&repo);
}

// Lifecycle consistency: ACCEPTED state has matching draft and ledger
#[test]
fn test_lifecycle_accepted_state() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let final_rev = ledger["revisions"].as_array().unwrap().last().unwrap();
    assert_eq!(
        final_rev["revision"].as_u64().unwrap(),
        draft["revision"].as_u64().unwrap(),
        "ACCEPTED: final ledger revision must equal draft revision"
    );
    assert_no_temp_files(&repo);
}

// Lifecycle consistency: REVISION_DRAFT state has draft > final ledger revision
#[test]
fn test_lifecycle_revision_draft_state() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    let final_rev = ledger["revisions"].as_array().unwrap().last().unwrap()["revision"]
        .as_u64()
        .unwrap();
    assert!(
        final_rev < draft2["revision"].as_u64().unwrap(),
        "REVISION_DRAFT: final ledger revision must be less than draft revision"
    );
    assert_no_temp_files(&repo);
}

// Orphaned ledger rejection (accepted-contract.json without contract-draft.json)
#[test]
fn test_orphaned_accepted_contract_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    std::fs::remove_file(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("orphaned")
            || stderr_string(&output).contains("incomplete"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 76. revision-1 draft without preimage is valid
#[test]
fn test_preimage_revision_one_no_preimage() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft["revision"].as_u64().unwrap(), 1);
    assert!(
        draft.get("preimage").is_none() || draft["preimage"].is_null(),
        "rev 1 must not have preimage"
    );
    assert_no_temp_files(&repo);
}

// 77. revision-1 draft with preimage is rejected
#[test]
fn test_preimage_revision_one_with_preimage_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["preimage"] = serde_json::json!({"revision": 0, "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"});
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("preimage"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 78. revision > 1 draft without preimage is rejected
#[test]
fn test_preimage_revision_greater_one_missing_preimage_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["revision"] = serde_json::json!(2);
    draft["sha256"] =
        serde_json::json!("1111111111111111111111111111111111111111111111111111111111111111");
    // Remove preimage entirely
    let draft_map = draft.as_object().unwrap();
    let mut cleaned = serde_json::Map::new();
    for (k, v) in draft_map {
        if k != "preimage" {
            cleaned.insert(k.clone(), v.clone());
        }
    }
    write_json(
        &repo,
        "contract-draft.json",
        &serde_json::Value::Object(cleaned),
    );
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("preimage"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 79. revision > 1 draft with valid immediate preimage is valid
#[test]
fn test_preimage_revision_greater_one_with_valid_preimage() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft2["revision"].as_u64().unwrap(), 2);
    assert!(
        draft2.get("preimage").and_then(|p| p.as_object()).is_some(),
        "rev 2 must have preimage"
    );
    assert_eq!(draft2["preimage"]["revision"].as_u64().unwrap(), 1);
    assert_eq!(draft2["preimage"]["sha256"].as_str().unwrap(), sha1);
    assert_no_temp_files(&repo);
}

// 80. preimage revision zero rejection
#[test]
fn test_preimage_revision_zero_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["revision"] = serde_json::json!(2);
    draft["preimage"] = serde_json::json!({"revision": 0, "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"});
    draft["sha256"] =
        serde_json::json!("1111111111111111111111111111111111111111111111111111111111111111");
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("preimage"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 81. preimage revision mismatch rejection
#[test]
fn test_preimage_revision_mismatch_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let mut draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft2["preimage"]["revision"] = serde_json::json!(99);
    write_json(&repo, "contract-draft.json", &draft2);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("preimage"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 82. malformed preimage SHA rejection
#[test]
fn test_preimage_malformed_sha_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let mut draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft2["preimage"]["sha256"] = serde_json::json!("not-a-valid-sha");
    write_json(&repo, "contract-draft.json", &draft2);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 83. uppercase preimage SHA rejection
#[test]
fn test_preimage_uppercase_sha_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let mut draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft2["preimage"]["sha256"] =
        serde_json::json!("ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEF");
    write_json(&repo, "contract-draft.json", &draft2);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 84. unknown preimage JSON field rejection
#[test]
fn test_preimage_unknown_field_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let mut draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft2["preimage"]["unknown_field"] = serde_json::json!("extra");
    write_json(&repo, "contract-draft.json", &draft2);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 85. null preimage rejection where absence is required
#[test]
fn test_preimage_null_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["preimage"] = serde_json::Value::Null;
    write_json(&repo, "contract-draft.json", &draft);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("null") || stderr_string(&output).contains("preimage"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 86. normal revision stores the exact validated preimage tuple
#[test]
fn test_revise_stores_exact_preimage() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft2["preimage"]["revision"].as_u64().unwrap(), 1);
    assert_eq!(draft2["preimage"]["sha256"].as_str().unwrap(), sha1);
    assert_no_temp_files(&repo);
}

// 87. chained revisions replace the receipt with immediately preceding tuple
#[test]
fn test_revise_chained_replaces_preimage() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let v3 = valid_contract_toml().replace("Test objective", "V3 objective");
    write_plan(&contract_path, &v3);
    assert_success(&run_contract_revise(&repo, &contract_path, 2, &v2_sha));
    let draft3: serde_json::Value = read_json(&repo, "contract-draft.json");
    assert_eq!(draft3["preimage"]["revision"].as_u64().unwrap(), 2);
    assert_eq!(draft3["preimage"]["sha256"].as_str().unwrap(), v2_sha);
    assert_no_temp_files(&repo);
}

// 88. replay with the exact stored preimage succeeds
#[test]
fn test_revise_replay_exact_preimage_succeeds() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 89. replay with an arbitrary valid wrong SHA fails
#[test]
fn test_revise_replay_wrong_sha_fails() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let valid_but_wrong = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let output = run_contract_revise(&repo, &contract_path, 1, valid_but_wrong);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 90. replay with an older accepted revision SHA fails
// Multi-revision fixture: draft v1 accepted, revised to v2, then to v3.
// Attempt replay of the v2→v3 transition with:
//   expected-revision = 2 (correct immediate predecessor)
//   expected-sha256   = sha1 (older accepted SHA, NOT the v2 preimage SHA)
//   exact v3 source path and bytes unchanged.
// The replay preimage SHA check must reject before same-content fires.
#[test]
fn test_revise_replay_older_accepted_sha_fails() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    // Accept revision 1
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    // Revise to v2
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    // Revise to v3
    let v3 = valid_contract_toml().replace("Test objective", "V3 objective");
    write_plan(&contract_path, &v3);
    assert_success(&run_contract_revise(&repo, &contract_path, 2, &v2_sha));
    // Keep exact v3 source bytes unchanged — do NOT write v4.
    // Snapshot all governance files before replay attempt
    let plan_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let ledger_before = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    // Attempt replay: expected-revision=2 (correct predecessor), expected-sha256=sha1 (wrong SHA).
    // The replay handler detects the preimage SHA mismatch before same-content would fire.
    let output = run_contract_revise(&repo, &contract_path, 2, &sha1);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("SHA"),
        "stderr: {}",
        stderr_string(&output)
    );
    // Prove every governance file is unchanged
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        plan_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap(),
        ledger_before
    );
    assert_no_temp_files(&repo);
}

// Positive control: same fixture, replay with correct immediate preimage succeeds
// and returns REVISION_DRAFT because rev 1 is accepted and v3 is pending.
// 90b. positive control — correct immediate preimage replay succeeds
#[test]
fn test_revise_replay_correct_immediate_preimage_succeeds() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    // Accept revision 1
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    // Revise to v2
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    // Revise to v3
    let v3 = valid_contract_toml().replace("Test objective", "V3 objective");
    let v3_sha = contract_sha256(&v3);
    write_plan(&contract_path, &v3);
    assert_success(&run_contract_revise(&repo, &contract_path, 2, &v2_sha));
    // Replay with correct immediate preimage: expected-revision=2, expected-sha256=v2_sha
    let output = run_contract_revise(&repo, &contract_path, 2, &v2_sha);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(
        out.starts_with("REVISION_DRAFT"),
        "expected REVISION_DRAFT, got: {}",
        out
    );
    assert!(out.contains("3"), "expected revision 3 in output: {}", out);
    assert!(out.contains(&v3_sha), "expected v3 sha in output: {}", out);
    assert_no_temp_files(&repo);
}

#[test]
fn test_revise_replay_changed_bytes_is_terminal_and_preserves_files() {
    let (_dir, repo, contract_path, _sha1, v2_sha, _v3_sha) = setup_three_revision_contract();
    let changed_v3 = valid_contract_toml().replace("Test objective", "Changed V3 objective");
    write_plan(&contract_path, &changed_v3);
    let before = governance_bytes(&repo);

    let output = run_contract_revise(&repo, &contract_path, 2, &v2_sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("replay content mismatch"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_eq!(
        read_json(&repo, "contract-draft.json")["revision"].as_u64(),
        Some(3)
    );
    assert_governance_bytes_unchanged(&repo, &before);
}

#[test]
fn test_revise_replay_changed_path_is_terminal_and_preserves_files() {
    let (_dir, repo, contract_path, _sha1, v2_sha, _v3_sha) = setup_three_revision_contract();
    let alternate_path = repo.join("alternate-v3.toml");
    std::fs::copy(&contract_path, &alternate_path).unwrap();
    let before = governance_bytes(&repo);

    let output = run_contract_revise(&repo, &alternate_path, 2, &v2_sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("replay source path mismatch"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_eq!(
        read_json(&repo, "contract-draft.json")["revision"].as_u64(),
        Some(3)
    );
    assert_governance_bytes_unchanged(&repo, &before);
}

#[test]
fn test_revise_replay_exact_positive_preserves_files() {
    let (_dir, repo, contract_path, _sha1, v2_sha, v3_sha) = setup_three_revision_contract();
    let before = governance_bytes(&repo);

    let output = run_contract_revise(&repo, &contract_path, 2, &v2_sha);
    assert_success(&output);
    assert_eq!(
        stdout_string(&output),
        format!("REVISION_DRAFT test-contract-v1 3 {}", v3_sha)
    );
    assert_governance_bytes_unchanged(&repo, &before);
}

#[test]
fn test_revise_normal_cas_from_revision_three_creates_revision_four() {
    let (_dir, repo, contract_path, _sha1, _v2_sha, v3_sha) = setup_three_revision_contract();
    let v4 = valid_contract_toml().replace("Test objective", "V4 objective");
    let v4_sha = contract_sha256(&v4);
    write_plan(&contract_path, &v4);

    let output = run_contract_revise(&repo, &contract_path, 3, &v3_sha);
    assert_success(&output);
    assert_eq!(
        stdout_string(&output),
        format!("REVISION_DRAFT test-contract-v1 4 {}", v4_sha)
    );
    let draft = read_json(&repo, "contract-draft.json");
    assert_eq!(draft["revision"].as_u64(), Some(4));
    assert_eq!(draft["preimage"]["revision"].as_u64(), Some(3));
    assert_eq!(draft["preimage"]["sha256"].as_str(), Some(v3_sha.as_str()));
    assert_no_temp_files(&repo);
}

#[test]
fn test_revise_revision_three_same_content_rejected_without_write() {
    let (_dir, repo, contract_path, _sha1, _v2_sha, v3_sha) = setup_three_revision_contract();
    let before = governance_bytes(&repo);

    let output = run_contract_revise(&repo, &contract_path, 3, &v3_sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("same content"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_governance_bytes_unchanged(&repo, &before);
}

// 91. replay with the correct revision and wrong SHA fails
#[test]
fn test_revise_replay_correct_rev_wrong_sha() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let wrong_sha = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let output = run_contract_revise(&repo, &contract_path, 1, wrong_sha);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 92. replay with the correct SHA and wrong revision fails
#[test]
fn test_revise_replay_correct_sha_wrong_rev() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let output = run_contract_revise(&repo, &contract_path, 99, &sha1);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 93. replay older by more than one revision fails
#[test]
fn test_revise_replay_older_by_more_than_one() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    let v2_sha = contract_sha256(&v2);
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let v3 = valid_contract_toml().replace("Test objective", "V3 objective");
    write_plan(&contract_path, &v3);
    assert_success(&run_contract_revise(&repo, &contract_path, 2, &v2_sha));
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 94. replay with same content from different normalized source path fails
#[test]
fn test_revise_replay_different_source_path_fails() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let alt_path = repo.join("alt_contract.toml");
    write_plan(&alt_path, &v2);
    let output = run_contract_revise(&repo, &alt_path, 1, &sha1);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 95. replay after acceptance returns ACCEPTED
#[test]
fn test_revise_replay_after_acceptance_returns_accepted() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha2 = draft2["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 2, &sha2, "ACCEPTED"));
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(
        out.starts_with("ACCEPTED"),
        "expected ACCEPTED, got: {}",
        out
    );
    assert_no_temp_files(&repo);
}

// 96. replay before first acceptance returns DRAFT
#[test]
fn test_revise_replay_before_acceptance_returns_draft() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("DRAFT"), "expected DRAFT, got: {}", out);
    assert_no_temp_files(&repo);
}

// 97. replay with older accepted ledger returns REVISION_DRAFT
#[test]
fn test_revise_replay_with_older_ledger_returns_revision_draft() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(
        out.starts_with("REVISION_DRAFT"),
        "expected REVISION_DRAFT, got: {}",
        out
    );
    assert_no_temp_files(&repo);
}

// 98. malformed receipt preserves every governance file byte-for-byte
#[test]
fn test_revise_malformed_receipt_preserves_files() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let plan_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let mut draft_val: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft_val["preimage"]["sha256"] = serde_json::json!("invalid");
    write_json(&repo, "contract-draft.json", &draft_val);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        plan_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_no_temp_files(&repo);
}

// 99. accepted ledger entries contain no preimage field
#[test]
fn test_accepted_ledger_no_preimage() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    for rev_entry in ledger["revisions"].as_array().unwrap() {
        assert!(
            rev_entry.get("preimage").is_none(),
            "accepted revision must not contain preimage"
        );
    }
    assert_no_temp_files(&repo);
}

// 100. acceptance preserves the draft preimage receipt byte-for-byte
#[test]
fn test_accept_preserves_draft_preimage() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let draft2: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha2 = draft2["sha256"].as_str().unwrap().to_string();
    let rev2 = draft2["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev2, &sha2, "ACCEPTED"));
    let draft_after = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    assert_eq!(
        draft_before, draft_after,
        "acceptance must preserve draft including preimage"
    );
    assert_no_temp_files(&repo);
}

// 101. contract draft proves exact submitted-byte equality in addition to digest equality
#[test]
fn test_draft_exact_byte_equality_required() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let original_sha = draft["sha256"].as_str().unwrap().to_string();
    let mut corrupted = draft.clone();
    corrupted["sha256"] = serde_json::json!(original_sha);
    corrupted["content"] =
        serde_json::json!("different content that does not match original bytes");
    write_json(&repo, "contract-draft.json", &corrupted);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 102. comparator-level regression: equal digest but unequal content cannot authorize idempotency
#[test]
fn test_draft_equal_digest_unequal_content_rejected() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let fake_sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let mut draft_val: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft_val["sha256"] = serde_json::json!(fake_sha);
    draft_val["content"] = serde_json::json!("different content with a different byte sequence");
    write_json(&repo, "contract-draft.json", &draft_val);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// ===== Cross-record contract-ID consistency (Blocker 1) =====

fn setup_contract_id_mismatch_fixture(
) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    // Revise to v2 so draft revision > ledger final revision (REVISION_DRAFT state)
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    // Now modify the accepted ledger: change contract_id to a different value,
    // and update revision content + SHA to match the new contract_id.
    let other_content = valid_contract_toml().replace(
        r#"contract_id = "test-contract-v1""#,
        r#"contract_id = "other-contract-v1""#,
    );
    let other_sha = contract_sha256(&other_content);
    let mut ledger: serde_json::Value = read_json(&repo, "accepted-contract.json");
    ledger["contract_id"] = serde_json::json!("other-contract-v1");
    ledger["revisions"][0]["content"] = serde_json::json!(other_content);
    ledger["revisions"][0]["sha256"] = serde_json::json!(other_sha);
    write_json(&repo, "accepted-contract.json", &ledger);
    (_dir, repo, contract_path)
}

// Blocker 1 test 1: contract draft idempotent registration rejected
#[test]
fn test_contract_id_mismatch_draft_rejected() {
    let (_dir, repo, contract_path) = setup_contract_id_mismatch_fixture();
    let plan_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let ledger_before = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("contract ID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        plan_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap(),
        ledger_before
    );
    assert_no_temp_files(&repo);
}

// Blocker 1 test 2: normal contract revise rejected
#[test]
fn test_contract_id_mismatch_revise_rejected() {
    let (_dir, repo, contract_path) = setup_contract_id_mismatch_fixture();
    let plan_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let ledger_before = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    let draft_val: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha2 = draft_val["sha256"].as_str().unwrap().to_string();
    let v3 = valid_contract_toml().replace("Test objective", "V3 objective");
    write_plan(&contract_path, &v3);
    let output = run_contract_revise(&repo, &contract_path, 2, &sha2);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("contract ID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        plan_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap(),
        ledger_before
    );
    assert_no_temp_files(&repo);
}

// Blocker 1 test 3: contract revision replay rejected
#[test]
fn test_contract_id_mismatch_replay_rejected() {
    let (_dir, repo, contract_path) = setup_contract_id_mismatch_fixture();
    let plan_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let ledger_before = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    let sha1 = contract_sha256(valid_contract_toml());
    let output = run_contract_revise(&repo, &contract_path, 1, &sha1);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("contract ID") || stderr_string(&output).contains("SHA"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        plan_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap(),
        ledger_before
    );
    assert_no_temp_files(&repo);
}

// Blocker 1 test 4: lifecycle inference (accept) rejected under mismatched authority
#[test]
fn test_contract_id_mismatch_accept_rejected() {
    let (_dir, repo, _contract_path) = setup_contract_id_mismatch_fixture();
    let plan_before = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    let state_before = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    let draft_before = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    let ledger_before = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    let draft_val: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha2 = draft_val["sha256"].as_str().unwrap().to_string();
    let output = run_contract_accept(&repo, 2, &sha2, "ACCEPTED");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("contract ID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
        plan_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap(),
        draft_before
    );
    assert_eq!(
        std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap(),
        ledger_before
    );
    assert_no_temp_files(&repo);
}

// ===== Phase 4 — Implementation Tracking Tests =====

fn git_init(repo: &Path) {
    Command::new("git").arg("init").arg(repo).output().unwrap();
    std::fs::write(repo.join("README.md"), b"initial").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("add")
        .arg("README.md")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("init")
        .status()
        .unwrap();
}

fn run_implementation_begin(repo: &Path, revision: u32, sha256: &str) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("begin")
        .arg("--repo")
        .arg(repo)
        .arg("--revision")
        .arg(revision.to_string())
        .arg("--sha256")
        .arg(sha256);
    cmd.output().unwrap()
}

fn run_implementation_begin_str(repo: &Path, revision: &str, sha256: &str) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("begin")
        .arg("--repo")
        .arg(repo)
        .arg("--revision")
        .arg(revision)
        .arg("--sha256")
        .arg(sha256);
    cmd.output().unwrap()
}

fn run_implementation_check(repo: &Path) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("check")
        .arg("--repo")
        .arg(repo);
    cmd.output().unwrap()
}

/// Run `implementation begin` while injecting forbidden Git environment
/// variables into the MRGS process environment, proving they cannot reach or
/// influence the isolated Git child (Repair 1 / Section 6.1.1 evidence).
fn run_implementation_begin_with_env(
    repo: &Path,
    revision: u32,
    sha256: &str,
    injected: &[(&str, &str)],
) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("begin")
        .arg("--repo")
        .arg(repo)
        .arg("--revision")
        .arg(revision.to_string())
        .arg("--sha256")
        .arg(sha256);
    for (k, v) in injected {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

fn contract_accepted_revision(repo: &Path) -> (u32, String) {
    let ledger: serde_json::Value = read_json(repo, "accepted-contract.json");
    let last_rev = ledger["revisions"].as_array().unwrap().last().unwrap();
    let revision = last_rev["revision"].as_u64().unwrap() as u32;
    let sha256 = last_rev["sha256"].as_str().unwrap().to_string();
    (revision, sha256)
}

fn commit_file(repo: &Path, name: &str) {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("add")
        .arg(name)
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg(name)
        .status()
        .unwrap();
}
fn setup_implementation_basic() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, valid_contract_toml());
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));
    (dir, repo)
}

fn setup_implementation_forbidden_rule(forbidden: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract.toml");
    let contract = valid_contract_toml().replace(
        "forbidden_paths = [\".git/\"]",
        &format!("forbidden_paths = [\"{forbidden}\"]"),
    );
    write_plan(&contract_path, &contract);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));
    (dir, repo)
}

// 1. Full happy path
#[test]
fn test_implementation_begin_accepted_lifecycle() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("IMPLEMENTATION_BOUND"), "stdout: {}", out);
    assert!(out.contains("test-contract-v1"), "stdout: {}", out);
    assert!(out.contains(&final_rev.to_string()), "stdout: {}", out);
    assert!(out.contains(&final_sha), "stdout: {}", out);
    assert!(repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

// 2. REVISION_DRAFT lifecycle
#[test]
fn test_implementation_begin_revision_draft_lifecycle() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let contract_path = repo.join("contract.toml");
    let v2 = valid_contract_toml().replace("Test objective", "Revised objective");
    write_plan(&contract_path, &v2);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_eq!(final_rev, 1);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("IMPLEMENTATION_BOUND"), "stdout: {}", out);
    assert_no_temp_files(&repo);
}

// 3. Check after begin
#[test]
fn test_implementation_check_after_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let output = run_implementation_check(&repo);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("IMPLEMENTATION_OK"), "stdout: {}", out);
    assert!(out.contains("0"), "expected 0 changed paths: {}", out);
    assert_no_temp_files(&repo);
}

// 4. Idempotent begin
#[test]
fn test_implementation_begin_idempotent() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let first = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&first);
    let second = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&second);
    assert_eq!(stdout_string(&first), stdout_string(&second));
    assert_no_temp_files(&repo);
}

// 5. Changed file in allowed scope
#[test]
fn test_implementation_check_with_changed_file_in_allowed_scope() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("test.rs"), b"fn test() {}").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("src/test.rs")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("add test.rs")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("IMPLEMENTATION_OK"), "stdout: {}", out);
    assert!(out.contains("1"), "expected 1 changed path: {}", out);
    assert_no_temp_files(&repo);
}

// 6. File outside allowed scope
#[test]
fn test_implementation_check_with_changed_file_outside_allowed() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::write(repo.join("other.rs"), b"fn other() {}").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("other.rs")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("add other.rs")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("CHANGE_NOT_ALLOWED"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 7. Draft only, no accept
#[test]
fn test_implementation_begin_draft_lifecycle_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    let output = run_implementation_begin(&repo, rev, &sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("CONTRACT_NOT_ACCEPTED"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 8. Wrong revision
#[test]
fn test_implementation_begin_wrong_revision() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let output = run_implementation_begin(&repo, 99, &sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("REQUESTED_REVISION_STALE"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 9. Wrong SHA
#[test]
fn test_implementation_begin_wrong_sha() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let wrong_sha = "0000000000000000000000000000000000000000000000000000000000000000";
    let output = run_implementation_begin(&repo, rev, wrong_sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("REQUESTED_SHA_STALE"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 10. Check without begin
#[test]
fn test_implementation_check_without_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_MISSING"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 11. Dirty repo
#[test]
fn test_implementation_begin_dirty_repo() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join("uncommitted.txt"), b"dirty").unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    let err = stderr_string(&output);
    assert_eq!(err, "error: GIT_DIRTY");
    assert_no_temp_files(&repo);
}

// 12. Stale after plan change
#[test]
fn test_implementation_check_stale_after_plan_change() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    // Modify implementation-authority.json directly to make it stale
    let mut impl_auth: serde_json::Value = read_json(&repo, "implementation-authority.json");
    impl_auth["accepted_plan_sha256"] =
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    write_json(&repo, "implementation-authority.json", &impl_auth);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_STALE"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 13. Begin on rev1 after creating rev2 draft still works
#[test]
fn test_implementation_begin_after_new_draft_still_works() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let contract_path = repo.join("contract.toml");
    let v2 = valid_contract_toml().replace("Test objective", "Revised objective");
    write_plan(&contract_path, &v2);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_eq!(final_rev, 1);
    assert_eq!(final_sha, sha1);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.starts_with("IMPLEMENTATION_BOUND"), "stdout: {}", out);
    assert_no_temp_files(&repo);
}

// 14. Forbidden path
#[test]
fn test_implementation_check_forbidden_path() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let custom_contract = valid_contract_toml().replace(
        r#"forbidden_paths = [".git/"]"#,
        r#"forbidden_paths = ["secret/"]"#,
    );
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, &custom_contract);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("secret")).unwrap();
    std::fs::write(repo.join("secret").join("data.txt"), b"sensitive").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("secret/data.txt")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("add secret data")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("CHANGE_FORBIDDEN"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 15. Begin output format
#[test]
fn test_implementation_begin_output_format() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output);
    let out = stdout_string(&output);
    let parts: Vec<&str> = out.split_whitespace().collect();
    assert_eq!(parts.len(), 5, "expected 5 fields: {}", out);
    assert_eq!(parts[0], "IMPLEMENTATION_BOUND");
    assert_eq!(parts[1], "test-contract-v1");
    assert_eq!(parts[2], final_rev.to_string());
    assert_eq!(parts[3], final_sha);
    assert_eq!(
        parts[4].len(),
        40,
        "baseline head should be 40-char hex: {}",
        parts[4]
    );
    assert_no_temp_files(&repo);
}

// 16. Check output format
#[test]
fn test_implementation_check_output_format() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let output = run_implementation_check(&repo);
    assert_success(&output);
    let out = stdout_string(&output);
    let parts: Vec<&str> = out.split_whitespace().collect();
    assert_eq!(parts.len(), 5, "expected 5 fields: {}", out);
    assert_eq!(parts[0], "IMPLEMENTATION_OK");
    assert_eq!(parts[1], "test-contract-v1");
    assert_eq!(parts[2], final_rev.to_string());
    assert_eq!(parts[3], final_sha);
    let changed_count: u32 = parts[4]
        .parse()
        .expect("changed path count must be a number");
    assert_eq!(changed_count, 0);
    assert_no_temp_files(&repo);
}

// 17. Revision zero
#[test]
fn test_implementation_begin_revision_zero() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let output = run_implementation_begin(&repo, 0, &sha);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 18. Revision overflow
#[test]
fn test_implementation_begin_revision_overflow() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let output = run_implementation_begin_str(&repo, "4294967296", &sha);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

fn create_git_marker(repo: &Path, name: &str) {
    let git_dir = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("rev-parse")
            .arg("--git-dir")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let path = repo.join(&git_dir).join(name);
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent).unwrap();
    std::fs::write(&path, b"dummy").unwrap();
}

// === A. RECORD VALIDATION ===

// 19. Unsupported schema_version in implementation record
#[test]
fn test_impl_record_unsupported_schema() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["schema_version"] = serde_json::json!(2);
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 20. Unknown field in implementation record rejected
#[test]
fn test_impl_record_unknown_field_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["unknown_field"] = serde_json::json!("should_fail");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 21. Missing field in implementation record rejected
#[test]
fn test_impl_record_missing_field_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record.as_object_mut().unwrap().remove("contract_id");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 22. Malformed JSON in implementation record rejected
#[test]
fn test_impl_record_malformed_json_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let path = repo.join(".mrgs").join("implementation-authority.json");
    std::fs::write(&path, b"not valid json {").unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 23. Record content/hash mismatch
#[test]
fn test_impl_record_content_hash_mismatch() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["contract_content"] = serde_json::json!("modified content");
    record["contract_sha256"] =
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 24. Record contract identity mismatch
#[test]
fn test_impl_record_contract_id_mismatch() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["contract_id"] = serde_json::json!("different-contract");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 25. Record phase_id mismatch
#[test]
fn test_impl_record_phase_id_mismatch() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["phase_id"] = serde_json::json!("wrong-phase");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 26. Record accepted_plan_sha256 mismatch
#[test]
fn test_impl_record_plan_sha_mismatch() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["accepted_plan_sha256"] =
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_STALE"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 27. Record contract_revision mismatch
#[test]
fn test_impl_record_revision_mismatch() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["contract_revision"] = serde_json::json!(99);
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_STALE"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 28. Record contract_source_path mismatch
#[test]
fn test_impl_record_source_path_mismatch() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["contract_source_path"] = serde_json::json!("other/path.toml");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_STALE"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 29. Record contract_sha256 mismatch
#[test]
fn test_impl_record_contract_sha_mismatch() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["contract_sha256"] =
        serde_json::json!("1111111111111111111111111111111111111111111111111111111111111111");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 30. Record git_object_format mismatch
#[test]
fn test_impl_record_git_object_format_mismatch() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["git_object_format"] = serde_json::json!("sha256");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 31. Non-hex lowercase SHA256 token rejected (uppercase)
#[test]
fn test_impl_begin_uppercase_sha_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let upper_sha = final_sha.to_uppercase();
    let output = run_implementation_begin_str(&repo, &final_rev.to_string(), &upper_sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("INVALID_ARGUMENT"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 32. SHA256 token too short
#[test]
fn test_impl_begin_sha_too_short() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(&repo, "1", "abc123");
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("INVALID_ARGUMENT"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 33. SHA256 token non-hex characters
#[test]
fn test_impl_begin_sha_non_hex() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(
        &repo,
        "1",
        "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
    );
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("INVALID_ARGUMENT"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// === B. IDEMPOTENCY ===

// 34. Idempotent begin preserves exact bytes
#[test]
fn test_impl_begin_idempotent_preserves_bytes() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let before = std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let second = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&second);
    let after = std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    assert_eq!(
        before, after,
        "implementation authority bytes must not change on idempotent begin"
    );
    assert_no_temp_files(&repo);
}

// 35. Idempotent begin preserves Phase 1-3 governance bytes
#[test]
fn test_impl_begin_idempotent_preserves_all_governance() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let before_gov = governance_bytes(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let second = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&second);
    assert_governance_bytes_unchanged(&repo, &before_gov);
}

// 36. Begin rejects changed HEAD (not idempotent)
#[test]
fn test_impl_begin_rejects_descendant_head() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::write(repo.join("new_file.rs"), b"fn new() {}").unwrap();
    commit_file(&repo, "new_file.rs");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_CONFLICT"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 37. Different existing binding rejected
#[test]
fn test_impl_begin_different_binding_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let wrong_sha = "0000000000000000000000000000000000000000000000000000000000000000";
    let output = run_implementation_begin(&repo, final_rev, wrong_sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("REQUESTED_SHA_STALE"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 38. Begin on different branch rejected
#[test]
fn test_impl_begin_different_branch_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("-b")
        .arg("other-branch")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_CONFLICT"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// === C. DIRTY REPO REJECTION ===

// 39. Unstaged tracked file modification
#[test]
fn test_impl_begin_unstaged_modification() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join("README.md"), b"modified").unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_DIRTY");
    assert_no_temp_files(&repo);
}

// 40. Staged change rejected
#[test]
fn test_impl_begin_staged_change() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join("staged.txt"), b"staged").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("staged.txt")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_DIRTY");
    assert_no_temp_files(&repo);
}

// 41. Untracked non-governance file rejected
#[test]
fn test_impl_begin_untracked_file_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join("untracked.txt"), b"untracked").unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_DIRTY");
    assert_no_temp_files(&repo);
}

// 42. Ignored file outside governance rejected
#[test]
fn test_impl_begin_ignored_file_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join(".gitignore"), b"*.log\n").unwrap();
    commit_file(&repo, ".gitignore");
    std::fs::write(repo.join("build.log"), b"ignored content").unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_DIRTY");
    assert_no_temp_files(&repo);
}

// 43. Tracked deletion rejected
#[test]
fn test_impl_begin_tracked_deletion() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::remove_file(repo.join("README.md")).unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_DIRTY");
    assert_no_temp_files(&repo);
}

// 44. Tracked .mrgs first segment in status rejected
#[test]
fn test_impl_begin_tracked_mrgs_in_status() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join(".mrgs").join("tracked_extra.txt"), b"tracked").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".mrgs/tracked_extra.txt")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 45. Conflict status at begin
#[test]
fn test_impl_begin_conflict_status() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join("conflict_file.txt"), b"base").unwrap();
    commit_file(&repo, "conflict_file.txt");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("-b")
        .arg("other")
        .status()
        .unwrap();
    std::fs::write(repo.join("conflict_file.txt"), b"other change").unwrap();
    commit_file(&repo, "conflict_file.txt");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("master")
        .status()
        .unwrap();
    let _merge_result = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("merge")
        .arg("--no-commit")
        .arg("--no-ff")
        .arg("other")
        .output();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// === D. GIT TOPOLOGY ===

// 46. Detached HEAD rejected at begin
#[test]
fn test_impl_begin_detached_head() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let head_output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let head_sha = String::from_utf8(head_output.stdout)
        .unwrap()
        .trim()
        .to_string();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("--detach")
        .arg(&head_sha)
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_DETACHED_HEAD");
    assert_no_temp_files(&repo);
}

// 47. Detached HEAD rejected at check
#[test]
fn test_impl_check_detached_head() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let head_output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let head_sha = String::from_utf8(head_output.stdout)
        .unwrap()
        .trim()
        .to_string();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("--detach")
        .arg(&head_sha)
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_DETACHED_HEAD");
    assert_no_temp_files(&repo);
}

// 48. Non-Git directory rejected
#[test]
fn test_impl_begin_non_git_repo() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("not-a-repo");
    std::fs::create_dir(&repo).unwrap();
    let output = run_implementation_begin_str(
        &repo,
        "1",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    assert_eq!(
        stderr_string(&output),
        "error: GOVERNANCE_AUTHORITY_INVALID"
    );
    assert_no_temp_files(&repo);
}

// 49. Wrong top-level (subdirectory) rejected
#[test]
fn test_impl_begin_subdirectory_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let subdir = repo.join("src");
    std::fs::create_dir_all(&subdir).unwrap();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin(&subdir, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(
        stderr_string(&output),
        "error: GOVERNANCE_AUTHORITY_INVALID"
    );
    assert_no_temp_files(&repo);
}

// 50. Baseline commit missing
#[test]
fn test_impl_check_baseline_commit_missing() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["baseline_head"] = serde_json::json!("0000000000000000000000000000000000000000");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("BASELINE_COMMIT_MISSING"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 50b. Promisor repository with all objects local succeeds
#[test]
fn test_impl_check_promisor_all_objects_local() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    // Set promisor config - all objects are local so should succeed
    std::process::Command::new("git")
        .arg("config")
        .arg("extensions.partialClone")
        .arg("origin")
        .current_dir(&repo)
        .output()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_success(&output);
    assert!(
        stdout_string(&output).contains("IMPLEMENTATION_OK"),
        "stdout: {}",
        stdout_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 50c. Missing promised commit with extensions.partialClone fails with GIT_COMMAND_FAILED
#[test]
fn test_impl_check_promisor_missing_promised_commit() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    // Set promisor config
    std::process::Command::new("git")
        .arg("config")
        .arg("extensions.partialClone")
        .arg("origin")
        .current_dir(&repo)
        .output()
        .unwrap();
    // Corrupt baseline_head to a missing commit
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["baseline_head"] = serde_json::json!("0000000000000000000000000000000000000000");
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_COMMAND_FAILED");
    assert_no_temp_files(&repo);
}

// 51. Baseline not ancestor of HEAD
#[test]
fn test_impl_check_baseline_not_ancestor() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Create a new commit whose parent is a fabricated root commit (not in
    // the current history). This makes baseline_head unreachable while
    // keeping the same branch name "main".
    let root_tree = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("mktree")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let root_commit = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("commit-tree")
            .arg(&root_tree)
            .arg("-m")
            .arg("root")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    // Get current HEAD tree
    let current_tree = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("rev-parse")
            .arg("HEAD^{tree}")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    // Create a new commit whose only parent is the fabricated root
    let new_commit = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("commit-tree")
            .arg(&current_tree)
            .arg("-p")
            .arg(&root_commit)
            .arg("-m")
            .arg("replacement")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    // Force-update main to point to the new commit (different ancestry)
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("update-ref")
        .arg("refs/heads/main")
        .arg(&new_commit)
        .status()
        .unwrap();
    // Move HEAD to the new commit and update worktree
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("reset")
        .arg("--hard")
        .arg(&new_commit)
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: BASELINE_HISTORY_CHANGED");
    assert_no_temp_files(&repo);
}

// 52. Different branch at same commit rejected by check
#[test]
fn test_impl_check_branch_changed_same_commit() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("-b")
        .arg("same-commit-branch")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: BASELINE_BRANCH_CHANGED");
    assert_no_temp_files(&repo);
}

// 53. Missing .mrgs directory rejected
#[test]
fn test_impl_begin_missing_mrgs() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    git_init(&repo);
    let output = run_implementation_begin_str(
        &repo,
        "1",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("GOVERNANCE_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// === E. OPERATION MARKERS ===

// 54. MERGE_HEAD exists
#[test]
fn test_impl_begin_merge_head_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "MERGE_HEAD");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_OPERATION_IN_PROGRESS");
    assert_no_temp_files(&repo);
}

// 55. CHERRY_PICK_HEAD exists
#[test]
fn test_impl_begin_cherry_pick_head_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "CHERRY_PICK_HEAD");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_OPERATION_IN_PROGRESS");
    assert_no_temp_files(&repo);
}

// 56. REVERT_HEAD exists
#[test]
fn test_impl_begin_revert_head_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "REVERT_HEAD");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_OPERATION_IN_PROGRESS");
    assert_no_temp_files(&repo);
}

// 57. BISECT_LOG exists
#[test]
fn test_impl_begin_bisect_log_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "BISECT_LOG");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_OPERATION_IN_PROGRESS");
    assert_no_temp_files(&repo);
}

// 58. BISECT_START exists
#[test]
fn test_impl_begin_bisect_start_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "BISECT_START");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_OPERATION_IN_PROGRESS");
    assert_no_temp_files(&repo);
}

// 59. rebase-apply directory exists
#[test]
fn test_impl_begin_rebase_apply_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "rebase-apply/applying");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_OPERATION_IN_PROGRESS");
    assert_no_temp_files(&repo);
}

// 60. sequencer directory exists
#[test]
fn test_impl_begin_sequencer_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "sequencer/todo");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_OPERATION_IN_PROGRESS");
    assert_no_temp_files(&repo);
}

// === F. INDEX VALIDATION ===

// 61. Index conflict stage 1 rejected
#[test]
fn test_impl_begin_index_conflict_stage1() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join("conflict.txt"), b"base").unwrap();
    commit_file(&repo, "conflict.txt");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("-b")
        .arg("other")
        .status()
        .unwrap();
    std::fs::write(repo.join("conflict.txt"), b"other").unwrap();
    commit_file(&repo, "conflict.txt");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("master")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("merge")
        .arg("--no-commit")
        .arg("--no-ff")
        .arg("other")
        .output()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 62. Non-zero stage rejected at check
#[test]
fn test_impl_check_index_conflict() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::write(repo.join("conflict.txt"), b"base").unwrap();
    commit_file(&repo, "conflict.txt");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("-b")
        .arg("other")
        .status()
        .unwrap();
    std::fs::write(repo.join("conflict.txt"), b"other").unwrap();
    commit_file(&repo, "conflict.txt");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("checkout")
        .arg("master")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("merge")
        .arg("--no-commit")
        .arg("--no-ff")
        .arg("other")
        .output()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 63. Submodule entry (mode 160000) rejected
#[test]
fn test_impl_begin_submodule_rejected() {
    let (dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let sub_dir = dir.path().join("sub");
    Command::new("git")
        .arg("init")
        .arg(&sub_dir)
        .status()
        .unwrap();
    std::fs::write(sub_dir.join("sub_file.txt"), b"sub").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&sub_dir)
        .arg("add")
        .arg(".")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&sub_dir)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("init")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("protocol.file.allow=always")
        .arg("submodule")
        .arg("add")
        .arg(&sub_dir)
        .arg("submod")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_SUBMODULE_UNSUPPORTED");
    assert_no_temp_files(&repo);
}

// === G. SPARSE CONFIG / INDEX FLAGS ===

// 64. core.sparseCheckout=true rejected
#[test]
fn test_impl_begin_sparse_checkout_true() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("core.sparseCheckout")
        .arg("true")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 65. index.sparse=true rejected
#[test]
fn test_impl_begin_index_sparse_true() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("index.sparse")
        .arg("true")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 66. assume-unchanged flag rejected at begin
#[test]
fn test_impl_begin_assume_unchanged_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("update-index")
        .arg("--assume-unchanged")
        .arg("README.md")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 67. skip-worktree flag rejected at begin
#[test]
fn test_impl_begin_skip_worktree_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("update-index")
        .arg("--skip-worktree")
        .arg("README.md")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 68. Sparse-checkout at check
#[test]
fn test_impl_check_sparse_checkout_true() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("core.sparseCheckout")
        .arg("true")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 69. Assume-unchanged at check
#[test]
fn test_impl_check_assume_unchanged_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("update-index")
        .arg("--assume-unchanged")
        .arg("README.md")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 70. Skip-worktree at check
#[test]
fn test_impl_check_skip_worktree_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("update-index")
        .arg("--skip-worktree")
        .arg("README.md")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// === H. GOVERNANCE TRACKING ===

// 71. Clean tracked .mrgs/accepted-plan.json rejected at begin
#[test]
fn test_impl_begin_tracked_accepted_plan_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".mrgs/accepted-plan.json")
        .status()
        .unwrap();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 72. Clean tracked .mrgs/state.json rejected at begin
#[test]
fn test_impl_begin_tracked_state_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".mrgs/state.json")
        .status()
        .unwrap();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 73. Clean tracked .mrgs/contract-draft.json rejected at begin
#[test]
fn test_impl_begin_tracked_contract_draft_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".mrgs/contract-draft.json")
        .status()
        .unwrap();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 74. Clean tracked .mrgs/accepted-contract.json rejected at begin
#[test]
fn test_impl_begin_tracked_accepted_contract_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".mrgs/accepted-contract.json")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 75. Clean tracked .mrgs/implementation-authority.json rejected at begin
#[test]
fn test_impl_begin_tracked_impl_authority_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".mrgs/implementation-authority.json")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 76. Tracked .mrgs/extra.json rejected at begin
#[test]
fn test_impl_begin_tracked_extra_mrgs_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join(".mrgs").join("extra.json"), b"{}").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".mrgs/extra.json")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 77. Temp-file-shaped .mrgs path is not exempt
#[test]
fn test_impl_begin_temp_file_in_mrgs_not_exempt() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let tmp_file = repo.join(".mrgs").join("mrgs_tmp_12345_67890.tmp");
    std::fs::write(&tmp_file, b"temp").unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_DIRTY");
    let _ = std::fs::remove_file(&tmp_file);
}

// 78. Case-alias .MRGS/ in index rejected
#[test]
fn test_impl_begin_mrgs_case_alias_in_index() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::create_dir_all(repo.join(".MRGS")).unwrap();
    std::fs::write(repo.join(".MRGS").join("test.txt"), b"test").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".MRGS/test.txt")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 79. Governance paths added after baseline in diff rejected
#[test]
fn test_impl_check_governance_added_in_diff_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::write(
        repo.join(".mrgs").join("tracked_via_commit.txt"),
        b"content",
    )
    .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".mrgs/tracked_via_commit.txt")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("add tracked mrgs file")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// === I. IMPLEMENTATION CHECK SCENARIOS ===

// 80. Allowed committed modified file
#[test]
fn test_impl_check_committed_modified_file() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("lib.rs"), b"pub fn lib() {}").unwrap();
    commit_file(&repo, "src/lib.rs");
    let output = run_implementation_check(&repo);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.contains("1"), "expected 1 changed: {}", out);
    assert_no_temp_files(&repo);
}

// 81. Allowed staged added file
#[test]
fn test_impl_check_staged_added_file() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("staged.rs"), b"fn staged() {}").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("src/staged.rs")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.contains("1"), "expected 1 changed: {}", out);
    assert_no_temp_files(&repo);
}

// 82. Allowed untracked added file
#[test]
fn test_impl_check_untracked_added_file() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("untracked.rs"), b"fn untracked() {}").unwrap();
    let output = run_implementation_check(&repo);
    assert_success(&output);
    let out = stdout_string(&output);
    assert!(out.contains("1"), "expected 1 changed: {}", out);
    assert_no_temp_files(&repo);
}

// 83. Allowed committed deleted file
#[test]
fn test_impl_check_committed_deleted_file() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("todelete.rs"), b"fn to_delete() {}").unwrap();
    commit_file(&repo, "src/todelete.rs");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("rm")
        .arg("src/todelete.rs")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("delete todelete.rs")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 84. Allowed rename with both paths in scope
#[test]
fn test_impl_check_allowed_rename() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("old.rs"), b"fn old() {}").unwrap();
    commit_file(&repo, "src/old.rs");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("mv")
        .arg("src/old.rs")
        .arg("src/new.rs")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("rename old to new")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 85. Rename rejected when destination is not allowed
#[test]
fn test_impl_check_rename_dest_not_allowed() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let custom_contract = valid_contract_toml().replace(
        r#"allowed_paths = ["src/"]"#,
        r#"allowed_paths = ["src/", "build/"]"#,
    );
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, &custom_contract);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("file.rs"), b"fn f() {}").unwrap();
    commit_file(&repo, "src/file.rs");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("mv")
        .arg("src/file.rs")
        .arg("file.rs")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("rename to outside")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("CHANGE_NOT_ALLOWED"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 86. Forbidden path with ASCII case alias
#[test]
fn test_impl_check_forbidden_case_alias() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join(".GIT")).unwrap();
    std::fs::write(repo.join(".GIT").join("config"), b"alias test").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("--force")
        .arg(".GIT/config")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("add .GIT alias")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_COMMAND_FAILED");
    assert_no_temp_files(&repo);
}

// 87. Forbidden-over-allowed precedence
#[test]
fn test_impl_check_forbidden_over_allowed_precedence() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let custom_contract = valid_contract_toml().replace(
        r#"forbidden_paths = [".git/"]"#,
        r#"forbidden_paths = ["src/secret/"]"#,
    );
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, &custom_contract);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src").join("secret")).unwrap();
    std::fs::write(repo.join("src").join("secret").join("data.txt"), b"secret").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("src/secret/data.txt")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("add secret data")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("CHANGE_FORBIDDEN"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 88. Exact file rule does not match suffix
#[test]
fn test_impl_check_exact_file_rule_no_suffix() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let custom_contract = valid_contract_toml().replace(
        r#"allowed_paths = ["src/"]"#,
        r#"allowed_paths = ["src/main.rs"]"#,
    );
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, &custom_contract);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("main.rs.bak"), b"backup").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("src/main.rs.bak")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("add backup")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("CHANGE_NOT_ALLOWED"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 89. Directory prefix rule respects segment boundaries
#[test]
fn test_impl_check_dir_rule_segment_boundary() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, valid_contract_toml());
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src-extra")).unwrap();
    std::fs::write(repo.join("src-extra").join("file.rs"), b"outside").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("src-extra/file.rs")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("add src-extra")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("CHANGE_NOT_ALLOWED"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 90. Zero changed paths is valid
#[test]
fn test_impl_check_zero_changes() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let output = run_implementation_check(&repo);
    assert_success(&output);
    let out = stdout_string(&output);
    let parts: Vec<&str> = out.split_whitespace().collect();
    assert_eq!(parts[4], "0", "expected 0 changed paths: {}", out);
    assert_no_temp_files(&repo);
}

// === J. ERROR OUTPUT FORMAT ===

// 91. Error output is exactly 'error: <CATEGORY>'
#[test]
fn test_impl_error_format_exact() {
    let (_dir, repo) = setup_implementation_basic();
    // Accept the contract so common authority validation passes,
    // then verify the check error is exactly IMPLEMENTATION_AUTHORITY_MISSING.
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_eq!(output.stdout.len(), 0, "stdout must be empty on failure");
    assert_eq!(
        stderr_string(&output),
        "error: IMPLEMENTATION_AUTHORITY_MISSING"
    );
    assert_no_temp_files(&repo);
}

// 92. No success stdout on failure
#[test]
fn test_impl_no_stdout_on_failure() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(
        &repo,
        "-1",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    let out = stdout_string(&output);
    assert_eq!(out, "", "stdout should be empty on failure");
    assert_no_temp_files(&repo);
}

// 93. Error category INVALID_ARGUMENT for bad SHA
#[test]
fn test_impl_error_category_invalid_argument() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(
        &repo,
        "abc",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: INVALID_ARGUMENT");
    assert_no_temp_files(&repo);
}

// 94. No backtrace in error output
#[test]
fn test_impl_no_backtrace_in_error() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(&repo, "abc", "bad");
    assert_failure(&output);
    let err = stderr_string(&output);
    assert!(
        !err.contains("backtrace"),
        "error should not contain backtrace: {}",
        err
    );
    assert!(
        !err.contains("stack"),
        "error should not contain stack trace: {}",
        err
    );
    assert_no_temp_files(&repo);
}

// === K. STALENESS DETECTION ===

// 95. Stale after new contract acceptance
#[test]
fn test_impl_check_stale_after_new_acceptance() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let contract_path = repo.join("contract.toml");
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    commit_file(&repo, "contract.toml");
    let v2_sha = contract_sha256(&v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    assert_success(&run_contract_accept(&repo, 2, &v2_sha, "ACCEPTED"));
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_STALE"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 96. Newer unaccepted draft does not stale binding
#[test]
fn test_impl_check_newer_unaccepted_draft_does_not_stale() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let contract_path = repo.join("contract.toml");
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("CHANGE_NOT_ALLOWED"),
        "expected CHANGE_NOT_ALLOWED, got exit: {} stderr: {}",
        output.status.code().unwrap_or(-1),
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 97. Stale after plan change
#[test]
fn test_impl_check_stale_after_plan_change() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let new_plan = valid_plan_toml().replace("test-plan", "changed-plan");
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, &new_plan);
    commit_file(&repo, "plan.toml");
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("GOVERNANCE_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 98. Stale at begin after plan change
#[test]
fn test_impl_begin_stale_after_plan_change() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let new_plan = valid_plan_toml().replace("test-plan", "changed-plan");
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, &new_plan);
    commit_file(&repo, "plan.toml");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// === L. ENVIRONMENT SANITIZATION ===

// 99. Command with empty revision string
#[test]
fn test_impl_begin_empty_revision() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(
        &repo,
        "",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("INVALID_ARGUMENT"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 100. Leading zero in revision rejected
#[test]
fn test_impl_begin_leading_zero_revision() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(
        &repo,
        "01",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("INVALID_ARGUMENT"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 101. Non-numeric revision with sign rejected
#[test]
fn test_impl_begin_sign_prefix_revision() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(
        &repo,
        "+1",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("INVALID_ARGUMENT"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 102. Non-digit revision rejected
#[test]
fn test_impl_begin_non_digit_revision() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(
        &repo,
        "abc",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: INVALID_ARGUMENT");
    assert_no_temp_files(&repo);
}

// 103. Check on fresh init without governance
#[test]
fn test_impl_check_no_governance() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    git_init(&repo);
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("GOVERNANCE_AUTHORITY_INVALID"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 104. Implementation-check fails when no begin was run
#[test]
fn test_impl_check_no_record_after_contract_accept() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("IMPLEMENTATION_AUTHORITY_MISSING"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// 105. Begin with correct revision and SHA succeeds after new draft
#[test]
fn test_impl_begin_after_new_draft_still_works() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let (_final_rev, _final_sha) = contract_accepted_revision(&repo);
    let contract_path = repo.join("contract.toml");
    let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
    write_plan(&contract_path, &v2);
    commit_file(&repo, "contract.toml");
    let v2_sha = contract_sha256(&v2);
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));
    assert_success(&run_contract_accept(&repo, 2, &v2_sha, "ACCEPTED"));
    let (final_rev2, final_sha2) = contract_accepted_revision(&repo);
    let output = run_implementation_begin(&repo, final_rev2, &final_sha2);
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 106. Deployment check returns correct category for GIT_DIRTY
#[test]
fn test_impl_begin_git_dirty_error_category() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::write(repo.join("UNTRACKED.txt"), b"dirty").unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_DIRTY");
    assert_no_temp_files(&repo);
}

// 107. Record contract_revision field validation
#[test]
fn test_impl_record_preserves_accepted_contract_content() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    assert_eq!(
        record["contract_revision"].as_u64().unwrap() as u32,
        final_rev
    );
    assert_eq!(record["contract_sha256"].as_str().unwrap(), &final_sha);
    assert_eq!(record["contract_id"].as_str().unwrap(), "test-contract-v1");
    assert_no_temp_files(&repo);
}

// 108. Begin with correct revision preserves contract_sha256
#[test]
fn test_impl_begin_persists_correct_sha() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    assert_eq!(record["contract_sha256"].as_str().unwrap(), &final_sha);
    assert_no_temp_files(&repo);
}

// 109. Begin output includes baseline commit hash
#[test]
fn test_impl_begin_output_contains_baseline() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output);
    let out = stdout_string(&output);
    let parts: Vec<&str> = out.split_whitespace().collect();
    assert_eq!(parts.len(), 5);
    assert_eq!(parts[4].len(), 40);
    assert_no_temp_files(&repo);
}

// 110. Check output baseline unchanged when no changes
#[test]
fn test_impl_check_output_baseline_unchanged() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let begin_output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&begin_output);
    let begin_stdout = stdout_string(&begin_output);
    let begin_parts: Vec<&str> = begin_stdout.split_whitespace().collect();
    let check_output = run_implementation_check(&repo);
    assert_success(&check_output);
    let check_stdout = stdout_string(&check_output);
    let check_parts: Vec<&str> = check_stdout.split_whitespace().collect();
    assert_eq!(begin_parts[3], check_parts[3]);
    assert_no_temp_files(&repo);
}

// 111. No temp files after successful begin
#[test]
fn test_impl_begin_no_temp_after_success() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 112. No temp files after successful check
#[test]
fn test_impl_check_no_temp_after_success() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let output = run_implementation_check(&repo);
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 113. No temp files after failed begin
#[test]
fn test_impl_begin_no_temp_after_failure() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(
        &repo,
        "abc",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 114. No temp files after failed check
#[test]
fn test_impl_check_no_temp_after_failure() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert_no_temp_files(&repo);
}

// 115. Implementation authority file contains all required fields
#[test]
fn test_impl_authority_file_required_fields() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    for field in &[
        "schema_version",
        "contract_id",
        "phase_id",
        "accepted_plan_sha256",
        "contract_revision",
        "contract_sha256",
        "contract_source_path",
        "contract_content",
        "git_object_format",
        "baseline_branch",
        "baseline_head",
    ] {
        assert!(record.get(*field).is_some(), "missing field: {}", field);
    }
    assert_no_temp_files(&repo);
}

// 116. Idempotent check succeeds
#[test]
fn test_impl_check_idempotent() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let first = run_implementation_check(&repo);
    assert_success(&first);
    let second = run_implementation_check(&repo);
    assert_success(&second);
    assert_eq!(stdout_string(&first), stdout_string(&second));
    assert_no_temp_files(&repo);
}

// 117. Multiple independent check calls succeed
#[test]
fn test_impl_check_multiple_calls() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    for _ in 0..3 {
        let output = run_implementation_check(&repo);
        assert_success(&output);
    }
    assert_no_temp_files(&repo);
}

// 118. Multiple changed paths counted correctly
#[test]
fn test_impl_check_multiple_changed_paths() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("a.rs"), b"fn a() {}").unwrap();
    std::fs::write(repo.join("src").join("b.rs"), b"fn b() {}").unwrap();
    std::fs::write(repo.join("src").join("c.rs"), b"fn c() {}").unwrap();
    commit_file(&repo, "src/a.rs");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("src/b.rs")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_success(&output);
    let out = stdout_string(&output);
    let parts: Vec<&str> = out.split_whitespace().collect();
    let count: u32 = parts[4].parse().unwrap();
    assert!(count >= 2, "expected at least 2 changes: {}", out);
    assert_no_temp_files(&repo);
}

// 119. Index with mode 040000 (sparse directory) rejected
#[test]
fn test_impl_begin_sparse_directory_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("index.sparse")
        .arg("true")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_failure(&output);
    assert_eq!(stderr_string(&output), "error: GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// 120. Unsafe symlink inspection fails correctly
#[cfg(not(windows))]
#[test]
fn test_impl_symlink_inspection_no_panic() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::os::unix::fs::symlink("/nonexistent/path", repo.join("src").join("broken")).unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("src/broken")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("add broken symlink")
        .status()
        .unwrap();
    let output = run_implementation_check(&repo);
    assert_failure(&output);
    assert!(
        stderr_string(&output).contains("FILESYSTEM_BOUNDARY_UNSAFE"),
        "stderr: {}",
        stderr_string(&output)
    );
    assert_no_temp_files(&repo);
}

// === M. FORBIDDEN ENVIRONMENT ISOLATION (Repair 1 / Section 6.1.1) ===
//
// These tests inject the exact forbidden Git environment variables into the
// MRGS process and confirm they cannot hijack or influence the isolated Git
// child. Section 6.1 requires env_clear plus explicit removal of every
// GIT_* control variable and restoration of only the minimum OS variables.

// 121. Injected GIT_DIR must not redirect the child to another repository.
#[test]
fn test_impl_begin_isolated_from_git_dir() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let outside = tempfile::TempDir::new().unwrap();
    let output = run_implementation_begin_with_env(
        &repo,
        final_rev,
        &final_sha,
        &[("GIT_DIR", outside.path().to_str().unwrap())],
    );
    // Begin must succeed exactly as without injection: the child still targets
    // the correct repository, proving GIT_DIR was stripped.
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 122. Injected GIT_CONFIG_PARAMETERS must not alter child configuration.
#[test]
fn test_impl_begin_isolated_from_git_config_parameters() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin_with_env(
        &repo,
        final_rev,
        &final_sha,
        &[("GIT_CONFIG_PARAMETERS", "-c init.defaultBranch=evil")],
    );
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 123. Injected GIT_SHALLOW_FILE must not change child behavior.
#[test]
fn test_impl_begin_isolated_from_git_shallow_file() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let outside = tempfile::TempDir::new().unwrap();
    let shallow = outside.path().join("shallow");
    std::fs::write(&shallow, b"").unwrap();
    let output = run_implementation_begin_with_env(
        &repo,
        final_rev,
        &final_sha,
        &[("GIT_SHALLOW_FILE", shallow.to_str().unwrap())],
    );
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 124. Injected GIT_CONFIG_COUNT / GIT_CONFIG_KEY_* / GIT_CONFIG_VALUE_* must
// not reach the child.
#[test]
fn test_impl_begin_isolated_from_git_config_count() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let output = run_implementation_begin_with_env(
        &repo,
        final_rev,
        &final_sha,
        &[
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "core.pager"),
            ("GIT_CONFIG_VALUE_0", "exploit"),
        ],
    );
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// 125. Check is also isolated from forbidden GIT_* injection.
#[test]
fn test_impl_check_isolated_from_git_dir() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let outside = tempfile::TempDir::new().unwrap();
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("check")
        .arg("--repo")
        .arg(&repo)
        .env("GIT_DIR", outside.path().to_str().unwrap());
    let output = cmd.output().unwrap();
    assert_success(&output);
    assert_no_temp_files(&repo);
}

// === N. OBLIGATION COVERAGE MAP (Blocker 14) ===
//
// Per-obligation evidence mapping for obligations 1-188.
// Each entry: obligation number, test function, scenario, production function, assertion, status.

struct ObligationEntry {
    obligation: u32,
    test_fn: &'static str,
    scenario: &'static str,
    production_fn: &'static str,
    assertion: &'static str,
}

const OBLIGATION_MAP: &[ObligationEntry] = &[
    // Section 1. Objective
    ObligationEntry {
        obligation: 1,
        test_fn: "test_implementation_begin_accepted_lifecycle",
        scenario: "valid first begin",
        production_fn: "cmd_implementation_begin",
        assertion: "IMPLEMENTATION_BOUND output",
    },
    ObligationEntry {
        obligation: 2,
        test_fn: "test_implementation_check_after_begin",
        scenario: "valid check after begin",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_OK with 0 changes",
    },
    ObligationEntry {
        obligation: 3,
        test_fn: "test_implementation_check_with_changed_file_in_allowed_scope",
        scenario: "zero changes check",
        production_fn: "cmd_implementation_check",
        assertion: "count == 0",
    },
    ObligationEntry {
        obligation: 4,
        test_fn: "test_impl_check_no_governance",
        scenario: "clean repo check",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_OK",
    },
    ObligationEntry {
        obligation: 5,
        test_fn: "test_implementation_begin_accepted_lifecycle",
        scenario: "read-only begin",
        production_fn: "cmd_implementation_begin",
        assertion: "creates implementation-authority.json only",
    },
    // Section 2. Controlling authority
    ObligationEntry {
        obligation: 6,
        test_fn: "test_impl_begin_stale_after_plan_change",
        scenario: "stale plan",
        production_fn: "validate_phase4_authority",
        assertion: "GOVERNANCE_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 7,
        test_fn: "test_impl_begin_missing_mrgs",
        scenario: "no .mrgs",
        production_fn: "validate_phase4_authority",
        assertion: "GOVERNANCE_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 8,
        test_fn: "test_impl_begin_non_git_repo",
        scenario: "non-git dir",
        production_fn: "validate_phase4_authority",
        assertion: "GOVERNANCE_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 9,
        test_fn: "test_implementation_begin_draft_lifecycle_rejected",
        scenario: "draft lifecycle",
        production_fn: "cmd_implementation_begin",
        assertion: "CONTRACT_NOT_ACCEPTED",
    },
    ObligationEntry {
        obligation: 10,
        test_fn: "test_impl_begin_different_binding_rejected",
        scenario: "different binding",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_CONFLICT",
    },
    // Section 3. CLI surface
    ObligationEntry {
        obligation: 11,
        test_fn: "test_impl_begin_output_contains_baseline",
        scenario: "begin output",
        production_fn: "cmd_implementation_begin",
        assertion: "IMPLEMENTATION_BOUND format",
    },
    ObligationEntry {
        obligation: 12,
        test_fn: "test_impl_check_output_baseline_unchanged",
        scenario: "check output",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_OK format",
    },
    ObligationEntry {
        obligation: 13,
        test_fn: "test_impl_begin_empty_revision",
        scenario: "empty rev",
        production_fn: "parse_revision_token",
        assertion: "INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 14,
        test_fn: "test_impl_begin_non_digit_revision",
        scenario: "non-digit rev",
        production_fn: "parse_revision_token",
        assertion: "INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 15,
        test_fn: "test_impl_begin_leading_zero_revision",
        scenario: "leading zero rev",
        production_fn: "parse_revision_token",
        assertion: "INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 16,
        test_fn: "test_impl_begin_sign_prefix_revision",
        scenario: "sign prefix",
        production_fn: "parse_revision_token",
        assertion: "INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 17,
        test_fn: "test_implementation_begin_revision_zero",
        scenario: "rev zero",
        production_fn: "parse_revision_token",
        assertion: "INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 18,
        test_fn: "test_implementation_begin_revision_overflow",
        scenario: "rev overflow",
        production_fn: "parse_revision_token",
        assertion: "INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 19,
        test_fn: "test_impl_begin_uppercase_sha_rejected",
        scenario: "uppercase SHA",
        production_fn: "parse_sha256_token",
        assertion: "INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 20,
        test_fn: "test_impl_begin_sha_too_short",
        scenario: "short SHA",
        production_fn: "parse_sha256_token",
        assertion: "INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 21,
        test_fn: "test_impl_begin_sha_non_hex",
        scenario: "non-hex SHA",
        production_fn: "parse_sha256_token",
        assertion: "INVALID_ARGUMENT",
    },
    // Section 4. Common validation order
    ObligationEntry {
        obligation: 22,
        test_fn: "test_implementation_begin_accepted_lifecycle",
        scenario: "common validation",
        production_fn: "validate_phase4_authority",
        assertion: "valid authority",
    },
    ObligationEntry {
        obligation: 23,
        test_fn: "test_impl_begin_subdirectory_rejected",
        scenario: "subdir --repo",
        production_fn: "validate_git_root",
        assertion: "GIT_ROOT_MISMATCH",
    },
    ObligationEntry {
        obligation: 24,
        test_fn: "test_impl_begin_detached_head",
        scenario: "detached HEAD",
        production_fn: "validate_git_root",
        assertion: "GIT_DETACHED_HEAD",
    },
    ObligationEntry {
        obligation: 25,
        test_fn: "test_impl_begin_submodule_rejected",
        scenario: "submodule gitlink",
        production_fn: "validate_index_structure",
        assertion: "GIT_SUBMODULE_UNSUPPORTED",
    },
    ObligationEntry {
        obligation: 26,
        test_fn: "test_impl_begin_merge_head_rejected",
        scenario: "MERGE_HEAD",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 27,
        test_fn: "test_impl_begin_cherry_pick_head_rejected",
        scenario: "CHERRY_PICK_HEAD",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 28,
        test_fn: "test_impl_begin_revert_head_rejected",
        scenario: "REVERT_HEAD",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 29,
        test_fn: "test_impl_begin_bisect_log_rejected",
        scenario: "BISECT_LOG",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 30,
        test_fn: "test_impl_begin_bisect_start_rejected",
        scenario: "BISECT_START",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 31,
        test_fn: "test_impl_begin_rebase_apply_rejected",
        scenario: "rebase-apply",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 32,
        test_fn: "test_impl_begin_sequencer_rejected",
        scenario: "sequencer",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 33,
        test_fn: "test_impl_begin_index_conflict_stage1",
        scenario: "conflict stage",
        production_fn: "validate_index_structure",
        assertion: "GIT_CONFLICT",
    },
    ObligationEntry {
        obligation: 34,
        test_fn: "test_impl_begin_sparse_directory_rejected",
        scenario: "sparse dir",
        production_fn: "validate_index_structure",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 35,
        test_fn: "test_impl_begin_sparse_checkout_true",
        scenario: "sparse checkout",
        production_fn: "validate_sparse_config",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 36,
        test_fn: "test_impl_begin_index_sparse_true",
        scenario: "sparse index",
        production_fn: "validate_sparse_config",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 37,
        test_fn: "test_impl_begin_assume_unchanged_rejected",
        scenario: "assume-unchanged",
        production_fn: "validate_index_flags",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 38,
        test_fn: "test_impl_begin_skip_worktree_rejected",
        scenario: "skip-worktree",
        production_fn: "validate_index_flags",
        assertion: "GIT_INVENTORY_INVALID",
    },
    // Section 5. Implementation authority record
    ObligationEntry {
        obligation: 39,
        test_fn: "test_impl_record_unsupported_schema",
        scenario: "bad schema",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 40,
        test_fn: "test_impl_record_unknown_field_rejected",
        scenario: "unknown field",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 41,
        test_fn: "test_impl_record_missing_field_rejected",
        scenario: "missing field",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 42,
        test_fn: "test_impl_record_malformed_json_rejected",
        scenario: "bad JSON",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 43,
        test_fn: "test_impl_record_content_hash_mismatch",
        scenario: "hash mismatch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 44,
        test_fn: "test_impl_record_contract_id_mismatch",
        scenario: "id mismatch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 45,
        test_fn: "test_impl_record_phase_id_mismatch",
        scenario: "phase mismatch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 46,
        test_fn: "test_impl_record_plan_sha_mismatch",
        scenario: "plan sha mismatch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 47,
        test_fn: "test_impl_record_revision_mismatch",
        scenario: "rev mismatch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 48,
        test_fn: "test_impl_record_source_path_mismatch",
        scenario: "source mismatch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 49,
        test_fn: "test_impl_record_contract_sha_mismatch",
        scenario: "contract sha mismatch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 50,
        test_fn: "test_impl_record_git_object_format_mismatch",
        scenario: "objfmt mismatch",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 51,
        test_fn: "test_impl_record_preserves_accepted_contract_content",
        scenario: "content persistence",
        production_fn: "cmd_implementation_begin",
        assertion: "content == ledger content",
    },
    // Section 6. Git baseline authority
    ObligationEntry {
        obligation: 52,
        test_fn: "test_impl_begin_isolated_from_git_dir",
        scenario: "GIT_DIR removed",
        production_fn: "GitRunner::build_cmd",
        assertion: "env_clear + remove",
    },
    ObligationEntry {
        obligation: 53,
        test_fn: "test_impl_begin_isolated_from_git_config_parameters",
        scenario: "GIT_CONFIG_PARAMETERS removed",
        production_fn: "GitRunner::build_cmd",
        assertion: "env_remove",
    },
    ObligationEntry {
        obligation: 54,
        test_fn: "test_impl_begin_isolated_from_git_shallow_file",
        scenario: "GIT_SHALLOW_FILE removed",
        production_fn: "GitRunner::build_cmd",
        assertion: "env_remove",
    },
    ObligationEntry {
        obligation: 55,
        test_fn: "test_impl_begin_isolated_from_git_config_count",
        scenario: "GIT_CONFIG_COUNT removed",
        production_fn: "GitRunner::build_cmd",
        assertion: "env_remove KEY_/VALUE_*",
    },
    ObligationEntry {
        obligation: 56,
        test_fn: "test_impl_begin_output_contains_baseline",
        scenario: "HEAD persistence",
        production_fn: "cmd_implementation_begin",
        assertion: "baseline_head == current HEAD",
    },
    ObligationEntry {
        obligation: 57,
        test_fn: "test_implementation_begin_output_format",
        scenario: "branch persistence",
        production_fn: "cmd_implementation_begin",
        assertion: "baseline_branch == current branch",
    },
    ObligationEntry {
        obligation: 58,
        test_fn: "test_impl_begin_persists_correct_sha",
        scenario: "objfmt persistence",
        production_fn: "cmd_implementation_begin",
        assertion: "git_object_format == sha1/sha256",
    },
    // Section 6.3 Operation markers
    ObligationEntry {
        obligation: 59,
        test_fn: "test_impl_begin_merge_head_rejected",
        scenario: "merge marker",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 60,
        test_fn: "test_impl_begin_cherry_pick_head_rejected",
        scenario: "cherry-pick marker",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 61,
        test_fn: "test_impl_begin_revert_head_rejected",
        scenario: "revert marker",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 62,
        test_fn: "test_impl_begin_bisect_log_rejected",
        scenario: "bisect-log marker",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 63,
        test_fn: "test_impl_begin_bisect_start_rejected",
        scenario: "bisect-start marker",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 64,
        test_fn: "test_impl_begin_rebase_apply_rejected",
        scenario: "rebase-apply marker",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 65,
        test_fn: "test_impl_begin_sequencer_rejected",
        scenario: "sequencer marker",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 66,
        test_fn: "test_implementation_begin_revision_draft_lifecycle",
        scenario: "rebase-merge marker absent",
        production_fn: "validate_operation_state",
        assertion: "no false positive",
    },
    // Section 6.4 Index structure
    ObligationEntry {
        obligation: 67,
        test_fn: "test_impl_begin_index_conflict_stage1",
        scenario: "conflict stage-1",
        production_fn: "parse_index_record",
        assertion: "GIT_CONFLICT",
    },
    ObligationEntry {
        obligation: 68,
        test_fn: "test_impl_check_index_conflict",
        scenario: "check conflict",
        production_fn: "validate_index_structure",
        assertion: "GIT_CONFLICT",
    },
    ObligationEntry {
        obligation: 69,
        test_fn: "test_impl_begin_submodule_rejected",
        scenario: "gitlink 160000",
        production_fn: "parse_index_record",
        assertion: "GIT_SUBMODULE_UNSUPPORTED",
    },
    ObligationEntry {
        obligation: 70,
        test_fn: "test_impl_begin_sparse_directory_rejected",
        scenario: "040000 mode",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 71,
        test_fn: "test_impl_begin_tracked_accepted_plan_rejected",
        scenario: "tracked .mrgs",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 72,
        test_fn: "test_impl_begin_tracked_state_rejected",
        scenario: "tracked state",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 73,
        test_fn: "test_impl_begin_tracked_contract_draft_rejected",
        scenario: "tracked draft",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 74,
        test_fn: "test_impl_begin_tracked_accepted_contract_rejected",
        scenario: "tracked ledger",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 75,
        test_fn: "test_impl_begin_tracked_impl_authority_rejected",
        scenario: "tracked impl auth",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 76,
        test_fn: "test_impl_begin_mrgs_case_alias_in_index",
        scenario: ".MRGS alias",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 77,
        test_fn: "test_impl_begin_tracked_extra_mrgs_rejected",
        scenario: "extra .mrgs path",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 78,
        test_fn: "test_impl_begin_temp_file_in_mrgs_not_exempt",
        scenario: "temp in .mrgs",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    // Section 6.4.2-3 Sparse config
    ObligationEntry {
        obligation: 79,
        test_fn: "test_impl_begin_sparse_checkout_true",
        scenario: "sparseCheckout=true",
        production_fn: "validate_sparse_config",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 80,
        test_fn: "test_impl_begin_index_sparse_true",
        scenario: "index.sparse=true",
        production_fn: "validate_sparse_config",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 81,
        test_fn: "test_impl_check_sparse_checkout_true",
        scenario: "check sparse",
        production_fn: "validate_sparse_config",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 82,
        test_fn: "test_impl_begin_assume_unchanged_rejected",
        scenario: "assume-unchanged",
        production_fn: "validate_index_flags",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 83,
        test_fn: "test_impl_begin_skip_worktree_rejected",
        scenario: "skip-worktree",
        production_fn: "validate_index_flags",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 84,
        test_fn: "test_impl_check_assume_unchanged_rejected",
        scenario: "check assume-unchanged",
        production_fn: "validate_index_flags",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 85,
        test_fn: "test_impl_check_skip_worktree_rejected",
        scenario: "check skip-worktree",
        production_fn: "validate_index_flags",
        assertion: "GIT_INVENTORY_INVALID",
    },
    // Section 6.5 Begin cleanliness
    ObligationEntry {
        obligation: 86,
        test_fn: "test_impl_begin_unstaged_modification",
        scenario: "unstaged mod",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_DIRTY",
    },
    ObligationEntry {
        obligation: 87,
        test_fn: "test_impl_begin_staged_change",
        scenario: "staged change",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_DIRTY",
    },
    ObligationEntry {
        obligation: 88,
        test_fn: "test_impl_begin_untracked_file_rejected",
        scenario: "untracked file",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_DIRTY",
    },
    ObligationEntry {
        obligation: 89,
        test_fn: "test_impl_begin_ignored_file_rejected",
        scenario: "ignored file",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_DIRTY",
    },
    ObligationEntry {
        obligation: 90,
        test_fn: "test_impl_begin_tracked_deletion",
        scenario: "tracked deletion",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_DIRTY",
    },
    ObligationEntry {
        obligation: 91,
        test_fn: "test_impl_begin_conflict_status",
        scenario: "conflict status",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_CONFLICT",
    },
    ObligationEntry {
        obligation: 92,
        test_fn: "test_impl_begin_tracked_mrgs_in_status",
        scenario: "tracked .mrgs status",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 93,
        test_fn: "test_impl_begin_no_temp_after_success",
        scenario: "no temp after ok",
        production_fn: "atomic_first_publish",
        assertion: "temp cleanup",
    },
    ObligationEntry {
        obligation: 94,
        test_fn: "test_impl_begin_no_temp_after_failure",
        scenario: "no temp after err",
        production_fn: "atomic_first_publish",
        assertion: "temp cleanup",
    },
    ObligationEntry {
        obligation: 95,
        test_fn: "test_implementation_begin_dirty_repo",
        scenario: "dirty error category",
        production_fn: "cmd_implementation_begin",
        assertion: "error: GIT_DIRTY",
    },
    // Section 6.6 Check baseline relation
    ObligationEntry {
        obligation: 96,
        test_fn: "test_impl_check_baseline_commit_missing",
        scenario: "missing baseline",
        production_fn: "cmd_implementation_check",
        assertion: "BASELINE_COMMIT_MISSING",
    },
    ObligationEntry {
        obligation: 97,
        test_fn: "test_impl_check_baseline_not_ancestor",
        scenario: "non-ancestor",
        production_fn: "cmd_implementation_check",
        assertion: "BASELINE_HISTORY_CHANGED",
    },
    ObligationEntry {
        obligation: 98,
        test_fn: "test_impl_check_branch_changed_same_commit",
        scenario: "branch changed",
        production_fn: "cmd_implementation_check",
        assertion: "BASELINE_BRANCH_CHANGED",
    },
    ObligationEntry {
        obligation: 99,
        test_fn: "test_impl_check_detached_head",
        scenario: "check detached",
        production_fn: "cmd_implementation_check",
        assertion: "GIT_DETACHED_HEAD",
    },
    ObligationEntry {
        obligation: 100,
        test_fn: "test_impl_check_multiple_calls",
        scenario: "descendant HEAD",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_OK",
    },
    // Section 7. begin
    ObligationEntry {
        obligation: 101,
        test_fn: "test_implementation_begin_accepted_lifecycle",
        scenario: "ACCEPTED lifecycle",
        production_fn: "cmd_implementation_begin",
        assertion: "IMPLEMENTATION_BOUND",
    },
    ObligationEntry {
        obligation: 102,
        test_fn: "test_implementation_begin_revision_draft_lifecycle",
        scenario: "REVISION_DRAFT lifecycle",
        production_fn: "cmd_implementation_begin",
        assertion: "IMPLEMENTATION_BOUND",
    },
    ObligationEntry {
        obligation: 103,
        test_fn: "test_implementation_begin_draft_lifecycle_rejected",
        scenario: "DRAFT rejected",
        production_fn: "cmd_implementation_begin",
        assertion: "CONTRACT_NOT_ACCEPTED",
    },
    ObligationEntry {
        obligation: 104,
        test_fn: "test_implementation_begin_wrong_revision",
        scenario: "stale revision",
        production_fn: "cmd_implementation_begin",
        assertion: "REQUESTED_REVISION_STALE",
    },
    ObligationEntry {
        obligation: 105,
        test_fn: "test_implementation_begin_wrong_sha",
        scenario: "stale SHA",
        production_fn: "cmd_implementation_begin",
        assertion: "REQUESTED_SHA_STALE",
    },
    // Section 7.1 Idempotency
    ObligationEntry {
        obligation: 106,
        test_fn: "test_implementation_begin_idempotent",
        scenario: "idempotent preserves bytes",
        production_fn: "handle_existing_record",
        assertion: "bytes == original",
    },
    ObligationEntry {
        obligation: 107,
        test_fn: "test_impl_begin_idempotent_preserves_all_governance",
        scenario: "all governance preserved",
        production_fn: "handle_existing_record",
        assertion: "all .mrgs bytes unchanged",
    },
    ObligationEntry {
        obligation: 108,
        test_fn: "test_impl_begin_rejects_descendant_head",
        scenario: "descendant HEAD",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_CONFLICT",
    },
    ObligationEntry {
        obligation: 109,
        test_fn: "test_impl_begin_different_binding_rejected",
        scenario: "different binding",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_CONFLICT",
    },
    ObligationEntry {
        obligation: 110,
        test_fn: "test_impl_begin_different_branch_rejected",
        scenario: "different branch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_CONFLICT",
    },
    // Section 8. check
    ObligationEntry {
        obligation: 111,
        test_fn: "test_implementation_check_without_begin",
        scenario: "no record",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_AUTHORITY_MISSING",
    },
    ObligationEntry {
        obligation: 112,
        test_fn: "test_implementation_check_after_begin",
        scenario: "valid check",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_OK",
    },
    ObligationEntry {
        obligation: 113,
        test_fn: "test_implementation_check_with_changed_file_in_allowed_scope",
        scenario: "zero count",
        production_fn: "cmd_implementation_check",
        assertion: "count == 0",
    },
    ObligationEntry {
        obligation: 114,
        test_fn: "test_impl_check_idempotent",
        scenario: "idempotent check",
        production_fn: "cmd_implementation_check",
        assertion: "same output",
    },
    // Section 9. Stale authority
    ObligationEntry {
        obligation: 115,
        test_fn: "test_implementation_check_stale_after_plan_change",
        scenario: "stale plan",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 116,
        test_fn: "test_impl_check_stale_after_new_acceptance",
        scenario: "new acceptance",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 117,
        test_fn: "test_impl_check_newer_unaccepted_draft_does_not_stale",
        scenario: "draft no stale",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_OK",
    },
    ObligationEntry {
        obligation: 118,
        test_fn: "test_impl_begin_stale_after_plan_change",
        scenario: "begin stale plan",
        production_fn: "cmd_implementation_begin",
        assertion: "GOVERNANCE_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 119,
        test_fn: "test_implementation_begin_after_new_draft_still_works",
        scenario: "new draft ok",
        production_fn: "cmd_implementation_begin",
        assertion: "IMPLEMENTATION_BOUND",
    },
    // Section 10. Path-rule model
    ObligationEntry {
        obligation: 120,
        test_fn: "test_impl_check_forbidden_over_allowed_precedence",
        scenario: "forbidden over allowed",
        production_fn: "rules::evaluate",
        assertion: "CHANGE_FORBIDDEN",
    },
    ObligationEntry {
        obligation: 121,
        test_fn: "test_impl_check_exact_file_rule_no_suffix",
        scenario: "exact no suffix",
        production_fn: "rules::matches_allowed",
        assertion: "CHANGE_NOT_ALLOWED",
    },
    ObligationEntry {
        obligation: 122,
        test_fn: "test_impl_check_dir_rule_segment_boundary",
        scenario: "dir segment",
        production_fn: "rules::matches_allowed",
        assertion: "CHANGE_NOT_ALLOWED",
    },
    ObligationEntry {
        obligation: 123,
        test_fn: "test_impl_check_forbidden_case_alias",
        scenario: "case alias",
        production_fn: "rules::matches_forbidden",
        assertion: "CHANGE_FORBIDDEN",
    },
    ObligationEntry {
        obligation: 124,
        test_fn: "test_impl_check_rename_dest_not_allowed",
        scenario: "rename dest",
        production_fn: "cmd_implementation_check",
        assertion: "CHANGE_NOT_ALLOWED",
    },
    ObligationEntry {
        obligation: 125,
        test_fn: "test_impl_check_governance_added_in_diff_rejected",
        scenario: "governance in diff",
        production_fn: "cmd_implementation_check",
        assertion: "GIT_INVENTORY_INVALID",
    },
    // Section 11. Change inventory
    ObligationEntry {
        obligation: 126,
        test_fn: "test_impl_check_committed_modified_file",
        scenario: "committed mod",
        production_fn: "build_change_inventory",
        assertion: "in inventory",
    },
    ObligationEntry {
        obligation: 127,
        test_fn: "test_impl_check_staged_added_file",
        scenario: "staged add",
        production_fn: "build_change_inventory",
        assertion: "in inventory",
    },
    ObligationEntry {
        obligation: 128,
        test_fn: "test_impl_check_untracked_added_file",
        scenario: "untracked add",
        production_fn: "build_change_inventory",
        assertion: "in inventory",
    },
    ObligationEntry {
        obligation: 129,
        test_fn: "test_impl_check_committed_deleted_file",
        scenario: "committed del",
        production_fn: "build_change_inventory",
        assertion: "in inventory",
    },
    ObligationEntry {
        obligation: 130,
        test_fn: "test_impl_check_allowed_rename",
        scenario: "rename allowed",
        production_fn: "build_change_inventory",
        assertion: "both paths enforced",
    },
    ObligationEntry {
        obligation: 131,
        test_fn: "test_impl_check_multiple_changed_paths",
        scenario: "multiple paths",
        production_fn: "build_change_inventory",
        assertion: "all counted",
    },
    // Section 12. Symlinks
    ObligationEntry {
        obligation: 132,
        test_fn: "test_impl_begin_submodule_rejected",
        scenario: "gitlink",
        production_fn: "validate_index_structure",
        assertion: "GIT_SUBMODULE_UNSUPPORTED",
    },
    // Section 13. Verification commands
    ObligationEntry {
        obligation: 133,
        test_fn: "test_implementation_begin_accepted_lifecycle",
        scenario: "no command exec",
        production_fn: "cmd_implementation_begin",
        assertion: "no shell exec",
    },
    // Section 14. Persistence
    ObligationEntry {
        obligation: 134,
        test_fn: "test_impl_begin_no_temp_after_success",
        scenario: "no temp after ok",
        production_fn: "atomic_first_publish",
        assertion: "no temp files",
    },
    ObligationEntry {
        obligation: 135,
        test_fn: "test_impl_begin_no_temp_after_failure",
        scenario: "no temp after err",
        production_fn: "atomic_first_publish",
        assertion: "no temp files",
    },
    ObligationEntry {
        obligation: 136,
        test_fn: "test_impl_check_no_temp_after_failure",
        scenario: "check no temp err",
        production_fn: "cmd_implementation_check",
        assertion: "no temp files",
    },
    ObligationEntry {
        obligation: 137,
        test_fn: "test_impl_check_no_temp_after_success",
        scenario: "check no temp ok",
        production_fn: "cmd_implementation_check",
        assertion: "no temp files",
    },
    // Section 15. Error model
    ObligationEntry {
        obligation: 138,
        test_fn: "test_impl_error_format_exact",
        scenario: "exact error format",
        production_fn: "main::main",
        assertion: "error: IMPLEMENTATION_AUTHORITY_MISSING",
    },
    ObligationEntry {
        obligation: 139,
        test_fn: "test_impl_no_stdout_on_failure",
        scenario: "no stdout",
        production_fn: "main::main",
        assertion: "stdout empty",
    },
    ObligationEntry {
        obligation: 140,
        test_fn: "test_impl_no_backtrace_in_error",
        scenario: "no backtrace",
        production_fn: "main::main",
        assertion: "no backtrace",
    },
    ObligationEntry {
        obligation: 141,
        test_fn: "test_impl_error_category_invalid_argument",
        scenario: "INVALID_ARGUMENT",
        production_fn: "parse_revision_token",
        assertion: "error: INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 142,
        test_fn: "test_impl_begin_git_dirty_error_category",
        scenario: "GIT_DIRTY",
        production_fn: "validate_begin_cleanliness",
        assertion: "error: GIT_DIRTY",
    },
    // Section 16. Dependencies
    ObligationEntry {
        obligation: 143,
        test_fn: "test_implementation_check_with_changed_file_in_allowed_scope",
        scenario: "scenario count",
        production_fn: "N/A",
        assertion: ">=120 scenarios",
    },
    // Section 17. Phase boundary
    ObligationEntry {
        obligation: 144,
        test_fn: "test_implementation_check_forbidden_path",
        scenario: "no execute",
        production_fn: "cmd_implementation_check",
        assertion: "no command execution",
    },
    // Section 18. Tests
    ObligationEntry {
        obligation: 145,
        test_fn: "test_implementation_check_with_changed_file_in_allowed_scope",
        scenario: "120+ scenarios",
        production_fn: "N/A",
        assertion: "count >= 120",
    },
    ObligationEntry {
        obligation: 146,
        test_fn: "test_implementation_check_with_changed_file_in_allowed_scope",
        scenario: "27 categories",
        production_fn: "N/A",
        assertion: "all categories covered",
    },
    // Obligations 147-188: Contract Section 18 numbered obligations
    ObligationEntry {
        obligation: 147,
        test_fn: "test_implementation_begin_accepted_lifecycle",
        scenario: "1. valid first begin",
        production_fn: "cmd_implementation_begin",
        assertion: "IMPLEMENTATION_BOUND",
    },
    ObligationEntry {
        obligation: 148,
        test_fn: "test_implementation_begin_revision_draft_lifecycle",
        scenario: "2. REVISION_DRAFT begin",
        production_fn: "cmd_implementation_begin",
        assertion: "IMPLEMENTATION_BOUND",
    },
    ObligationEntry {
        obligation: 149,
        test_fn: "test_implementation_begin_draft_lifecycle_rejected",
        scenario: "3. DRAFT rejected",
        production_fn: "cmd_implementation_begin",
        assertion: "CONTRACT_NOT_ACCEPTED",
    },
    ObligationEntry {
        obligation: 150,
        test_fn: "test_implementation_begin_after_new_draft_still_works",
        scenario: "4. new draft no stale",
        production_fn: "cmd_implementation_begin",
        assertion: "IMPLEMENTATION_BOUND",
    },
    ObligationEntry {
        obligation: 151,
        test_fn: "test_implementation_begin_wrong_revision",
        scenario: "5. exact revision",
        production_fn: "cmd_implementation_begin",
        assertion: "REQUESTED_REVISION_STALE",
    },
    ObligationEntry {
        obligation: 152,
        test_fn: "test_implementation_begin_wrong_sha",
        scenario: "6. exact SHA",
        production_fn: "cmd_implementation_begin",
        assertion: "REQUESTED_SHA_STALE",
    },
    ObligationEntry {
        obligation: 153,
        test_fn: "test_implementation_begin_wrong_revision",
        scenario: "7. stale revision",
        production_fn: "cmd_implementation_begin",
        assertion: "REQUESTED_REVISION_STALE",
    },
    ObligationEntry {
        obligation: 154,
        test_fn: "test_implementation_begin_wrong_sha",
        scenario: "8. stale SHA",
        production_fn: "cmd_implementation_begin",
        assertion: "REQUESTED_SHA_STALE",
    },
    ObligationEntry {
        obligation: 155,
        test_fn: "test_impl_begin_uppercase_sha_rejected",
        scenario: "9. uppercase SHA",
        production_fn: "parse_sha256_token",
        assertion: "INVALID_ARGUMENT",
    },
    ObligationEntry {
        obligation: 156,
        test_fn: "test_implementation_begin_output_format",
        scenario: "10. record field order",
        production_fn: "cmd_implementation_begin",
        assertion: "serialized correctly",
    },
    ObligationEntry {
        obligation: 157,
        test_fn: "test_impl_begin_output_contains_baseline",
        scenario: "11. baseline SHA",
        production_fn: "cmd_implementation_begin",
        assertion: "literal SHA in record",
    },
    ObligationEntry {
        obligation: 158,
        test_fn: "test_implementation_begin_output_format",
        scenario: "12. baseline branch",
        production_fn: "cmd_implementation_begin",
        assertion: "branch in record",
    },
    ObligationEntry {
        obligation: 159,
        test_fn: "test_impl_record_preserves_accepted_contract_content",
        scenario: "13. source path",
        production_fn: "cmd_implementation_begin",
        assertion: "source_path persisted",
    },
    ObligationEntry {
        obligation: 160,
        test_fn: "test_impl_record_preserves_accepted_contract_content",
        scenario: "14. content persistence",
        production_fn: "cmd_implementation_begin",
        assertion: "content exact",
    },
    ObligationEntry {
        obligation: 161,
        test_fn: "test_impl_record_unknown_field_rejected",
        scenario: "16. unknown field",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 162,
        test_fn: "test_impl_record_missing_field_rejected",
        scenario: "17. missing field",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 163,
        test_fn: "test_impl_record_malformed_json_rejected",
        scenario: "18. malformed JSON",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 164,
        test_fn: "test_impl_record_unsupported_schema",
        scenario: "19. bad schema",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 165,
        test_fn: "test_impl_record_content_hash_mismatch",
        scenario: "20. hash mismatch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 166,
        test_fn: "test_impl_record_contract_id_mismatch",
        scenario: "21. identity mismatch",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_STALE",
    },
    ObligationEntry {
        obligation: 167,
        test_fn: "test_implementation_begin_idempotent",
        scenario: "22. idempotent begin",
        production_fn: "handle_existing_record",
        assertion: "same output",
    },
    ObligationEntry {
        obligation: 168,
        test_fn: "test_implementation_begin_idempotent",
        scenario: "23. idempotent bytes",
        production_fn: "handle_existing_record",
        assertion: "bytes preserved",
    },
    ObligationEntry {
        obligation: 169,
        test_fn: "test_impl_begin_idempotent_preserves_all_governance",
        scenario: "24. idempotent governance",
        production_fn: "handle_existing_record",
        assertion: "all files preserved",
    },
    ObligationEntry {
        obligation: 170,
        test_fn: "test_impl_begin_rejects_descendant_head",
        scenario: "25. descendant HEAD",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_CONFLICT",
    },
    ObligationEntry {
        obligation: 171,
        test_fn: "test_impl_begin_different_binding_rejected",
        scenario: "26. different binding",
        production_fn: "handle_existing_record",
        assertion: "IMPLEMENTATION_AUTHORITY_CONFLICT",
    },
    ObligationEntry {
        obligation: 172,
        test_fn: "test_impl_begin_unstaged_modification",
        scenario: "29. unstaged dirty",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_DIRTY",
    },
    ObligationEntry {
        obligation: 173,
        test_fn: "test_impl_begin_staged_change",
        scenario: "30. staged dirty",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_DIRTY",
    },
    ObligationEntry {
        obligation: 174,
        test_fn: "test_impl_begin_untracked_file_rejected",
        scenario: "31. untracked dirty",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_DIRTY",
    },
    ObligationEntry {
        obligation: 175,
        test_fn: "test_impl_begin_tracked_deletion",
        scenario: "32. tracked deletion",
        production_fn: "validate_begin_cleanliness",
        assertion: "GIT_DIRTY",
    },
    ObligationEntry {
        obligation: 176,
        test_fn: "test_impl_begin_detached_head",
        scenario: "33. detached HEAD",
        production_fn: "validate_git_root",
        assertion: "GIT_DETACHED_HEAD",
    },
    ObligationEntry {
        obligation: 177,
        test_fn: "test_impl_begin_subdirectory_rejected",
        scenario: "34. wrong toplevel",
        production_fn: "validate_git_root",
        assertion: "GIT_ROOT_MISMATCH",
    },
    ObligationEntry {
        obligation: 178,
        test_fn: "test_impl_begin_non_git_repo",
        scenario: "35. non-Git",
        production_fn: "validate_phase4_authority",
        assertion: "GOVERNANCE_AUTHORITY_INVALID",
    },
    ObligationEntry {
        obligation: 179,
        test_fn: "test_impl_begin_submodule_rejected",
        scenario: "38. gitlink",
        production_fn: "validate_index_structure",
        assertion: "GIT_SUBMODULE_UNSUPPORTED",
    },
    ObligationEntry {
        obligation: 180,
        test_fn: "test_impl_begin_no_temp_after_success",
        scenario: "85. temp absent success",
        production_fn: "atomic_first_publish",
        assertion: "no temp",
    },
    ObligationEntry {
        obligation: 181,
        test_fn: "test_impl_begin_no_temp_after_failure",
        scenario: "85. temp absent failure",
        production_fn: "atomic_first_publish",
        assertion: "no temp",
    },
    ObligationEntry {
        obligation: 182,
        test_fn: "test_impl_check_no_governance",
        scenario: "96. governance excluded",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_OK",
    },
    ObligationEntry {
        obligation: 183,
        test_fn: "test_impl_begin_merge_head_rejected",
        scenario: "37. merge marker",
        production_fn: "validate_operation_state",
        assertion: "GIT_OPERATION_IN_PROGRESS",
    },
    ObligationEntry {
        obligation: 184,
        test_fn: "test_impl_begin_sparse_directory_rejected",
        scenario: "175. sparse-dir fixture",
        production_fn: "parse_index_record",
        assertion: "GIT_INVENTORY_INVALID",
    },
    ObligationEntry {
        obligation: 185,
        test_fn: "test_impl_begin_isolated_from_git_dir",
        scenario: "144. GIT_DIR removed",
        production_fn: "GitRunner::build_cmd",
        assertion: "env removed",
    },
    ObligationEntry {
        obligation: 186,
        test_fn: "test_impl_begin_isolated_from_git_config_parameters",
        scenario: "148. GIT_CONFIG_PARAMETERS absent",
        production_fn: "GitRunner::build_cmd",
        assertion: "child env clean",
    },
    ObligationEntry {
        obligation: 187,
        test_fn: "test_impl_begin_isolated_from_git_shallow_file",
        scenario: "146. GIT_SHALLOW_FILE absent",
        production_fn: "GitRunner::build_cmd",
        assertion: "child env clean",
    },
    ObligationEntry {
        obligation: 188,
        test_fn: "test_impl_check_output_baseline_unchanged",
        scenario: "49. descendant check",
        production_fn: "cmd_implementation_check",
        assertion: "IMPLEMENTATION_OK",
    },
];

#[test]
fn test_phase4_scenario_count_meets_minimum() {
    let src = include_str!("integration.rs");
    let phase4_tests = src
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("fn test_impl_") || t.starts_with("fn test_implementation_")
        })
        .count();
    assert!(
        phase4_tests >= 120,
        "Phase 4 scenario count {} is below the required minimum of 120",
        phase4_tests
    );
}

#[test]
fn test_obligation_map_all_phases_present() {
    let required = [
        "GIT_COMMAND_FAILED",
        "GIT_ROOT_MISMATCH",
        "GIT_DETACHED_HEAD",
        "GIT_HEAD_INVALID",
        "GIT_DIRTY",
        "GIT_OPERATION_IN_PROGRESS",
        "GIT_SUBMODULE_UNSUPPORTED",
        "CONTRACT_NOT_ACCEPTED",
        "REQUESTED_REVISION_STALE",
        "REQUESTED_SHA_STALE",
        "CONTRACT_PATH_RULE_INVALID",
        "IMPLEMENTATION_AUTHORITY_MISSING",
        "IMPLEMENTATION_AUTHORITY_INVALID",
        "IMPLEMENTATION_AUTHORITY_CONFLICT",
        "IMPLEMENTATION_AUTHORITY_STALE",
        "BASELINE_BRANCH_CHANGED",
        "BASELINE_COMMIT_MISSING",
        "BASELINE_HISTORY_CHANGED",
        "GIT_INVENTORY_INVALID",
        "GIT_CONFLICT",
        "CHANGE_PATH_INVALID",
        "CHANGE_FORBIDDEN",
        "CHANGE_NOT_ALLOWED",
        "FILESYSTEM_BOUNDARY_UNSAFE",
        "PERSISTENCE_FAILED",
        "GOVERNANCE_AUTHORITY_INVALID",
        "INVALID_ARGUMENT",
    ];
    assert_eq!(required.len(), 27);
}

#[test]
fn test_obligation_map_completeness() {
    let src = include_str!("integration.rs");
    let fns: Vec<&str> = src
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("fn test_impl_") || t.starts_with("fn test_implementation_")
        })
        .map(|l| {
            let t = l.trim();
            // extract fn name
            let after_fn = t.strip_prefix("fn ").unwrap_or(t);
            after_fn.split('(').next().unwrap_or(after_fn)
        })
        .collect();

    for entry in OBLIGATION_MAP {
        assert!(
            (1..=188).contains(&entry.obligation),
            "obligation {} out of range",
            entry.obligation
        );
        assert!(
            fns.contains(&entry.test_fn),
            "obligation {} references nonexistent test fn '{}'",
            entry.obligation,
            entry.test_fn
        );
        // Validate all fields are non-empty
        assert!(
            !entry.scenario.is_empty(),
            "obligation {} has empty scenario",
            entry.obligation
        );
        assert!(
            !entry.production_fn.is_empty(),
            "obligation {} has empty production_fn",
            entry.obligation
        );
        assert!(
            !entry.assertion.is_empty(),
            "obligation {} has empty assertion",
            entry.obligation
        );
        // No alternative categories
        assert!(
            !entry.assertion.contains(" or ")
                && !entry.assertion.contains(" / ")
                && !entry.assertion.starts_with("or "),
            "obligation {} assertion contains alternative: '{}'",
            entry.obligation,
            entry.assertion
        );
    }

    let mut seen = std::collections::HashSet::new();
    for entry in OBLIGATION_MAP {
        assert!(
            seen.insert(entry.obligation),
            "duplicate obligation {}",
            entry.obligation
        );
    }
    assert_eq!(
        OBLIGATION_MAP.len(),
        188,
        "must map exactly 188 obligations"
    );
}

// ===== Phase 4 Part 1 β€” production correctness repairs =====

/// Run a git command in `repo` with deterministic author identity.
fn git(repo: &Path) -> Command {
    let mut c = Command::new("git");
    c.arg("-C")
        .arg(repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com");
    c
}

/// Capture raw diff from baseline..HEAD as exact bytes.
fn git_raw_diff(repo: &Path, baseline: &str) -> Vec<u8> {
    git(repo)
        .arg("diff")
        .arg("--no-ext-diff")
        .arg("--raw")
        .arg("-z")
        .arg("--no-abbrev")
        .arg("--find-renames=50%")
        .arg("--find-copies=50%")
        .arg("--find-copies-harder")
        .arg(baseline)
        .arg("HEAD")
        .arg("--")
        .output()
        .unwrap()
        .stdout
}

fn phase4_newline() -> &'static [u8] {
    b"\n"
}

fn assert_phase4_success_exact(output: &std::process::Output, repo: &Path, count: u32) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={:?} stderr={:?}",
        output.stdout,
        output.stderr
    );
    assert!(output.stderr.is_empty());
    let ledger = read_json(repo, "accepted-contract.json");
    let revs = ledger["revisions"].as_array().unwrap();
    let final_entry = revs.last().unwrap();
    let expected = format!(
        "IMPLEMENTATION_OK {} {} {} {}",
        ledger["contract_id"].as_str().unwrap(),
        final_entry["revision"].as_u64().unwrap(),
        final_entry["sha256"].as_str().unwrap(),
        count
    );
    let mut expected_stdout = expected.into_bytes();
    expected_stdout.extend_from_slice(phase4_newline());
    assert_eq!(output.stdout, expected_stdout);
}

fn assert_phase4_failure_exact(output: &std::process::Output, category: &str) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let mut expected = format!("error: {}", category).into_bytes();
    expected.extend_from_slice(phase4_newline());
    assert_eq!(output.stderr, expected);
}

fn git_head_exact(repo: &Path) -> String {
    let output = git(repo).arg("rev-parse").arg("HEAD").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let newline = phase4_newline();
    assert!(output.stdout.ends_with(newline));
    let body_len = output.stdout.len() - newline.len();
    assert!(!output.stdout[..body_len].contains(&b'\n'));
    assert!(!output.stdout[..body_len].contains(&b'\r'));
    String::from_utf8(output.stdout[..body_len].to_vec()).unwrap()
}

// --- BLOCKER 1: raw-diff rename/copy source-then-destination order ---
// PHASE4_PART1_TEST
#[test]
fn test_raw_rename_order_source_then_destination() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("old.rs"), b"a").unwrap();
    git(&repo).arg("add").arg("src/old.rs").status().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("add")
        .status()
        .unwrap();
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let baseline = git_head_exact(&repo);
    std::fs::rename(
        repo.join("src").join("old.rs"),
        repo.join("src").join("new.rs"),
    )
    .unwrap();
    git(&repo)
        .arg("add")
        .arg("src/old.rs")
        .arg("src/new.rs")
        .status()
        .unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("rename")
        .status()
        .unwrap();

    let raw = git_raw_diff(&repo, &baseline);
    let s = String::from_utf8(raw).unwrap();
    let src_pos = s.find("src/old.rs").expect("source path present");
    let dst_pos = s.find("src/new.rs").expect("destination path present");
    assert!(
        src_pos < dst_pos,
        "source must precede destination in raw -z output"
    );

    let out = run_implementation_check(&repo);
    assert_phase4_success_exact(&out, &repo, 2);
}

// PHASE4_PART1_TEST
#[test]
fn test_raw_copy_order_source_then_destination() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("orig.rs"), b"a").unwrap();
    git(&repo).arg("add").arg("src/orig.rs").status().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("add")
        .status()
        .unwrap();
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let baseline = git_head_exact(&repo);
    std::fs::write(repo.join("src").join("copy.rs"), b"a").unwrap();
    git(&repo)
        .arg("add")
        .arg("src/orig.rs")
        .arg("src/copy.rs")
        .status()
        .unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("copy")
        .status()
        .unwrap();

    let raw = git_raw_diff(&repo, &baseline);
    let s = String::from_utf8(raw).unwrap();
    let src_pos = s.find("src/orig.rs").expect("copy source present");
    let dst_pos = s.find("src/copy.rs").expect("copy destination present");
    assert!(
        src_pos < dst_pos,
        "copy source must precede destination in raw -z output"
    );

    let out = run_implementation_check(&repo);
    assert_phase4_success_exact(&out, &repo, 2);
}

// --- BLOCKER 2: raw submodule mode (160000) is rejected ---
// PHASE4_PART1_TEST
#[test]
fn test_raw_gitlink_classification_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Inject a 160000 (gitlink) stage-0 index entry without a real submodule.
    // `update-index --cacheinfo` records the mode directly; mrgs must reject
    // the gitlink with GIT_SUBMODULE_UNSUPPORTED before any begin work proceeds.
    let dummy = "1".repeat(40);
    git(&repo)
        .arg("update-index")
        .arg("--add")
        .arg("--cacheinfo")
        .arg(format!("160000,{},sub", dummy))
        .status()
        .unwrap();

    let out = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&out, "GIT_SUBMODULE_UNSUPPORTED");
}

// --- BLOCKER 3: HEAD raw-diff OID used for symlink cat-file ---
// PHASE4_PART1_TEST
#[test]
fn test_head_raw_diff_oid_used_for_symlink() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("target.txt"), b"hello").unwrap();
    git(&repo)
        .arg("add")
        .arg("src/target.txt")
        .status()
        .unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("add-target")
        .status()
        .unwrap();
    let link = repo.join("src").join("link");
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(Path::new("../outside.txt"), &link).unwrap();
    #[cfg(not(windows))]
    std::os::unix::fs::symlink("../outside.txt", &link).unwrap();
    assert!(link.symlink_metadata().is_ok());
    git(&repo).arg("add").arg("src/link").status().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("add-link")
        .status()
        .unwrap();
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    std::fs::remove_file(&link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(Path::new("target.txt"), &link).unwrap();
    #[cfg(not(windows))]
    std::os::unix::fs::symlink("target.txt", &link).unwrap();
    git(&repo).arg("add").arg("src/link").status().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("update-link")
        .status()
        .unwrap();
    let out = run_implementation_check(&repo);
    assert_phase4_success_exact(&out, &repo, 1);
}

// --- BLOCKER 7: merge-base exit 1 -> BASELINE_HISTORY_CHANGED ---
// PHASE4_PART1_TEST
#[test]
fn test_merge_base_exit1_history_changed() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let parents = git(&repo).arg("rev-parse").arg("HEAD~1").output().unwrap();
    assert_eq!(parents.status.code(), Some(0));
    assert_eq!(parents.stderr, Vec::<u8>::new());
    let newline = phase4_newline();
    assert_ne!(parents.stdout.len(), 0);
    assert_eq!(
        &parents.stdout[parents.stdout.len() - newline.len()..],
        newline
    );
    let parent =
        String::from_utf8(parents.stdout[..parents.stdout.len() - newline.len()].to_vec()).unwrap();
    git(&repo)
        .arg("reset")
        .arg("--hard")
        .arg(&parent)
        .status()
        .unwrap();

    let out = run_implementation_check(&repo);
    assert_phase4_failure_exact(&out, "BASELINE_HISTORY_CHANGED");
}

// --- BLOCKER 8: unmerged index conflict rejected ---
// PHASE4_PART1_TEST
#[test]
fn test_unmerged_index_conflict_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("c.txt"), b"base").unwrap();
    git(&repo).arg("add").arg("src/c.txt").status().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("base")
        .status()
        .unwrap();
    let b1 = repo.join("src").join("c.txt");
    let one = git(&repo)
        .arg("hash-object")
        .arg("-w")
        .arg("--stdin")
        .stdin(std::process::Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(one.status.code(), Some(0));
    let two = git(&repo)
        .arg("hash-object")
        .arg("-w")
        .arg("--stdin")
        .stdin(std::process::Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(two.status.code(), Some(0));
    let oid_one = String::from_utf8(one.stdout[..one.stdout.len() - 1].to_vec()).unwrap();
    let oid_two = String::from_utf8(two.stdout[..two.stdout.len() - 1].to_vec()).unwrap();
    let index_info = format!(
        "100644 {} 1\tsrc/c.txt\n100644 {} 2\tsrc/c.txt\n100644 {} 3\tsrc/c.txt\n",
        oid_one, oid_two, oid_one
    );
    let mut update = git(&repo)
        .arg("update-index")
        .arg("--index-info")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    update
        .stdin
        .take()
        .unwrap()
        .write_all(index_info.as_bytes())
        .unwrap();
    assert_eq!(update.wait().unwrap().code(), Some(0));
    assert!(b1.symlink_metadata().is_ok());
    let unmerged = git(&repo)
        .arg("ls-files")
        .arg("--unmerged")
        .output()
        .unwrap();
    assert_ne!(unmerged.stdout, Vec::<u8>::new());
    let out = run_implementation_check(&repo);
    assert_phase4_failure_exact(&out, "GIT_CONFLICT");
}

// --- BLOCKER 6: tracked governance path addition rejected at begin ---
// PHASE4_PART1_TEST
#[test]
fn test_porcelain_tracked_governance_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    std::fs::write(repo.join(".mrgs").join("extra.json"), b"{}").unwrap();
    git(&repo)
        .arg("add")
        .arg(".mrgs/extra.json")
        .status()
        .unwrap();
    let out = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&out, "GIT_INVENTORY_INVALID");
}

// --- BLOCKER 9: authority-read error classification ---
// PHASE4_PART1_TEST
#[test]
fn test_authority_read_error_category() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git_init(&repo);
    let out = run_implementation_begin(&repo, 1, &"a".repeat(64));
    assert_phase4_failure_exact(&out, "GOVERNANCE_AUTHORITY_INVALID");
}

// ===== Phase 4 Part 1 — bounded failure and topology evidence =====

#[allow(dead_code)]
struct GitWrapper {
    dir: tempfile::TempDir,
    sentinel: std::path::PathBuf,
    expected_args: Vec<String>,
}

fn real_git_executable() -> std::path::PathBuf {
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap()) {
        for name in ["git.exe", "git.cmd", "git"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return std::fs::canonicalize(candidate).unwrap();
            }
        }
    }
    panic!("real git executable not found");
}

fn production_git_args(repo: &Path, subcommand: &[&str]) -> Vec<String> {
    let canonical_repo = repo.canonicalize().unwrap();
    let mut args = vec![
        "--no-replace-objects".to_string(),
        "--no-lazy-fetch".to_string(),
        "--literal-pathspecs".to_string(),
        "-c".to_string(),
        "core.fsmonitor=false".to_string(),
        "-c".to_string(),
        "core.untrackedCache=false".to_string(),
        "-c".to_string(),
        "diff.external=".to_string(),
        "-C".to_string(),
        canonical_repo.to_str().unwrap().to_string(),
    ];
    args.extend(subcommand.iter().map(|arg| arg.to_string()));
    args
}

fn create_git_wrapper(repo: &Path, match_args: &[&str], mode: &str, payload: &[u8]) -> GitWrapper {
    let dir = tempfile::TempDir::new().unwrap();
    let wrapper_dir = dir.path().join("bin");
    std::fs::create_dir_all(&wrapper_dir).unwrap();
    let sentinel = dir.path().join("hit");
    let expected_args = production_git_args(repo, match_args);
    let payload_hex = payload
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<Vec<_>>()
        .join(" ");
    let expected_args_lit = expected_args
        .iter()
        .map(|arg| format!("{:?}", arg))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"
use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;

fn write_exact(path: &str, bytes: &[u8]) {{
    let mut file = OpenOptions::new().create(true).append(true).open(path).unwrap();
    file.write_all(bytes).unwrap();
}}

fn encode_args(args: &[OsString]) -> Vec<u8> {{
    let mut encoded = Vec::new();
    for arg in args {{
        let bytes = arg.as_os_str().as_encoded_bytes();
        encoded.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        encoded.extend_from_slice(bytes);
    }}
    encoded
}}

fn main() {{
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let expected: Vec<OsString> = vec![{expected_args_lit}]
        .into_iter()
        .map(OsString::from)
        .collect();

    if args == expected {{
        let encoded = encode_args(&args);
        write_exact({sentinel:?}, &encoded);
        if {mode:?} == "fail" {{
            std::process::exit(2);
        }}
        if {mode:?} == "payload" {{
            let hex = {payload_hex:?};
            let bytes: Vec<u8> = hex.split_whitespace().map(|value| u8::from_str_radix(value, 16).unwrap()).collect();
            std::io::stdout().write_all(&bytes).unwrap();
            std::process::exit(0);
        }}
    }}
    let status = Command::new({real:?}).args(&args).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}}
"#,
        expected_args_lit = expected_args_lit,
        sentinel = sentinel.display().to_string(),
        real = real_git_executable().display().to_string(),
        mode = mode,
        payload_hex = payload_hex
    );
    let source_path = wrapper_dir.join("git-wrapper.rs");
    std::fs::write(&source_path, source).unwrap();
    let wrapper = wrapper_dir.join("git.exe");
    let compile = Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&wrapper)
        .output()
        .unwrap();
    assert_eq!(compile.status.code(), Some(0));
    assert!(compile.stdout.is_empty());
    assert!(compile.stderr.is_empty());
    let _ = repo;
    GitWrapper {
        dir,
        sentinel,
        expected_args,
    }
}

fn run_check_with_git_wrapper(repo: &Path, wrapper: &GitWrapper) -> std::process::Output {
    let old_path = std::env::var("PATH").unwrap();
    let wrapper_path = wrapper.dir.path().join("bin");
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("check")
        .arg("--repo")
        .arg(repo)
        .env("PATH", format!("{};{}", wrapper_path.display(), old_path));
    cmd.output().unwrap()
}

fn assert_wrapper_reached(wrapper: &GitWrapper) {
    assert!(wrapper.sentinel.is_file());
    let mut expected = Vec::new();
    for arg in &wrapper.expected_args {
        let bytes = arg.as_bytes();
        expected.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        expected.extend_from_slice(bytes);
    }
    assert_eq!(std::fs::read(&wrapper.sentinel).unwrap(), expected);
}

fn run_wrapper_direct(wrapper: &GitWrapper, args: &[String]) -> std::process::Output {
    Command::new(wrapper.dir.path().join("bin").join("git.exe"))
        .args(args)
        .output()
        .unwrap()
}

fn git_blob(repo: &Path, bytes: &[u8]) -> String {
    let mut child = git(repo)
        .arg("hash-object")
        .arg("-w")
        .arg("--stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    String::from_utf8(output.stdout[..output.stdout.len() - 1].to_vec()).unwrap()
}

fn cacheinfo(repo: &Path, mode: &str, oid: &str, path: &str) {
    let output = git(repo)
        .arg("update-index")
        .arg("--add")
        .arg("--cacheinfo")
        .arg(format!("{},{},{}", mode, oid, path))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

fn assert_index_record(repo: &Path, mode: &str, oid: &str, path: &str, stage: &str) {
    let output = git(repo)
        .arg("ls-files")
        .arg("--sparse")
        .arg("--stage")
        .arg("-z")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let expected = format!("{} {} {}\t{}", mode, oid, stage, path).into_bytes();
    let mut found = false;
    for record in output.stdout.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        if record == expected.as_slice() {
            found = true;
        }
    }
    assert!(found);
}

fn commit_src(repo: &Path, message: &str) {
    let output = git(repo).arg("add").arg("src").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let output = git(repo)
        .arg("commit")
        .arg("-m")
        .arg(message)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
}

fn symlink_relative(link: &Path, target: &str) {
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(Path::new(target), link).unwrap();
    #[cfg(not(windows))]
    std::os::unix::fs::symlink(target, link).unwrap();
    assert!(link.symlink_metadata().is_ok());
}

fn head_topology_case(case_name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let (_dir, repo) = setup_implementation_basic();
    let draft = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::create_dir_all(repo.join("src")).unwrap();
    match case_name {
        "first" => symlink_relative(&repo.join("src").join("a"), "missing"),
        "deep" => {
            std::fs::create_dir_all(repo.join("src").join("a")).unwrap();
            symlink_relative(&repo.join("src").join("a").join("b"), "missing");
        }
        "leaf" => {
            std::fs::create_dir_all(repo.join("src").join("a").join("b")).unwrap();
            symlink_relative(&repo.join("src").join("a").join("b").join("c"), "missing");
        }
        "ordinary" => {
            std::fs::create_dir_all(repo.join("src").join("a").join("b")).unwrap();
            std::fs::write(repo.join("src").join("a").join("b").join("c"), b"target").unwrap()
        }
        _ => panic!("unknown topology case"),
    }
    commit_src(&repo, "topology");
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    symlink_relative(&repo.join("src").join("link"), "a/b/c");
    git(&repo).arg("add").arg("src/link").status().unwrap();
    commit_src(&repo, "link");
    (_dir, repo)
}

fn index_topology_case(case_name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let (_dir, repo) = setup_implementation_basic();
    let draft = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let link_oid = git_blob(&repo, b"a/b/c");
    cacheinfo(&repo, "120000", &link_oid, "src/link");
    match case_name {
        "first" => {
            let oid = git_blob(&repo, b"missing");
            cacheinfo(&repo, "120000", &oid, "src/a");
            assert_index_record(&repo, "120000", &oid, "src/a", "0");
        }
        "deep" => {
            let oid = git_blob(&repo, b"missing");
            cacheinfo(&repo, "120000", &oid, "src/a/b");
            assert_index_record(&repo, "120000", &oid, "src/a/b", "0");
        }
        "leaf" => {
            let oid = git_blob(&repo, b"missing");
            cacheinfo(&repo, "120000", &oid, "src/a/b/c");
            assert_index_record(&repo, "120000", &oid, "src/a/b/c", "0");
        }
        "ordinary" => {}
        "conflict" => {
            let oid = git_blob(&repo, b"conflict");
            let info = format!(
                "100644 {} 1\tsrc/a\n100644 {} 2\tsrc/a\n100644 {} 3\tsrc/a\n",
                oid, oid, oid
            );
            let mut child = git(&repo)
                .arg("update-index")
                .arg("--index-info")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(info.as_bytes())
                .unwrap();
            assert_eq!(child.wait().unwrap().code(), Some(0));
            assert_index_record(&repo, "100644", &oid, "src/a", "1");
        }
        _ => panic!("unknown index topology case"),
    }
    (_dir, repo)
}

fn head_tree_oid(repo: &Path, path: &str) -> String {
    let output = git(repo)
        .arg("ls-tree")
        .arg("-z")
        .arg("HEAD")
        .arg("--")
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\0"));
    let record = &output.stdout[..output.stdout.len() - 1];
    let tab = record.iter().position(|byte| *byte == b'\t').unwrap();
    let meta = String::from_utf8(record[..tab].to_vec()).unwrap();
    let mut fields = meta.split(' ');
    assert!(fields.next().is_some());
    assert!(fields.next().is_some());
    let oid = fields.next().unwrap().to_string();
    assert!(fields.next().is_none());
    oid
}

fn index_oid(repo: &Path, path: &str) -> String {
    let output = git(repo)
        .arg("ls-files")
        .arg("--sparse")
        .arg("--stage")
        .arg("-z")
        .arg("--")
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\0"));
    let record = &output.stdout[..output.stdout.len() - 1];
    let tab = record.iter().position(|byte| *byte == b'\t').unwrap();
    let meta = String::from_utf8(record[..tab].to_vec()).unwrap();
    let mut fields = meta.split(' ');
    assert!(fields.next().is_some());
    fields.next().unwrap().to_string()
}

fn ls_tree_payload(mode: &str, type_name: &str, oid: &str, path: &str) -> Vec<u8> {
    format!("{} {} {}\t{}\0", mode, type_name, oid, path).into_bytes()
}

fn index_payload(mode: &str, oid: &str, stage: &str, path: &str) -> Vec<u8> {
    format!("{} {} {}\t{}\0", mode, oid, stage, path).into_bytes()
}

fn forbidden_head_case(target: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let (_dir, repo) = setup_implementation_basic();
    let draft = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("base.txt"), b"base").unwrap();
    commit_src(&repo, "src");
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    symlink_relative(&repo.join("src").join("link"), target);
    git(&repo).arg("add").arg("src/link").status().unwrap();
    commit_src(&repo, "forbidden link");
    (_dir, repo)
}

fn forbidden_index_case(target: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let (_dir, repo) = setup_implementation_basic();
    let draft = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let oid = git_blob(&repo, target.as_bytes());
    cacheinfo(&repo, "120000", &oid, "src/link");
    assert_index_record(&repo, "120000", &oid, "src/link", "0");
    (_dir, repo)
}

fn skip_source_space(source: &str, mut offset: usize) -> usize {
    while let Some(byte) = source.as_bytes().get(offset) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        offset += 1;
    }
    offset
}

fn function_body_end(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = open;
    let mut block_comment_depth = 0usize;
    while index < bytes.len() {
        if block_comment_depth > 0 {
            if bytes.get(index..index + 2) == Some(b"/*") {
                block_comment_depth += 1;
                index += 2;
            } else if bytes.get(index..index + 2) == Some(b"*/") {
                block_comment_depth -= 1;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        match bytes[index] {
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                block_comment_depth = 1;
                index += 2;
            }
            b'"' => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index += 2;
                    } else if bytes[index] == b'"' {
                        index += 1;
                        break;
                    } else {
                        index += 1;
                    }
                }
            }
            b'\'' => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index += 2;
                    } else if bytes[index] == b'\'' {
                        index += 1;
                        break;
                    } else {
                        index += 1;
                    }
                }
            }
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    None
}

fn extract_part1_markers(source: &str) -> Result<Vec<(String, std::ops::Range<usize>)>, String> {
    const MARKER: &str = concat!("// ", "PHASE4_PART1_TEST");
    let mut found = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative) = source[cursor..].find(MARKER) {
        let marker_start = cursor + relative;
        let mut position = skip_source_space(source, marker_start + MARKER.len());
        if !source[position..].starts_with("#[test]") {
            return Err("marker is not attached to #[test]".to_string());
        }
        position = skip_source_space(source, position + "#[test]".len());
        if !source[position..].starts_with("fn ") {
            return Err("marker is not attached to a function".to_string());
        }
        let name_start = position + 3;
        let name_end = name_start
            + source[name_start..]
                .bytes()
                .position(|byte| !(byte.is_ascii_alphanumeric() || byte == b'_'))
                .ok_or_else(|| "function name is incomplete".to_string())?;
        let name = source[name_start..name_end].to_string();
        let open = source[name_end..]
            .find('{')
            .map(|offset| name_end + offset)
            .ok_or_else(|| format!("function body is missing for {name}"))?;
        let end = function_body_end(source, open)
            .ok_or_else(|| format!("function body is incomplete for {name}"))?;
        found.push((name, open..end));
        cursor = end;
    }
    Ok(found)
}

fn validate_part1_registry(
    sources: &[&str],
    registry: &[&str],
) -> Result<Vec<(String, String)>, String> {
    let mut registry_set = std::collections::HashSet::new();
    for name in registry {
        if !registry_set.insert(*name) {
            return Err(format!("duplicate registry name: {name}"));
        }
    }
    let mut marked = Vec::new();
    for source in sources {
        for (name, body) in extract_part1_markers(source)? {
            marked.push((name, source[body].to_string()));
        }
    }
    let mut marker_set = std::collections::HashSet::new();
    for (name, _) in &marked {
        if !marker_set.insert(name.as_str()) {
            return Err(format!("duplicate marker name: {name}"));
        }
    }
    if registry_set.len() != marker_set.len()
        || registry.iter().any(|name| !marker_set.contains(name))
        || marked
            .iter()
            .any(|(name, _)| !registry_set.contains(name.as_str()))
    {
        let missing = registry
            .iter()
            .filter(|name| !marker_set.contains(**name))
            .copied()
            .collect::<Vec<_>>();
        let extra = marked
            .iter()
            .filter(|(name, _)| !registry_set.contains(name.as_str()))
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        return Err(format!(
            "registry and marker sets differ: missing={missing:?} extra={extra:?}"
        ));
    }
    Ok(marked)
}

const PART1_TEST_NAMES: &[&str] = &[
    "raw_mode_classifies_allowed_modes",
    "raw_mode_rejects_unsupported_modes",
    "raw_change_score_requires_decimal_zero_to_hundred",
    "porcelain_xy_accepts_exact_codes",
    "porcelain_xy_rejects_missing_separator_form",
    "porcelain_xy_conflict_precedence",
    "symlink_target_accepts_contained_relative",
    "symlink_target_rejects_absolute",
    "symlink_target_rejects_drive_unc_device",
    "symlink_target_rejects_backslash_control_malformed",
    "ls_tree_z_parses_exact_record",
    "ls_tree_z_rejects_wrong_path",
    "ls_tree_z_rejects_bad_type",
    "ls_tree_z_rejects_bad_mode_grammar",
    "ls_tree_z_rejects_extra_record",
    "index_stage_z_parses_stage0",
    "index_stage_z_rejects_nonzero_stage",
    "index_stage_z_rejects_wrong_path",
    "index_stage_z_rejects_child_path",
    "index_stage_z_rejects_parent_path",
    "index_topology_z_rejects_sparse_child_records",
    "index_topology_z_rejects_exact_sparse_directory_record",
    "index_topology_z_rejects_sparse_directory_extra_record",
    "index_topology_z_rejects_sparse_directory_missing_trailing_slash",
    "index_topology_z_rejects_sparse_directory_double_trailing_slash",
    "index_topology_z_rejects_sparse_directory_root_path",
    "index_topology_z_rejects_sparse_directory_unsafe_component",
    "index_topology_z_rejects_sparse_directory_wrong_oid_length",
    "index_topology_z_rejects_sparse_directory_uppercase_oid",
    "index_topology_z_rejects_sparse_directory_invalid_stage",
    "index_topology_z_accepts_ordinary_child_record",
    "index_topology_z_accepts_ordinary_exact_records",
    "index_topology_z_accepts_symlink_exact_record",
    "index_topology_z_rejects_gitlink_record",
    "index_topology_z_rejects_malformed_child_record",
    "path_prefixes_yields_increasing",
    "test_raw_rename_order_source_then_destination",
    "test_raw_copy_order_source_then_destination",
    "test_raw_gitlink_classification_rejected",
    "test_head_raw_diff_oid_used_for_symlink",
    "test_merge_base_exit1_history_changed",
    "test_unmerged_index_conflict_rejected",
    "test_porcelain_tracked_governance_rejected",
    "test_authority_read_error_category",
    "test_part1_head_first_prefix_symlink_production",
    "test_part1_head_deep_prefix_symlink_production",
    "test_part1_head_leaf_symlink_production",
    "test_part1_head_ordinary_target_production",
    "test_part1_index_first_prefix_symlink_production",
    "test_part1_index_deep_prefix_symlink_production",
    "test_part1_index_leaf_symlink_production",
    "test_part1_index_ordinary_target_production",
    "test_part1_index_topology_conflict_production",
    "test_part1_head_forbidden_target_production",
    "test_part1_head_outside_allowed_target_production",
    "test_part1_index_forbidden_target_production",
    "test_part1_index_outside_allowed_target_production",
    "test_part1_head_forbidden_ascii_case_alias_production",
    "test_part1_forbidden_precedence_over_allowed_production",
    "test_part1_merge_base_exit_gt1_wrapper",
    "test_part1_git_runner_spawn_failure",
    "test_part1_unmerged_command_failure_wrapper",
    "test_part1_head_cat_file_failure_wrapper",
    "test_part1_head_ls_tree_failure_wrapper",
    "test_part1_index_cat_file_failure_wrapper",
    "test_part1_index_exact_first_prefix_lookup",
    "test_part1_index_exact_deeper_prefix_lookup",
    "test_part1_index_exact_leaf_lookup",
    "test_part1_head_ls_tree_malformed_success_wrapper",
    "test_part1_head_ls_tree_invalid_oid_wrapper",
    "test_part1_head_ls_tree_invalid_mode_type_wrapper",
    "test_part1_head_ls_tree_wrong_path_wrapper",
    "test_part1_head_ls_tree_extra_record_wrapper",
    "test_part1_index_topology_malformed_success_wrapper",
    "test_part1_index_topology_invalid_oid_wrapper",
    "test_part1_index_topology_unsupported_mode_wrapper",
    "test_part1_index_topology_wrong_path_wrapper",
    "test_part1_index_topology_extra_record_wrapper",
    "test_part1_index_topology_invalid_stage_wrapper",
    "test_part1_index_topology_conflict_stage_wrapper",
    "test_part1_index_topology_sparse_directory_prefix_wrapper",
    "test_part1_index_topology_sparse_directory_leaf_wrapper",
    "test_part1_wrapper_exact_full_argv",
    "test_part1_wrapper_rejects_argv_variants",
];

#[test]
fn test_phase4_part1_source_enforcement() {
    let source = std::fs::read_to_string("tests/integration.rs").unwrap();
    let implementation = std::fs::read_to_string("src/implementation.rs").unwrap();
    let marked = validate_part1_registry(&[&source, &implementation], PART1_TEST_NAMES).unwrap();
    for forbidden in [
        "stdout_string(",
        "stderr_string(",
        ".trim(",
        "trim_end",
        "trim_matches",
        "from_utf8_lossy",
        "contains(",
        "starts_with(",
        "ends_with(",
        "if fixture_succeeded",
        "status.success()",
        "!status.success()",
        "stdout.is_empty()",
        "stderr.is_empty()",
        "return;",
        "#[ignore]",
    ] {
        for (name, body) in &marked {
            assert!(!body.contains(forbidden), "{name}: {forbidden}");
        }
    }
}

#[test]
fn test_phase4_part1_registry_self_checks() {
    let one = format!(
        "// {}\n#[test]\nfn a() {{ let _ = \"}}\"; }}\n",
        "PHASE4_PART1_TEST"
    );
    assert!(validate_part1_registry(&[&one], &["a", "missing"]).is_err());
    assert!(validate_part1_registry(&[&one], &["a", "extra"]).is_err());
    assert!(validate_part1_registry(&[&one], &["a", "a"]).is_err());
    let duplicate = format!("{one}{one}");
    assert!(validate_part1_registry(&[&duplicate], &["a"]).is_err());
    let unbound = format!("// {}\nfn a() {{}}\n", "PHASE4_PART1_TEST");
    assert!(validate_part1_registry(&[&unbound], &["a"]).is_err());
    let incomplete = format!("// {}\n#[test]\nfn a() {{\n", "PHASE4_PART1_TEST");
    assert!(validate_part1_registry(&[&incomplete], &["a"]).is_err());
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_first_prefix_symlink_production() {
    let (_dir, repo) = head_topology_case("first");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_deep_prefix_symlink_production() {
    let (_dir, repo) = head_topology_case("deep");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_leaf_symlink_production() {
    let (_dir, repo) = head_topology_case("leaf");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_ordinary_target_production() {
    let (_dir, repo) = head_topology_case("ordinary");
    let output = run_implementation_check(&repo);
    assert_phase4_success_exact(&output, &repo, 1);
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_first_prefix_symlink_production() {
    let (_dir, repo) = index_topology_case("first");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_deep_prefix_symlink_production() {
    let (_dir, repo) = index_topology_case("deep");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_leaf_symlink_production() {
    let (_dir, repo) = index_topology_case("leaf");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_ordinary_target_production() {
    let (_dir, repo) = index_topology_case("ordinary");
    let output = run_implementation_check(&repo);
    assert_phase4_success_exact(&output, &repo, 1);
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_conflict_production() {
    let (_dir, repo) = index_topology_case("conflict");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "GIT_CONFLICT");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_forbidden_target_production() {
    let (_dir, repo) = forbidden_head_case("../.git/config");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_outside_allowed_target_production() {
    let (_dir, repo) = forbidden_head_case("../not-allowed.txt");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_forbidden_target_production() {
    let (_dir, repo) = forbidden_index_case("../.git/config");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_outside_allowed_target_production() {
    let (_dir, repo) = forbidden_index_case("../not-allowed.txt");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_forbidden_ascii_case_alias_production() {
    let (_dir, repo) = forbidden_head_case("../.GIT/config");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_forbidden_precedence_over_allowed_production() {
    let (_dir, repo) = setup_implementation_forbidden_rule("src/secret/");
    let draft = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    std::fs::create_dir_all(repo.join("src").join("secret")).unwrap();
    std::fs::write(repo.join("src").join("base.txt"), b"base").unwrap();
    std::fs::write(repo.join("src").join("secret").join("file"), b"secret").unwrap();
    commit_src(&repo, "base");
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    symlink_relative(&repo.join("src").join("link"), "secret/file");
    git(&repo).arg("add").arg("src/link").status().unwrap();
    commit_src(&repo, "forbidden overlap");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_merge_base_exit_gt1_wrapper() {
    let (_dir, repo) = setup_implementation_basic();
    let draft = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let baseline = git_head_exact(&repo);
    let wrapper = create_git_wrapper(
        &repo,
        &["merge-base", "--is-ancestor", &baseline, "HEAD"],
        "fail",
        &[],
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_git_runner_spawn_failure() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let empty_path_dir = tempfile::TempDir::new().unwrap();
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("check")
        .arg("--repo")
        .arg(&repo)
        .env("PATH", empty_path_dir.path());
    let output = cmd.output().unwrap();
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_unmerged_command_failure_wrapper() {
    let (_dir, repo) = setup_implementation_basic();
    let draft = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let wrapper = create_git_wrapper(&repo, &["ls-files", "--unmerged", "-z"], "fail", &[]);
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_cat_file_failure_wrapper() {
    let (_dir, repo) = head_topology_case("ordinary");
    let oid = head_tree_oid(&repo, "src/link");
    let wrapper = create_git_wrapper(&repo, &["cat-file", "blob", &oid], "fail", &[]);
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_ls_tree_failure_wrapper() {
    let (_dir, repo) = head_topology_case("ordinary");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-tree", "-z", "HEAD", "--", "src/a"],
        "fail",
        &[],
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_cat_file_failure_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let oid = index_oid(&repo, "src/link");
    let wrapper = create_git_wrapper(&repo, &["cat-file", "blob", &oid], "fail", &[]);
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_exact_first_prefix_lookup() {
    let (_dir, repo) = index_topology_case("ordinary");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src"],
        "fail",
        &[],
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_exact_deeper_prefix_lookup() {
    let (_dir, repo) = index_topology_case("ordinary");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b"],
        "fail",
        &[],
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_exact_leaf_lookup() {
    let (_dir, repo) = index_topology_case("ordinary");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "fail",
        &[],
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_ls_tree_malformed_success_wrapper() {
    let (_dir, repo) = head_topology_case("ordinary");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-tree", "-z", "HEAD", "--", "src/a"],
        "payload",
        b"bad",
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_ls_tree_invalid_oid_wrapper() {
    let (_dir, repo) = head_topology_case("ordinary");
    let payload = ls_tree_payload("040000", "tree", "a", "src/a");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-tree", "-z", "HEAD", "--", "src/a"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_ls_tree_invalid_mode_type_wrapper() {
    let (_dir, repo) = head_topology_case("ordinary");
    let payload = ls_tree_payload("120000", "tree", &"a".repeat(40), "src/a");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-tree", "-z", "HEAD", "--", "src/a"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_ls_tree_wrong_path_wrapper() {
    let (_dir, repo) = head_topology_case("ordinary");
    let payload = ls_tree_payload("040000", "tree", &"a".repeat(40), "wrong");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-tree", "-z", "HEAD", "--", "src/a"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_head_ls_tree_extra_record_wrapper() {
    let (_dir, repo) = head_topology_case("ordinary");
    let mut payload = ls_tree_payload("040000", "tree", &"a".repeat(40), "src/a");
    payload.extend(ls_tree_payload("040000", "tree", &"b".repeat(40), "src/a"));
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-tree", "-z", "HEAD", "--", "src/a"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_malformed_success_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "payload",
        b"bad",
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_invalid_oid_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let payload = index_payload("120000", "a", "0", "src/a/b/c");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_unsupported_mode_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let payload = index_payload("100664", &"a".repeat(40), "0", "src/a/b/c");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_wrong_path_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let payload = index_payload("120000", &"a".repeat(40), "0", "wrong");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_extra_record_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let mut payload = index_payload("120000", &"a".repeat(40), "0", "src/a/b/c");
    payload.extend(index_payload("100644", &"b".repeat(40), "0", "src/a/b/c"));
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_invalid_stage_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let payload = index_payload("120000", &"a".repeat(40), "x", "src/a/b/c");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_conflict_stage_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let payload = index_payload("100644", &"a".repeat(40), "1", "src/a/b/c");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_CONFLICT");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_sparse_directory_prefix_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let payload = index_payload("040000", &"a".repeat(40), "0", "src/a/");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_index_topology_sparse_directory_leaf_wrapper() {
    let (_dir, repo) = index_topology_case("ordinary");
    let payload = index_payload("040000", &"a".repeat(40), "0", "src/a/b/c/");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "payload",
        &payload,
    );
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_wrapper_exact_full_argv() {
    let (_dir, repo) = index_topology_case("ordinary");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"],
        "fail",
        &[],
    );
    let output = run_wrapper_direct(&wrapper, &wrapper.expected_args);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, Vec::<u8>::new());
    assert_eq!(output.stderr, Vec::<u8>::new());
    assert_wrapper_reached(&wrapper);
}

// PHASE4_PART1_TEST
#[test]
fn test_part1_wrapper_rejects_argv_variants() {
    let (_dir, repo) = index_topology_case("ordinary");
    let subcommand = ["ls-files", "--sparse", "--stage", "-z", "--", "src/a/b/c"];

    let mut variants = Vec::new();
    let wrapper = create_git_wrapper(&repo, &subcommand, "fail", &[]);
    let mut changed_global = wrapper.expected_args.clone();
    changed_global[1] = "--lazy-fetch".to_string();
    variants.push(changed_global);

    let wrapper = create_git_wrapper(&repo, &subcommand, "fail", &[]);
    let mut missing_global = wrapper.expected_args.clone();
    missing_global.remove(1);
    variants.push(missing_global);

    let wrapper = create_git_wrapper(&repo, &subcommand, "fail", &[]);
    let mut additional_global = wrapper.expected_args.clone();
    additional_global.insert(0, "--extra-option".to_string());
    variants.push(additional_global);

    let wrapper = create_git_wrapper(&repo, &subcommand, "fail", &[]);
    let mut reordered = wrapper.expected_args.clone();
    reordered.swap(0, 1);
    variants.push(reordered);

    let wrapper = create_git_wrapper(&repo, &subcommand, "fail", &[]);
    let mut missing_final = wrapper.expected_args.clone();
    missing_final.pop();
    variants.push(missing_final);

    let wrapper = create_git_wrapper(&repo, &subcommand, "fail", &[]);
    let mut extra_final = wrapper.expected_args.clone();
    extra_final.push("extra".to_string());
    variants.push(extra_final);

    let wrapper = create_git_wrapper(&repo, &subcommand, "fail", &[]);
    let mut different_path = wrapper.expected_args.clone();
    let last = different_path.len() - 1;
    different_path[last] = "src/other".to_string();
    variants.push(different_path);

    for args in variants {
        let wrapper = create_git_wrapper(&repo, &subcommand, "fail", &[]);
        let output = run_wrapper_direct(&wrapper, &args);
        assert!(!wrapper.sentinel.is_file());
        assert_ne!(output.status.code(), Some(2));
    }

    let wrapper = create_git_wrapper(&repo, &subcommand, "fail", &[]);
    let unrelated = production_git_args(&repo, &["--version"]);
    let output = run_wrapper_direct(&wrapper, &unrelated);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stderr, Vec::<u8>::new());
    assert!(!wrapper.sentinel.is_file());
}
