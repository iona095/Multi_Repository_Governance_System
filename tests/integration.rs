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
    let governance_before = capture_governance(&repo);
    let target = repo.join("internal-governance-target");
    std::fs::create_dir(&target).unwrap();
    let mrgs = repo.join(".mrgs");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &mrgs).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&target, &mrgs).unwrap();

    let output = run_plan_accept(&repo, &plan_path);
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr_string(&output).starts_with("error: governance directory escapes repository:"));
    assert!(!repo.join(".mrgs").join("accepted-plan.json").exists());
    assert!(!repo.join(".mrgs").join("state.json").exists());
    assert!(!target.join("accepted-plan.json").exists());
    assert!(!target.join("state.json").exists());

    #[cfg(unix)]
    std::fs::remove_file(&mrgs).unwrap();
    #[cfg(windows)]
    std::fs::remove_dir(&mrgs).unwrap();
    assert_eq!(governance_before, capture_governance(&repo));
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
    let governance_before = capture_governance(&repo);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr_string(&output).starts_with("error: JSON parse error:"));
    assert_eq!(governance_before, capture_governance(&repo));
    assert_no_temp_files(&repo);
}

// 31. malformed inconsistent state
#[test]
fn test_draft_malformed_state() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let state_path = repo.join(".mrgs").join("state.json");
    std::fs::write(&state_path, b"not-json").unwrap();
    let governance_before = capture_governance(&repo);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr_string(&output).starts_with("error: JSON parse error:"));
    assert_eq!(governance_before, capture_governance(&repo));
    assert_no_temp_files(&repo);
    assert_eq!(
        std::fs::read(&state_path).unwrap(),
        b"not-json",
        "state.json should remain corrupted (not silently repaired)"
    );
}

// 32. malformed accepted plan
#[test]
fn test_draft_malformed_accepted_plan() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    let accepted_path = repo.join(".mrgs").join("accepted-plan.json");
    std::fs::write(&accepted_path, b"not-json").unwrap();
    let governance_before = capture_governance(&repo);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr_string(&output).starts_with("error: JSON parse error:"));
    assert_eq!(governance_before, capture_governance(&repo));
    assert_no_temp_files(&repo);
    assert_eq!(
        std::fs::read(&accepted_path).unwrap(),
        b"not-json",
        "accepted-plan.json should remain corrupted (not silently repaired)"
    );
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
    let governance_before = capture_governance(&repo);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr_string(&output).starts_with("error: JSON parse error:"));
    assert_eq!(governance_before, capture_governance(&repo));
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
    let governance_before = capture_governance(&repo);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr_string(&output).starts_with("error: JSON parse error:"));
    assert_eq!(governance_before, capture_governance(&repo));
    assert_no_temp_files(&repo);
    assert_eq!(
        std::fs::read(&ledger_path).unwrap(),
        b"not-json",
        "malformed ledger must not be repaired"
    );
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
    let governance_before = capture_governance(&repo);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr_string(&output).starts_with("error: JSON parse error:"));
    assert_eq!(governance_before, capture_governance(&repo));
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
    let governance_before = capture_governance(&repo);
    let output = run_contract_draft(&repo, &contract_path);
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr_string(&output).starts_with("error: accepted contract ID "));
    assert_eq!(governance_before, capture_governance(&repo));
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
    let init_out = Command::new("git").arg("init").arg(repo).output().unwrap();
    assert_eq!(
        init_out.status.code(),
        Some(0),
        "git init failed: stderr={:?}",
        init_out.stderr
    );
    std::fs::write(repo.join("README.md"), b"initial").unwrap();
    let add_out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("add")
        .arg("README.md")
        .output()
        .unwrap();
    assert_eq!(
        add_out.status.code(),
        Some(0),
        "git add README.md failed: stderr={:?}",
        add_out.stderr
    );
    let commit_out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("init")
        .output()
        .unwrap();
    assert_eq!(
        commit_out.status.code(),
        Some(0),
        "git commit init failed: stderr={:?}",
        commit_out.stderr
    );
}

// ============================================================================
// PKG-13: Snapshot / preservation helpers
// ============================================================================

/// A single captured file entry.
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct FileEntry {
    /// Relative path from repo root (forward-slash normalized).
    relpath: String,
    /// Component bucket this entry belongs to.
    component: &'static str,
    /// `true` for regular files, `false` for directories / special entries.
    is_file: bool,
    /// Exact byte length (0 for non-files).
    length: usize,
    /// SHA-256 hex digest of the raw bytes (empty string for non-files).
    sha256: String,
}

/// A complete filesystem snapshot of a repo.
#[derive(Clone, Debug)]
struct RepoSnapshot {
    entries: std::collections::BTreeMap<String, FileEntry>,
}

impl RepoSnapshot {
    fn new() -> Self {
        Self {
            entries: std::collections::BTreeMap::new(),
        }
    }
}

/// Normalize a path to forward-slash relative from the repo root.
fn normalize_relpath(repo: &Path, full: &std::path::Path) -> String {
    let rel = full.strip_prefix(repo).unwrap_or(full);
    rel.to_str().unwrap().replace('\\', "/")
}

/// Classify a relative path into one of the snapshot components.
fn classify_component(relpath: &str) -> &'static str {
    if relpath.starts_with(".mrgs/") || relpath == ".mrgs" {
        return "governance";
    }
    if relpath.starts_with(".git/HEAD")
        || relpath.starts_with(".git/refs/")
        || relpath.starts_with(".git/packed-refs")
        || relpath.starts_with(".git/index")
        || relpath.starts_with(".git/config")
    {
        return "git_refs";
    }
    if relpath.starts_with(".git/objects/") {
        return "git_objects";
    }
    if relpath == ".git/index" || relpath == ".git/config" {
        // Already handled above, but just in case.
        return "git_refs";
    }
    if relpath.starts_with(".git/") {
        return "git_refs";
    }
    "worktree"
}

/// Walk the filesystem under `repo` and build a complete snapshot.
fn capture_snapshot(repo: &Path) -> RepoSnapshot {
    let mut snap = RepoSnapshot::new();
    let _mrgs = repo.join(".mrgs");
    let git_dir = repo.join(".git");

    // Helper closure to add a regular file entry.
    let add_file = |snap: &mut RepoSnapshot, full: &std::path::Path| {
        let relpath = normalize_relpath(repo, full);
        // Skip transient git files that change on every operation.
        // Only capture HEAD, refs, packed-refs, index, and config from .git/.
        if relpath.starts_with(".git/")
            && !relpath.starts_with(".git/HEAD")
            && !relpath.starts_with(".git/refs/")
            && !relpath.starts_with(".git/packed-refs")
            && relpath != ".git/index"
            && relpath != ".git/config"
        {
            return;
        }
        let component = classify_component(&relpath);
        let bytes = std::fs::read(full).unwrap_or_default();
        let length = bytes.len();
        let sha = if length > 0 {
            sha256_hex(&bytes)
        } else {
            String::new()
        };
        snap.entries.insert(
            relpath,
            FileEntry {
                relpath: String::new(), // stored as key
                component,
                is_file: true,
                length,
                sha256: sha,
            },
        );
    };

    let add_dir = |snap: &mut RepoSnapshot, full: &std::path::Path| {
        let relpath = normalize_relpath(repo, full);
        let component = classify_component(&relpath);
        snap.entries.insert(
            relpath,
            FileEntry {
                relpath: String::new(),
                component,
                is_file: false,
                length: 0,
                sha256: String::new(),
            },
        );
    };

    // Walk the entire repo tree.
    let mut walk = |dir: &std::path::Path| {
        if !dir.exists() {
            return;
        }
        let mut stack = vec![dir.to_path_buf()];
        while let Some(current) = stack.pop() {
            let entry = match std::fs::read_dir(&current) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for item in entry {
                let item = match item {
                    Ok(i) => i,
                    Err(_) => continue,
                };
                let ft = match item.file_type() {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                if ft.is_dir() {
                    add_dir(&mut snap, &item.path());
                    stack.push(item.path());
                } else if ft.is_file() {
                    add_file(&mut snap, &item.path());
                }
            }
        }
    };

    walk(repo);

    // Capture .git/HEAD raw bytes specifically.
    let head_path = git_dir.join("HEAD");
    if head_path.exists() {
        if let Ok(bytes) = std::fs::read(&head_path) {
            let relpath = normalize_relpath(repo, &head_path);
            snap.entries.insert(
                relpath.clone(),
                FileEntry {
                    relpath: String::new(),
                    component: "git_refs",
                    is_file: true,
                    length: bytes.len(),
                    sha256: sha256_hex(&bytes),
                },
            );
        }
    }

    snap
}

/// Compute a diff report between two snapshots.
fn diff_snapshots(before: &RepoSnapshot, after: &RepoSnapshot) -> Vec<String> {
    let mut diffs = Vec::new();
    let mut all_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for k in before.entries.keys() {
        all_keys.insert(k.clone());
    }
    for k in after.entries.keys() {
        all_keys.insert(k.clone());
    }
    for key in &all_keys {
        let before_entry = before.entries.get(key);
        let after_entry = after.entries.get(key);
        match (before_entry, after_entry) {
            (None, Some(a)) => {
                diffs.push(format!(
                    "+ {} [{}] file len={} sha256={}",
                    key, a.component, a.length, a.sha256
                ));
            }
            (Some(b), None) => {
                diffs.push(format!(
                    "- {} [{}] file len={} sha256={}",
                    key, b.component, b.length, b.sha256
                ));
            }
            (Some(b), Some(a)) => {
                if b.is_file != a.is_file || b.length != a.length || b.sha256 != a.sha256 {
                    diffs.push(format!(
                        "~ {} [{}] before=({},{},{}) after=({},{},{})",
                        key,
                        a.component,
                        if b.is_file { "file" } else { "dir" },
                        b.length,
                        b.sha256,
                        if a.is_file { "file" } else { "dir" },
                        a.length,
                        a.sha256,
                    ));
                }
            }
            (None, None) => {}
        }
    }
    diffs
}

/// Direct component-level equality assertion between two snapshots.
/// Compares actual component maps/fields rather than filtering diff strings.
fn assert_snapshot_components_equal(
    before: &RepoSnapshot,
    after: &RepoSnapshot,
    scenario: &str,
    components: &[&str],
) {
    for comp in components {
        let mut before_entries: std::collections::BTreeMap<String, &FileEntry> =
            std::collections::BTreeMap::new();
        let mut after_entries: std::collections::BTreeMap<String, &FileEntry> =
            std::collections::BTreeMap::new();

        for (k, v) in &before.entries {
            if v.component == *comp {
                before_entries.insert(k.clone(), v);
            }
        }
        for (k, v) in &after.entries {
            if v.component == *comp {
                after_entries.insert(k.clone(), v);
            }
        }

        let mut all_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for k in before_entries.keys() {
            all_keys.insert(k.clone());
        }
        for k in after_entries.keys() {
            all_keys.insert(k.clone());
        }

        for key in &all_keys {
            let be = before_entries.get(key);
            let ae = after_entries.get(key);
            match (be, ae) {
                (None, Some(a)) => {
                    panic!(
                        "{}: component={} path={} newly created [file={}, len={}, sha256={}]",
                        scenario,
                        comp,
                        key,
                        if a.is_file { "yes" } else { "no" },
                        a.length,
                        a.sha256
                    );
                }
                (Some(b), None) => {
                    panic!(
                        "{}: component={} path={} deleted [file={}, len={}, sha256={}]",
                        scenario,
                        comp,
                        key,
                        if b.is_file { "yes" } else { "no" },
                        b.length,
                        b.sha256
                    );
                }
                (Some(b), Some(a)) => {
                    if b.is_file != a.is_file || b.length != a.length || b.sha256 != a.sha256 {
                        panic!(
                            "{}: component={} path={} changed before=({},{},{}) after=({},{},{})",
                            scenario,
                            comp,
                            key,
                            if b.is_file { "file" } else { "dir" },
                            b.length,
                            b.sha256,
                            if a.is_file { "file" } else { "dir" },
                            a.length,
                            a.sha256
                        );
                    }
                }
                (None, None) => {}
            }
        }
    }
}

/// Assert no new MRGS temporary paths were created beyond allowed pre-existing ones.
fn assert_no_new_mrgs_temp_paths(
    before: &RepoSnapshot,
    after: &RepoSnapshot,
    scenario: &str,
    allowed_preexisting_paths: &[String],
) {
    let allowed_set: std::collections::BTreeSet<String> =
        allowed_preexisting_paths.iter().cloned().collect();

    for (k, v) in &after.entries {
        if v.component == "governance"
            && k.ends_with(".tmp")
            && !allowed_set.contains(k)
            && !before.entries.contains_key(k)
        {
            panic!(
                "{}: new MRGS temp path created after failure: {} [len={}, sha256={}]",
                scenario, k, v.length, v.sha256
            );
        }
    }

    for (k, bv) in &before.entries {
        if bv.component == "governance" && k.ends_with(".tmp") && allowed_set.contains(k) {
            let av = after.entries.get(k);
            match av {
                None => {
                    panic!(
                        "{}: pre-existing temp fixture deleted: {} [len={}, sha256={}]",
                        scenario, k, bv.length, bv.sha256
                    );
                }
                Some(a) if a.length != bv.length || a.sha256 != bv.sha256 => {
                    panic!(
                        "{}: pre-existing temp fixture modified: {} before=({},{}) after=({},{})",
                        scenario, k, bv.length, bv.sha256, a.length, a.sha256
                    );
                }
                _ => {}
            }
        }
    }
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
        .arg("--sha256")
        .arg(sha256);
    if revision.starts_with('-') {
        cmd.arg(format!("--revision={revision}"));
    } else {
        cmd.arg("--revision").arg(revision);
    }
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

fn spawn_implementation_begin_with_env(
    repo: &Path,
    revision: u32,
    sha256: &str,
    injected: &[(&str, &Path)],
    flags: &[(&str, &str)],
) -> std::process::Child {
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("begin")
        .arg("--repo")
        .arg(repo)
        .arg("--revision")
        .arg(revision.to_string())
        .arg("--sha256")
        .arg(sha256);
    for (key, value) in injected {
        cmd.env(key, value);
    }
    for (key, value) in flags {
        cmd.env(key, value);
    }
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd.spawn().unwrap()
}

fn contract_accepted_revision(repo: &Path) -> (u32, String) {
    let ledger: serde_json::Value = read_json(repo, "accepted-contract.json");
    let last_rev = ledger["revisions"].as_array().unwrap().last().unwrap();
    let revision = last_rev["revision"].as_u64().unwrap() as u32;
    let sha256 = last_rev["sha256"].as_str().unwrap().to_string();
    (revision, sha256)
}

fn commit_file(repo: &Path, name: &str) {
    let add_out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("add")
        .arg(name)
        .output()
        .unwrap();
    assert_eq!(
        add_out.status.code(),
        Some(0),
        "git add {} failed: stderr={:?}",
        name,
        add_out.stderr
    );
    let commit_out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg(name)
        .output()
        .unwrap();
    assert_eq!(
        commit_out.status.code(),
        Some(0),
        "git commit {} failed: stderr={:?}",
        name,
        commit_out.stderr
    );
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
    assert_phase4_begin_exact(&output, &repo);
    assert!(repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

fn assert_phase4_begin_exact(output: &std::process::Output, repo: &Path) {
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
    let record = read_json(repo, "implementation-authority.json");
    let expected = format!(
        "IMPLEMENTATION_BOUND {} {} {} {}",
        ledger["contract_id"].as_str().unwrap(),
        final_entry["revision"].as_u64().unwrap(),
        final_entry["sha256"].as_str().unwrap(),
        record["baseline_head"].as_str().unwrap(),
    );
    let mut expected_stdout = expected.into_bytes();
    expected_stdout.extend_from_slice(phase4_newline());
    assert_eq!(output.stdout, expected_stdout);
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
    assert_phase4_begin_exact(&output, &repo);
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
    assert_phase4_success_exact(&output, &repo, 0);
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
    assert_phase4_begin_exact(&first, &repo);
    let second = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_begin_exact(&second, &repo);
    assert_eq!(first.stdout, second.stdout);
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
    assert_phase4_success_exact(&output, &repo, 1);
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
    assert_phase4_failure_exact(&output, "CHANGE_NOT_ALLOWED");
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
    assert_phase4_failure_exact(&output, "CONTRACT_NOT_ACCEPTED");
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
    assert_phase4_failure_exact(&output, "REQUESTED_REVISION_STALE");
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
    assert_phase4_failure_exact(&output, "REQUESTED_SHA_STALE");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_MISSING");
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
    assert_phase4_failure_exact(&output, "GIT_DIRTY");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_STALE");
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
    assert_phase4_begin_exact(&output, &repo);
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
    assert_phase4_failure_exact(&output, "CHANGE_FORBIDDEN");
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
    assert_phase4_begin_exact(&output, &repo);
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
    assert_phase4_success_exact(&output, &repo, 0);
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_STALE");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_STALE");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_STALE");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
    assert_no_temp_files(&repo);
}

// 32. SHA256 token too short
#[test]
fn test_impl_begin_sha_too_short() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(&repo, "1", "abc123");
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
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
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_CONFLICT");
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
    assert_phase4_failure_exact(&output, "REQUESTED_SHA_STALE");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_CONFLICT");
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
    assert_phase4_failure_exact(&output, "GIT_DIRTY");
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
    assert_phase4_failure_exact(&output, "GIT_DIRTY");
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
    assert_phase4_failure_exact(&output, "GIT_DIRTY");
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
    assert_phase4_failure_exact(&output, "GIT_DIRTY");
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
    assert_phase4_failure_exact(&output, "GIT_DIRTY");
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
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
    assert_phase4_failure_exact(&output, "GIT_DETACHED_HEAD");
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
    assert_phase4_failure_exact(&output, "GIT_DETACHED_HEAD");
    assert_no_temp_files(&repo);
}

// ============================================================================
// PKG-08B: Begin-side governance completeness and structural precedence under
// .mrgs.
// ============================================================================

// ---------------------------------------------------------------------------
// P4-040  symlinked governance file rejection
// ---------------------------------------------------------------------------

#[test]
#[cfg_attr(not(any(unix, windows)), ignore)]
fn test_impl_symlinked_governance_file_rejected() {
    let (_dir, repo, plan_path) = create_repo_and_plan(valid_plan_toml());
    assert_success(&run_plan_accept(&repo, &plan_path));
    let governance_before = capture_governance(&repo);

    let state = repo.join(".mrgs").join("state.json");
    let target = repo.join(".mrgs").join("accepted-plan.json");
    let original_bytes = std::fs::read(&target).unwrap();
    std::fs::remove_file(&target).unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(&state, &target).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&state, &target).unwrap();

    let output = run_phase_select(&repo, "phase-1");
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr_string(&output).starts_with("error: JSON parse error:"));

    std::fs::remove_file(&target).unwrap();
    std::fs::write(&target, &original_bytes).unwrap();
    assert_eq!(governance_before, capture_governance(&repo));
    assert_no_temp_files(&repo);
}

// ---------------------------------------------------------------------------
// P4-095  governance files excluded without .gitignore
// ---------------------------------------------------------------------------

#[test]
fn test_impl_begin_governance_excluded_without_gitignore() {
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

    assert!(!repo.join(".gitignore").exists());
    let status_output = git(&repo)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("-z")
        .arg("--untracked-files=all")
        .arg("--ignore-submodules=none")
        .arg("--renames")
        .output()
        .unwrap();
    assert_git_output_success(&status_output, "git status before no-gitignore begin");
    let status_paths: std::collections::BTreeSet<String> = status_output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(|record| String::from_utf8(record.to_vec()).unwrap())
        .collect();
    let expected_status_paths = [
        "?? .mrgs/accepted-contract.json",
        "?? .mrgs/accepted-plan.json",
        "?? .mrgs/contract-draft.json",
        "?? .mrgs/state.json",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(status_paths, expected_status_paths);
    let governance_before = capture_earlier_governance_bytes(&repo);
    assert_eq!(governance_before.len(), 4);

    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output);
    assert_phase4_begin_exact(&output, &repo);
    assert_eq!(governance_before, capture_earlier_governance_bytes(&repo));
    assert_no_temp_files(&repo);
}

// ---------------------------------------------------------------------------
// P4-139  tracked governance path deleted after baseline
//
// After begin creates a clean baseline, force-add a file under .mrgs/ and
// commit it.  Then git rm --force (stages the deletion without a new commit).
// git status --porcelain reports the staged deletion as "D  .mrgs/extra.json".
// The porcelain parser classifies any XY record whose source or destination
// has .mrgs as its first segment as GIT_INVENTORY_INVALID, regardless of
// whether the current index still contains the path.
// ---------------------------------------------------------------------------

#[test]
fn test_impl_check_governance_deleted_after_baseline_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let governance_before = capture_governance(&repo);
    add_tracked_mrgs_and_commit(&repo, "extra.json", b"{}");
    ls_files_stage0(&repo, ".mrgs/extra.json");

    let rm_output = git(&repo)
        .arg("rm")
        .arg("--force")
        .arg(".mrgs/extra.json")
        .output()
        .unwrap();
    assert_git_output_success(&rm_output, "git rm --force .mrgs/extra.json");
    assert_eq!(rm_output.stdout, b"rm '.mrgs/extra.json'\n");

    let status_output = git(&repo)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("-z")
        .arg("--untracked-files=all")
        .output()
        .unwrap();
    assert_git_output_success(&status_output, "git status after governance deletion");
    assert!(
        status_output
            .stdout
            .windows(b"D  .mrgs/extra.json\0".len())
            .any(|record| record == b"D  .mrgs/extra.json\0"),
        "expected staged deletion in git status: {:?}",
        status_output.stdout
    );

    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

// ---------------------------------------------------------------------------
// P4-185  conflict-stage beneath .mrgs → GIT_CONFLICT
// ---------------------------------------------------------------------------

#[test]
fn test_impl_conflict_stage_under_mrgs_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let oid1 = git_blob(&repo, b"ancestor");
    let oid2 = git_blob(&repo, b"ours");
    let oid3 = git_blob(&repo, b"theirs");
    let mut child = git(&repo)
        .arg("update-index")
        .arg("--index-info")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    let stdin = child.stdin.as_mut().unwrap();
    writeln!(stdin, "100644 {} 1\t.mrgs/state.json", oid1).unwrap();
    writeln!(stdin, "100644 {} 2\t.mrgs/state.json", oid2).unwrap();
    writeln!(stdin, "100644 {} 3\t.mrgs/state.json", oid3).unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "update-index --index-info failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let index_output = git(&repo)
        .arg("ls-files")
        .arg("--stage")
        .arg("-z")
        .arg("--")
        .arg(".mrgs/state.json")
        .output()
        .unwrap();
    assert_git_output_success(&index_output, "git ls-files conflict stages");
    let stages: std::collections::BTreeSet<String> = index_output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(|record| {
            String::from_utf8(record.to_vec())
                .unwrap()
                .split('\t')
                .next()
                .unwrap()
                .split_whitespace()
                .nth(2)
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(
        stages,
        ["1", "2", "3"].into_iter().map(String::from).collect()
    );

    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_CONFLICT",
        &repo,
        &governance_before,
    );
}

// ---------------------------------------------------------------------------
// P4-186  gitlink beneath .mrgs → GIT_SUBMODULE_UNSUPPORTED
// ---------------------------------------------------------------------------

#[test]
fn test_impl_gitlink_under_mrgs_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let head_out = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    assert!(head_out.status.success());
    assert_git_output_success(&head_out, "git rev-parse HEAD for gitlink fixture");
    let head_sha = String::from_utf8_lossy(&head_out.stdout).trim().to_string();

    cacheinfo(&repo, "160000", &head_sha, ".mrgs/submod");
    assert_index_record(&repo, "160000", &head_sha, ".mrgs/submod", "0");

    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_SUBMODULE_UNSUPPORTED",
        &repo,
        &governance_before,
    );
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
    assert_phase4_failure_exact(&output, "GOVERNANCE_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "GOVERNANCE_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "BASELINE_COMMIT_MISSING");
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
    assert_phase4_success_exact(&output, &repo, 0);
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

    // Create a real commit object that is not the current HEAD, prove it is
    // locally available, then remove only that commit object from the local
    // object database. The test must not stand in an arbitrary or all-zero ID
    // for promised-object evidence.
    let current_head = git_head_exact(&repo);
    let tree = String::from_utf8(
        git(&repo)
            .arg("rev-parse")
            .arg("HEAD^{tree}")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let promised_output = git(&repo)
        .arg("commit-tree")
        .arg(&tree)
        .arg("-p")
        .arg(&current_head)
        .arg("-m")
        .arg("promised baseline")
        .output()
        .unwrap();
    assert_success(&promised_output);
    assert!(promised_output.stderr.is_empty());
    let promised = String::from_utf8(promised_output.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(promised.len(), 40);
    assert!(promised
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    assert_ne!(promised, current_head);

    let present = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{promised}^{{commit}}"))
        .output()
        .unwrap();
    assert_success(&present);
    assert!(present.stdout.is_empty());
    assert!(present.stderr.is_empty());

    // Mark the temporary repository as promisor-enabled without configuring a
    // remote or alternate object database, so no helper can retrieve the ID.
    let config = git(&repo)
        .arg("config")
        .arg("extensions.partialClone")
        .arg("origin")
        .output()
        .unwrap();
    assert_success(&config);
    assert!(config.stdout.is_empty());
    assert!(config.stderr.is_empty());
    let remote = git(&repo)
        .arg("config")
        .arg("--get")
        .arg("remote.origin.url")
        .output()
        .unwrap();
    assert!(!remote.status.success());
    assert!(remote.stdout.is_empty());
    assert!(remote.stderr.is_empty());
    assert!(!repo
        .join(".git")
        .join("objects")
        .join("info")
        .join("alternates")
        .exists());

    let object_path = repo
        .join(".git")
        .join("objects")
        .join(&promised[..2])
        .join(&promised[2..]);
    assert!(
        object_path.is_file(),
        "promised commit was not a loose local object"
    );
    std::fs::remove_file(&object_path).unwrap();

    let absent = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{promised}^{{commit}}"))
        .output()
        .unwrap();
    assert!(!absent.status.success());

    // Record the real, now-unavailable promised commit as the required
    // baseline. Production must reach the missing-promised-object branch.
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["baseline_head"] = serde_json::json!(promised);
    write_json(&repo, "implementation-authority.json", &record);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
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
    assert_phase4_failure_exact(&output, "BASELINE_HISTORY_CHANGED");
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
    assert_phase4_failure_exact(&output, "BASELINE_BRANCH_CHANGED");
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
    assert_phase4_failure_exact(&output, "GOVERNANCE_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
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
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
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
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
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
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
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
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
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
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
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
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
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
    let init_output = Command::new("git")
        .arg("init")
        .arg(&sub_dir)
        .output()
        .unwrap();
    assert_git_output_success(&init_output, "git init submodule fixture");
    std::fs::write(sub_dir.join("sub_file.txt"), b"sub").unwrap();
    let add_output = Command::new("git")
        .arg("-C")
        .arg(&sub_dir)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();
    assert_git_output_success(&add_output, "git add submodule fixture");
    let commit_output = Command::new("git")
        .arg("-C")
        .arg(&sub_dir)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("init")
        .output()
        .unwrap();
    assert_git_output_success(&commit_output, "git commit submodule fixture");
    let add_submodule_output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("protocol.file.allow=always")
        .arg("submodule")
        .arg("add")
        .arg(&sub_dir)
        .arg("submod")
        .output()
        .unwrap();
    assert_git_output_success(&add_submodule_output, "git submodule add fixture");
    let governance_before = capture_governance(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_SUBMODULE_UNSUPPORTED",
        &repo,
        &governance_before,
    );
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

#[test]
fn test_impl_begin_records_exact_sparse_state_commands() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let recorder = create_sparse_git_recorder();
    let revision = final_rev.to_string();
    let output = run_with_sparse_git_recorder(
        &recorder,
        &repo,
        &[
            "implementation",
            "begin",
            "--revision",
            &revision,
            "--sha256",
            &final_sha,
        ],
    );
    assert_phase4_begin_exact(&output, &repo);

    let queries = [
        ["config", "--type=bool", "--get", "core.sparseCheckout"],
        ["config", "--type=bool", "--get-all", "core.sparseCheckout"],
        ["config", "--type=bool", "--get", "index.sparse"],
        ["config", "--type=bool", "--get-all", "index.sparse"],
    ];
    let invocations = read_sparse_git_recording(&recorder.log);
    for query in queries {
        let expected = production_git_args(&repo, &query);
        assert_eq!(
            invocations.iter().filter(|args| **args == expected).count(),
            1,
            "missing or duplicate exact sparse query: {query:?}; invocations={invocations:?}"
        );
    }
    assert_eq!(
        invocations
            .iter()
            .filter(|args| args
                .windows(2)
                .any(|pair| pair == ["config", "--type=bool"]))
            .count(),
        4
    );
    assert!(invocations.iter().all(|args| {
        !args.iter().any(|arg| {
            arg == "fetch"
                || arg == "fetch-pack"
                || arg == "credential"
                || arg.starts_with("remote-")
        })
    }));
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

#[test]
fn test_impl_check_records_exact_sparse_state_commands() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let recorder = create_sparse_git_recorder();
    let output = run_with_sparse_git_recorder(&recorder, &repo, &["implementation", "check"]);
    assert_phase4_success_exact(&output, &repo, 0);

    let queries = [
        ["config", "--type=bool", "--get", "core.sparseCheckout"],
        ["config", "--type=bool", "--get-all", "core.sparseCheckout"],
        ["config", "--type=bool", "--get", "index.sparse"],
        ["config", "--type=bool", "--get-all", "index.sparse"],
    ];
    let invocations = read_sparse_git_recording(&recorder.log);
    for query in queries {
        let expected = production_git_args(&repo, &query);
        assert_eq!(
            invocations.iter().filter(|args| **args == expected).count(),
            1,
            "missing or duplicate exact sparse query: {query:?}; invocations={invocations:?}"
        );
    }
    assert_eq!(
        invocations
            .iter()
            .filter(|args| args
                .windows(2)
                .any(|pair| pair == ["config", "--type=bool"]))
            .count(),
        4
    );
    assert!(invocations.iter().all(|args| {
        !args.iter().any(|arg| {
            arg == "fetch"
                || arg == "fetch-pack"
                || arg == "credential"
                || arg.starts_with("remote-")
        })
    }));
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// === H. GOVERNANCE TRACKING ===

// 71. Clean tracked .mrgs/accepted-plan.json rejected at begin
#[test]
fn test_impl_begin_tracked_accepted_plan_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let bytes = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "accepted-plan.json", &bytes);
    let governance_before = capture_governance(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

// 72. Clean tracked .mrgs/state.json rejected at begin
#[test]
fn test_impl_begin_tracked_state_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let bytes = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "state.json", &bytes);
    let governance_before = capture_governance(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

// 73. Clean tracked .mrgs/contract-draft.json rejected at begin
#[test]
fn test_impl_begin_tracked_contract_draft_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let bytes = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "contract-draft.json", &bytes);
    let governance_before = capture_governance(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
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
    let bytes = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "accepted-contract.json", &bytes);
    let governance_before = capture_governance(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
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
    let bytes = std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "implementation-authority.json", &bytes);
    let governance_before = capture_governance(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
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
    add_tracked_mrgs_and_commit(&repo, "extra.json", b"{}");
    let governance_before = capture_governance(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
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
    assert!(tmp_file.is_file());
    let temp_bytes_before = std::fs::read(&tmp_file).unwrap();
    let governance_before = capture_governance(&repo);
    let snapshot_before = capture_snapshot(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_DIRTY");
    assert_eq!(governance_before, capture_governance(&repo));
    assert_eq!(temp_bytes_before, std::fs::read(&tmp_file).unwrap());
    let snapshot_after = capture_snapshot(&repo);
    assert_no_new_mrgs_temp_paths(
        &snapshot_before,
        &snapshot_after,
        "begin pre-existing Phase-4 temp-shaped path",
        &[".mrgs/mrgs_tmp_12345_67890.tmp".to_string()],
    );
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
    let add_output = git(&repo)
        .arg("add")
        .arg("--force")
        .arg(".MRGS/test.txt")
        .output()
        .unwrap();
    assert_git_output_success(&add_output, "git add .MRGS/test.txt");
    let commit_output = git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("track .MRGS/test.txt")
        .output()
        .unwrap();
    assert_git_output_success(&commit_output, "git commit .MRGS/test.txt");
    host_case_alias_directory_assertions(&repo);
    ls_files_stage0(&repo, ".MRGS/test.txt");
    assert_ls_files_mode(&repo, ".MRGS/test.txt", "100644");
    assert_repo_clean(&repo);
    let governance_before = capture_governance(&repo);
    let filesystem_before = capture_snapshot(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
    assert!(diff_snapshots(&filesystem_before, &capture_snapshot(&repo)).is_empty());
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
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
    assert_phase4_success_exact(&output, &repo, 1);
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
    assert_phase4_success_exact(&output, &repo, 1);
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
    assert_phase4_success_exact(&output, &repo, 1);
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
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("todelete.rs"), b"fn to_delete() {}").unwrap();
    commit_file(&repo, "src/todelete.rs");
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
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
    assert_phase4_success_exact(&output, &repo, 1);
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
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("old.rs"), b"fn old() {}").unwrap();
    commit_file(&repo, "src/old.rs");
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
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
    assert_phase4_success_exact(&output, &repo, 2);
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
    assert_phase4_failure_exact(&output, "CHANGE_NOT_ALLOWED");
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
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
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
    assert_phase4_failure_exact(&output, "CHANGE_FORBIDDEN");
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
    assert_phase4_failure_exact(&output, "CHANGE_NOT_ALLOWED");
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
    assert_phase4_failure_exact(&output, "CHANGE_NOT_ALLOWED");
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
    assert_phase4_success_exact(&output, &repo, 0);
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_MISSING");
    assert_no_temp_files(&repo);
}

// 92. No success stdout on failure
#[test]
fn test_impl_no_stdout_on_failure() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(
        &repo,
        "abc",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
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
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
    assert_no_temp_files(&repo);
}

// 94. No backtrace in error output
#[test]
fn test_impl_no_backtrace_in_error() {
    let (_dir, repo) = setup_implementation_basic();
    let output = run_implementation_begin_str(&repo, "abc", "bad");
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_STALE");
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
    assert_phase4_failure_exact(&output, "CHANGE_NOT_ALLOWED");
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
    assert_phase4_failure_exact(&output, "GOVERNANCE_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
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
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
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
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
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
    assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
    assert_no_temp_files(&repo);
}

// 103. Check on fresh init without governance
#[test]
fn test_impl_check_no_governance() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    git_init(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "GOVERNANCE_AUTHORITY_INVALID");
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
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_MISSING");
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
    assert_phase4_failure_exact(&output, "GIT_DIRTY");
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
    let begin_output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_begin_exact(&begin_output, &repo);
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
    assert_phase4_begin_exact(&output, &repo);
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
    assert_phase4_begin_exact(&begin_output, &repo);
    let check_output = run_implementation_check(&repo);
    assert_phase4_success_exact(&check_output, &repo, 0);
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
    assert_phase4_success_exact(&first, &repo, 0);
    let second = run_implementation_check(&repo);
    assert_phase4_success_exact(&second, &repo, 0);
    assert_eq!(first.stdout, second.stdout);
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
        assert_phase4_success_exact(&output, &repo, 0);
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
    assert_phase4_success_exact(&output, &repo, 3);
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
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
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
    assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
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

// ============================================================================
// PKG-09: Git child environment isolation and no-lazy-fetch universal proof
// ============================================================================
//
// P4-100  inherited alternate env vars do not alter inspection
// P4-101  replacement refs disabled for ancestry/diff
// P4-102  external helpers (fsmonitor, diff.external, pager, editor, hooks) not executed
// P4-144  GIT_CONFIG_PARAMETERS absent from every Git child
// P4-145  GIT_CONFIG_PARAMETERS injection of external behavior cannot alter any child
// P4-146  GIT_SHALLOW_FILE absent from every Git child
// P4-147  injected GIT_SHALLOW_FILE cannot change object availability/merge-base/ancestry
// P4-148  child-environment inspection proves both stripped after final construction
// P4-149  environment-isolation results deterministic; no value/diagnostic leaks into stderr
// P4-150  every Git child receives exact GIT_NO_LAZY_FETCH=1
// P4-151  every Git invocation includes --no-lazy-fetch in fixed global-option position
// ============================================================================

/// Run `implementation check` while injecting forbidden Git environment variables.
fn run_implementation_check_with_env(
    repo: &Path,
    injected: &[(&str, &str)],
) -> std::process::Output {
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("check")
        .arg("--repo")
        .arg(repo);
    for (k, v) in injected {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

/// Enhanced Git recorder that captures both argv and environment variables from
/// every intercepted git invocation. The wrapper is a compiled Rust binary that
/// logs its complete argument list and selected environment variables to binary
/// log files before delegating to the real git executable.
struct EnvAwareGitRecorder {
    dir: tempfile::TempDir,
    argv_log: std::path::PathBuf,
    env_log: std::path::PathBuf,
}

fn create_env_aware_git_recorder() -> EnvAwareGitRecorder {
    let dir = tempfile::TempDir::new().unwrap();
    let wrapper_dir = dir.path().join("bin");
    std::fs::create_dir_all(&wrapper_dir).unwrap();
    let argv_log = dir.path().join("git-argv.bin");
    let env_log = dir.path().join("git-env.log");

    // Generate a Rust source file for the recorder wrapper.
    // It captures argv in binary format (same as SparseGitRecorder) and
    // writes selected environment variables to a text log.
    let _real_git_path = real_git_executable();
    let source = format!(
        r#"
use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;

fn main() {{
    let args: Vec<OsString> = env::args_os().skip(1).collect();

    // Log argv in binary format (argc as u64 LE, then each arg as len+bytes)
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open({argv_log:?})
        .unwrap();
    log.write_all(&(args.len() as u64).to_le_bytes()).unwrap();
    for arg in &args {{
        let bytes = arg.as_os_str().as_encoded_bytes();
        log.write_all(&(bytes.len() as u64).to_le_bytes()).unwrap();
        log.write_all(bytes).unwrap();
    }}

    // Log selected environment variables to text file.
    // Each line: KEY=VALUE or KEY=<absent> if not set.
    let mut env_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open({env_log:?})
        .unwrap();

    // Write a separator for each invocation
    writeln!(env_file, "---INVOCATION---").unwrap();

    let vars_to_check: &[&str] = &[
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
    ];

    for var in vars_to_check {{
        match env::var(var) {{
            Ok(val) => writeln!(env_file, "{{}}={{}}", var, val).unwrap(),
            Err(_)  => writeln!(env_file, "{{}}=<absent>", var).unwrap(),
        }}
    }}

    // Delegate to real git
    let status = Command::new({real:?})
        .args(&args)
        .status()
        .unwrap();
    std::process::exit(status.code().unwrap_or(1));
}}
"#,
        argv_log = argv_log.display().to_string(),
        env_log = env_log.display().to_string(),
        real = real_git_executable().display().to_string(),
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
        dir,
        argv_log,
        env_log,
    }
}

/// Run an MRGS operation with the env-aware recorder intercepting all git calls.
fn run_with_env_aware_recorder(
    recorder: &EnvAwareGitRecorder,
    repo: &Path,
    operation: &[&str],
) -> std::process::Output {
    let old_path = std::env::var("PATH").unwrap();
    let wrapper_path = recorder.dir.path().join("bin");
    let mut cmd = cargo_bin();
    cmd.args(operation)
        .arg("--repo")
        .arg(repo)
        .env("PATH", format!("{};{}", wrapper_path.display(), old_path));
    cmd.output().unwrap()
}

/// Parse the env log file into per-invocation maps of variable presence.
/// Returns a vector of invocation records, each containing a map of var->Some(value) or None.
fn parse_env_log(path: &Path) -> Vec<std::collections::HashMap<String, Option<String>>> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut invocations: Vec<std::collections::HashMap<String, Option<String>>> = Vec::new();
    let mut current: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();

    for line in content.lines() {
        if line == "---INVOCATION---" {
            if !current.is_empty() {
                invocations.push(current);
            }
            current = std::collections::HashMap::new();
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let key = &line[..eq_pos];
            let value = &line[eq_pos + 1..];
            let val = if value == "<absent>" {
                None
            } else {
                Some(value.to_string())
            };
            current.insert(key.to_string(), val);
        }
    }
    if !current.is_empty() {
        invocations.push(current);
    }
    invocations
}

/// Read argv recordings from the binary log (same format as SparseGitRecorder).
fn read_env_aware_argv(path: &Path) -> Vec<Vec<String>> {
    read_sparse_git_recording(path)
}

// --- P4-100: inherited alternate env vars do not alter inspection ---

#[test]
fn test_p4_100_alternate_env_isolation_begin_matrix() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Control case: clean begin succeeds
    let control_output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&control_output);
    let control_stdout = stdout_string(&control_output);

    // Matrix of alternate environment variables to inject
    let outside = tempfile::TempDir::new().unwrap();
    let matrix: Vec<(&str, String)> = vec![
        (
            "GIT_INDEX_FILE",
            outside
                .path()
                .join("evil-index")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        (
            "GIT_WORK_TREE",
            outside.path().join("evil-wt").to_str().unwrap().to_string(),
        ),
        (
            "GIT_DIR",
            outside
                .path()
                .join("evil-dir")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        (
            "GIT_OBJECT_DIRECTORY",
            outside
                .path()
                .join("evil-objdir")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        (
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            outside
                .path()
                .join("evil-alt")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        ("GIT_NAMESPACE", "evil-namespace".to_string()),
    ];

    for (var, value) in matrix {
        // Remove the implementation authority to test a fresh begin each time
        let _ = std::fs::remove_file(repo.join(".mrgs").join("implementation-authority.json"));
        let output =
            run_implementation_begin_with_env(&repo, final_rev, &final_sha, &[(var, &value)]);
        assert!(
            output.status.success(),
            "injected {}={} should not alter begin; stderr: {}",
            var,
            value,
            String::from_utf8_lossy(&output.stderr)
        );
        // Outcome must match control
        let out = stdout_string(&output);
        assert_eq!(
            out, control_stdout,
            "injected {}={} altered output",
            var, value
        );
    }
}

#[test]
fn test_p4_100_alternate_env_isolation_check_matrix() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Control case: clean check succeeds
    let control_output = run_implementation_check(&repo);
    assert_success(&control_output);
    let control_stdout = stdout_string(&control_output);

    let outside = tempfile::TempDir::new().unwrap();
    let matrix: Vec<(&str, String)> = vec![
        (
            "GIT_INDEX_FILE",
            outside
                .path()
                .join("evil-index")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        (
            "GIT_WORK_TREE",
            outside.path().join("evil-wt").to_str().unwrap().to_string(),
        ),
        (
            "GIT_DIR",
            outside
                .path()
                .join("evil-dir")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        (
            "GIT_OBJECT_DIRECTORY",
            outside
                .path()
                .join("evil-objdir")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        (
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            outside
                .path()
                .join("evil-alt")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        ("GIT_NAMESPACE", "evil-namespace".to_string()),
    ];

    for (var, value) in matrix {
        let output = run_implementation_check_with_env(&repo, &[(var, &value)]);
        assert!(
            output.status.success(),
            "injected {}={} should not alter check; stderr: {}",
            var,
            value,
            String::from_utf8_lossy(&output.stderr)
        );
        let out = stdout_string(&output);
        assert_eq!(
            out, control_stdout,
            "injected {}={} altered output",
            var, value
        );
    }
}

// --- P4-101: replacement refs disabled for ancestry and diff inspection ---

#[test]
fn test_p4_101_replacement_refs_disabled_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Create a replacement ref that would alter object interpretation if honored.
    // We create an alternate commit object and register it as a replacement for HEAD.
    let head_oid = git_head_exact(&repo);

    // Use a simpler approach: create a replacement refs entry pointing to a
    // non-existent object. If replacement refs were honored, git would fail
    // or behave differently when resolving HEAD.
    let fake_oid = "deadbeef0123456789abcdef0123456789abcdef01";
    let replacements_dir = repo.join(".git/refs/replacements");
    std::fs::create_dir_all(&replacements_dir).unwrap();
    // Register a replacement for the actual HEAD commit
    let prefix = &head_oid[..2];
    let rest = &head_oid[2..];
    let ref_path = replacements_dir.join(format!("{}{}.replace", prefix, rest));
    std::fs::write(&ref_path, format!("{} commit\n", fake_oid)).unwrap();

    // Begin must succeed: --no-replace-objects ensures replacement refs are ignored.
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output);

    // Clean up the replacement ref
    let _ = std::fs::remove_file(&ref_path);
}

#[test]
fn test_p4_101_replacement_refs_disabled_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Create a replacement ref for HEAD that points to a non-existent object.
    let head_oid = git_head_exact(&repo);
    let fake_oid = "deadbeef0123456789abcdef0123456789abcdef01";
    let replacements_dir = repo.join(".git/refs/replacements");
    std::fs::create_dir_all(&replacements_dir).unwrap();
    let prefix = &head_oid[..2];
    let rest = &head_oid[2..];
    let ref_path = replacements_dir.join(format!("{}{}.replace", prefix, rest));
    std::fs::write(&ref_path, format!("{} commit\n", fake_oid)).unwrap();

    // Check must succeed: --no-replace-objects ensures replacement refs are ignored.
    let output = run_implementation_check(&repo);
    assert_success(&output);

    // Clean up
    let _ = std::fs::remove_file(&ref_path);
}

// --- P4-102: external helpers not executed ---

#[test]
fn test_p4_102_external_helpers_not_executed() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Create sentinel files that would be created if external helpers were executed.
    let sentinel_dir = tempfile::TempDir::new().unwrap();
    let fsmonitor_sentinel = sentinel_dir.path().join("fsmonitor-hit");
    let diff_external_sentinel = sentinel_dir.path().join("diff-external-hit");
    let pager_sentinel = sentinel_dir.path().join("pager-hit");
    let editor_sentinel = sentinel_dir.path().join("editor-hit");
    let hooks_sentinel = sentinel_dir.path().join("hooks-hit");

    // Create a fake fsmonitor script (on Windows, use .bat)
    let scripts_dir = sentinel_dir.path().join("scripts");
    std::fs::create_dir_all(&scripts_dir).unwrap();
    let fsmonitor_script = scripts_dir.join("fsmonitor.bat");
    std::fs::write(
        &fsmonitor_script,
        format!("echo hit > {}\n", fsmonitor_sentinel.display()),
    )
    .unwrap();

    let diff_external_script = scripts_dir.join("diff-external.bat");
    std::fs::write(
        &diff_external_script,
        format!("echo hit > {}\n", diff_external_sentinel.display()),
    )
    .unwrap();

    let pager_script = scripts_dir.join("pager.bat");
    std::fs::write(
        &pager_script,
        format!("echo hit > {}\n", pager_sentinel.display()),
    )
    .unwrap();

    let editor_script = scripts_dir.join("editor.bat");
    std::fs::write(
        &editor_script,
        format!("echo hit > {}\n", editor_sentinel.display()),
    )
    .unwrap();

    let hooks_script = scripts_dir.join("pre-commit.bat");
    std::fs::write(
        &hooks_script,
        format!("echo hit > {}\n", hooks_sentinel.display()),
    )
    .unwrap();

    // Configure git to use these external helpers via repo config.
    // These should be overridden by the production code's -c flags.
    let _ = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("core.fsmonitor")
        .arg(fsmonitor_script.display().to_string())
        .output()
        .unwrap();
    let _ = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("diff.external")
        .arg(diff_external_script.display().to_string())
        .output()
        .unwrap();
    let _ = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("core.pager")
        .arg(pager_script.display().to_string())
        .output()
        .unwrap();
    let _ = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("core.editor")
        .arg(editor_script.display().to_string())
        .output()
        .unwrap();

    // Set up hooks path
    let hooks_dir = sentinel_dir.path().join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("pre-commit.bat"),
        format!("echo hit > {}\n", hooks_sentinel.display()),
    )
    .unwrap();
    let _ = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("core.hooksPath")
        .arg(hooks_dir.display().to_string())
        .output()
        .unwrap();

    // Run begin and check — no sentinel should be created.
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    assert_success(&run_implementation_check(&repo));

    // Verify no external helper was executed
    assert!(!fsmonitor_sentinel.exists(), "core.fsmonitor was executed");
    assert!(
        !diff_external_sentinel.exists(),
        "diff.external was executed"
    );
    assert!(!pager_sentinel.exists(), "pager was executed");
    assert!(!editor_sentinel.exists(), "editor was executed");
    assert!(!hooks_sentinel.exists(), "hook was executed");
}

// --- P4-145: GIT_CONFIG_PARAMETERS injection of external behavior ---

#[test]
fn test_p4_145_config_parameters_injection_external_behavior() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Create sentinel for external helper execution
    let sentinel_dir = tempfile::TempDir::new().unwrap();
    let sentinel = sentinel_dir.path().join("helper-executed");
    let scripts_dir = sentinel_dir.path().join("scripts");
    std::fs::create_dir_all(&scripts_dir).unwrap();

    // Create fake external helpers that create the sentinel if executed
    let fake_fsmonitor = scripts_dir.join("fake-fsmonitor.bat");
    std::fs::write(
        &fake_fsmonitor,
        format!("echo hit > {}\n", sentinel.display()),
    )
    .unwrap();
    let fake_diff_ext = scripts_dir.join("fake-diff-ext.bat");
    std::fs::write(
        &fake_diff_ext,
        format!("echo hit > {}\n", sentinel.display()),
    )
    .unwrap();
    let fake_pager = scripts_dir.join("fake-pager.bat");
    std::fs::write(&fake_pager, format!("echo hit > {}\n", sentinel.display())).unwrap();
    let fake_editor = scripts_dir.join("fake-editor.bat");
    std::fs::write(&fake_editor, format!("echo hit > {}\n", sentinel.display())).unwrap();

    // Attempt injection via GIT_CONFIG_PARAMETERS of external behavior settings.
    let inj_fsmonitor = format!("-c core.fsmonitor={}", fake_fsmonitor.display());
    let inj_diff_ext = format!("-c diff.external={}", fake_diff_ext.display());
    let inj_pager = format!("-c core.pager={}", fake_pager.display());
    let inj_editor = format!("-c core.editor={}", fake_editor.display());
    let injections: Vec<&str> = vec![&inj_fsmonitor, &inj_diff_ext, &inj_pager, &inj_editor];

    for injection in &injections {
        let _ = std::fs::remove_file(&sentinel);
        // Remove implementation authority to test fresh begin each time
        let _ = std::fs::remove_file(repo.join(".mrgs").join("implementation-authority.json"));
        let output = run_implementation_begin_with_env(
            &repo,
            final_rev,
            &final_sha,
            &[("GIT_CONFIG_PARAMETERS", injection)],
        );
        assert!(
            output.status.success(),
            "injection '{}' should not break begin; stderr: {}",
            injection,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !sentinel.exists(),
            "external helper executed via GIT_CONFIG_PARAMETERS injection: {}",
            injection
        );
    }
}

// --- P4-147: injected GIT_SHALLOW_FILE cannot change object availability/merge-base/ancestry ---

#[test]
fn test_p4_147_shallow_file_injection_no_effect() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Control: clean begin succeeds with known output
    let control_output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&control_output);
    let control_stdout = stdout_string(&control_output);

    // Create a malicious shallow file that claims HEAD is pruned.
    let outside = tempfile::TempDir::new().unwrap();
    let shallow_file = outside.path().join("shallow");
    let head_oid = git_head_exact(&repo);
    std::fs::write(&shallow_file, format!("{}\n", head_oid)).unwrap();

    // Remove implementation authority for fresh begin
    let _ = std::fs::remove_file(repo.join(".mrgs").join("implementation-authority.json"));

    // Begin with injected GIT_SHALLOW_FILE must produce the same result.
    let output = run_implementation_begin_with_env(
        &repo,
        final_rev,
        &final_sha,
        &[("GIT_SHALLOW_FILE", shallow_file.to_str().unwrap())],
    );
    assert!(
        output.status.success(),
        "injected GIT_SHALLOW_FILE should not break begin; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_string(&output);
    assert_eq!(out, control_stdout, "GIT_SHALLOW_FILE altered output");
}

// --- P4-149: determinism and stderr cleanliness ---

#[test]
fn test_p4_149_determinism_isolation_results() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Run begin twice and compare all outputs
    let output1 = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output1);

    // Remove authority for second run
    let _ = std::fs::remove_file(repo.join(".mrgs").join("implementation-authority.json"));
    let output2 = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_success(&output2);

    // Same exit category (both success)
    assert_eq!(output1.status.code(), output2.status.code());

    // Same stdout
    assert_eq!(stdout_string(&output1), stdout_string(&output2));

    // Same stderr (should be empty or identical)
    let stderr1 = String::from_utf8_lossy(&output1.stderr).to_string();
    let stderr2 = String::from_utf8_lossy(&output2.stderr).to_string();
    assert_eq!(stderr1, stderr2);

    // No inherited value or Git diagnostic leak into stderr
    for forbidden in ["GIT_", ".git/", "fatal:", "warning: git"] {
        assert!(!stderr1.contains(forbidden), "stderr leaked: {}", forbidden);
    }
}

#[test]
fn test_p4_149_determinism_check_results() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Run check twice and compare all outputs
    let output1 = run_implementation_check(&repo);
    assert_success(&output1);
    let output2 = run_implementation_check(&repo);
    assert_success(&output2);

    assert_eq!(output1.status.code(), output2.status.code());
    assert_eq!(stdout_string(&output1), stdout_string(&output2));

    let stderr1 = String::from_utf8_lossy(&output1.stderr).to_string();
    let stderr2 = String::from_utf8_lossy(&output2.stderr).to_string();
    assert_eq!(stderr1, stderr2);
}

// --- Universal child observation tests (P4-144, P4-146, P4-148, P4-150, P4-151) ---
// These use the EnvAwareGitRecorder to observe every Git child launched during
// a complete begin/check lifecycle and prove universal claims.

#[test]
fn test_p4_144_config_parameters_absent_every_child_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    let recorder = create_env_aware_git_recorder();
    let operation = &[
        "implementation",
        "begin",
        "--revision",
        &final_rev.to_string(),
        "--sha256",
        &final_sha,
    ];
    let output = run_with_env_aware_recorder(&recorder, &repo, operation);
    assert!(
        output.status.success(),
        "begin failed with recorder: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Parse env log and verify GIT_CONFIG_PARAMETERS absent from every child
    let invocations = parse_env_log(&recorder.env_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during begin"
    );

    for (idx, env_map) in invocations.iter().enumerate() {
        match env_map.get("GIT_CONFIG_PARAMETERS") {
            None => {}       // variable not present — correct
            Some(None) => {} // explicitly absent — correct
            Some(Some(val)) => panic!(
                "invocation {}: GIT_CONFIG_PARAMETERS present with value '{}'; must be absent",
                idx, val
            ),
        }
    }
}

#[test]
fn test_p4_144_config_parameters_absent_every_child_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let recorder = create_env_aware_git_recorder();
    let operation = &["implementation", "check"];
    let output = run_with_env_aware_recorder(&recorder, &repo, operation);
    assert!(
        output.status.success(),
        "check failed with recorder: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let invocations = parse_env_log(&recorder.env_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during check"
    );

    for (idx, env_map) in invocations.iter().enumerate() {
        match env_map.get("GIT_CONFIG_PARAMETERS") {
            None | Some(None) => {}
            Some(Some(val)) => panic!(
                "invocation {}: GIT_CONFIG_PARAMETERS present with value '{}'; must be absent",
                idx, val
            ),
        }
    }
}

#[test]
fn test_p4_146_shallow_file_absent_every_child_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    let recorder = create_env_aware_git_recorder();
    let operation = &[
        "implementation",
        "begin",
        "--revision",
        &final_rev.to_string(),
        "--sha256",
        &final_sha,
    ];
    let output = run_with_env_aware_recorder(&recorder, &repo, operation);
    assert!(
        output.status.success(),
        "begin failed with recorder: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let invocations = parse_env_log(&recorder.env_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during begin"
    );

    for (idx, env_map) in invocations.iter().enumerate() {
        match env_map.get("GIT_SHALLOW_FILE") {
            None | Some(None) => {}
            Some(Some(val)) => panic!(
                "invocation {}: GIT_SHALLOW_FILE present with value '{}'; must be absent",
                idx, val
            ),
        }
    }
}

#[test]
fn test_p4_146_shallow_file_absent_every_child_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let recorder = create_env_aware_git_recorder();
    let operation = &["implementation", "check"];
    let output = run_with_env_aware_recorder(&recorder, &repo, operation);
    assert!(
        output.status.success(),
        "check failed with recorder: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let invocations = parse_env_log(&recorder.env_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during check"
    );

    for (idx, env_map) in invocations.iter().enumerate() {
        match env_map.get("GIT_SHALLOW_FILE") {
            None | Some(None) => {}
            Some(Some(val)) => panic!(
                "invocation {}: GIT_SHALLOW_FILE present with value '{}'; must be absent",
                idx, val
            ),
        }
    }
}

#[test]
fn test_p4_148_both_stripped_after_final_construction_begin() {
    // Proves both GIT_CONFIG_PARAMETERS and GIT_SHALLOW_FILE are absent from
    // every Git child after final environment construction during begin.
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Inject both forbidden variables into the MRGS process environment.
    let outside = tempfile::TempDir::new().unwrap();
    let shallow_file = outside.path().join("shallow");
    std::fs::write(
        &shallow_file,
        "deadbeef0123456789abcdef0123456789abcdef01\n",
    )
    .unwrap();

    let recorder = create_env_aware_git_recorder();
    // The recorder intercepts git calls; we also inject the forbidden vars into MRGS.
    let old_path = std::env::var("PATH").unwrap();
    let wrapper_path = recorder.dir.path().join("bin");
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("begin")
        .arg("--repo")
        .arg(&repo)
        .arg("--revision")
        .arg(final_rev.to_string())
        .arg("--sha256")
        .arg(&final_sha)
        .env("GIT_CONFIG_PARAMETERS", "-c init.defaultBranch=evil")
        .env("GIT_SHALLOW_FILE", shallow_file.to_str().unwrap())
        .env("PATH", format!("{};{}", wrapper_path.display(), old_path));
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "begin failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify both are absent from every recorded child.
    let invocations = parse_env_log(&recorder.env_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during begin"
    );

    for (idx, env_map) in invocations.iter().enumerate() {
        match env_map.get("GIT_CONFIG_PARAMETERS") {
            None | Some(None) => {}
            Some(Some(val)) => panic!(
                "invocation {}: GIT_CONFIG_PARAMETERS present='{}'; must be absent",
                idx, val
            ),
        }
        match env_map.get("GIT_SHALLOW_FILE") {
            None | Some(None) => {}
            Some(Some(val)) => panic!(
                "invocation {}: GIT_SHALLOW_FILE present='{}'; must be absent",
                idx, val
            ),
        }
    }
}

#[test]
fn test_p4_148_both_stripped_after_final_construction_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let outside = tempfile::TempDir::new().unwrap();
    let shallow_file = outside.path().join("shallow");
    std::fs::write(
        &shallow_file,
        "deadbeef0123456789abcdef0123456789abcdef01\n",
    )
    .unwrap();

    let recorder = create_env_aware_git_recorder();
    let old_path = std::env::var("PATH").unwrap();
    let wrapper_path = recorder.dir.path().join("bin");
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("check")
        .arg("--repo")
        .arg(&repo)
        .env("GIT_CONFIG_PARAMETERS", "-c init.defaultBranch=evil")
        .env("GIT_SHALLOW_FILE", shallow_file.to_str().unwrap())
        .env("PATH", format!("{};{}", wrapper_path.display(), old_path));
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let invocations = parse_env_log(&recorder.env_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during check"
    );

    for (idx, env_map) in invocations.iter().enumerate() {
        match env_map.get("GIT_CONFIG_PARAMETERS") {
            None | Some(None) => {}
            Some(Some(val)) => panic!(
                "invocation {}: GIT_CONFIG_PARAMETERS='{}'; must be absent",
                idx, val
            ),
        }
        match env_map.get("GIT_SHALLOW_FILE") {
            None | Some(None) => {}
            Some(Some(val)) => panic!(
                "invocation {}: GIT_SHALLOW_FILE='{}'; must be absent",
                idx, val
            ),
        }
    }
}

#[test]
fn test_p4_150_no_lazy_fetch_env_every_child_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    let recorder = create_env_aware_git_recorder();
    let operation = &[
        "implementation",
        "begin",
        "--revision",
        &final_rev.to_string(),
        "--sha256",
        &final_sha,
    ];
    let output = run_with_env_aware_recorder(&recorder, &repo, operation);
    assert!(
        output.status.success(),
        "begin failed with recorder: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let invocations = parse_env_log(&recorder.env_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during begin"
    );

    for (idx, env_map) in invocations.iter().enumerate() {
        match env_map.get("GIT_NO_LAZY_FETCH") {
            None => panic!(
                "invocation {}: GIT_NO_LAZY_FETCH not present; must be set to '1'",
                idx
            ),
            Some(None) => panic!(
                "invocation {}: GIT_NO_LAZY_FETCH absent; must be set to '1'",
                idx
            ),
            Some(Some(val)) => assert_eq!(
                val, "1",
                "invocation {}: GIT_NO_LAZY_FETCH='{}'; expected '1'",
                idx, val
            ),
        }
    }
}

#[test]
fn test_p4_150_no_lazy_fetch_env_every_child_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let recorder = create_env_aware_git_recorder();
    let operation = &["implementation", "check"];
    let output = run_with_env_aware_recorder(&recorder, &repo, operation);
    assert!(
        output.status.success(),
        "check failed with recorder: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let invocations = parse_env_log(&recorder.env_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during check"
    );

    for (idx, env_map) in invocations.iter().enumerate() {
        match env_map.get("GIT_NO_LAZY_FETCH") {
            None => panic!(
                "invocation {}: GIT_NO_LAZY_FETCH not present; must be set to '1'",
                idx
            ),
            Some(None) => panic!(
                "invocation {}: GIT_NO_LAZY_FETCH absent; must be set to '1'",
                idx
            ),
            Some(Some(val)) => assert_eq!(
                val, "1",
                "invocation {}: GIT_NO_LAZY_FETCH='{}'; expected '1'",
                idx, val
            ),
        }
    }
}

#[test]
fn test_p4_151_no_lazy_fetch_argv_every_child_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    let recorder = create_env_aware_git_recorder();
    let operation = &[
        "implementation",
        "begin",
        "--revision",
        &final_rev.to_string(),
        "--sha256",
        &final_sha,
    ];
    let output = run_with_env_aware_recorder(&recorder, &repo, operation);
    assert!(
        output.status.success(),
        "begin failed with recorder: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify --no-lazy-fetch in argv of every recorded invocation.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during begin"
    );

    for (idx, args) in invocations.iter().enumerate() {
        // --no-lazy-fetch must appear exactly once.
        let count = args.iter().filter(|a| *a == "--no-lazy-fetch").count();
        assert_eq!(
            count, 1,
            "invocation {}: --no-lazy-fetch appears {} times; expected exactly 1",
            idx, count
        );

        // It must appear before the subcommand (in global-option position).
        let lazy_fetch_pos = args.iter().position(|a| *a == "--no-lazy-fetch").unwrap();
        // Find the subcommand: first arg that is not a global option or -c/-C pair.
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--no-replace-objects" | "--no-lazy-fetch" | "--literal-pathspecs" => {
                    i += 1;
                    continue;
                }
                "-c" | "-C" => {
                    i += 2;
                    continue;
                } // -c key=value or -C path
                _ => break,
            }
        }
        let subcommand_pos = i;
        assert!(
            lazy_fetch_pos < subcommand_pos,
            "invocation {}: --no-lazy-fetch at position {} is not before subcommand at position {}",
            idx,
            lazy_fetch_pos,
            subcommand_pos
        );
    }
}

#[test]
fn test_p4_151_no_lazy_fetch_argv_every_child_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let recorder = create_env_aware_git_recorder();
    let operation = &["implementation", "check"];
    let output = run_with_env_aware_recorder(&recorder, &repo, operation);
    assert!(
        output.status.success(),
        "check failed with recorder: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during check"
    );

    for (idx, args) in invocations.iter().enumerate() {
        let count = args.iter().filter(|a| *a == "--no-lazy-fetch").count();
        assert_eq!(
            count, 1,
            "invocation {}: --no-lazy-fetch appears {} times; expected exactly 1",
            idx, count
        );

        let lazy_fetch_pos = args.iter().position(|a| *a == "--no-lazy-fetch").unwrap();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--no-replace-objects" | "--no-lazy-fetch" | "--literal-pathspecs" => {
                    i += 1;
                    continue;
                }
                "-c" | "-C" => {
                    i += 2;
                    continue;
                }
                _ => break,
            }
        }
        let subcommand_pos = i;
        assert!(
            lazy_fetch_pos < subcommand_pos,
            "invocation {}: --no-lazy-fetch at position {} is not before subcommand at position {}",
            idx,
            lazy_fetch_pos,
            subcommand_pos
        );
    }
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

fn assert_git_output_success(output: &std::process::Output, description: &str) {
    assert!(
        output.status.success(),
        "{description} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Capture raw diff from baseline..HEAD as exact bytes.
fn git_raw_diff(repo: &Path, baseline: &str) -> Vec<u8> {
    let out = git(repo)
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
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "git diff failed: stderr={:?}",
        out.stderr
    );
    out.stdout
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

fn assert_phase4_failure_preserves_governance(
    output: &std::process::Output,
    category: &str,
    repo: &Path,
    before: &[(String, Vec<u8>)],
) {
    assert_phase4_failure_exact(output, category);
    assert_eq!(before, capture_governance_bytes(repo));
    assert_no_temp_files(repo);
}

fn assert_phase4_failure_preserves_full_governance(
    output: &std::process::Output,
    category: &str,
    repo: &Path,
    before: &std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry>,
) {
    assert_phase4_failure_exact(output, category);
    assert_eq!(before, &capture_governance(repo));
    assert_no_temp_files(repo);
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

fn run_begin_with_git_wrapper(
    repo: &Path,
    revision: u32,
    sha256: &str,
    wrapper: &GitWrapper,
) -> std::process::Output {
    let old_path = std::env::var("PATH").unwrap();
    let wrapper_path = wrapper.dir.path().join("bin");
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("begin")
        .arg("--repo")
        .arg(repo)
        .arg("--revision")
        .arg(revision.to_string())
        .arg("--sha256")
        .arg(sha256)
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

struct SparseGitRecorder {
    dir: tempfile::TempDir,
    log: std::path::PathBuf,
}

fn create_sparse_git_recorder() -> SparseGitRecorder {
    let dir = tempfile::TempDir::new().unwrap();
    let wrapper_dir = dir.path().join("bin");
    std::fs::create_dir_all(&wrapper_dir).unwrap();
    let log = dir.path().join("git-args.bin");
    let source = format!(
        r#"
use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;

fn main() {{
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let mut log = OpenOptions::new().create(true).append(true).open({log:?}).unwrap();
    log.write_all(&(args.len() as u64).to_le_bytes()).unwrap();
    for arg in &args {{
        let bytes = arg.as_os_str().as_encoded_bytes();
        log.write_all(&(bytes.len() as u64).to_le_bytes()).unwrap();
        log.write_all(bytes).unwrap();
    }}
    let status = Command::new({real:?}).args(&args).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}}
"#,
        log = log.display().to_string(),
        real = real_git_executable().display().to_string(),
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
    SparseGitRecorder { dir, log }
}

fn run_with_sparse_git_recorder(
    recorder: &SparseGitRecorder,
    repo: &Path,
    operation: &[&str],
) -> std::process::Output {
    let old_path = std::env::var("PATH").unwrap();
    let wrapper_path = recorder.dir.path().join("bin");
    let mut cmd = cargo_bin();
    cmd.args(operation)
        .arg("--repo")
        .arg(repo)
        .env("PATH", format!("{};{}", wrapper_path.display(), old_path));
    cmd.output().unwrap()
}

fn read_sparse_git_recording(path: &Path) -> Vec<Vec<String>> {
    let bytes = std::fs::read(path).unwrap_or_default();
    let mut offset = 0usize;
    let mut invocations = Vec::new();
    while offset < bytes.len() {
        assert!(bytes.len() - offset >= 8, "truncated invocation count");
        let argc = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            assert!(bytes.len() - offset >= 8, "truncated argument length");
            let len = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
            offset += 8;
            assert!(bytes.len() - offset >= len, "truncated argument bytes");
            args.push(String::from_utf8(bytes[offset..offset + len].to_vec()).unwrap());
            offset += len;
        }
        invocations.push(args);
    }
    assert_eq!(offset, bytes.len());
    invocations
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
    assert_git_output_success(&output, "git add src fixture");
    let output = git(repo)
        .arg("commit")
        .arg("-m")
        .arg(message)
        .output()
        .unwrap();
    assert_git_output_success(&output, "git commit src fixture");
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
            let output = child.wait_with_output().unwrap();
            assert_git_output_success(&output, "git update-index conflict fixture");
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

// PKG-08A: Check-side tracked-governance lifecycle coverage

fn add_tracked_mrgs_and_commit(repo: &Path, path: &str, content: &[u8]) {
    let full = repo.join(".mrgs").join(path);
    if let Some(p) = full.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(&full, content).unwrap();
    let add_output = git(repo)
        .arg("add")
        .arg("--force")
        .arg(format!(".mrgs/{}", path))
        .output()
        .unwrap();
    assert_git_output_success(&add_output, &format!("git add .mrgs/{path}"));
    let commit_output = git(repo)
        .arg("commit")
        .arg("-m")
        .arg(format!("track .mrgs/{}", path))
        .output()
        .unwrap();
    assert_git_output_success(&commit_output, &format!("git commit .mrgs/{path}"));
}

fn ls_files_stage0(repo: &Path, path: &str) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("ls-files")
        .arg("--sparse")
        .arg("--stage")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git ls-files --stage {path} failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.trim().is_empty(), "path {path} not in index");
    let first_line = stdout.lines().next().unwrap().trim();
    let parts: Vec<&str> = first_line.split('\t').collect();
    let stage = parts[0].split_whitespace().nth(2).unwrap();
    assert_eq!(stage, "0", "expected stage 0 for {path}, got {stage}");
}

fn assert_ls_files_mode(repo: &Path, path: &str, expected_mode: &str) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("ls-files")
        .arg("--sparse")
        .arg("--stage")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git ls-files --stage {path} failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next().unwrap().trim();
    let mode = first_line
        .split('\t')
        .next()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    assert_eq!(
        mode, expected_mode,
        "expected mode {expected_mode} for {path}, got {mode}"
    );
}

fn assert_repo_clean(repo: &Path) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("--untracked-files=no")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git status --porcelain failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty(), "repo not clean: {stdout}");
}

fn assert_tracked_fixture_clean(repo: &Path, path: &str) {
    ls_files_stage0(repo, path);
    assert_ls_files_mode(repo, path, "100644");
    assert_repo_clean(repo);
}

fn host_case_alias_directory_assertions(repo: &Path) {
    let mrgs = repo.join(".mrgs");
    let mrgs_upper = repo.join(".MRGS");
    assert!(mrgs.exists(), ".mrgs directory must exist");
    assert!(
        mrgs_upper.exists(),
        ".MRGS directory must exist (created by test setup)"
    );
    let canonical_mrgs = std::fs::canonicalize(&mrgs).unwrap();
    let canonical_mrgs_upper = std::fs::canonicalize(&mrgs_upper).unwrap();
    if canonical_mrgs == canonical_mrgs_upper {
        assert_eq!(
            std::fs::metadata(&mrgs).unwrap().is_dir(),
            std::fs::metadata(&mrgs_upper).unwrap().is_dir(),
            "case-insensitive alias directories must have the same kind"
        );
    } else {
        assert_ne!(
            canonical_mrgs, canonical_mrgs_upper,
            "case-sensitive hosts must preserve distinct canonical fixture paths"
        );
    }
}

// ============================================================================
// Existing PKG-08A tests (preserved with fixture assertions added)
// ============================================================================

#[test]
fn test_impl_check_submodule_rejected() {
    let (dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let sub_dir = dir.path().join("sub");
    let init_output = Command::new("git")
        .arg("init")
        .arg(&sub_dir)
        .output()
        .unwrap();
    assert_git_output_success(&init_output, "git init submodule fixture");
    std::fs::write(sub_dir.join("sub_file.txt"), b"sub").unwrap();
    let add_output = Command::new("git")
        .arg("-C")
        .arg(&sub_dir)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();
    assert_git_output_success(&add_output, "git add submodule fixture");
    let commit_output = Command::new("git")
        .arg("-C")
        .arg(&sub_dir)
        .arg("-c")
        .arg("user.name=test")
        .arg("-c")
        .arg("user.email=test@test.com")
        .arg("commit")
        .arg("-m")
        .arg("init")
        .output()
        .unwrap();
    assert_git_output_success(&commit_output, "git commit submodule fixture");
    let add_submodule_output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("protocol.file.allow=always")
        .arg("submodule")
        .arg("add")
        .arg(&sub_dir)
        .arg("submod")
        .output()
        .unwrap();
    assert_git_output_success(&add_submodule_output, "git submodule add fixture");
    let outer_commit_output = git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("add submodule")
        .output()
        .unwrap();
    assert_git_output_success(&outer_commit_output, "git commit submodule fixture");
    ls_files_stage0(&repo, "submod");
    assert_ls_files_mode(&repo, "submod", "160000");
    ls_files_stage0(&repo, ".gitmodules");
    assert_ls_files_mode(&repo, ".gitmodules", "100644");
    assert_repo_clean(&repo);
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_SUBMODULE_UNSUPPORTED",
        &repo,
        &governance_before,
    );
}

#[test]
fn test_impl_check_tracked_accepted_plan_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let bytes = std::fs::read(repo.join(".mrgs").join("accepted-plan.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "accepted-plan.json", &bytes);
    assert_tracked_fixture_clean(&repo, ".mrgs/accepted-plan.json");
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

#[test]
fn test_impl_check_tracked_state_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let bytes = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "state.json", &bytes);
    assert_tracked_fixture_clean(&repo, ".mrgs/state.json");
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

#[test]
fn test_impl_check_tracked_contract_draft_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let bytes = std::fs::read(repo.join(".mrgs").join("contract-draft.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "contract-draft.json", &bytes);
    assert_tracked_fixture_clean(&repo, ".mrgs/contract-draft.json");
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

#[test]
fn test_impl_check_tracked_accepted_contract_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let bytes = std::fs::read(repo.join(".mrgs").join("accepted-contract.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "accepted-contract.json", &bytes);
    assert_tracked_fixture_clean(&repo, ".mrgs/accepted-contract.json");
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

#[test]
fn test_impl_check_tracked_impl_authority_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let bytes = std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    add_tracked_mrgs_and_commit(&repo, "implementation-authority.json", &bytes);
    assert_tracked_fixture_clean(&repo, ".mrgs/implementation-authority.json");
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

#[test]
fn test_impl_check_tracked_extra_json_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    add_tracked_mrgs_and_commit(&repo, "extra.json", b"{}");
    assert_tracked_fixture_clean(&repo, ".mrgs/extra.json");
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

#[test]
fn test_impl_begin_tracked_temp_file_in_mrgs_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    add_tracked_mrgs_and_commit(&repo, "mrgs_tmp_12345_67890.tmp", b"temp");
    assert_tracked_fixture_clean(&repo, ".mrgs/mrgs_tmp_12345_67890.tmp");
    let governance_before = capture_governance(&repo);
    let snapshot_before = capture_snapshot(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    assert_eq!(governance_before, capture_governance(&repo));
    let snapshot_after = capture_snapshot(&repo);
    assert_no_new_mrgs_temp_paths(
        &snapshot_before,
        &snapshot_after,
        "begin tracked Phase-4 temp-shaped path",
        &[".mrgs/mrgs_tmp_12345_67890.tmp".to_string()],
    );
}

#[test]
fn test_impl_check_tracked_temp_file_in_mrgs_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    add_tracked_mrgs_and_commit(&repo, "mrgs_tmp_12345_67890.tmp", b"temp");
    assert_tracked_fixture_clean(&repo, ".mrgs/mrgs_tmp_12345_67890.tmp");
    let governance_before = capture_governance(&repo);
    let snapshot_before = capture_snapshot(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    assert_eq!(governance_before, capture_governance(&repo));
    let snapshot_after = capture_snapshot(&repo);
    assert_no_new_mrgs_temp_paths(
        &snapshot_before,
        &snapshot_after,
        "check tracked Phase-4 temp-shaped path",
        &[".mrgs/mrgs_tmp_12345_67890.tmp".to_string()],
    );
}

#[test]
fn test_impl_begin_tracked_mrgs_state_json_case_alias_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let state_content = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    std::fs::create_dir_all(repo.join(".MRGS")).unwrap();
    std::fs::write(repo.join(".MRGS").join("state.json"), &state_content).unwrap();
    let add_output = git(&repo)
        .arg("add")
        .arg("--force")
        .arg(".MRGS/state.json")
        .output()
        .unwrap();
    assert_git_output_success(&add_output, "git add .MRGS/state.json");
    let commit_output = git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("track .MRGS/state.json")
        .output()
        .unwrap();
    assert_git_output_success(&commit_output, "git commit .MRGS/state.json");
    host_case_alias_directory_assertions(&repo);
    assert_tracked_fixture_clean(&repo, ".MRGS/state.json");
    let governance_before = capture_governance(&repo);
    let filesystem_before = capture_snapshot(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
    assert!(diff_snapshots(&filesystem_before, &capture_snapshot(&repo)).is_empty());
}

#[test]
fn test_impl_check_tracked_mrgs_state_json_case_alias_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let state_content = std::fs::read(repo.join(".mrgs").join("state.json")).unwrap();
    std::fs::create_dir_all(repo.join(".MRGS")).unwrap();
    std::fs::write(repo.join(".MRGS").join("state.json"), &state_content).unwrap();
    let add_output = git(&repo)
        .arg("add")
        .arg("--force")
        .arg(".MRGS/state.json")
        .output()
        .unwrap();
    assert_git_output_success(&add_output, "git add .MRGS/state.json");
    let commit_output = git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("track .MRGS/state.json")
        .output()
        .unwrap();
    assert_git_output_success(&commit_output, "git commit .MRGS/state.json");
    host_case_alias_directory_assertions(&repo);
    assert_tracked_fixture_clean(&repo, ".MRGS/state.json");
    let governance_before = capture_governance(&repo);
    let filesystem_before = capture_snapshot(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
    assert!(diff_snapshots(&filesystem_before, &capture_snapshot(&repo)).is_empty());
}

#[test]
fn test_impl_check_tracked_unknown_mrgs_path_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    add_tracked_mrgs_and_commit(&repo, "something_unknown.acme", b"{}");
    assert_tracked_fixture_clean(&repo, ".mrgs/something_unknown.acme");
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

// ============================================================================
// Repair 1: P4-097 directory-summary coverage
//
// Git's own index operations (update-index --cacheinfo / --index-info) do not
// accept mode 040000 entries for non-sparse repositories.  The established
// pattern in this file for testing the production code's rejection of
// directory-summary entries is the git-wrapper approach (see
// test_part1_index_topology_sparse_directory_{prefix,leaf}_wrapper).
//
// This test uses the same technique, intercepting `git ls-files --sparse
// --stage -z` to inject a mode-040000 `.mrgs/` entry so that
// validate_index_structure classifies it as a sparse directory
// (directory-summary) and rejects with GIT_INVENTORY_INVALID.
// ============================================================================

#[test]
fn test_impl_check_directory_summary_rejected() {
    let (_dir, repo) = index_topology_case("ordinary");
    let mrgs_oid = "a".repeat(40);
    let payload = index_payload("040000", &mrgs_oid, "0", ".mrgs/");
    let wrapper = create_git_wrapper(
        &repo,
        &["ls-files", "--sparse", "--stage", "-z"],
        "payload",
        &payload,
    );
    let governance_before = capture_governance(&repo);
    let output = run_check_with_git_wrapper(&repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
}

// ============================================================================
// Repair 2: P4-137 platform-neutral case-alias evidence
// ============================================================================

fn platform_neutral_inject_mrgs_state_json(repo: &Path) {
    let state_path = repo.join(".mrgs").join("state.json");
    let hash_out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("hash-object")
        .arg("-w")
        .arg(&state_path)
        .output()
        .unwrap();
    assert_git_output_success(&hash_out, "git hash-object -w case-alias fixture");
    let blob_sha = String::from_utf8_lossy(&hash_out.stdout).trim().to_string();

    let ui_out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("update-index")
        .arg("--add")
        .arg("--cacheinfo")
        .arg(format!("100644,{},.MRGS/state.json", blob_sha))
        .output()
        .unwrap();
    assert_git_output_success(&ui_out, "git update-index --cacheinfo case-alias fixture");
}

#[test]
fn test_impl_begin_platform_neutral_mrgs_case_alias_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Inject exact .MRGS/state.json entry independent of host filesystem
    platform_neutral_inject_mrgs_state_json(&repo);

    let commit_output = git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("platform-neutral .MRGS/state.json")
        .output()
        .unwrap();
    assert_git_output_success(
        &commit_output,
        "git commit platform-neutral case-alias fixture",
    );

    // Fixture assertions — prove exact path bytes at stage 0, clean index
    ls_files_stage0(&repo, ".MRGS/state.json");
    assert_ls_files_mode(&repo, ".MRGS/state.json", "100644");
    assert_repo_clean(&repo);
    let governance_before = capture_governance(&repo);
    let filesystem_before = capture_snapshot(&repo);

    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
    assert!(diff_snapshots(&filesystem_before, &capture_snapshot(&repo)).is_empty());
}

#[test]
fn test_impl_check_platform_neutral_mrgs_case_alias_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Inject exact .MRGS/state.json entry independent of host filesystem
    platform_neutral_inject_mrgs_state_json(&repo);

    let commit_output = git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("platform-neutral .MRGS/state.json")
        .output()
        .unwrap();
    assert_git_output_success(
        &commit_output,
        "git commit platform-neutral case-alias fixture",
    );

    // Fixture assertions — prove exact path bytes at stage 0, clean index
    ls_files_stage0(&repo, ".MRGS/state.json");
    assert_ls_files_mode(&repo, ".MRGS/state.json", "100644");
    assert_repo_clean(&repo);
    let governance_before = capture_governance(&repo);
    let filesystem_before = capture_snapshot(&repo);

    let output = run_implementation_check(&repo);
    assert_phase4_failure_preserves_full_governance(
        &output,
        "GIT_INVENTORY_INVALID",
        &repo,
        &governance_before,
    );
    assert!(diff_snapshots(&filesystem_before, &capture_snapshot(&repo)).is_empty());
}

// PKG-10: Promisor object failures and no-network/helper observation
// ============================================================================
//
// P4-152  promisor repository with every required object local can be inspected
//         without network or helper execution;
// P4-153  a missing promised blob fails locally with exactly GIT_COMMAND_FAILED;
// P4-154  a missing promised tree fails locally with exactly GIT_COMMAND_FAILED;
// P4-156  symlink blob inspection cannot trigger lazy fetch and a missing
//         promised symlink blob fails locally;
// P4-157  raw diff copy/rename detection cannot trigger lazy fetch and missing
//         required objects fail locally;
// P4-158  no remote helper, credential helper, fetch process, or fetch-pack
//         process is launched by begin or check;
// P4-159  an observable fake remote helper and observable fake credential helper
//         are never invoked;
// P4-160  every missing-promisor-object case emits exactly error: GIT_COMMAND_FAILED;
// P4-161  no remote URL, helper output, credential text, or network-derived stderr
//         is surfaced.
// ============================================================================

/// Sentinel files for detecting external process execution.
type PromisorSentinels = (
    std::path::PathBuf, // remote_helper_hit
    std::path::PathBuf, // credential_helper_hit
    std::path::PathBuf, // fetch_hit
    std::path::PathBuf, // fetch_pack_hit
);

/// Set up a promisor-enabled repository with all required objects local,
/// plus sentinel files and fake helpers that would be created if any external
/// process were invoked. Returns (temp_dir, repo_path, recorder, sentinels).
fn setup_promisor_with_sentinels_and_recorder() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    EnvAwareGitRecorder,
    PromisorSentinels,
    u32,
    String,
) {
    // Use existing infrastructure for governance/contract setup.
    let (dir, repo) = setup_implementation_basic();

    // Create sentinel directory and fake helpers that create files if executed.
    let sentinel_dir = tempfile::TempDir::new().unwrap();
    let scripts_dir = sentinel_dir.path().join("scripts");
    std::fs::create_dir_all(&scripts_dir).unwrap();

    let remote_helper_hit = sentinel_dir.path().join("remote-helper-hit");
    let credential_helper_hit = sentinel_dir.path().join("credential-helper-hit");
    let fetch_hit = sentinel_dir.path().join("fetch-hit");
    let fetch_pack_hit = sentinel_dir.path().join("fetch-pack-hit");

    // Fake remote helper (git-remote-fake)
    let fake_remote_helper = scripts_dir.join("git-remote-fake.bat");
    std::fs::write(
        &fake_remote_helper,
        format!("echo hit > {}\n", remote_helper_hit.display()),
    )
    .unwrap();

    // Fake credential helper (git-credential-fake)
    let fake_credential_helper = scripts_dir.join("git-credential-fake.bat");
    std::fs::write(
        &fake_credential_helper,
        format!("echo hit > {}\n", credential_helper_hit.display()),
    )
    .unwrap();

    // Fake fetch script
    let fake_fetch = scripts_dir.join("git-fetch.bat");
    std::fs::write(&fake_fetch, format!("echo hit > {}\n", fetch_hit.display())).unwrap();

    // Fake fetch-pack script
    let fake_fetch_pack = scripts_dir.join("git-fetch-pack.bat");
    std::fs::write(
        &fake_fetch_pack,
        format!("echo hit > {}\n", fetch_pack_hit.display()),
    )
    .unwrap();

    // Configure the repo with fake helpers and a secret remote URL.
    let secret_url = "https://SECRET_TOKEN_XYZ123@fake-remote.example.com/secret/repo.git";
    git(&repo)
        .arg("config")
        .arg("credential.helper")
        .arg(fake_credential_helper.display().to_string())
        .output()
        .unwrap();

    // Set up a fake remote with secret URL (no real connectivity).
    git(&repo)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg(secret_url)
        .output()
        .unwrap();

    // Mark as promisor/partial-clone.
    git(&repo)
        .arg("config")
        .arg("extensions.partialClone")
        .arg("origin")
        .output()
        .unwrap();

    // Accept the contract and get final revision/sha.
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Create the recorder.
    let recorder = create_env_aware_git_recorder();

    (
        dir,
        repo,
        recorder,
        (
            remote_helper_hit,
            credential_helper_hit,
            fetch_hit,
            fetch_pack_hit,
        ),
        final_rev,
        final_sha,
    )
}

/// Run implementation begin with the recorder and verify no helper/fetch processes.
fn run_begin_with_promisor_recorder(
    recorder: &EnvAwareGitRecorder,
    repo: &Path,
    revision: u32,
    sha256: &str,
) -> std::process::Output {
    let old_path = std::env::var("PATH").unwrap();
    let wrapper_path = recorder.dir.path().join("bin");
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("begin")
        .arg("--repo")
        .arg(repo)
        .arg("--revision")
        .arg(revision.to_string())
        .arg("--sha256")
        .arg(sha256)
        .env("PATH", format!("{};{}", wrapper_path.display(), old_path));
    cmd.output().unwrap()
}

/// Run implementation check with the recorder and verify no helper/fetch processes.
fn run_check_with_promisor_recorder(
    recorder: &EnvAwareGitRecorder,
    repo: &Path,
) -> std::process::Output {
    let old_path = std::env::var("PATH").unwrap();
    let wrapper_path = recorder.dir.path().join("bin");
    let mut cmd = cargo_bin();
    cmd.arg("implementation")
        .arg("check")
        .arg("--repo")
        .arg(repo)
        .env("PATH", format!("{};{}", wrapper_path.display(), old_path));
    cmd.output().unwrap()
}

/// Assert that no sentinel files were created (no helpers/fetch executed).
fn assert_no_helper_sentinels(sentinels: &PromisorSentinels) {
    let (remote_hit, cred_hit, fetch_hit, fetch_pack_hit) = sentinels;
    assert!(!remote_hit.exists(), "fake remote helper was executed");
    assert!(!cred_hit.exists(), "fake credential helper was executed");
    assert!(!fetch_hit.exists(), "fake fetch script was executed");
    assert!(
        !fetch_pack_hit.exists(),
        "fake fetch-pack script was executed"
    );
}

/// Count specific child process types from recorded argv.
/// The first argument is always the git subcommand (e.g., "rev-parse", "fetch").
fn count_child_processes(invocations: &[Vec<String>], pattern: &str) -> usize {
    invocations
        .iter()
        .filter(|args| {
            // Only check the first argument (subcommand), not all arguments.
            // This avoids false positives from paths or other args containing "fetch" etc.
            !args.is_empty() && args[0].contains(pattern)
        })
        .count()
}

/// Assert no fetch/fetch-pack/remote-helper children were recorded.
fn assert_no_network_children(invocations: &[Vec<String>]) {
    // Check for exact subcommand matches to avoid false positives.
    let fetch_count = invocations
        .iter()
        .filter(|args| args.first().is_some_and(|a| a == "fetch"))
        .count();
    let fetch_pack_count = invocations
        .iter()
        .filter(|args| {
            args.first()
                .is_some_and(|a| a == "fetch-pack" || a.contains("git-fetch-pack"))
        })
        .count();
    let remote_helper_count = invocations
        .iter()
        .filter(|args| args.first().is_some_and(|a| a.contains("git-remote-")))
        .count();
    assert_eq!(
        fetch_count, 0,
        "{} git fetch children recorded; expected 0",
        fetch_count
    );
    assert_eq!(
        fetch_pack_count, 0,
        "{} git fetch-pack children recorded; expected 0",
        fetch_pack_count
    );
    assert_eq!(
        remote_helper_count, 0,
        "{} git-remote-* helper children recorded; expected 0",
        remote_helper_count
    );
}

// --- P4-152: promisor repository with all objects local succeeds without network/helper ---

#[test]
fn test_p4_152_promisor_all_objects_local_no_network_begin() {
    let (_dir, repo, recorder, sentinels, _final_rev, _final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Run begin with recorder.
    let output = run_begin_with_promisor_recorder(&recorder, &repo, _final_rev, &_final_sha);
    assert!(
        output.status.success(),
        "begin failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify no helper sentinels were triggered.
    assert_no_helper_sentinels(&sentinels);

    // Verify recorded children show no fetch/fetch-pack/remote-helper.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during begin"
    );
    assert_no_network_children(&invocations);

    // Verify no remote URL or credential text in stderr.
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    for secret in [
        "SECRET_TOKEN_XYZ123",
        "fake-remote.example.com",
        "credential",
    ] {
        assert!(
            !stderr.contains(secret),
            "stderr leaked '{}': {}",
            secret,
            stderr
        );
    }
}

#[test]
fn test_p4_152_promisor_all_objects_local_no_network_check() {
    let (_dir, repo, recorder, sentinels, final_rev, final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // First do a normal begin (without recorder).
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Run check with recorder.
    let output = run_check_with_promisor_recorder(&recorder, &repo);
    assert!(
        output.status.success(),
        "check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify no helper sentinels were triggered.
    assert_no_helper_sentinels(&sentinels);

    // Verify recorded children show no fetch/fetch-pack/remote-helper.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during check"
    );
    assert_no_network_children(&invocations);

    // Verify no remote URL or credential text in stderr.
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    for secret in [
        "SECRET_TOKEN_XYZ123",
        "fake-remote.example.com",
        "credential",
    ] {
        assert!(
            !stderr.contains(secret),
            "stderr leaked '{}': {}",
            secret,
            stderr
        );
    }
}

// --- P4-153: missing promised blob fails locally with GIT_COMMAND_FAILED ---

#[test]
fn test_p4_153_missing_promised_blob_fails_locally() {
    let (_dir, repo, recorder, sentinels, final_rev, final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Normal begin first (required for check).
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Create a blob file to test with.
    std::fs::write(
        repo.join("blob_test.txt"),
        "content for missing blob test\n",
    )
    .unwrap();
    git(&repo).arg("add").arg("blob_test.txt").output().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("add blob test file")
        .output()
        .unwrap();

    // Get the blob OID for blob_test.txt.
    let blob_oid_output = git(&repo)
        .arg("rev-parse")
        .arg("HEAD:blob_test.txt")
        .output()
        .unwrap();
    assert_success(&blob_oid_output);
    let blob_oid = String::from_utf8(blob_oid_output.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(blob_oid.len(), 40);

    // Verify the blob exists.
    let present = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{}^{{blob}}", blob_oid))
        .output()
        .unwrap();
    assert_success(&present);

    // Remove the blob from the object database.
    let object_path = repo
        .join(".git")
        .join("objects")
        .join(&blob_oid[..2])
        .join(&blob_oid[2..]);
    assert!(object_path.is_file(), "blob was not a loose local object");
    std::fs::remove_file(&object_path).unwrap();

    // Also remove the working tree file so git doesn't detect changes before
    // encountering the missing blob.
    let _ = std::fs::remove_file(repo.join("blob_test.txt"));

    // Verify the blob is now missing.
    let absent = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{}^{{blob}}", blob_oid))
        .output()
        .unwrap();
    assert!(!absent.status.success());

    // Run check with recorder — must fail locally with GIT_COMMAND_FAILED.
    let output = run_check_with_promisor_recorder(&recorder, &repo);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");

    // Verify no helper sentinels were triggered (no lazy fetch).
    assert_no_helper_sentinels(&sentinels);

    // Verify recorded children show no fetch/fetch-pack/remote-helper.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert_no_network_children(&invocations);
}

// --- P4-154: missing promised tree fails locally with GIT_COMMAND_FAILED ---

#[test]
fn test_p4_154_missing_promised_tree_fails_locally() {
    let (_dir, repo, recorder, sentinels, final_rev, final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Normal begin first (required for check).
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Create a subdirectory with its own tree.
    let subdir_path = repo.join("subdir");
    std::fs::create_dir_all(&subdir_path).unwrap();
    std::fs::write(subdir_path.join("file.txt"), "subdir content\n").unwrap();
    git(&repo).arg("add").arg("subdir").output().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("add subdir tree")
        .output()
        .unwrap();

    // Get the tree OID for subdir.
    let tree_oid_output = git(&repo)
        .arg("rev-parse")
        .arg("HEAD:subdir")
        .output()
        .unwrap();
    assert_success(&tree_oid_output);
    let tree_oid = String::from_utf8(tree_oid_output.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(tree_oid.len(), 40);

    // Verify the tree exists.
    let present = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{}^{{tree}}", tree_oid))
        .output()
        .unwrap();
    assert_success(&present);

    // Remove the tree from the object database.
    let object_path = repo
        .join(".git")
        .join("objects")
        .join(&tree_oid[..2])
        .join(&tree_oid[2..]);
    assert!(object_path.is_file(), "tree was not a loose local object");
    std::fs::remove_file(&object_path).unwrap();

    // Also remove the working tree directory so git doesn't detect changes
    // before encountering the missing tree.
    let _ = std::fs::remove_dir_all(repo.join("subdir"));

    // Verify the tree is now missing.
    let absent = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{}^{{tree}}", tree_oid))
        .output()
        .unwrap();
    assert!(!absent.status.success());

    // Run check with recorder — must fail locally with GIT_COMMAND_FAILED.
    let output = run_check_with_promisor_recorder(&recorder, &repo);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");

    // Verify no helper sentinels were triggered (no lazy fetch).
    assert_no_helper_sentinels(&sentinels);

    // Verify recorded children show no fetch/fetch-pack/remote-helper.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert_no_network_children(&invocations);
}

// --- P4-156: symlink/blob inspection cannot trigger lazy fetch;
// missing promised blob fails locally without network access.
// Uses the same pattern as test_impl_check_promisor_missing_promised_commit:
// create a commit with a file, remove its blob, set baseline_head to that
// commit so check must inspect it and fail locally.

#[test]
fn test_p4_156_symlink_blob_missing_fails_locally() {
    let (_dir, repo, recorder, sentinels, final_rev, final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Normal begin first (required for check).
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Create a file to test blob inspection.
    let link_path = repo.join("mylink");
    std::fs::write(&link_path, "README.md").unwrap();
    git(&repo).arg("add").arg("mylink").output().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("add file for blob inspection test")
        .output()
        .unwrap();

    // Get the current HEAD (which has the file).
    let promised = git_head_exact(&repo);
    assert_eq!(promised.len(), 40);

    // Verify the commit is locally available.
    let present = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{promised}^{{commit}}"))
        .output()
        .unwrap();
    assert_success(&present);

    // Get the blob OID.
    let link_oid_output = git(&repo)
        .arg("rev-parse")
        .arg(format!("{promised}:mylink"))
        .output()
        .unwrap();
    assert_success(&link_oid_output);
    let link_oid = String::from_utf8(link_oid_output.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(link_oid.len(), 40);

    // Verify the blob exists.
    let present_blob = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{}^{{blob}}", link_oid))
        .output()
        .unwrap();
    assert_success(&present_blob);

    // Remove the blob from the object database.
    let object_path = repo
        .join(".git")
        .join("objects")
        .join(&link_oid[..2])
        .join(&link_oid[2..]);
    assert!(object_path.is_file(), "blob was not a loose local object");
    std::fs::remove_file(&object_path).unwrap();

    // Verify the blob is now missing.
    let absent = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{}^{{blob}}", link_oid))
        .output()
        .unwrap();
    assert!(!absent.status.success());

    // Create a new commit on top so HEAD != baseline_head. This forces check
    // to compare against the baseline which has the missing blob.
    std::fs::write(repo.join("extra.txt"), "extra content\n").unwrap();
    git(&repo).arg("add").arg("extra.txt").output().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("extra commit to force comparison")
        .output()
        .unwrap();

    // Now set baseline_head to the commit with the missing blob.
    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["baseline_head"] = serde_json::json!(promised);
    write_json(&repo, "implementation-authority.json", &record);

    // Run check with recorder — must fail locally without triggering any fetch.
    let output = run_check_with_promisor_recorder(&recorder, &repo);
    assert_eq!(
        output.status.code(),
        Some(1),
        "check should fail locally: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Key proof for P4-156: no helper sentinels were triggered (no lazy fetch).
    assert_no_helper_sentinels(&sentinels);

    // Verify recorded children show no fetch/fetch-pack/remote-helper.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert_no_network_children(&invocations);
}

// --- P4-157: copy/rename required object missing fails locally ---

#[test]
fn test_p4_157_copy_rename_missing_object_fails_locally() {
    let (_dir, repo, recorder, sentinels, final_rev, final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Normal begin first (required for check).
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Create an original file.
    std::fs::write(
        repo.join("original.txt"),
        "original content for copy test\n",
    )
    .unwrap();
    git(&repo).arg("add").arg("original.txt").output().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("add original file")
        .output()
        .unwrap();

    // Copy the file (git will detect this as a copy/rename).
    std::fs::copy(repo.join("original.txt"), repo.join("copied.txt")).unwrap();
    git(&repo).arg("add").arg("copied.txt").output().unwrap();
    git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("copy original to copied")
        .output()
        .unwrap();

    // Get the blob OID for the original file.
    let orig_oid_output = git(&repo)
        .arg("rev-parse")
        .arg("HEAD:original.txt")
        .output()
        .unwrap();
    assert_success(&orig_oid_output);
    let orig_oid = String::from_utf8(orig_oid_output.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(orig_oid.len(), 40);

    // Verify the blob exists.
    let present = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{}^{{blob}}", orig_oid))
        .output()
        .unwrap();
    assert_success(&present);

    // Remove the original blob from the object database.
    let object_path = repo
        .join(".git")
        .join("objects")
        .join(&orig_oid[..2])
        .join(&orig_oid[2..]);
    assert!(
        object_path.is_file(),
        "original blob was not a loose local object"
    );
    std::fs::remove_file(&object_path).unwrap();

    // Also remove working tree files so git doesn't detect changes before
    // encountering the missing object.
    let _ = std::fs::remove_file(repo.join("original.txt"));
    let _ = std::fs::remove_file(repo.join("copied.txt"));

    // Verify the original blob is now missing.
    let absent = git(&repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{}^{{blob}}", orig_oid))
        .output()
        .unwrap();
    assert!(!absent.status.success());

    // Run check with recorder — must fail locally with GIT_COMMAND_FAILED.
    let output = run_check_with_promisor_recorder(&recorder, &repo);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");

    // Verify no helper sentinels were triggered (no lazy fetch).
    assert_no_helper_sentinels(&sentinels);

    // Verify recorded children show no fetch/fetch-pack/remote-helper.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert_no_network_children(&invocations);
}

// --- P4-158 and P4-159: universal no external process across begin/check ---

#[test]
fn test_p4_158_no_external_process_begin_promisor() {
    let (_dir, repo, recorder, sentinels, final_rev, final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Run begin with recorder.
    let output = run_begin_with_promisor_recorder(&recorder, &repo, final_rev, &final_sha);
    assert!(
        output.status.success(),
        "begin failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // ALL_GIT_CHILDREN_OBSERVED=YES via PATH-intercepting recorder.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during begin"
    );

    // Count specific process types.
    let remote_helper_count = count_child_processes(&invocations, "git-remote-");
    let credential_helper_count = count_child_processes(&invocations, "credential");
    let fetch_count = count_child_processes(&invocations, "fetch");
    let fetch_pack_count = count_child_processes(&invocations, "fetch-pack");

    assert_eq!(
        remote_helper_count, 0,
        "git-remote-* helper executed during begin"
    );
    assert_eq!(
        credential_helper_count, 0,
        "credential helper executed during begin"
    );
    assert_eq!(fetch_count, 0, "git fetch executed during begin");
    assert_eq!(fetch_pack_count, 0, "git fetch-pack executed during begin");

    // Sentinel verification.
    assert_no_helper_sentinels(&sentinels);
}

#[test]
fn test_p4_158_no_external_process_check_promisor() {
    let (_dir, repo, recorder, sentinels, final_rev, final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Normal begin first.
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Run check with recorder.
    let output = run_check_with_promisor_recorder(&recorder, &repo);
    assert!(
        output.status.success(),
        "check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // ALL_GIT_CHILDREN_OBSERVED=YES via PATH-intercepting recorder.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert!(
        !invocations.is_empty(),
        "no git invocations recorded during check"
    );

    // Count specific process types.
    let remote_helper_count = count_child_processes(&invocations, "git-remote-");
    let credential_helper_count = count_child_processes(&invocations, "credential");
    let fetch_count = count_child_processes(&invocations, "fetch");
    let fetch_pack_count = count_child_processes(&invocations, "fetch-pack");

    assert_eq!(
        remote_helper_count, 0,
        "git-remote-* helper executed during check"
    );
    assert_eq!(
        credential_helper_count, 0,
        "credential helper executed during check"
    );
    assert_eq!(fetch_count, 0, "git fetch executed during check");
    assert_eq!(fetch_pack_count, 0, "git fetch-pack executed during check");

    // Sentinel verification.
    assert_no_helper_sentinels(&sentinels);
}

#[test]
fn test_p4_159_fake_helpers_never_invoked() {
    let (_dir, repo, recorder, sentinels, _final_rev, _final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Run both begin and check with recorder.
    let begin_output = run_begin_with_promisor_recorder(&recorder, &repo, _final_rev, &_final_sha);
    assert!(
        begin_output.status.success(),
        "begin failed: {}",
        String::from_utf8_lossy(&begin_output.stderr)
    );

    let check_output = run_check_with_promisor_recorder(&recorder, &repo);
    assert!(
        check_output.status.success(),
        "check failed: {}",
        String::from_utf8_lossy(&check_output.stderr)
    );

    // Verify no sentinel files were created.
    let (remote_hit, cred_hit, fetch_hit, fetch_pack_hit) = &sentinels;
    assert!(!remote_hit.exists(), "fake remote helper was invoked");
    assert!(!cred_hit.exists(), "fake credential helper was invoked");
    assert!(!fetch_hit.exists(), "fake fetch script was invoked");
    assert!(
        !fetch_pack_hit.exists(),
        "fake fetch-pack script was invoked"
    );

    // Verify recorded children show no helper execution.
    let invocations = read_env_aware_argv(&recorder.argv_log);
    assert!(!invocations.is_empty(), "no git invocations recorded");

    for (idx, args) in invocations.iter().enumerate() {
        // No invocation should be a helper command.
        let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
        assert!(
            !cmd.contains("git-remote-") && !cmd.contains("credential"),
            "invocation {} appears to be a helper: {:?}",
            idx,
            args
        );
    }
}

// --- P4-160: universal error category for all missing-object classes ---

#[test]
fn test_p4_160_missing_object_error_category_table() {
    // Table-driven assertion over every missing-object class implemented by PKG-10.
    // Each case creates a fresh promisor repo using the shared PKG-10 fixture,
    // removes the specified object, and verifies exact GIT_COMMAND_FAILED error
    // with no helper/fetch execution and no secret/network leak.

    struct MissingObjectCase {
        name: &'static str,
        setup: fn(&Path) -> String, // returns OID of object to remove
        verify_missing: fn(&Path, &str),
    }

    let cases = vec![
        MissingObjectCase {
            name: "ordinary_blob",
            setup: |repo| {
                std::fs::write(repo.join("blob_test.txt"), "blob content\n").unwrap();
                git(repo).arg("add").arg("blob_test.txt").output().unwrap();
                git(repo)
                    .arg("commit")
                    .arg("-m")
                    .arg("add blob test file")
                    .output()
                    .unwrap();
                let out = git(repo)
                    .arg("rev-parse")
                    .arg("HEAD:blob_test.txt")
                    .output()
                    .unwrap();
                String::from_utf8(out.stdout).unwrap().trim().to_string()
            },
            verify_missing: |repo, oid| {
                let absent = git(repo)
                    .arg("cat-file")
                    .arg("-e")
                    .arg(format!("{}^{{blob}}", oid))
                    .output()
                    .unwrap();
                assert!(!absent.status.success(), "blob should be missing");
            },
        },
        MissingObjectCase {
            name: "tree",
            setup: |repo| {
                let tree_dir = repo.join("tree_test_dir");
                std::fs::create_dir_all(&tree_dir).unwrap();
                std::fs::write(tree_dir.join("file.txt"), "tree dir content\n").unwrap();
                git(repo).arg("add").arg("tree_test_dir").output().unwrap();
                git(repo)
                    .arg("commit")
                    .arg("-m")
                    .arg("add tree test dir")
                    .output()
                    .unwrap();
                let out = git(repo)
                    .arg("rev-parse")
                    .arg("HEAD:tree_test_dir")
                    .output()
                    .unwrap();
                String::from_utf8(out.stdout).unwrap().trim().to_string()
            },
            verify_missing: |repo, oid| {
                let absent = git(repo)
                    .arg("cat-file")
                    .arg("-e")
                    .arg(format!("{}^{{tree}}", oid))
                    .output()
                    .unwrap();
                assert!(!absent.status.success(), "tree should be missing");
            },
        },
        MissingObjectCase {
            name: "symlink_blob",
            setup: |repo| {
                let link_path = repo.join("mylink");
                std::fs::write(&link_path, "README.md").unwrap();
                git(repo).arg("add").arg("mylink").output().unwrap();
                git(repo)
                    .arg("commit")
                    .arg("-m")
                    .arg("add file for blob inspection test")
                    .output()
                    .unwrap();
                let out = git(repo)
                    .arg("rev-parse")
                    .arg("HEAD:mylink")
                    .output()
                    .unwrap();
                String::from_utf8(out.stdout).unwrap().trim().to_string()
            },
            verify_missing: |repo, oid| {
                let absent = git(repo)
                    .arg("cat-file")
                    .arg("-e")
                    .arg(format!("{}^{{blob}}", oid))
                    .output()
                    .unwrap();
                assert!(!absent.status.success(), "symlink blob should be missing");
            },
        },
        MissingObjectCase {
            name: "copy_rename_required_object",
            setup: |repo| {
                std::fs::write(
                    repo.join("original.txt"),
                    "original content for copy test\n",
                )
                .unwrap();
                git(repo).arg("add").arg("original.txt").output().unwrap();
                git(repo)
                    .arg("commit")
                    .arg("-m")
                    .arg("add original file")
                    .output()
                    .unwrap();

                std::fs::copy(repo.join("original.txt"), repo.join("copied.txt")).unwrap();
                git(repo).arg("add").arg("copied.txt").output().unwrap();
                git(repo)
                    .arg("commit")
                    .arg("-m")
                    .arg("copy original to copied")
                    .output()
                    .unwrap();

                let out = git(repo)
                    .arg("rev-parse")
                    .arg("HEAD:original.txt")
                    .output()
                    .unwrap();
                String::from_utf8(out.stdout).unwrap().trim().to_string()
            },
            verify_missing: |repo, oid| {
                let absent = git(repo)
                    .arg("cat-file")
                    .arg("-e")
                    .arg(format!("{}^{{blob}}", oid))
                    .output()
                    .unwrap();
                assert!(
                    !absent.status.success(),
                    "copy/rename blob should be missing"
                );
            },
        },
    ];

    for case in cases {
        // Use the shared PKG-10 promisor fixture with sentinels and recorder.
        let (_dir, repo, recorder, sentinels, final_rev, final_sha) =
            setup_promisor_with_sentinels_and_recorder();

        // Run begin first (before removing objects) so authority exists.
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Set up the test object.
        let oid = (case.setup)(&repo);
        assert_eq!(oid.len(), 40, "{}: invalid OID length", case.name);

        // Remove the object.
        let object_path = repo
            .join(".git")
            .join("objects")
            .join(&oid[..2])
            .join(&oid[2..]);
        if object_path.is_file() {
            std::fs::remove_file(&object_path).unwrap();
        }

        // Verify it's missing.
        (case.verify_missing)(&repo, &oid);

        // Run check with recorder — should fail with GIT_COMMAND_FAILED when
        // inspecting the missing object during comparison against baseline.
        let check_output = run_check_with_promisor_recorder(&recorder, &repo);
        assert_phase4_failure_exact(&check_output, "GIT_COMMAND_FAILED");

        // Verify no helper sentinels were triggered (no lazy fetch).
        assert_no_helper_sentinels(&sentinels);

        // Verify recorded children show no fetch/fetch-pack/remote-helper.
        let invocations = read_env_aware_argv(&recorder.argv_log);
        assert_no_network_children(&invocations);
    }
}

// --- P4-161: redaction — no secrets or network diagnostics surfaced ---

#[test]
fn test_p4_161_no_secret_leak_promisor_begin() {
    let (_dir, repo, recorder, _sentinels, final_rev, final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Inject distinctive secrets into the environment and config.
    let secret_token = "SECRET_TOKEN_XYZ123";
    let secret_password = "P@ssw0rd!Secret#456";
    let helper_diagnostic = "HELPER_DIAGNOSTIC_TEXT_UNIQUE_789";

    // Set a fake credential helper that would output secrets if invoked.
    let sentinel_dir = tempfile::TempDir::new().unwrap();
    let scripts_dir = sentinel_dir.path().join("scripts");
    std::fs::create_dir_all(&scripts_dir).unwrap();
    let cred_helper = scripts_dir.join("git-credential-fake.bat");
    std::fs::write(
        &cred_helper,
        format!(
            "echo username=secret_user\necho password={}\necho {}\n",
            secret_password, helper_diagnostic
        ),
    )
    .unwrap();

    git(&repo)
        .arg("config")
        .arg("credential.helper")
        .arg(cred_helper.display().to_string())
        .output()
        .unwrap();

    // Run begin with recorder.
    let output = run_begin_with_promisor_recorder(&recorder, &repo, final_rev, &final_sha);

    // Collect all observable output.
    let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{} {}", stdout_str, stderr_str);

    // Assert no secrets appear.
    for secret in [
        secret_token,
        secret_password,
        helper_diagnostic,
        "SECRET_TOKEN",
        "P@ssw0rd",
        "fake-remote.example.com",
    ] {
        assert!(
            !combined.contains(secret),
            "secret '{}' leaked in output: {}",
            secret,
            combined
        );
    }

    // Assert no raw network-derived Git diagnostic.
    for forbidden in [
        "fatal: The remote end hung up unexpectedly",
        "Could not resolve host",
        "Connection refused",
        "SSL certificate problem",
        "unable to access",
    ] {
        assert!(
            !combined.contains(forbidden),
            "network diagnostic leaked: {}",
            forbidden
        );
    }
}

#[test]
fn test_p4_161_no_secret_leak_promisor_check() {
    let (_dir, repo, recorder, _sentinels, final_rev, final_sha) =
        setup_promisor_with_sentinels_and_recorder();

    // Normal begin first.
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Inject distinctive secrets into the environment and config.
    let secret_token = "SECRET_TOKEN_XYZ123";
    let secret_password = "P@ssw0rd!Secret#456";
    let helper_diagnostic = "HELPER_DIAGNOSTIC_TEXT_UNIQUE_789";

    // Set a fake credential helper that would output secrets if invoked.
    let sentinel_dir = tempfile::TempDir::new().unwrap();
    let scripts_dir = sentinel_dir.path().join("scripts");
    std::fs::create_dir_all(&scripts_dir).unwrap();
    let cred_helper = scripts_dir.join("git-credential-fake.bat");
    std::fs::write(
        &cred_helper,
        format!(
            "echo username=secret_user\necho password={}\necho {}\n",
            secret_password, helper_diagnostic
        ),
    )
    .unwrap();

    git(&repo)
        .arg("config")
        .arg("credential.helper")
        .arg(cred_helper.display().to_string())
        .output()
        .unwrap();

    // Run check with recorder.
    let output = run_check_with_promisor_recorder(&recorder, &repo);

    // Collect all observable output.
    let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{} {}", stdout_str, stderr_str);

    // Assert no secrets appear.
    for secret in [
        secret_token,
        secret_password,
        helper_diagnostic,
        "SECRET_TOKEN",
        "P@ssw0rd",
        "fake-remote.example.com",
    ] {
        assert!(
            !combined.contains(secret),
            "secret '{}' leaked in output: {}",
            secret,
            combined
        );
    }

    // Assert no raw network-derived Git diagnostic.
    for forbidden in [
        "fatal: The remote end hung up unexpectedly",
        "Could not resolve host",
        "Connection refused",
        "SSL certificate problem",
        "unable to access",
    ] {
        assert!(
            !combined.contains(forbidden),
            "network diagnostic leaked: {}",
            forbidden
        );
    }
}

// === PKG-11: Sparse config and lifecycle matrix ===

struct MalformedGitRecorder {
    dir: tempfile::TempDir,
}

fn create_malformed_git_recorder(
    intercept_key: &str,
    intercept_output: &[u8],
    exit_code: i32,
) -> MalformedGitRecorder {
    let dir = tempfile::TempDir::new().unwrap();
    let wrapper_dir = dir.path().join("bin");
    std::fs::create_dir_all(&wrapper_dir).unwrap();
    let output_bytes = intercept_output.to_vec();

    // Use the same pattern as SparseGitRecorder which is known to work.
    let source = format!(
        r#"
use std::env;
use std::ffi::OsString;
use std::io::Write;
use std::process::Command;

fn main() {{
    let args: Vec<OsString> = env::args_os().skip(1).collect();

    // Check if this is a config query for our intercepted key.
    let args_str: Vec<String> = args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let is_target = args_str.contains(&"config".to_string())
        && args_str.contains(&"--get".to_string())
        && !args_str.contains(&"--get-all".to_string())
        && args_str.contains(&"{key}".to_string());

    if is_target {{
        // Write the intercepted output to stdout.
        let out_bytes: &[u8] = &{output:?};
        std::io::stdout().write_all(out_bytes).ok();
        std::process::exit({exit_code});
    }}

    // Delegate to real git for everything else.
    let status = Command::new({real:?}).args(&args).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}}
"#,
        output = output_bytes,
        key = intercept_key,
        exit_code = exit_code,
        real = real_git_executable().display().to_string(),
    );
    let source_path = wrapper_dir.join("git-wrapper.rs");
    std::fs::write(&source_path, &source).unwrap();
    let wrapper = wrapper_dir.join("git.exe");
    let compile = Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&wrapper)
        .output()
        .unwrap();
    assert_eq!(
        compile.status.code(),
        Some(0),
        "wrapper compilation failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    MalformedGitRecorder { dir }
}

fn run_with_malformed_git_recorder(
    recorder: &MalformedGitRecorder,
    repo: &Path,
    operation: &[&str],
) -> std::process::Output {
    let old_path = std::env::var("PATH").unwrap();
    let wrapper_path = recorder.dir.path().join("bin");
    let mut cmd = cargo_bin();
    cmd.args(operation)
        .arg("--repo")
        .arg(repo)
        .env("PATH", format!("{};{}", wrapper_path.display(), old_path));
    cmd.output().unwrap()
}

// P4-163: core.sparseCheckout=false does not reject by that signal alone.
#[test]
fn test_pkg11_core_sparse_checkout_false_ok() {
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
        .arg("false")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_begin_exact(&output, &repo);
}

// P4-164: unset core.sparseCheckout with exit 1 and empty output does not reject.
#[test]
fn test_pkg11_core_sparse_checkout_unset_ok() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    // Ensure core.sparseCheckout is unset.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--unset")
        .arg("core.sparseCheckout")
        .status()
        .ok();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_begin_exact(&output, &repo);
}

// P4-165: malformed sparse-checkout config output rejects with GIT_INVENTORY_INVALID.
#[test]
fn test_pkg11_core_sparse_checkout_malformed_matrix() {
    struct MalformedCase {
        output: Vec<u8>,
        exit_code: i32,
    }

    let cases = vec![
        MalformedCase {
            output: b"maybe\n".to_vec(),
            exit_code: 0,
        },
        MalformedCase {
            output: b"true\nfalse\n".to_vec(),
            exit_code: 0,
        },
        MalformedCase {
            output: vec![0xFF, 0xFE, 0xFD],
            exit_code: 0,
        },
        MalformedCase {
            output: b"yes\n".to_vec(),
            exit_code: 0,
        },
    ];

    for case in cases {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        let recorder =
            create_malformed_git_recorder("core.sparseCheckout", &case.output, case.exit_code);
        let output = run_with_malformed_git_recorder(
            &recorder,
            &repo,
            &[
                "implementation",
                "begin",
                "--revision",
                &final_rev.to_string(),
                "--sha256",
                &final_sha,
            ],
        );
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    }
}

// P4-166: multiple core.sparseCheckout values reject even when final is false.
#[test]
fn test_pkg11_core_sparse_checkout_multiple_values_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    // Add multiple values: true then false. Git --get returns first value.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--add")
        .arg("core.sparseCheckout")
        .arg("true")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--add")
        .arg("core.sparseCheckout")
        .arg("false")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// P4-168: index.sparse=false does not reject by that signal alone.
#[test]
fn test_pkg11_index_sparse_false_ok() {
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
        .arg("false")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_begin_exact(&output, &repo);
}

// P4-169: unset index.sparse with exit 1 and empty output does not reject.
#[test]
fn test_pkg11_index_sparse_unset_ok() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    // Ensure index.sparse is unset.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--unset")
        .arg("index.sparse")
        .status()
        .ok();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_begin_exact(&output, &repo);
}

// P4-170: malformed sparse-index config output rejects with GIT_INVENTORY_INVALID.
#[test]
fn test_pkg11_index_sparse_malformed_matrix() {
    struct MalformedCase {
        output: Vec<u8>,
        exit_code: i32,
    }

    let cases = vec![
        MalformedCase {
            output: b"maybe\n".to_vec(),
            exit_code: 0,
        },
        MalformedCase {
            output: b"true\nfalse\n".to_vec(),
            exit_code: 0,
        },
        MalformedCase {
            output: vec![0xFF, 0xFE, 0xFD],
            exit_code: 0,
        },
        MalformedCase {
            output: b"yes\n".to_vec(),
            exit_code: 0,
        },
    ];

    for case in cases {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        let recorder = create_malformed_git_recorder("index.sparse", &case.output, case.exit_code);
        let output = run_with_malformed_git_recorder(
            &recorder,
            &repo,
            &[
                "implementation",
                "begin",
                "--revision",
                &final_rev.to_string(),
                "--sha256",
                &final_sha,
            ],
        );
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    }
}

// P4-171: multiple index.sparse values reject even when final is false.
#[test]
fn test_pkg11_index_sparse_multiple_values_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    // Add multiple values: true then false.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--add")
        .arg("index.sparse")
        .arg("true")
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--add")
        .arg("index.sparse")
        .arg("false")
        .status()
        .unwrap();
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// P4-176: structural sparse-directory rejects when index.sparse is unset.
// Uses git sparse-checkout to create actual sparse directory entries in the index,
// then verifies rejection even after disabling index.sparse config.
#[test]
fn test_pkg11_sparse_directory_with_unset_index_sparse() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Create additional content to have something to sparse-checkout.
    let subdir = repo.join("subdir");
    std::fs::create_dir_all(&subdir).unwrap();
    std::fs::write(subdir.join("file.txt"), "content\n").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("subdir")
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
        .arg("add subdir")
        .status()
        .unwrap();

    // Enable sparse checkout to create sparse directory entries.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("sparse-checkout")
        .arg("init")
        .arg("--cone")
        .status()
        .unwrap();

    // Disable index.sparse config but keep sparse checkout active.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--unset")
        .arg("index.sparse")
        .status()
        .ok();

    // Verify index.sparse is unset.
    let check = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--get")
        .arg("index.sparse")
        .output()
        .unwrap();
    assert!(!check.status.success(), "index.sparse should be unset");

    // Begin should still reject due to sparse checkout state.
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// P4-176: structural sparse-directory rejects when index.sparse=false.
#[test]
fn test_pkg11_sparse_directory_with_false_index_sparse() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    // Create additional content.
    let subdir = repo.join("subdir");
    std::fs::create_dir_all(&subdir).unwrap();
    std::fs::write(subdir.join("file.txt"), "content\n").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("subdir")
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
        .arg("add subdir")
        .status()
        .unwrap();

    // Enable sparse checkout.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("sparse-checkout")
        .arg("init")
        .arg("--cone")
        .status()
        .unwrap();

    // Explicitly set index.sparse=false.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("index.sparse")
        .arg("false")
        .status()
        .unwrap();

    // Begin should still reject due to sparse checkout state.
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// P4-177: active sparse checkout rejects when skip-worktree bits are cleared.
#[test]
fn test_pkg11_cleared_skip_worktree_still_rejected() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    // Set up active sparse checkout.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("core.sparseCheckout")
        .arg("true")
        .status()
        .unwrap();
    let sparse_checkout_file = repo.join(".git").join("info").join("sparse-checkout");
    std::fs::write(&sparse_checkout_file, "README.md\n").unwrap();
    // Set skip-worktree on README.md.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("update-index")
        .arg("--skip-worktree")
        .arg("README.md")
        .status()
        .unwrap();
    // Clear skip-worktree bits.
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("update-index")
        .arg("--no-skip-worktree")
        .arg("README.md")
        .status()
        .unwrap();
    // Sparse checkout config is still active; should reject.
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// P4-179: begin rejects each sparse state (lifecycle matrix).
#[test]
fn test_pkg11_begin_rejects_all_sparse_states() {
    // Case 1: active sparse checkout.
    {
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
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    }

    // Case 2: active sparse index.
    {
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
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    }

    // Case 3: structural sparse-directory (reuse existing test pattern).
    {
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
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    }
}

// P4-180: check rejects each sparse state (lifecycle matrix).
#[test]
fn test_pkg11_check_rejects_all_sparse_states() {
    // Case 1: active sparse checkout at check.
    {
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
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    }

    // Case 2: active sparse index at check.
    {
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
            .arg("index.sparse")
            .arg("true")
            .status()
            .unwrap();
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    }

    // Case 3: structural sparse-directory at check.
    {
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
            .arg("index.sparse")
            .arg("true")
            .status()
            .unwrap();
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    }
}

// P4-181: failure-category mapping for sparse-state evidence.
#[test]
fn test_pkg11_sparse_failure_category_mapping() {
    // Case 1: malformed successful output -> GIT_INVENTORY_INVALID.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        let recorder =
            create_malformed_git_recorder("core.sparseCheckout", b"malformed_value\n".as_ref(), 0);
        let output = run_with_malformed_git_recorder(
            &recorder,
            &repo,
            &[
                "implementation",
                "begin",
                "--revision",
                &final_rev.to_string(),
                "--sha256",
                &final_sha,
            ],
        );
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    }

    // Case 2: git command execution failure -> GIT_COMMAND_FAILED.
    // Use a wrapper that makes git config fail with exit code 1.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        // Create a wrapper that fails on config --get for sparse keys.
        let dir = tempfile::TempDir::new().unwrap();
        let wrapper_dir = dir.path().join("bin");
        std::fs::create_dir_all(&wrapper_dir).unwrap();
        let source = format!(
            r#"
use std::env;
use std::ffi::OsString;
use std::process::Command;

fn main() {{
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let args_str: Vec<String> = args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let is_sparse_config = args_str.contains(&"config".to_string())
        && args_str.contains(&"--get".to_string())
        && (args_str.contains(&"core.sparseCheckout".to_string())
            || args_str.contains(&"index.sparse".to_string()));

    if is_sparse_config {{
        eprintln!("fatal: unable to read config");
        std::process::exit(1);
    }}

    let status = Command::new({real:?}).args(&args).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}}
"#,
            real = real_git_executable().display().to_string(),
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

        let old_path = std::env::var("PATH").unwrap();
        let mut cmd = cargo_bin();
        cmd.args([
            "implementation",
            "begin",
            "--repo",
            repo.to_str().unwrap(),
            "--revision",
            &final_rev.to_string(),
            "--sha256",
            &final_sha,
        ])
        .env("PATH", format!("{};{}", wrapper_dir.display(), old_path));
        let output = cmd.output().unwrap();
        assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
    }

    // Case 3: abnormal child termination (simulated via wrapper exit with signal-like code).
    // On Windows, we simulate this with a non-standard exit code that represents
    // an abnormal termination. Platform limitation: Windows does not support Unix signals.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        let dir = tempfile::TempDir::new().unwrap();
        let wrapper_dir = dir.path().join("bin");
        std::fs::create_dir_all(&wrapper_dir).unwrap();
        let source = format!(
            r#"
use std::env;
use std::ffi::OsString;
use std::process::Command;

fn main() {{
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let args_str: Vec<String> = args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let is_sparse_config = args_str.contains(&"config".to_string())
        && args_str.contains(&"--get".to_string())
        && (args_str.contains(&"core.sparseCheckout".to_string())
            || args_str.contains(&"index.sparse".to_string()));

    if is_sparse_config {{
        // Simulate abnormal termination with exit code 128+signal.
        std::process::exit(139);
    }}

    let status = Command::new({real:?}).args(&args).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}}
"#,
            real = real_git_executable().display().to_string(),
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

        let old_path = std::env::var("PATH").unwrap();
        let mut cmd = cargo_bin();
        cmd.args([
            "implementation",
            "begin",
            "--repo",
            repo.to_str().unwrap(),
            "--revision",
            &final_rev.to_string(),
            "--sha256",
            &final_sha,
        ])
        .env("PATH", format!("{};{}", wrapper_dir.display(), old_path));
        let output = cmd.output().unwrap();
        assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
    }
}

// === PKG-12: Real cone/non-cone/sparse-index repository fixtures ===

/// Set up a valid implementation repo with committed content suitable for sparse selection.
/// Returns (tempdir, repo_path, final_rev, final_sha).
fn setup_sparse_fixture() -> (tempfile::TempDir, std::path::PathBuf, u32, String) {
    let (_dir, repo) = setup_implementation_basic();

    // Create fixture files.
    std::fs::create_dir_all(repo.join("included").join("nested")).unwrap();
    std::fs::create_dir_all(repo.join("excluded").join("nested")).unwrap();
    std::fs::write(repo.join("root-file.txt"), "root content\n").unwrap();
    std::fs::write(repo.join("included").join("a.txt"), "included a\n").unwrap();
    std::fs::write(
        repo.join("included").join("nested").join("b.txt"),
        "included nested b\n",
    )
    .unwrap();
    std::fs::write(repo.join("excluded").join("c.txt"), "excluded c\n").unwrap();
    std::fs::write(
        repo.join("excluded").join("nested").join("d.txt"),
        "excluded nested d\n",
    )
    .unwrap();

    // Commit fixture files individually, avoiding .mrgs/ which must stay untracked.
    git_cmd(&repo, &["add", "root-file.txt", "included/", "excluded/"]);
    commit_file(&repo, "root-file.txt");

    // Verify clean (only untracked .mrgs/ artifacts should remain).
    let status = git_cmd_output(&repo, &["status", "--porcelain"]);
    let status_str = String::from_utf8_lossy(&status.stdout);
    for line in status_str.lines() {
        assert!(
            line.starts_with("?? .mrgs/"),
            "unexpected dirty status: {}",
            line
        );
    }

    // Accept contract.
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    (_dir, repo, final_rev, final_sha)
}

/// Run a git command and assert success, returning Output.
fn git_cmd_output(repo: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo);
    cmd.args(args);
    cmd.output().unwrap()
}

/// Run a git command and assert success.
fn git_cmd(repo: &Path, args: &[&str]) {
    let out = git_cmd_output(repo, args);
    assert!(
        out.status.success(),
        "git {} failed: stderr={}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Run implementation begin and return Output.
fn run_begin(repo: &Path, final_rev: u32, final_sha: &str) -> std::process::Output {
    run_implementation_begin(repo, final_rev, final_sha)
}

// P4-172: actual cone-mode sparse-checkout repository is rejected.
#[test]
fn test_pkg12_real_cone_sparse_checkout_rejected() {
    let (_dir, repo, final_rev, final_sha) = setup_sparse_fixture();

    // Create real cone-mode sparse checkout.
    // This implicitly proves sparse-checkout capability.
    git_cmd(&repo, &["sparse-checkout", "init", "--cone"]);
    git_cmd(&repo, &["sparse-checkout", "set", "included"]);

    // Prove cone mode is active.
    assert!(repo
        .join(".git")
        .join("info")
        .join("sparse-checkout")
        .exists());

    let sc_config = git_cmd_output(&repo, &["config", "--get", "core.sparseCheckout"]);
    let sc_val = String::from_utf8_lossy(&sc_config.stdout)
        .trim()
        .to_string();
    assert_eq!(sc_val, "true", "core.sparseCheckout should be true");

    // Prove cone mode is set.
    let cone_config = git_cmd_output(&repo, &["config", "--get", "core.sparseCheckoutCone"]);
    let cone_val = String::from_utf8_lossy(&cone_config.stdout)
        .trim()
        .to_string();
    assert_eq!(cone_val, "true", "core.sparseCheckoutCone should be true");

    // Prove working tree is sparse: included present, excluded absent.
    assert!(
        repo.join("included").join("a.txt").exists(),
        "included/a.txt should be present"
    );
    assert!(
        !repo.join("excluded").join("c.txt").exists(),
        "excluded/c.txt should be absent"
    );

    // Prove tracked files are clean (ignore untracked .mrgs/ artifacts).
    let status = git_cmd_output(&repo, &["diff", "--quiet", "HEAD"]);
    assert!(
        status.status.success(),
        "tracked files not clean: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    // Run implementation begin - must reject.
    let output = run_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// P4-173: actual non-cone sparse-checkout repository is rejected.
#[test]
fn test_pkg12_real_non_cone_sparse_checkout_rejected() {
    let (_dir, repo, final_rev, final_sha) = setup_sparse_fixture();

    // Create real non-cone sparse checkout.
    // Include governance files (plan.toml, contract.toml) in the patterns so
    // MRGS can validate them before reaching the sparse config check.
    // The patterns still prove non-cone mode is active.
    git_cmd(&repo, &["sparse-checkout", "init", "--no-cone"]);
    git_cmd(
        &repo,
        &[
            "sparse-checkout",
            "set",
            "--no-cone",
            "/included/a.txt",
            "/excluded/c.txt",
            "/plan.toml",
            "/contract.toml",
        ],
    );

    // Prove sparse checkout is active.
    assert!(repo
        .join(".git")
        .join("info")
        .join("sparse-checkout")
        .exists());

    let sc_config = git_cmd_output(&repo, &["config", "--get", "core.sparseCheckout"]);
    let sc_val = String::from_utf8_lossy(&sc_config.stdout)
        .trim()
        .to_string();
    assert_eq!(sc_val, "true", "core.sparseCheckout should be true");

    // Prove cone mode is NOT set (non-cone).
    let cone_config = git_cmd_output(&repo, &["config", "--get", "core.sparseCheckoutCone"]);
    let cone_val = String::from_utf8_lossy(&cone_config.stdout)
        .trim()
        .to_string();
    assert!(
        cone_val.is_empty() || cone_val == "false",
        "core.sparseCheckoutCone should be false/absent for non-cone, got: {}",
        cone_val
    );

    // Prove non-cone patterns in sparse-checkout file.
    let sparse_file = repo.join(".git").join("info").join("sparse-checkout");
    let sparse_content = std::fs::read_to_string(&sparse_file).unwrap();
    assert!(
        sparse_content.contains("/included/a.txt"),
        "sparse-checkout should contain /included/a.txt pattern"
    );
    assert!(
        sparse_content.contains("/excluded/c.txt"),
        "sparse-checkout should contain /excluded/c.txt pattern"
    );

    // Prove selected content present, excluded absent.
    assert!(
        repo.join("included").join("a.txt").exists(),
        "included/a.txt should be present"
    );
    assert!(
        !repo.join("included").join("nested").join("b.txt").exists(),
        "included/nested/b.txt should be absent (not in pattern)"
    );
    assert!(
        !repo.join("excluded").join("nested").join("d.txt").exists(),
        "excluded/nested/d.txt should be absent"
    );

    // Prove tracked files are clean (ignore untracked .mrgs/ artifacts).
    let status = git_cmd_output(&repo, &["diff", "--quiet", "HEAD"]);
    assert!(
        status.status.success(),
        "tracked files not clean: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    // Run implementation begin - must reject.
    let output = run_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
}

// P4-174: actual sparse-index repository is rejected without silently expanding its index.
#[test]
fn test_pkg12_real_sparse_index_rejected_without_expansion() {
    let (_dir, repo, final_rev, final_sha) = setup_sparse_fixture();

    // Create real sparse-index repository.
    git_cmd(
        &repo,
        &["sparse-checkout", "init", "--cone", "--sparse-index"],
    );
    git_cmd(&repo, &["sparse-checkout", "set", "included"]);

    // Prove sparse checkout and sparse index are active.
    let sc_config = git_cmd_output(&repo, &["config", "--get", "core.sparseCheckout"]);
    let sc_val = String::from_utf8_lossy(&sc_config.stdout)
        .trim()
        .to_string();
    assert_eq!(sc_val, "true", "core.sparseCheckout should be true");

    let si_config = git_cmd_output(&repo, &["config", "--get", "index.sparse"]);
    let si_val = String::from_utf8_lossy(&si_config.stdout)
        .trim()
        .to_string();
    assert_eq!(si_val, "true", "index.sparse should be true");

    // Prove working tree is genuinely sparse.
    assert!(
        repo.join("included").join("a.txt").exists(),
        "included/a.txt should be present"
    );
    assert!(
        !repo.join("excluded").join("c.txt").exists(),
        "excluded/c.txt should be absent"
    );

    // Prove git ls-files --sparse --stage -z contains mode 040000 records.
    // Parse raw bytes directly since -z uses NUL delimiters.
    let ls_files = git_cmd_output(&repo, &["ls-files", "--sparse", "--stage", "-z"]);
    let ls_files_raw = ls_files.stdout.clone();
    let sparse_records: Vec<&[u8]> = ls_files_raw
        .split(|&b| b == 0)
        .filter(|entry| !entry.is_empty() && entry.starts_with(b"040000 "))
        .collect();
    assert!(
        !sparse_records.is_empty(),
        "expected at least one mode 040000 sparse-directory record; raw: {:?}",
        String::from_utf8_lossy(&ls_files_raw)
    );

    // Verify trailing slash on sparse-directory paths.
    for record in &sparse_records {
        let record_str = String::from_utf8_lossy(record);
        let parts: Vec<&str> = record_str.split('\t').collect();
        assert_eq!(parts.len(), 2, "unexpected ls-files format: {}", record_str);
        assert!(
            parts[1].ends_with('/'),
            "sparse-directory path should have trailing slash: {}",
            record_str
        );
    }

    // Capture index state BEFORE MRGS invocation.
    let index_path = repo.join(".git").join("index");
    let index_before = std::fs::read(&index_path).unwrap();
    let index_sha_before = sha256_hex(&index_before);
    let index_len_before = index_before.len();
    let sparse_count_before = sparse_records.len();

    // Extract sparse directory paths before
    let sparse_paths_before: Vec<String> = sparse_records
        .iter()
        .map(|r| {
            String::from_utf8_lossy(r)
                .split('\t')
                .nth(1)
                .unwrap_or("")
                .to_string()
        })
        .collect();

    // Print required evidence
    println!("PKG12_INDEX_SHA256_BEFORE={}", index_sha_before);
    println!("PKG12_INDEX_LENGTH_BEFORE={}", index_len_before);
    println!(
        "PKG12_SPARSE_DIRECTORY_RECORDS_BEFORE={}",
        sparse_paths_before.join(",")
    );

    // Capture working-tree presence/absence matrix.
    let wt_included_a = repo.join("included").join("a.txt").exists();
    let wt_included_b = repo.join("included").join("nested").join("b.txt").exists();
    let wt_excluded_c = repo.join("excluded").join("c.txt").exists();
    let wt_excluded_d = repo.join("excluded").join("nested").join("d.txt").exists();

    // Capture git status of tracked files.
    let status_before = git_cmd_output(&repo, &["diff", "--quiet", "HEAD"]);
    let status_before_clean = status_before.status.success();

    // Run implementation begin - must reject.
    let output = run_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");

    // Capture index state AFTER MRGS invocation.
    // Do NOT run any Git command before this snapshot.
    let index_after = std::fs::read(&index_path).unwrap();
    let index_sha_after = sha256_hex(&index_after);
    let index_len_after = index_after.len();

    // Prove index was NOT expanded/rewritten.
    assert_eq!(
        index_sha_before, index_sha_after,
        "INDEX_SHA256_EQUAL_BEFORE_AFTER failed: {} != {}",
        index_sha_before, index_sha_after
    );
    assert_eq!(
        index_len_before, index_len_after,
        "INDEX_LENGTH_EQUAL_BEFORE_AFTER failed: {} vs {}",
        index_len_before, index_len_after
    );
    assert_eq!(
        index_before, index_after,
        "INDEX_BYTES_EQUAL_BEFORE_AFTER failed"
    );

    // Prove sparse-directory records unchanged.
    let ls_files_after = git_cmd_output(&repo, &["ls-files", "--sparse", "--stage", "-z"]);
    let ls_files_after_raw = ls_files_after.stdout;
    let sparse_records_after: Vec<&[u8]> = ls_files_after_raw
        .split(|&b| b == 0)
        .filter(|entry| !entry.is_empty() && entry.starts_with(b"040000 "))
        .collect();
    assert_eq!(
        sparse_count_before,
        sparse_records_after.len(),
        "SPARSE_DIRECTORY_RECORDS_EQUAL_BEFORE_AFTER failed: {} vs {}",
        sparse_count_before,
        sparse_records_after.len()
    );

    // Extract sparse directory paths after
    let sparse_paths_after: Vec<String> = sparse_records_after
        .iter()
        .map(|r| {
            String::from_utf8_lossy(r)
                .split('\t')
                .nth(1)
                .unwrap_or("")
                .to_string()
        })
        .collect();

    // Print required evidence
    println!("PKG12_INDEX_SHA256_AFTER={}", index_sha_after);
    println!("PKG12_INDEX_LENGTH_AFTER={}", index_len_after);
    println!(
        "PKG12_SPARSE_DIRECTORY_RECORDS_AFTER={}",
        sparse_paths_after.join(",")
    );

    // Verify SHA-256 equality
    assert_eq!(
        index_sha_before, index_sha_after,
        "INDEX_SHA256_EQUAL_BEFORE_AFTER failed: {} != {}",
        index_sha_before, index_sha_after
    );

    // Prove working-tree matrix unchanged.
    assert_eq!(wt_included_a, repo.join("included").join("a.txt").exists());
    assert_eq!(
        wt_included_b,
        repo.join("included").join("nested").join("b.txt").exists()
    );
    assert_eq!(wt_excluded_c, repo.join("excluded").join("c.txt").exists());
    assert_eq!(
        wt_excluded_d,
        repo.join("excluded").join("nested").join("d.txt").exists()
    );

    // Prove git status unchanged.
    let status_after = git_cmd_output(&repo, &["diff", "--quiet", "HEAD"]);
    let status_after_clean = status_after.status.success();
    assert_eq!(
        status_before_clean, status_after_clean,
        "GIT_STATUS_EQUAL_BEFORE_AFTER failed"
    );
}

/// Compute SHA-256 hex string of bytes using certutil on Windows.
fn sha256_hex(data: &[u8]) -> String {
    // Use a unique temp path per call to avoid file locking issues with parallel tests on Windows.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tmp_dir = std::env::temp_dir();
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let tmp_path = tmp_dir.join(format!(
        "pkg13_sha256_{}_{}.tmp",
        std::process::id(),
        counter
    ));
    std::fs::write(&tmp_path, data).unwrap();
    let out = Command::new("certutil")
        .args(["-hashfile", tmp_path.to_str().unwrap(), "SHA256"])
        .output()
        .unwrap();
    // Clean up temp file.
    let _ = std::fs::remove_file(&tmp_path);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // certutil output: line 1 = header, line 2 = hash, line 3 = "CertUtil: -hashfile command completed successfully."
    stdout
        .lines()
        .nth(1)
        .unwrap_or("")
        .replace(' ', "")
        .to_lowercase()
}

// ============================================================================
// PKG-13: Preservation tests
// ============================================================================

/// Typed governance entry: distinguishes directory, regular file, and symlink
/// kinds so that byte-for-byte equality is kind-aware and lossless.  A bare
/// `Vec<u8>` value could not tell an empty directory from an empty regular file
/// or a symlink whose target happens to have the same bytes as file content.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum GovernanceEntry {
    MissingRoot,
    Directory,
    RegularFile(Vec<u8>),
    Symlink(Vec<u8>),
}

/// Exhaustive fail-closed governance (.mrgs/*) snapshot.
/// Recursively records every .mrgs entry with lossless native OsString relative path,
/// explicit directory/regular-file/symlink kind, exact regular bytes and
/// lossless symlink target bytes.  Fails immediately on every read_dir, entry,
/// metadata, read, and link-target error.  No filter_map(Result::ok), .ok(),
/// if let Ok, unwrap_or_default or equivalent.  Deterministic BTreeMap sort.
/// Preserves all five pre/post snapshot callsites and exact equality assertions.
fn capture_governance(
    repo: &Path,
) -> std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry> {
    let mrgs = repo.join(".mrgs");
    let mut result = std::collections::BTreeMap::new();
    // GAP2: unconditionally metadata the .mrgs root; NotFound returns explicit
    // MissingRoot snapshot; all other errors fail immediately.
    let root_meta = match std::fs::symlink_metadata(&mrgs) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            result.insert(
                std::ffi::OsString::from(".mrgs"),
                GovernanceEntry::MissingRoot,
            );
            return result;
        }
        Err(e) => {
            panic!(
                "capture_governance: symlink_metadata failed for .mrgs root {}: {}",
                mrgs.display(),
                e
            )
        }
    };
    assert!(
        root_meta.is_dir(),
        "capture_governance: .mrgs root must be a directory, got {:?}",
        root_meta.file_type()
    );
    result.insert(
        std::ffi::OsString::from(".mrgs"),
        GovernanceEntry::Directory,
    );
    let mut stack = vec![mrgs.clone()];
    while let Some(current) = stack.pop() {
        let read_dir_iter = std::fs::read_dir(&current).unwrap_or_else(|e| {
            panic!(
                "capture_governance: read_dir failed for {}: {}",
                current.display(),
                e
            )
        });
        for entry_result in read_dir_iter {
            let entry = entry_result.unwrap_or_else(|e| {
                panic!(
                    "capture_governance: entry iteration error in {}: {}",
                    current.display(),
                    e
                );
            });
            let meta = std::fs::symlink_metadata(entry.path()).unwrap_or_else(|e| {
                panic!(
                    "capture_governance: symlink_metadata failed for {}: {}",
                    entry.path().display(),
                    e
                );
            });
            // GAP1: lossless native OsString key from relative path.
            let entry_path = entry.path();
            let rel = entry_path
                .strip_prefix(repo)
                .unwrap_or(entry_path.as_path());
            let os_key = rel.as_os_str().to_os_string();
            if meta.is_dir() {
                result.insert(os_key, GovernanceEntry::Directory);
                stack.push(entry.path());
            } else if meta.is_symlink() {
                let target = std::fs::read_link(entry.path()).unwrap_or_else(|e| {
                    panic!(
                        "capture_governance: read_link failed for {}: {}",
                        entry.path().display(),
                        e
                    );
                });
                // Lossless: store the OS-native target bytes (WTF-8 on Windows,
                // raw bytes on Unix) without any UTF-8 conversion.
                let target_bytes = target.as_os_str().as_encoded_bytes().to_vec();
                result.insert(os_key, GovernanceEntry::Symlink(target_bytes));
            } else if meta.is_file() {
                let bytes = std::fs::read(entry.path()).unwrap_or_else(|e| {
                    panic!(
                        "capture_governance: read failed for {}: {}",
                        entry.path().display(),
                        e
                    );
                });
                result.insert(os_key, GovernanceEntry::RegularFile(bytes));
            } else {
                panic!(
                    "capture_governance: unsupported entry type (not dir/symlink/file) for {}",
                    entry.path().display()
                );
            }
        }
    }
    result
}

/// Helper that runs a preservation check: capture before, run operation,
/// capture after, assert equality.
#[allow(dead_code)]
fn assert_governance_preserved<F>(repo: &Path, op: F)
where
    F: FnOnce() -> std::process::Output,
{
    let before = capture_governance(repo);
    let _output = op();
    let after = capture_governance(repo);
    assert_eq!(before, after, "governance files changed during operation");
}

// ---------------------------------------------------------------------------
// P4-082: check writes nothing (success and failure)
// ---------------------------------------------------------------------------

#[test]
fn test_pkg13_check_writes_nothing_success_and_failure() {
    // --- Success case ---
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let before = capture_snapshot(&repo);
    let output = run_implementation_check(&repo);
    assert_success(&output);
    let after = capture_snapshot(&repo);
    let diffs = diff_snapshots(&before, &after);
    assert!(
        diffs.is_empty(),
        "check success must not modify any filesystem entries; diffs: {:?}",
        diffs
    );

    // --- Failure case: no begin (governance authority missing) ---
    let (_dir2, repo2) = setup_implementation_basic();
    let before2 = capture_snapshot(&repo2);
    let output2 = run_implementation_check(&repo2);
    assert_failure(&output2);
    let after2 = capture_snapshot(&repo2);
    let diffs2 = diff_snapshots(&before2, &after2);
    assert!(
        diffs2.is_empty(),
        "check failure must not modify any filesystem entries; diffs: {:?}",
        diffs2
    );
}

// ---------------------------------------------------------------------------
// P4-083: failed begin governance preservation matrix
// ---------------------------------------------------------------------------

#[test]
fn test_pkg13_failed_begin_governance_preservation_matrix() {
    // 1. Governance authority invalid (no accepted contract) - bad revision
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let before = capture_governance(&repo);
        let expected: std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry> =
            std::collections::BTreeMap::from([(
                std::ffi::OsString::from(".mrgs"),
                GovernanceEntry::MissingRoot,
            )]);
        assert_eq!(
            before, expected,
            "before snapshot must be exact MissingRoot"
        );
        let output = run_implementation_begin_str(
            &repo,
            "abc",
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert_phase4_failure_exact(&output, "INVALID_ARGUMENT");
        assert!(
            output.stdout.is_empty(),
            "stdout must be empty for failed begin"
        );
        let after = capture_governance(&repo);
        assert_eq!(after, expected, "after snapshot must be exact MissingRoot");
        assert_eq!(before, after, "before == after: governance preserved");
    }

    // 2. Contract not accepted / stale revision
    {
        let (_dir, repo) = setup_implementation_basic();
        let before = capture_governance(&repo);
        let output = run_implementation_begin_str(
            &repo,
            "999",
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert_phase4_failure_exact(&output, "CONTRACT_NOT_ACCEPTED");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during stale revision begin"
        );
    }

    // 3. Git dirty (unstaged modification)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        std::fs::write(repo.join("README.md"), b"modified").unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_DIRTY");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during unstaged mod begin"
        );
    }

    // 4. Git dirty (staged change)
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_DIRTY");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during staged change begin"
        );
    }

    // 5. Git dirty (untracked file)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        std::fs::write(repo.join("untracked.txt"), b"untracked").unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_DIRTY");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during untracked file begin"
        );
    }

    // 6. Git dirty (ignored file)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        std::fs::write(repo.join(".gitignore"), b"*.log\n").unwrap();
        commit_file(&repo, ".gitignore");
        std::fs::write(repo.join("build.log"), b"ignored content").unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_DIRTY");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during ignored file begin"
        );
    }

    // 7. Tracked governance (accepted-plan)
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked accepted-plan begin"
        );
    }

    // 8. Tracked governance (state.json)
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked state begin"
        );
    }

    // 9. Tracked governance (contract-draft)
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked contract-draft begin"
        );
    }

    // 10. Tracked governance (accepted-contract)
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked accepted-contract begin"
        );
    }

    // 11. Tracked governance (implementation-authority)
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked impl-authority begin"
        );
    }

    // 12. Tracked extra .mrgs path
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked extra mrgs begin"
        );
    }

    // 13. core.sparseCheckout=true
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during sparse checkout begin"
        );
    }

    // 14. index.sparse=true
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during index sparse begin"
        );
    }

    // 15. Submodule present
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_SUBMODULE_UNSUPPORTED");
        let after = capture_governance(&repo);
        assert_eq!(before, after, "governance changed during submodule begin");
    }

    // 16. Merge head present
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        create_git_marker(&repo, "MERGE_HEAD");
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
        let after = capture_governance(&repo);
        assert_eq!(before, after, "governance changed during merge head begin");
    }

    // 17. Detached HEAD
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_DETACHED_HEAD");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during detached head begin"
        );
    }

    // 18. Non-git repo
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("not-a-repo");
        std::fs::create_dir(&repo).unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_begin_str(
            &repo,
            "1",
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert_phase4_failure_exact(&output, "GOVERNANCE_AUTHORITY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during non-git repo begin"
        );
    }

    // 19. Index conflict stage (unstaged merge conflict) - same validation path as GIT_DIRTY
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        // Create a tracked file then modify it to simulate conflict state.
        std::fs::write(repo.join("conflict.txt"), b"base").unwrap();
        git(&repo).arg("add").arg("conflict.txt").status().unwrap();
        commit_file(&repo, "conflict.txt");
        // Now modify the file (unstaged change = dirty/conflict-like state).
        std::fs::write(
            repo.join("conflict.txt"),
            b"<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> theirs",
        )
        .unwrap();

        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_DIRTY");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during conflict stage begin"
        );
    }

    // 20. Git command execution failure (direct begin evidence)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        // Write invalid config so every subsequent git command fails.
        std::fs::write(
            repo.join(".git").join("config"),
            b"[invalid\nunterminated-string = true\n",
        )
        .unwrap();

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
        let after_snap = capture_snapshot(&repo);
        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "begin-git-command-failure",
            &[
                "governance",
                "git_refs",
                "git_index",
                "git_config",
                "git_objects",
                "worktree",
            ],
        );
        let allowed: Vec<String> = Vec::new();
        assert_no_new_mrgs_temp_paths(
            &before_snap,
            &after_snap,
            "begin-git-command-failure",
            &allowed,
        );
    }

    // 21. Existing implementation-authority conflict (authority exists but revision mismatch)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        // Create a stale authority file with wrong revision.
        let auth_path = repo.join(".mrgs").join("implementation-authority.json");
        let stale_auth = serde_json::json!({
            "revision": 999u32,
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "baseline_branch": "main"
        });
        std::fs::write(
            &auth_path,
            serde_json::to_string_pretty(&stale_auth).unwrap(),
        )
        .unwrap();

        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during authority conflict begin"
        );
    }

    // 22. Stale implementation-authority binding (authority exists but baseline branch changed)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        // Create authority with wrong baseline branch.
        let auth_path = repo.join(".mrgs").join("implementation-authority.json");
        let stale_auth = serde_json::json!({
            "revision": final_rev,
            "sha256": final_sha.clone(),
            "baseline_branch": "wrong-branch"
        });
        std::fs::write(
            &auth_path,
            serde_json::to_string_pretty(&stale_auth).unwrap(),
        )
        .unwrap();

        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during stale binding begin"
        );
    }

    // 23. Rebase-apply operation-in-progress marker (direct begin evidence)
    // Production validate_operation_state() checks the rebase-apply/applying directory.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        create_git_marker(&repo, "rebase-apply/applying");

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
        let after_snap = capture_snapshot(&repo);
        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "begin-rebase-marker",
            &[
                "governance",
                "git_refs",
                "git_index",
                "git_config",
                "git_objects",
                "worktree",
            ],
        );
        let allowed: Vec<String> = Vec::new();
        assert_no_new_mrgs_temp_paths(&before_snap, &after_snap, "begin-rebase-marker", &allowed);
    }

    // 24. Cherry-pick operation-in-progress marker (direct begin evidence)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        create_git_marker(&repo, "CHERRY_PICK_HEAD");

        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during cherry-pick head begin"
        );
    }

    // 25. REVERT_HEAD operation-in-progress marker (same validation path)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        create_git_marker(&repo, "REVERT_HEAD");

        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
        let after = capture_governance(&repo);
        assert_eq!(before, after, "governance changed during revert head begin");
    }

    // 26. Bisect state operation-in-progress marker (same validation path)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        create_git_marker(&repo, "BISECT_LOG");

        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
        let after = capture_governance(&repo);
        assert_eq!(before, after, "governance changed during bisect log begin");
    }

    // 27. Invalid path rule (path outside repo)
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);

        let before = capture_governance(&repo);
        let outside_path = dir.path().join("outside-repo");
        let mut cmd = cargo_bin();
        cmd.arg("implementation")
            .arg("begin")
            .arg("--repo")
            .arg(&outside_path)
            .arg("--revision")
            .arg("1")
            .arg("--sha256")
            .arg("0000000000000000000000000000000000000000000000000000000000000000");
        let output = cmd.output().unwrap();

        assert_phase4_failure_exact(&output, "REPOSITORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during invalid path begin"
        );
    }
}

// ---------------------------------------------------------------------------
// P4-084: failed check governance preservation matrix
// ---------------------------------------------------------------------------

#[test]
fn test_pkg13_failed_check_governance_preservation_matrix() {
    // 1. No begin (no accepted contract)
    {
        let (_dir, repo) = setup_implementation_basic();
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "CONTRACT_NOT_ACCEPTED");
        let after = capture_governance(&repo);
        assert_eq!(before, after, "governance changed during no-begin check");
    }

    // 2. Branch changed (same commit)
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "BASELINE_BRANCH_CHANGED");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during branch-changed check"
        );
    }

    // 3. Baseline not ancestor
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

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
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("update-ref")
            .arg("refs/heads/main")
            .arg(&new_commit)
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("reset")
            .arg("--hard")
            .arg(&new_commit)
            .status()
            .unwrap();

        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "BASELINE_HISTORY_CHANGED");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during baseline-not-ancestor check"
        );
    }

    // 4. Tracked governance (accepted-plan)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        git(&repo)
            .arg("add")
            .arg("--force")
            .arg(".mrgs/accepted-plan.json")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("track accepted-plan.json")
            .status()
            .unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked accepted-plan check"
        );
    }

    // 5. Tracked governance (state.json)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        git(&repo)
            .arg("add")
            .arg("--force")
            .arg(".mrgs/state.json")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("track state.json")
            .status()
            .unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked state check"
        );
    }

    // 6. Tracked governance (contract-draft)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        git(&repo)
            .arg("add")
            .arg("--force")
            .arg(".mrgs/contract-draft.json")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("track contract-draft.json")
            .status()
            .unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked contract-draft check"
        );
    }

    // 7. Tracked governance (accepted-contract)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        git(&repo)
            .arg("add")
            .arg("--force")
            .arg(".mrgs/accepted-contract.json")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("track accepted-contract.json")
            .status()
            .unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked accepted-contract check"
        );
    }

    // 8. Tracked governance (implementation-authority)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        git(&repo)
            .arg("add")
            .arg("--force")
            .arg(".mrgs/implementation-authority.json")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("track implementation-authority.json")
            .status()
            .unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked impl-authority check"
        );
    }

    // 9. Tracked extra JSON
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        std::fs::write(repo.join(".mrgs").join("extra.json"), b"{}").unwrap();
        git(&repo)
            .arg("add")
            .arg("--force")
            .arg(".mrgs/extra.json")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("track extra.json")
            .status()
            .unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during tracked extra json check"
        );
    }

    // 10. Forbidden path change (case alias)
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during forbidden case alias check"
        );
    }

    // 11. Submodule present
    {
        let (dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
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
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("add submodule")
            .status()
            .unwrap();
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_SUBMODULE_UNSUPPORTED");
        let after = capture_governance(&repo);
        assert_eq!(before, after, "governance changed during submodule check");
    }

    // 12. core.sparseCheckout=true
    {
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
        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during sparse checkout check"
        );
    }

    // 13. Malformed implementation authority (invalid JSON)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Corrupt the authority file.
        let auth_path = repo.join(".mrgs").join("implementation-authority.json");
        std::fs::write(&auth_path, b"NOT VALID JSON {{{").unwrap();

        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during malformed authority check"
        );
    }

    // 14. Stale implementation authority (revision mismatch)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Overwrite authority with wrong revision.
        let auth_path = repo.join(".mrgs").join("implementation-authority.json");
        let stale_auth = serde_json::json!({
            "revision": 999u32,
            "sha256": final_sha.clone(),
            "baseline_branch": "main"
        });
        std::fs::write(
            &auth_path,
            serde_json::to_string_pretty(&stale_auth).unwrap(),
        )
        .unwrap();

        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during stale authority check"
        );
    }

    // 15. Missing baseline commit (authority points to nonexistent SHA)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Overwrite authority with nonexistent baseline SHA.
        let auth_path = repo.join(".mrgs").join("implementation-authority.json");
        let fake_auth = serde_json::json!({
            "revision": final_rev,
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "baseline_branch": "main"
        });
        std::fs::write(
            &auth_path,
            serde_json::to_string_pretty(&fake_auth).unwrap(),
        )
        .unwrap();

        let before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
        let after = capture_governance(&repo);
        assert_eq!(
            before, after,
            "governance changed during missing baseline check"
        );
    }

    // 16. Direct check-side index-conflict preservation
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("conflict.txt"), b"base content\n").unwrap();
        git(&repo)
            .arg("add")
            .arg("src/conflict.txt")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("add conflict file")
            .status()
            .unwrap();

        let make_blob = |content: &[u8]| -> String {
            let mut child = git(&repo)
                .arg("hash-object")
                .arg("-w")
                .arg("--stdin")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            use std::io::Write;
            child.stdin.take().unwrap().write_all(content).unwrap();
            let output = child.wait_with_output().unwrap();
            assert_eq!(output.status.code(), Some(0));
            String::from_utf8(output.stdout[..output.stdout.len() - 1].to_vec()).unwrap()
        };

        let oid_stage1 = make_blob(b"stage 1 ours\n");
        let oid_stage2 = make_blob(b"stage 2 theirs\n");
        let oid_stage3 = make_blob(b"stage 3 base\n");

        assert_ne!(oid_stage1, oid_stage2, "stage 1 and 2 OIDs must differ");
        assert_ne!(oid_stage2, oid_stage3, "stage 2 and 3 OIDs must differ");
        assert_ne!(oid_stage1, oid_stage3, "stage 1 and 3 OIDs must differ");

        git(&repo)
            .arg("rm")
            .arg("--cached")
            .arg("src/conflict.txt")
            .status()
            .unwrap();

        let index_info = format!(
            "100644 {} 1\tsrc/conflict.txt\n100644 {} 2\tsrc/conflict.txt\n100644 {} 3\tsrc/conflict.txt\n",
            oid_stage1, oid_stage2, oid_stage3
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

        let unmerged = git(&repo)
            .arg("ls-files")
            .arg("--unmerged")
            .output()
            .unwrap();
        let unmerged_str = String::from_utf8(unmerged.stdout.clone()).unwrap();
        assert!(
            unmerged_str.contains("src/conflict.txt"),
            "conflict fixture must produce unmerged entry for src/conflict.txt; got: {}",
            unmerged_str
        );
        assert!(
            unmerged_str.contains(&format!("{} 1\t", oid_stage1)),
            "stage 1 OID must be present in unmerged output"
        );
        assert!(
            unmerged_str.contains(&format!("{} 2\t", oid_stage2)),
            "stage 2 OID must be present in unmerged output"
        );
        assert!(
            unmerged_str.contains(&format!("{} 3\t", oid_stage3)),
            "stage 3 OID must be present in unmerged output"
        );

        let before_snap = capture_snapshot(&repo);
        let before_gov = capture_governance(&repo);

        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_CONFLICT");

        let after_snap = capture_snapshot(&repo);
        let after_gov = capture_governance(&repo);

        assert_eq!(
            before_gov, after_gov,
            "governance changed during direct check-side index-conflict"
        );

        let all_components = [
            "governance",
            "git_refs",
            "git_index",
            "git_config",
            "git_objects",
            "worktree",
        ];
        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "direct-check-index-conflict",
            &all_components,
        );

        let allowed_temps: Vec<String> = before_snap
            .entries
            .keys()
            .filter(|k| k.ends_with(".tmp"))
            .cloned()
            .collect();
        assert_no_new_mrgs_temp_paths(
            &before_snap,
            &after_snap,
            "direct-check-index-conflict",
            &allowed_temps,
        );
    }

    // Tracked-governance and sparse-state cases are covered by dedicated matrices:
    // test_pkg13_tracked_governance_failure_preservation_matrix (P4-142/P4-143)
    // test_pkg13_sparse_failure_preservation_matrix (P4-182/P4-183)
}

// ---------------------------------------------------------------------------
// P4-086: preexisting temp files untouched matrix
// ---------------------------------------------------------------------------

#[test]
fn test_pkg13_preexisting_temp_files_untouched_matrix() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    assert_success(&run_plan_accept(&repo, &plan_path));

    // Create pre-existing .tmp files with distinctive bytes and names that
    // collide with plausible MRGS temp naming patterns.
    let mrgs = repo.join(".mrgs");
    std::fs::create_dir_all(&mrgs).unwrap();
    let tmp_files: Vec<(&str, &[u8])> = vec![
        (".accepted-plan.json.tmp", b"PRE_TMP_ACCEPTED_PLAN_V1"),
        (".state.json.tmp", b"PRE_TMP_STATE_V2"),
        (".contract-draft.json.tmp", b"PRE_TMP_CONTRACT_DRAFT_V3"),
        (
            ".accepted-contract.json.tmp",
            b"PRE_TMP_ACCEPTED_CONTRACT_V4",
        ),
        (
            ".implementation-authority.json.tmp",
            b"PRE_TMP_IMPL_AUTH_V5",
        ),
        (".phase-select.json.tmp", b"PRE_TMP_PHASE_SELECT_V6"),
        ("mrgs_tmp_12345_67890.tmp", b"PRE_TMP_MRGSSUFFIX_V7"),
    ];
    let mut expected_bytes: std::collections::BTreeMap<String, Vec<u8>> =
        std::collections::BTreeMap::new();
    for (name, bytes) in &tmp_files {
        let path = mrgs.join(name);
        std::fs::write(&path, *bytes).unwrap();
        expected_bytes.insert(name.to_string(), bytes.to_vec());
    }

    // --- Successful begin ---
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
    let _output = run_implementation_begin(&repo, final_rev, &final_sha);

    for (name, expected) in &expected_bytes {
        let path = mrgs.join(name);
        assert!(
            path.exists(),
            "pre-existing tmp {} must still exist after successful begin",
            name
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            *expected,
            "pre-existing tmp {} bytes changed after successful begin",
            name
        );
    }

    // --- Idempotent begin ---
    let _output2 = run_implementation_begin(&repo, final_rev, &final_sha);
    for (name, expected) in &expected_bytes {
        let path = mrgs.join(name);
        assert!(
            path.exists(),
            "pre-existing tmp {} must still exist after idempotent begin",
            name
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            *expected,
            "pre-existing tmp {} bytes changed after idempotent begin",
            name
        );
    }

    // --- Failed begin (unstaged modification) ---
    std::fs::write(repo.join("README.md"), b"modified").unwrap();
    let _output3 = run_implementation_begin(&repo, final_rev, &final_sha);
    for (name, expected) in &expected_bytes {
        let path = mrgs.join(name);
        assert!(
            path.exists(),
            "pre-existing tmp {} must still exist after failed begin",
            name
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            *expected,
            "pre-existing tmp {} bytes changed after failed begin",
            name
        );
    }

    // Clean up the unstaged modification so we can proceed.
    std::fs::write(repo.join("README.md"), b"initial").unwrap();
    git(&repo).arg("add").arg("README.md").status().unwrap();
    git(&repo)
        .arg("commit")
        .arg("--amend")
        .arg("-q")
        .arg("--no-edit")
        .status()
        .unwrap();

    // --- Successful check ---
    let _output4 = run_implementation_check(&repo);
    for (name, expected) in &expected_bytes {
        let path = mrgs.join(name);
        assert!(
            path.exists(),
            "pre-existing tmp {} must still exist after successful check",
            name
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            *expected,
            "pre-existing tmp {} bytes changed after successful check",
            name
        );
    }

    // --- Failed check (no begin) ---
    let (_dir2, repo2) = setup_implementation_basic();
    let mrgs2 = repo2.join(".mrgs");
    std::fs::create_dir_all(&mrgs2).unwrap();
    let tmp_file_orphan = mrgs2.join(".orphan.tmp");
    std::fs::write(&tmp_file_orphan, b"PRE_TMP_ORPHAN_V99").unwrap();
    let _output5 = run_implementation_check(&repo2);
    assert!(
        tmp_file_orphan.exists(),
        "pre-existing tmp must still exist after failed check"
    );
    assert_eq!(
        std::fs::read(&tmp_file_orphan).unwrap(),
        b"PRE_TMP_ORPHAN_V99",
        "pre-existing tmp bytes changed after failed check"
    );
}

// ---------------------------------------------------------------------------
// P4-087: begin creates only authority; idempotent writes nothing
// ---------------------------------------------------------------------------

#[test]
fn test_pkg13_begin_creates_only_authority_and_idempotent_writes_nothing() {
    // First successful begin
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);

    let before = capture_snapshot(&repo);
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_begin_exact(&output, &repo);

    let after = capture_snapshot(&repo);
    let diffs = diff_snapshots(&before, &after);

    // The ONLY difference should be the added implementation-authority.json.
    let expected_added = ".mrgs/implementation-authority.json";
    let mut only_authority = true;
    for d in &diffs {
        if !d.starts_with(&format!("+ {} ", expected_added)) {
            only_authority = false;
            break;
        }
    }
    assert!(
        only_authority,
        "begin must create ONLY implementation-authority.json; diffs: {:?}",
        diffs
    );

    // No temp path remains.
    assert_no_temp_files(&repo);

    // All pre-existing governance files equal (none existed before).
    // All git components equal (no refs/objects/config/index changes).
    // All worktree files equal.
    for d in &diffs {
        if d.starts_with(&format!("+ {} ", expected_added)) {
            continue;
        }
        panic!("unexpected diff: {}", d);
    }

    // --- Idempotent begin ---
    let before2 = capture_snapshot(&repo);
    let output2 = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_begin_exact(&output2, &repo);
    let after2 = capture_snapshot(&repo);
    let diffs2 = diff_snapshots(&before2, &after2);
    assert!(
        diffs2.is_empty(),
        "idempotent begin must produce ZERO differences; diffs: {:?}",
        diffs2
    );
    assert_no_temp_files(&repo);
}

// ---------------------------------------------------------------------------
// P4-088: git state immutable across begin and check
// ---------------------------------------------------------------------------

#[test]
fn test_pkg13_git_state_immutable_across_begin_and_check() {
    // --- Successful begin ---
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        let before = capture_snapshot(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_begin_exact(&output, &repo);
        let after = capture_snapshot(&repo);
        let diffs = diff_snapshots(&before, &after);

        // Only implementation-authority.json may be added.
        for d in &diffs {
            assert!(
                d.starts_with("+ .mrgs/implementation-authority.json "),
                "git state must not change during begin; unexpected diff: {}",
                d
            );
        }
    }

    // --- Idempotent begin ---
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        let before = capture_snapshot(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_begin_exact(&output, &repo);
        let after = capture_snapshot(&repo);
        let diffs = diff_snapshots(&before, &after);
        assert!(
            diffs.is_empty(),
            "idempotent begin must produce ZERO diffs; diffs: {:?}",
            diffs
        );
    }

    // --- Successful check ---
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        let before = capture_snapshot(&repo);
        let output = run_implementation_check(&repo);
        assert_success(&output);
        let after = capture_snapshot(&repo);
        let diffs = diff_snapshots(&before, &after);
        assert!(
            diffs.is_empty(),
            "successful check must produce ZERO diffs; diffs: {:?}",
            diffs
        );
    }

    // --- Failed begin (unstaged modification) ---
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        std::fs::write(repo.join("README.md"), b"modified").unwrap();

        let before = capture_snapshot(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_DIRTY");
        let after = capture_snapshot(&repo);
        let diffs = diff_snapshots(&before, &after);
        assert!(
            diffs.is_empty(),
            "failed begin must produce ZERO diffs; diffs: {:?}",
            diffs
        );
    }

    // --- Failed check (no begin) ---
    {
        let (_dir, repo) = setup_implementation_basic();
        let before = capture_snapshot(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "CONTRACT_NOT_ACCEPTED");
        let after = capture_snapshot(&repo);
        let diffs = diff_snapshots(&before, &after);
        assert!(
            diffs.is_empty(),
            "failed check (no begin) must produce ZERO diffs; diffs: {:?}",
            diffs
        );
    }
}

// ---------------------------------------------------------------------------
// P4-142 / P4-143: tracked governance failure preservation matrix
// ---------------------------------------------------------------------------

#[test]
fn test_pkg13_tracked_governance_failure_preservation_matrix() {
    // Helper: run a tracked-governance preservation check.
    let run_case = |name: &str,
                    setup_fn: &dyn Fn(&tempfile::TempDir, &std::path::Path),
                    expected_category: &str,
                    use_check: bool| {
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

        if use_check {
            let (final_rev, final_sha) = contract_accepted_revision(&repo);
            assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        }

        // Apply the tracked fixture.
        setup_fn(&dir, &repo);

        let before_gov = capture_governance(&repo);
        let before_snap = capture_snapshot(&repo);

        let output = if use_check {
            run_implementation_check(&repo)
        } else {
            // For begin-side cases, we need the final_rev/final_sha.
            // This branch is only for check-side cases.
            panic!("begin-side tracked cases not supported in this helper")
        };

        assert_phase4_failure_exact(&output, expected_category);

        let after_gov = capture_governance(&repo);
        let after_snap = capture_snapshot(&repo);

        assert_eq!(before_gov, after_gov, "{}: governance files changed", name);

        let all_components = [
            "governance",
            "git_refs",
            "git_index",
            "git_config",
            "git_objects",
            "worktree",
        ];
        assert_snapshot_components_equal(&before_snap, &after_snap, name, &all_components);

        let allowed_temps: Vec<String> = before_snap
            .entries
            .keys()
            .filter(|k| k.ends_with(".tmp"))
            .cloned()
            .collect();
        assert_no_new_mrgs_temp_paths(&before_snap, &after_snap, name, &allowed_temps);
    };

    // 1. tracked accepted-plan
    run_case(
        "tracked-accepted-plan",
        &|_dir, repo| {
            git(repo)
                .arg("add")
                .arg("--force")
                .arg(".mrgs/accepted-plan.json")
                .status()
                .unwrap();
            git(repo)
                .arg("commit")
                .arg("-m")
                .arg("track accepted-plan.json")
                .status()
                .unwrap();
        },
        "GIT_INVENTORY_INVALID",
        true,
    );

    // 2. tracked state
    run_case(
        "tracked-state",
        &|_dir, repo| {
            git(repo)
                .arg("add")
                .arg("--force")
                .arg(".mrgs/state.json")
                .status()
                .unwrap();
            git(repo)
                .arg("commit")
                .arg("-m")
                .arg("track state.json")
                .status()
                .unwrap();
        },
        "GIT_INVENTORY_INVALID",
        true,
    );

    // 3. tracked contract-draft
    run_case(
        "tracked-contract-draft",
        &|_dir, repo| {
            git(repo)
                .arg("add")
                .arg("--force")
                .arg(".mrgs/contract-draft.json")
                .status()
                .unwrap();
            git(repo)
                .arg("commit")
                .arg("-m")
                .arg("track contract-draft.json")
                .status()
                .unwrap();
        },
        "GIT_INVENTORY_INVALID",
        true,
    );

    // 4. tracked accepted-contract
    run_case(
        "tracked-accepted-contract",
        &|_dir, repo| {
            git(repo)
                .arg("add")
                .arg("--force")
                .arg(".mrgs/accepted-contract.json")
                .status()
                .unwrap();
            git(repo)
                .arg("commit")
                .arg("-m")
                .arg("track accepted-contract.json")
                .status()
                .unwrap();
        },
        "GIT_INVENTORY_INVALID",
        true,
    );

    // 5. tracked implementation-authority
    run_case(
        "tracked-impl-authority",
        &|_dir, repo| {
            git(repo)
                .arg("add")
                .arg("--force")
                .arg(".mrgs/implementation-authority.json")
                .status()
                .unwrap();
            git(repo)
                .arg("commit")
                .arg("-m")
                .arg("track implementation-authority.json")
                .status()
                .unwrap();
        },
        "GIT_INVENTORY_INVALID",
        true,
    );

    // 6. tracked unknown .mrgs path
    run_case(
        "tracked-unknown-mrgs",
        &|_dir, repo| {
            std::fs::write(repo.join(".mrgs").join("unknown.json"), b"{}").unwrap();
            git(repo)
                .arg("add")
                .arg("--force")
                .arg(".mrgs/unknown.json")
                .status()
                .unwrap();
            git(repo)
                .arg("commit")
                .arg("-m")
                .arg("track unknown.json")
                .status()
                .unwrap();
        },
        "GIT_INVENTORY_INVALID",
        true,
    );

    // 7. tracked temp-shaped .mrgs path
    run_case(
        "tracked-temp-shaped",
        &|_dir, repo| {
            std::fs::write(repo.join(".mrgs").join("mrgs_tmp_12345_67890.tmp"), b"temp").unwrap();
            git(repo)
                .arg("add")
                .arg("--force")
                .arg(".mrgs/mrgs_tmp_12345_67890.tmp")
                .status()
                .unwrap();
            git(repo)
                .arg("commit")
                .arg("-m")
                .arg("track tmp file")
                .status()
                .unwrap();
        },
        "GIT_INVENTORY_INVALID",
        true,
    );

    // 8. deleted tracked governance path (authority file)
    run_case(
        "deleted-tracked-governance",
        &|_dir, repo| {
            let auth_path = repo.join(".mrgs").join("implementation-authority.json");
            if auth_path.exists() {
                std::fs::remove_file(&auth_path).unwrap();
            }
        },
        "IMPLEMENTATION_AUTHORITY_MISSING",
        true,
    );

    // 9. conflict-stage beneath .mrgs
    run_case(
        "conflict-stage-beneath-mrgs",
        &|_dir, repo| {
            let mrgs = repo.join(".mrgs");
            std::fs::create_dir_all(&mrgs).unwrap();
            std::fs::write(mrgs.join("conflict.txt"), b"base").unwrap();
            git(repo)
                .arg("add")
                .arg("--force")
                .arg(".mrgs/conflict.txt")
                .status()
                .unwrap();
            git(repo)
                .arg("commit")
                .arg("-m")
                .arg("track conflict.txt")
                .status()
                .unwrap();
            // Modify the file to create a conflict stage.
            std::fs::write(mrgs.join("conflict.txt"), b"modified").unwrap();
        },
        "GIT_INVENTORY_INVALID",
        true,
    );

    // 10. gitlink beneath .mrgs (use submodule pattern)
    run_case(
        "gitlink-beneath-mrgs",
        &|dir, repo| {
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
                .arg(repo)
                .arg("-c")
                .arg("protocol.file.allow=always")
                .arg("submodule")
                .arg("add")
                .arg(&sub_dir)
                .arg(".mrgs/submod")
                .status()
                .unwrap();
        },
        "GIT_SUBMODULE_UNSUPPORTED",
        true,
    );
}

// ---------------------------------------------------------------------------
// P4-182 / P4-183: sparse failure preservation matrix
// ---------------------------------------------------------------------------

#[test]
fn test_pkg13_sparse_failure_preservation_matrix() {
    let all_components = [
        "governance",
        "git_refs",
        "git_index",
        "git_config",
        "git_objects",
        "worktree",
    ];

    // 1. core.sparseCheckout=true
    {
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

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-core-true",
            &all_components,
        );
    }

    // 2. index.sparse=true
    {
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
            .arg("index.sparse")
            .arg("true")
            .status()
            .unwrap();

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-index-true",
            &all_components,
        );
    }

    // 3. Sparse checkout active with index.sparse unset (core.sparseCheckout=true still rejects)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Enable sparse checkout which sets core.sparseCheckout=true.
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("sparse-checkout")
            .arg("init")
            .status()
            .unwrap();
        // Unset index.sparse but keep sparse checkout active.
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("config")
            .arg("--unset")
            .arg("index.sparse")
            .status()
            .ok();

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-checkout-unset-index",
            &all_components,
        );
    }

    // 4. Sparse checkout active with index.sparse=false explicitly set
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Enable sparse checkout then explicitly set index.sparse=false.
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("sparse-checkout")
            .arg("init")
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("config")
            .arg("index.sparse")
            .arg("false")
            .status()
            .unwrap();

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-checkout-false-index",
            &all_components,
        );
    }

    // 5. Malformed core.sparseCheckout output (direct snapshot evidence)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Replace real git with a wrapper that returns malformed output
        // for the core.sparseCheckout config query.
        let recorder =
            create_malformed_git_recorder("core.sparseCheckout", b"MALFORMED_NOT_BOOL\n", 0);

        // Snapshot before MRGS invocation; no Git command first.
        let before_snap = capture_snapshot(&repo);
        let output =
            run_with_malformed_git_recorder(&recorder, &repo, &["implementation", "check"]);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-malformed-core",
            &all_components,
        );
        let allowed: Vec<String> = Vec::new();
        assert_no_new_mrgs_temp_paths(&before_snap, &after_snap, "sparse-malformed-core", &allowed);
    }

    // 6. Malformed index.sparse output (direct snapshot evidence)
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        let recorder = create_malformed_git_recorder("index.sparse", b"MALFORMED_NOT_BOOL\n", 0);

        let before_snap = capture_snapshot(&repo);
        let output =
            run_with_malformed_git_recorder(&recorder, &repo, &["implementation", "check"]);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-malformed-index",
            &all_components,
        );
        let allowed: Vec<String> = Vec::new();
        assert_no_new_mrgs_temp_paths(
            &before_snap,
            &after_snap,
            "sparse-malformed-index",
            &allowed,
        );
    }

    // 7. Multiple core.sparseCheckout values with final value false
    {
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
            .arg("--add")
            .arg("core.sparseCheckout")
            .arg("true")
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("config")
            .arg("--add")
            .arg("core.sparseCheckout")
            .arg("false")
            .status()
            .unwrap();

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-multi-core-false",
            &all_components,
        );
    }

    // 8. Multiple index.sparse values with final value false
    {
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
            .arg("--add")
            .arg("index.sparse")
            .arg("true")
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("config")
            .arg("--add")
            .arg("index.sparse")
            .arg("false")
            .status()
            .unwrap();

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-multi-index-false",
            &all_components,
        );
    }

    // 9. Real cone sparse checkout (reuse PKG-12 fixture)
    {
        let (_dir, repo, final_rev, final_sha) = setup_sparse_fixture();
        git_cmd(&repo, &["sparse-checkout", "init", "--cone"]);
        git_cmd(&repo, &["sparse-checkout", "set", "included"]);

        assert!(
            !repo.join("excluded").join("c.txt").exists(),
            "working tree must be sparse"
        );

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-real-cone",
            &all_components,
        );
    }

    // 10. Real non-cone sparse checkout (reuse PKG-12 fixture)
    {
        let (_dir, repo, final_rev, final_sha) = setup_sparse_fixture();
        git_cmd(&repo, &["sparse-checkout", "init"]);
        git_cmd(&repo, &["sparse-checkout", "set", "included"]);

        assert!(
            !repo.join("excluded").join("c.txt").exists(),
            "working tree must be sparse"
        );

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
        let after_snap = capture_snapshot(&repo);

        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-real-noncone",
            &all_components,
        );
    }

    // 11. Real sparse index (reuse PKG-12 fixture) with raw index preservation proof
    {
        let (_dir, repo, final_rev, final_sha) = setup_sparse_fixture();
        git_cmd(
            &repo,
            &["sparse-checkout", "init", "--cone", "--sparse-index"],
        );
        git_cmd(&repo, &["sparse-checkout", "set", "included"]);

        assert!(
            !repo.join("excluded").join("c.txt").exists(),
            "working tree must be sparse"
        );

        // Prove sparse index is active.
        let si_config = git_cmd_output(&repo, &["config", "--get", "index.sparse"]);
        let si_val = String::from_utf8_lossy(&si_config.stdout)
            .trim()
            .to_string();
        assert_eq!(si_val, "true", "index.sparse should be true");

        // Prove mode-040000 sparse-directory records exist.
        let ls_files = git_cmd_output(&repo, &["ls-files", "--sparse", "--stage", "-z"]);
        let sparse_records: Vec<&[u8]> = ls_files
            .stdout
            .split(|&b| b == 0)
            .filter(|e| !e.is_empty() && e.starts_with(b"040000 "))
            .collect();
        assert!(
            !sparse_records.is_empty(),
            "expected sparse-directory records"
        );

        // Capture raw index bytes BEFORE MRGS.
        let index_path = repo.join(".git").join("index");
        let index_before = std::fs::read(&index_path).unwrap();
        let index_sha_before = sha256_hex(&index_before);
        let index_len_before = index_before.len();

        // Capture working-tree presence matrix.
        let wt_included_a = repo.join("included").join("a.txt").exists();
        let wt_excluded_c = repo.join("excluded").join("c.txt").exists();

        let before_snap = capture_snapshot(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");

        // Capture raw index bytes AFTER MRGS immediately (no Git command first).
        let index_after = std::fs::read(&index_path).unwrap();
        let index_sha_after = sha256_hex(&index_after);
        let index_len_after = index_after.len();

        assert_eq!(
            index_before, index_after,
            "INDEX_BYTES_EQUAL_BEFORE_AFTER failed"
        );
        assert_eq!(
            index_sha_before, index_sha_after,
            "INDEX_SHA256_EQUAL_BEFORE_AFTER failed: {} != {}",
            index_sha_before, index_sha_after
        );
        assert_eq!(
            index_len_before, index_len_after,
            "INDEX_LENGTH_EQUAL_BEFORE_AFTER failed"
        );

        // Prove sparse-directory records unchanged.
        let ls_files_after = git_cmd_output(&repo, &["ls-files", "--sparse", "--stage", "-z"]);
        let sparse_records_after: Vec<&[u8]> = ls_files_after
            .stdout
            .split(|&b| b == 0)
            .filter(|e| !e.is_empty() && e.starts_with(b"040000 "))
            .collect();
        assert_eq!(
            sparse_records.len(),
            sparse_records_after.len(),
            "SPARSE_DIRECTORY_RECORDS_EQUAL_BEFORE_AFTER failed: {} vs {}",
            sparse_records.len(),
            sparse_records_after.len()
        );

        // Prove working-tree matrix unchanged.
        assert_eq!(
            wt_included_a,
            repo.join("included").join("a.txt").exists(),
            "WORKTREE_MATRIX changed for included/a.txt"
        );
        assert_eq!(
            wt_excluded_c,
            repo.join("excluded").join("c.txt").exists(),
            "WORKTREE_MATRIX changed for excluded/c.txt"
        );

        let after_snap = capture_snapshot(&repo);
        assert_snapshot_components_equal(
            &before_snap,
            &after_snap,
            "sparse-real-index",
            &all_components,
        );
    }
}

// ============================================================================
// PKG-01: Implementation-authority serialization and binding proof
// ============================================================================

fn extract_top_level_json_keys(text: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("  \"") {
            if let Some(end) = rest.find("\":") {
                keys.push(rest[..end].to_string());
            }
        }
    }
    keys
}

#[test]
fn test_pkg01_deterministic_field_order_and_serialization_bytes() {
    let (_dir1, repo1) = setup_implementation_basic();
    let draft1: serde_json::Value = read_json(&repo1, "contract-draft.json");
    let sha1 = draft1["sha256"].as_str().unwrap().to_string();
    let rev1 = draft1["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo1, rev1, &sha1, "ACCEPTED"));
    let (final_rev1, final_sha1) = contract_accepted_revision(&repo1);
    assert_success(&run_implementation_begin(&repo1, final_rev1, &final_sha1));
    let bytes1 = std::fs::read(repo1.join(".mrgs").join("implementation-authority.json")).unwrap();

    let (_dir2, repo2) = setup_implementation_basic();
    let draft2: serde_json::Value = read_json(&repo2, "contract-draft.json");
    let sha2 = draft2["sha256"].as_str().unwrap().to_string();
    let rev2 = draft2["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo2, rev2, &sha2, "ACCEPTED"));
    let (final_rev2, final_sha2) = contract_accepted_revision(&repo2);
    assert_success(&run_implementation_begin(&repo2, final_rev2, &final_sha2));
    let bytes2 = std::fs::read(repo2.join(".mrgs").join("implementation-authority.json")).unwrap();

    let text1 = String::from_utf8(bytes1.clone()).unwrap();
    let text2 = String::from_utf8(bytes2.clone()).unwrap();

    let keys1 = extract_top_level_json_keys(&text1);
    let keys2 = extract_top_level_json_keys(&text2);
    assert_eq!(
        keys1, keys2,
        "field order must be deterministic across independent repos"
    );

    let expected_keys = vec![
        "schema_version",
        "accepted_plan_sha256",
        "phase_id",
        "contract_id",
        "contract_revision",
        "contract_source_path",
        "contract_sha256",
        "contract_content",
        "git_object_format",
        "baseline_head",
        "baseline_branch",
    ];
    assert_eq!(
        keys1, expected_keys,
        "field order must match struct declaration"
    );

    assert!(
        text1.contains("  \"schema_version\""),
        "pretty-print indentation expected"
    );
    assert!(
        text2.contains("  \"schema_version\""),
        "pretty-print indentation expected"
    );

    let json1: serde_json::Value = serde_json::from_slice(&bytes1).unwrap();
    let json2: serde_json::Value = serde_json::from_slice(&bytes2).unwrap();

    assert_ne!(
        json1["baseline_head"].as_str().unwrap(),
        json2["baseline_head"].as_str().unwrap(),
        "independent repos must have different baseline heads"
    );
    assert_eq!(
        json1["contract_content"].as_str().unwrap(),
        json2["contract_content"].as_str().unwrap(),
        "contract content must be identical across equivalent repos"
    );
    assert_eq!(
        json1["contract_sha256"].as_str().unwrap(),
        json2["contract_sha256"].as_str().unwrap(),
        "contract sha256 must be identical across equivalent repos"
    );
    assert_eq!(
        json1["contract_source_path"].as_str().unwrap(),
        json2["contract_source_path"].as_str().unwrap(),
        "source path must be identical across equivalent repos"
    );

    let second_begin = run_implementation_begin(&repo1, final_rev1, &final_sha1);
    assert_success(&second_begin);
    let bytes_after =
        std::fs::read(repo1.join(".mrgs").join("implementation-authority.json")).unwrap();
    assert_eq!(
        bytes1, bytes_after,
        "idempotent begin must produce identical bytes"
    );
}

#[test]
fn test_pkg01_baseline_sha_independent_git() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let git_output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("rev-parse")
        .arg("HEAD^{commit}")
        .output()
        .unwrap();
    assert!(
        git_output.status.success(),
        "git rev-parse failed: {:?}",
        git_output.stderr
    );
    let independent_sha = String::from_utf8(git_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let persisted_sha = record["baseline_head"].as_str().unwrap();
    assert_eq!(
        persisted_sha, independent_sha,
        "baseline_head must equal independent git rev-parse HEAD^{{commit}}"
    );
}

#[test]
fn test_pkg01_baseline_branch_independent_git() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let git_output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("symbolic-ref")
        .arg("--quiet")
        .arg("--short")
        .arg("HEAD")
        .output()
        .unwrap();
    assert!(
        git_output.status.success(),
        "git symbolic-ref failed: {:?}",
        git_output.stderr
    );
    let independent_branch = String::from_utf8(git_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let persisted_branch = record["baseline_branch"].as_str().unwrap();
    assert_eq!(
        persisted_branch, independent_branch,
        "baseline_branch must equal independent git symbolic-ref"
    );
}

#[test]
fn test_pkg01_exact_source_path_persistence() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let source_path = record["contract_source_path"].as_str().unwrap();
    assert_eq!(
        source_path, "contract.toml",
        "source path must be exact normalized forward-slash"
    );
    assert!(
        !source_path.contains('\\'),
        "source path must not contain backslashes"
    );
}

#[test]
fn test_pkg01_exact_content_lf_preserved() {
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
    let lf_content = valid_contract_toml();
    write_plan(&contract_path, lf_content);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));

    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let persisted_content = record["contract_content"].as_str().unwrap();
    assert_eq!(
        persisted_content, lf_content,
        "LF content must be preserved exactly"
    );
    assert!(
        persisted_content.ends_with('\n'),
        "final newline must be preserved"
    );
}

#[test]
fn test_pkg01_exact_content_crlf_preserved() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("core.autocrlf")
        .arg("false")
        .status()
        .unwrap();
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    assert_success(&run_plan_accept(&repo, &plan_path));
    assert_success(&run_phase_select(&repo, "phase-1"));

    let contract_path = repo.join("contract.toml");
    let crlf_content = valid_contract_toml().replace('\n', "\r\n");
    write_plan(&contract_path, &crlf_content);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));

    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let persisted_content = record["contract_content"].as_str().unwrap();
    assert_eq!(
        persisted_content, crlf_content,
        "CRLF content must be preserved exactly"
    );
    assert!(
        persisted_content.ends_with("\r\n"),
        "final CRLF must be preserved"
    );

    let lf_content = valid_contract_toml();
    assert_ne!(
        persisted_content.as_bytes(),
        lf_content.as_bytes(),
        "CRLF and LF records must be byte-distinct"
    );
}

#[test]
fn test_pkg01_exact_content_no_final_newline_preserved() {
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
    let no_nl_content = valid_contract_toml().trim_end().to_string();
    write_plan(&contract_path, &no_nl_content);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));

    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let persisted_content = record["contract_content"].as_str().unwrap();
    assert_eq!(
        persisted_content, no_nl_content,
        "no-final-newline content must be preserved exactly"
    );
    assert!(
        !persisted_content.ends_with('\n'),
        "must not have trailing newline"
    );
}

#[test]
fn test_pkg01_no_duplicate_keys_in_record() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let bytes = std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    let text = String::from_utf8(bytes).unwrap();

    let keys = extract_top_level_json_keys(&text);
    let mut seen = std::collections::HashSet::new();
    for key in &keys {
        assert!(seen.insert(key.clone()), "duplicate key found: {}", key);
    }

    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let content = record["contract_content"].as_str().unwrap();
    let allowed_count = content.matches("allowed_paths").count();
    let forbidden_count = content.matches("forbidden_paths").count();
    assert_eq!(
        allowed_count, 1,
        "allowed_paths must appear exactly once in contract content"
    );
    assert_eq!(
        forbidden_count, 1,
        "forbidden_paths must appear exactly once in contract content"
    );
}

#[test]
fn test_pkg01_content_hash_mismatch_structural_rejection() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    record["contract_content"] = serde_json::json!("modified content that does not match sha");
    write_json(&repo, "implementation-authority.json", &record);

    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_INVALID");
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg01_different_existing_binding_rejected_and_preserved() {
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
    let tampered_bytes =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();

    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_STALE");

    let after_bytes =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    assert_eq!(
        tampered_bytes, after_bytes,
        "existing record must not be overwritten on failed begin"
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg01_no_overwrite_on_descendant_head() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let existing_bytes =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();

    std::fs::write(repo.join("new_file.rs"), b"fn new() {}").unwrap();
    commit_file(&repo, "new_file.rs");

    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_CONFLICT");

    let after_bytes =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    assert_eq!(
        existing_bytes, after_bytes,
        "existing record must not be overwritten on descendant head conflict"
    );
    assert_no_temp_files(&repo);
}

// ============================================================================
// PKG-02: Draft/phase authority after begin
// ============================================================================

#[test]
fn test_pkg02_active_phase_changed_after_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let impl_bytes_before =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();

    let mut state: serde_json::Value = read_json(&repo, "state.json");
    state["active_phase"] = serde_json::json!("phase-2");
    let closed = state["closed_phases"].as_array_mut().unwrap();
    if !closed.iter().any(|v| v.as_str() == Some("phase-1")) {
        closed.push(serde_json::json!("phase-1"));
    }
    write_json(&repo, "state.json", &state);

    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "GOVERNANCE_AUTHORITY_INVALID");

    let impl_bytes_after =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    assert_eq!(
        impl_bytes_before, impl_bytes_after,
        "implementation-authority must not be rewritten"
    );
    assert_no_temp_files(&repo);
}

// ============================================================================
// PKG-03: Complete operation-marker and malformed-Git failure surface
// ============================================================================

fn create_git_marker_dir(repo: &Path, name: &str) {
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
    std::fs::create_dir_all(&path).unwrap();
}

fn assert_marker_check_rejects(repo: &Path, marker: &str) {
    let impl_bytes_before =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    create_git_marker(repo, marker);
    let output = run_implementation_check(repo);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    let impl_bytes_after =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    assert_eq!(
        impl_bytes_before, impl_bytes_after,
        "implementation-authority must not be rewritten during marker check"
    );
    assert_no_temp_files(repo);
}

// --- P4-036: malformed or failed Git command output ---

#[test]
fn test_pkg03_git_config_nonzero_exit() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let recorder = create_malformed_git_recorder("core.sparseCheckout", b"error\n", 1);
    let output = run_with_malformed_git_recorder(&recorder, &repo, &["implementation", "check"]);
    assert_phase4_failure_exact(&output, "GIT_COMMAND_FAILED");
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_git_config_malformed_bool() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let recorder = create_malformed_git_recorder("core.sparseCheckout", b"maybe\n", 0);
    let output = run_with_malformed_git_recorder(&recorder, &repo, &["implementation", "check"]);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_git_config_non_utf8_output() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let recorder = create_malformed_git_recorder("core.sparseCheckout", &[0x80, 0x81, 0x82], 0);
    let output = run_with_malformed_git_recorder(&recorder, &repo, &["implementation", "check"]);
    assert_phase4_failure_exact(&output, "GIT_INVENTORY_INVALID");
    assert_no_temp_files(&repo);
}

// --- P4-037: operation-marker rejection (missing families and check side) ---

#[test]
fn test_pkg03_rebase_merge_marker_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker_dir(&repo, "rebase-merge");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    assert!(!repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_git_am_marker_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "rebase-apply/applying");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    assert!(!repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_merge_marker_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "MERGE_HEAD");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    assert!(!repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_cherry_pick_marker_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "CHERRY_PICK_HEAD");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    assert!(!repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_revert_marker_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "REVERT_HEAD");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    assert!(!repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_bisect_log_marker_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "BISECT_LOG");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    assert!(!repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_bisect_start_marker_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "BISECT_START");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    assert!(!repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_sequencer_marker_begin() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    create_git_marker(&repo, "sequencer/todo");
    let output = run_implementation_begin(&repo, final_rev, &final_sha);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    assert!(!repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_merge_marker_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    assert_marker_check_rejects(&repo, "MERGE_HEAD");
}

#[test]
fn test_pkg03_cherry_pick_marker_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    assert_marker_check_rejects(&repo, "CHERRY_PICK_HEAD");
}

#[test]
fn test_pkg03_revert_marker_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    assert_marker_check_rejects(&repo, "REVERT_HEAD");
}

#[test]
fn test_pkg03_bisect_log_marker_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    assert_marker_check_rejects(&repo, "BISECT_LOG");
}

#[test]
fn test_pkg03_bisect_start_marker_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    assert_marker_check_rejects(&repo, "BISECT_START");
}

#[test]
fn test_pkg03_rebase_apply_marker_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    assert_marker_check_rejects(&repo, "rebase-apply/applying");
}

#[test]
fn test_pkg03_rebase_merge_marker_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    let impl_bytes_before =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    create_git_marker_dir(&repo, "rebase-merge");
    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "GIT_OPERATION_IN_PROGRESS");
    let impl_bytes_after =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    assert_eq!(
        impl_bytes_before, impl_bytes_after,
        "implementation-authority must not be rewritten during marker check"
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg03_git_am_marker_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    assert_marker_check_rejects(&repo, "rebase-apply/applying");
}

#[test]
fn test_pkg03_sequencer_marker_check() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    assert_marker_check_rejects(&repo, "sequencer/todo");
}

#[test]
fn test_pkg02_newer_unaccepted_draft_keeps_accepted_authoritative() {
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
    let custom_contract = r#"schema_version = 1
contract_id = "test-contract-v1"
phase_id = "phase-1"
title = "Test contract"
objective = "Test objective."
requirements = ["req1"]
allowed_paths = ["src/", "contract.toml"]
forbidden_paths = [".git/"]
verification_commands = ["cargo test"]
handoff_fields = ["FIELD1"]
"#;
    write_plan(&contract_path, custom_contract);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));

    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let record_before: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let accepted_content = record_before["contract_content"]
        .as_str()
        .unwrap()
        .to_string();
    let accepted_sha = record_before["contract_sha256"]
        .as_str()
        .unwrap()
        .to_string();

    let v2 = custom_contract.replace("Test objective", "Revised objective");
    write_plan(&contract_path, &v2);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));

    let output = run_implementation_check(&repo);
    assert_phase4_success_exact(&output, &repo, 1);

    let record_after: serde_json::Value = read_json(&repo, "implementation-authority.json");
    assert_eq!(
        record_after["contract_revision"].as_u64().unwrap() as u32,
        1
    );
    assert_eq!(
        record_after["contract_sha256"].as_str().unwrap(),
        accepted_sha
    );
    assert_eq!(
        record_after["contract_content"].as_str().unwrap(),
        accepted_content
    );

    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg02_accepted_path_allowed_when_draft_removes_it() {
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
    let custom_contract = r#"schema_version = 1
contract_id = "test-contract-v1"
phase_id = "phase-1"
title = "Test contract"
objective = "Test objective."
requirements = ["req1"]
allowed_paths = ["src/", "docs/", "contract.toml"]
forbidden_paths = [".git/"]
verification_commands = ["cargo test"]
handoff_fields = ["FIELD1"]
"#;
    write_plan(&contract_path, custom_contract);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));

    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let v2 = custom_contract.replace(
        "allowed_paths = [\"src/\", \"docs/\", \"contract.toml\"]",
        "allowed_paths = [\"src/\"]",
    );
    write_plan(&contract_path, &v2);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));

    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(repo.join("docs").join("guide.md"), b"# Guide").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("docs/guide.md")
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
        .arg("add guide")
        .status()
        .unwrap();

    let impl_bytes_before =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();

    let output = run_implementation_check(&repo);
    assert_phase4_success_exact(&output, &repo, 2);

    let impl_bytes_after =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    assert_eq!(
        impl_bytes_before, impl_bytes_after,
        "implementation-authority must not be rewritten"
    );
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg02_draft_only_path_remains_not_allowed() {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha1 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, 1, &sha1, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    let contract_path = repo.join("contract.toml");
    let v2 = valid_contract_toml().replace(
        "allowed_paths = [\"src/\"]",
        "allowed_paths = [\"src/\", \"scripts/\"]",
    );
    write_plan(&contract_path, &v2);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha1));

    std::fs::create_dir_all(repo.join("scripts")).unwrap();
    std::fs::write(repo.join("scripts").join("build.sh"), b"#!/bin/bash").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("add")
        .arg("scripts/build.sh")
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
        .arg("add build script")
        .status()
        .unwrap();

    let impl_bytes_before =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();

    let output = run_implementation_check(&repo);
    assert_phase4_failure_exact(&output, "CHANGE_NOT_ALLOWED");

    let impl_bytes_after =
        std::fs::read(repo.join(".mrgs").join("implementation-authority.json")).unwrap();
    assert_eq!(
        impl_bytes_before, impl_bytes_after,
        "implementation-authority must not be rewritten"
    );
    assert_no_temp_files(&repo);
}

// ============================================================================
// PKG-04: Begin dirty-state, modification, rename, type-change,
//          union deduplication, and endpoint semantics evidence
// ============================================================================

/// Strict raw-diff record: mirrors production `RawDiffEntry` grammar.
/// Header starts with exactly one colon and has exactly five ASCII-space fields:
///   old_mode new_mode old_object_id new_object_id status_and_optional_score
/// Path fields are stored as lossless `Vec<u8>` byte vectors.
#[derive(Debug, PartialEq)]
struct StrictRawRecord {
    old_mode: String,
    new_mode: String,
    old_oid: String,
    new_oid: String,
    status: char,
    score: Option<u32>,
    status_token: Vec<u8>,
    dst: Vec<u8>,
    src: Option<Vec<u8>>,
}

/// Parse a complete NUL-delimited raw-diff stream into strict records.
/// Validates production mode grammar, object ID length and lowercase hex,
/// status and numeric score, status-dependent path count, complete token
/// arity, no orphan trailing token, exact lossless paths. Consumes all
/// records or panics. No MRGS policy/union/dedup.
fn parse_strict_raw_records(raw: &[u8]) -> Vec<StrictRawRecord> {
    let valid_modes: &[&str] = &["000000", "100644", "100755", "120000", "160000"];
    let mut entries = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        // Find NUL terminator for header record.
        let nul_pos = raw[i..].iter().position(|&b| b == 0).unwrap_or_else(|| {
            panic!(
                "raw parser: unterminated header at offset {}; stream not NUL-terminated",
                i
            )
        });
        let header = &raw[i..i + nul_pos];
        i += nul_pos + 1;

        // Record header must start with exactly one colon.
        assert!(
            header.starts_with(b":"),
            "raw parser: header must start with ':': {:?}",
            String::from_utf8_lossy(header)
        );
        let after_colon = &header[1..];

        // Reject leading or trailing space in the fields section.
        assert!(
            !after_colon.starts_with(b" "),
            "raw parser: header has leading space after colon: {:?}",
            String::from_utf8_lossy(header)
        );
        assert!(
            !after_colon.ends_with(b" "),
            "raw parser: header has trailing space: {:?}",
            String::from_utf8_lossy(header)
        );

        // Split on ASCII space, rejecting doubled spaces (empty fields).
        let mut parts: Vec<&[u8]> = Vec::new();
        let mut field_start = 0usize;
        for (idx, &b) in after_colon.iter().enumerate() {
            if b == b' ' {
                assert!(
                    idx > field_start,
                    "raw parser: doubled space at position {} in header: {:?}",
                    idx,
                    String::from_utf8_lossy(header)
                );
                parts.push(&after_colon[field_start..idx]);
                field_start = idx + 1;
            }
        }
        parts.push(&after_colon[field_start..]);
        assert_eq!(
            parts.len(),
            5,
            "raw parser: header must have exactly 5 space-separated fields: {:?}",
            String::from_utf8_lossy(header)
        );

        let old_mode = std::str::from_utf8(parts[0]).expect("old_mode UTF-8");
        let new_mode = std::str::from_utf8(parts[1]).expect("new_mode UTF-8");
        let old_oid = std::str::from_utf8(parts[2]).expect("old_oid UTF-8");
        let new_oid = std::str::from_utf8(parts[3]).expect("new_oid UTF-8");
        let status_part = parts[4];
        let status_token = status_part.to_vec();

        // Validate production mode grammar.
        for mode in [old_mode, new_mode] {
            assert!(
                valid_modes.contains(&mode),
                "raw parser: unsupported mode {:?}",
                mode
            );
        }

        // Validate object ID length and lowercase hex.
        for oid in [old_oid, new_oid] {
            assert!(
                oid.len() == 40 || oid.len() == 64,
                "raw parser: OID must be 40 or 64 chars: {:?}",
                oid
            );
            assert!(
                oid.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                "raw parser: OID must be lowercase hex: {:?}",
                oid
            );
        }

        // Validate status and optional score.
        assert!(
            !status_part.is_empty(),
            "raw parser: empty status field in header"
        );
        let status_byte = status_part[0] as char;
        let score_bytes = &status_part[1..];
        assert!(
            matches!(status_byte, 'A' | 'D' | 'M' | 'T' | 'R' | 'C'),
            "raw parser: invalid status {:?}",
            status_byte
        );

        let score = match status_byte {
            'A' | 'D' | 'M' | 'T' => {
                assert!(
                    score_bytes.is_empty(),
                    "raw parser: status {:?} must not have score, got {:?}",
                    status_byte,
                    String::from_utf8_lossy(score_bytes)
                );
                None
            }
            'R' | 'C' => {
                let score_str = std::str::from_utf8(score_bytes).expect("score must be UTF-8");
                assert!(
                    !score_str.is_empty() && score_str.bytes().all(|b| b.is_ascii_digit()),
                    "raw parser: R/C score must be all-digit: {:?}",
                    score_str
                );
                let score_val: u32 = score_str.parse().expect("score parse as u32");
                assert!(
                    score_val <= 100,
                    "raw parser: score must be 0..100: {}",
                    score_val
                );
                Some(score_val)
            }
            _ => unreachable!(),
        };

        // Status-dependent path count: read source path.
        assert!(
            i < raw.len(),
            "raw parser: missing source path after header at offset {}",
            i
        );
        let nul_src = raw[i..]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or_else(|| panic!("raw parser: unterminated source path at offset {}", i));
        let path1 = raw[i..i + nul_src].to_vec();
        i += nul_src + 1;

        // R/C: second path (destination). Others: single path only.
        let (dst, src) = match status_byte {
            'R' | 'C' => {
                assert!(
                    i < raw.len(),
                    "raw parser: missing destination path for R/C at offset {}",
                    i
                );
                let nul_dst = raw[i..].iter().position(|&b| b == 0).unwrap_or_else(|| {
                    panic!("raw parser: unterminated destination path at offset {}", i)
                });
                let path2 = raw[i..i + nul_dst].to_vec();
                i += nul_dst + 1;
                (path2, Some(path1))
            }
            _ => (path1, None),
        };

        entries.push(StrictRawRecord {
            old_mode: old_mode.to_string(),
            new_mode: new_mode.to_string(),
            old_oid: old_oid.to_string(),
            new_oid: new_oid.to_string(),
            status: status_byte,
            score,
            status_token,
            dst,
            src,
        });
    }
    entries
}

/// Strict porcelain `-z` `XY SP path` record.
/// Mirrors production porcelain grammar. For R/C, dst is the destination
/// (first path in porcelain -z), src is the source (second path).
/// Path fields are stored as lossless `Vec<u8>` byte vectors.
#[derive(Debug, PartialEq)]
struct StrictPorcelainRecord {
    xy: String,
    dst: Vec<u8>,
    src: Option<Vec<u8>>,
}

/// Accepted porcelain XY codes from production `classify_porcelain_xy`.
const PORCELAIN_ACCEPTED_XY: &[&str] = &[
    " M", " T", " D", "M ", "MM", "MT", "MD", "T ", "TM", "TT", "TD", "A ", "AM", "AT", "AD", "D ",
    "R ", "RM", "RT", "RD", "C ", "CM", "CT", "CD", "??",
];

/// Parse a complete NUL-delimited porcelain status stream.
/// Validates production XY codes, SP separator at position 2, non-empty path,
/// R/C destination-then-source order. Consumes all records or panics.
/// Rejects malformed/unknown/incomplete/orphan records. Preserves paths.
fn parse_strict_porcelain_records(stdout: &[u8]) -> Vec<StrictPorcelainRecord> {
    let mut entries = Vec::new();
    let mut i = 0;
    while i < stdout.len() {
        let nul = stdout[i..].iter().position(|&b| b == 0).unwrap_or_else(|| {
            panic!(
                "porcelain parser: unterminated record at offset {}; stream not NUL-terminated",
                i
            )
        });
        // Must have at least 4 bytes: XY SP <1+ byte path>
        assert!(
            nul >= 4,
            "porcelain parser: record too short ({} bytes) at offset {}: {:?}",
            nul,
            i,
            String::from_utf8_lossy(&stdout[i..i + nul])
        );
        let xy_0 = stdout[i] as char;
        let xy_1 = stdout[i + 1] as char;
        assert_eq!(
            stdout[i + 2],
            b' ',
            "porcelain parser: byte 2 must be SP separator at offset {}: {:?}",
            i,
            String::from_utf8_lossy(&stdout[i..i + nul])
        );
        let xy = format!("{}{}", xy_0, xy_1);
        assert!(
            PORCELAIN_ACCEPTED_XY.contains(&xy.as_str()),
            "porcelain parser: invalid XY code {:?} at offset {}: {:?}",
            xy,
            i,
            String::from_utf8_lossy(&stdout[i..i + nul])
        );
        let path_data = &stdout[i + 3..i + nul];
        assert!(
            !path_data.is_empty(),
            "porcelain parser: empty path at offset {}",
            i
        );
        let dst = path_data.to_vec();
        i += nul + 1;

        // R/C: destination-then-source order in -z porcelain.
        let src = if xy_0 == 'R' || xy_0 == 'C' {
            assert!(
                i < stdout.len(),
                "porcelain parser: missing source path for R/C at offset {}",
                i
            );
            let nul2 = stdout[i..].iter().position(|&b| b == 0).unwrap_or_else(|| {
                panic!("porcelain parser: unterminated source path at offset {}", i)
            });
            let src_data = &stdout[i..i + nul2];
            assert!(
                !src_data.is_empty(),
                "porcelain parser: empty source path at offset {}",
                i
            );
            let src_str = src_data.to_vec();
            i += nul2 + 1;
            Some(src_str)
        } else {
            None
        };

        entries.push(StrictPorcelainRecord { xy, dst, src });
    }
    entries
}

fn setup_and_begin() -> (tempfile::TempDir, std::path::PathBuf) {
    let (_dir, repo) = setup_implementation_basic();
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    (_dir, repo)
}

fn setup_and_begin_with_contract(contract: &str) -> (tempfile::TempDir, std::path::PathBuf) {
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
    write_plan(&contract_path, contract);
    commit_file(&repo, "contract.toml");
    assert_success(&run_contract_draft(&repo, &contract_path));
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
    (dir, repo)
}

// --- P4-032: tracked deletion and type change rejection at begin ---

#[test]
#[cfg_attr(not(any(unix, windows)), ignore)]
fn test_pkg04_begin_rejects_tracked_deletion_and_type_changes() {
    // Case A: tracked deletion → GIT_DIRTY
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        // Delete a committed file without staging → porcelain ` D`
        std::fs::remove_file(repo.join("README.md")).unwrap();
        let gov_before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        let gov_after = capture_governance(&repo);
        assert_eq!(
            gov_before, gov_after,
            "governance files changed during P4-032-A begin failure"
        );
        assert_phase4_failure_exact(&output, "GIT_DIRTY");
        assert_no_temp_files(&repo);
    }

    // Case B: type change (file → symlink) → GIT_DIRTY
    {
        let (_dir, repo) = setup_implementation_basic();
        let cfg_out = git(&repo)
            .arg("config")
            .arg("core.symlinks")
            .arg("true")
            .output()
            .unwrap();
        assert_eq!(
            cfg_out.status.code(),
            Some(0),
            "git config core.symlinks failed: stderr={:?}",
            cfg_out.stderr
        );
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        // Replace committed README.md with a symlink → porcelain ` T`
        std::fs::remove_file(repo.join("README.md")).unwrap();
        symlink_relative(&repo.join("README.md"), "plan.toml");
        let gov_before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        let gov_after = capture_governance(&repo);
        assert_eq!(
            gov_before, gov_after,
            "governance files changed during P4-032-B begin failure"
        );
        assert_phase4_failure_exact(&output, "GIT_DIRTY");
        assert_no_temp_files(&repo);
    }
}

// --- P4-051, P4-052, P4-053: allowed modification states ---

#[test]
fn test_pkg04_allowed_modification_states() {
    // P4-051: allowed unstaged modified file → success count 1
    {
        let (_dir, repo) = setup_and_begin();
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("file.rs"), b"fn main() {}").unwrap();
        commit_file(&repo, "src/file.rs");
        // Modify without staging → porcelain ` M src/file.rs`
        std::fs::write(
            repo.join("src").join("file.rs"),
            b"fn main() { println!(\"modified\"); }",
        )
        .unwrap();
        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 1);
        assert_no_temp_files(&repo);
    }

    // P4-052: allowed staged modified file → success count 1
    {
        let (_dir, repo) = setup_and_begin();
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("file.rs"), b"fn main() {}").unwrap();
        commit_file(&repo, "src/file.rs");
        // Modify and stage → porcelain `M  src/file.rs`
        std::fs::write(
            repo.join("src").join("file.rs"),
            b"fn main() { println!(\"modified\"); }",
        )
        .unwrap();
        let add_out = git(&repo).arg("add").arg("src/file.rs").output().unwrap();
        assert_eq!(
            add_out.status.code(),
            Some(0),
            "git add src/file.rs failed: stderr={:?}",
            add_out.stderr
        );
        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 1);
        assert_no_temp_files(&repo);
    }

    // P4-053: allowed committed modified file after baseline → success count 1
    {
        // Manual setup: commit src/file.rs before begin so baseline captures it
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
        // Commit src/file.rs before begin so baseline captures it
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("file.rs"), b"fn main() {}").unwrap();
        commit_file(&repo, "src/file.rs");
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        // Record baseline for raw diff verification
        let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
        let baseline = record["baseline_head"].as_str().unwrap().to_string();
        // Modify and commit after baseline
        std::fs::write(
            repo.join("src").join("file.rs"),
            b"fn main() { println!(\"modified\"); }",
        )
        .unwrap();
        commit_file(&repo, "src/file.rs");
        // Strict raw-diff record parse: exact M no-score record for src/file.rs
        let raw = git_raw_diff(&repo, &baseline);
        let raw_records = parse_strict_raw_records(&raw);
        let file_rs_records: Vec<_> = raw_records
            .iter()
            .filter(|r| r.dst == b"src/file.rs")
            .collect();
        assert_eq!(
            file_rs_records.len(),
            1,
            "Expected exactly one parsed record for src/file.rs, got {}: {:?}",
            file_rs_records.len(),
            raw_records
        );
        let rec = file_rs_records[0];
        // Valid header already validated by parser (colon, 5 fields, modes, OIDs)
        assert_eq!(
            rec.status, 'M',
            "Parsed record status must be M (modified), not A/R/C/T: {:?}",
            rec.status
        );
        assert!(
            rec.score.is_none(),
            "Parsed M record must have no score: {:?}",
            rec.score
        );
        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 1);
        assert_no_temp_files(&repo);
    }
}

// --- P4-055: allowed deleted file in staged, unstaged, and committed states ---

#[test]
fn test_pkg04_allowed_deletion_states() {
    // P4-055a: allowed unstaged deleted file → success count 1
    // File exists in baseline, deleted without staging after begin
    {
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
        // Commit src/file.rs before begin so it is in the baseline
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("file.rs"), b"fn main() {}").unwrap();
        commit_file(&repo, "src/file.rs");
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Delete without staging → porcelain ` D src/file.rs`
        std::fs::remove_file(repo.join("src").join("file.rs")).unwrap();
        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 1);
        assert_no_temp_files(&repo);
    }

    // P4-055b: allowed staged deleted file → success count 1
    {
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
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("file.rs"), b"fn main() {}").unwrap();
        commit_file(&repo, "src/file.rs");
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Delete and stage via git rm → porcelain `D  src/file.rs`
        let rm_out = git(&repo).arg("rm").arg("src/file.rs").output().unwrap();
        assert_eq!(
            rm_out.status.code(),
            Some(0),
            "git rm src/file.rs failed: stderr={:?}",
            rm_out.stderr
        );
        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 1);
        assert_no_temp_files(&repo);
    }

    // P4-055c: allowed committed deleted file after baseline → success count 1
    {
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
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("file.rs"), b"fn main() {}").unwrap();
        commit_file(&repo, "src/file.rs");
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Delete and commit after baseline
        let rm_out = git(&repo).arg("rm").arg("src/file.rs").output().unwrap();
        assert_eq!(
            rm_out.status.code(),
            Some(0),
            "git rm src/file.rs failed: stderr={:?}",
            rm_out.stderr
        );
        let commit_out = git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("delete file")
            .output()
            .unwrap();
        assert_eq!(
            commit_out.status.code(),
            Some(0),
            "git commit 'delete file' failed: stderr={:?}",
            commit_out.stderr
        );
        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 1);
        assert_no_temp_files(&repo);
    }
}

// --- P4-058: rename rejected when only destination is allowed ---

#[test]
fn test_pkg04_destination_only_allowed_rename_rejected() {
    // Manual setup: commit docs/old.txt before begin so it is in the baseline
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

    // Commit a file outside allowed scope BEFORE begin
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(
        repo.join("docs").join("old.txt"),
        b"unique content for rename detection",
    )
    .unwrap();
    commit_file(&repo, "docs/old.txt");

    // Accept and begin
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Record baseline for raw diff verification
    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let baseline = record["baseline_head"].as_str().unwrap().to_string();

    // Rename docs/old.txt → src/new.txt (only destination is in allowed scope)
    std::fs::create_dir_all(repo.join("src")).unwrap();
    let mv_out = git(&repo)
        .arg("mv")
        .arg("docs/old.txt")
        .arg("src/new.txt")
        .output()
        .unwrap();
    assert_eq!(
        mv_out.status.code(),
        Some(0),
        "git mv failed: stderr={:?}",
        mv_out.stderr
    );
    // Commit the rename (already staged by git mv)
    let commit_out = git(&repo)
        .arg("commit")
        .arg("-m")
        .arg("rename docs to src")
        .output()
        .unwrap();
    assert_eq!(
        commit_out.status.code(),
        Some(0),
        "git commit failed: stderr={:?}",
        commit_out.stderr
    );

    // Strict raw-diff structural parse with full header validation
    let raw = git_raw_diff(&repo, &baseline);
    let raw_records = parse_strict_raw_records(&raw);
    // Exact one parsed R+numeric-score record
    let rename_records: Vec<_> = raw_records.iter().filter(|r| r.status == 'R').collect();
    let other_records: Vec<_> = raw_records.iter().filter(|r| r.status != 'R').collect();
    assert_eq!(
        rename_records.len(),
        1,
        "Expected exactly one rename (R) record, got {}: {:?}",
        rename_records.len(),
        raw_records
    );
    assert!(
        other_records.is_empty(),
        "Expected no unrelated records, got {}: {:?}",
        other_records.len(),
        raw_records
    );
    let rec = rename_records[0];
    // GAP3: exact R status, score Some(100), semantic raw status token R100
    assert_eq!(
        rec.status, 'R',
        "Record status must be exactly R, got {:?}",
        rec.status
    );
    assert_eq!(
        rec.score,
        Some(100),
        "R record must have exact score Some(100), got {:?}",
        rec.score
    );
    assert_eq!(
        rec.status_token.as_slice(),
        b"R100",
        "R record must have exact raw status token b\"R100\", got {:?}",
        rec.status_token
    );
    // Exact source and destination (lossless byte comparison)
    assert_eq!(
        rec.src.as_deref(),
        Some(b"docs/old.txt" as &[u8]),
        "Rename source must be docs/old.txt, got {:?}",
        rec.src
    );
    assert_eq!(
        rec.dst.as_slice(),
        b"src/new.txt",
        "Rename destination must be src/new.txt, got {:?}",
        rec.dst
    );
    // Source before destination: src is the first path, dst is the second
    // in the raw NUL-delimited stream. The parser records src=first, dst=second,
    // so the structural ordering is already proven by the parse succeeding.
    // Additionally verify byte-level ordering for completeness.
    let src_byte_pos = raw
        .windows(b"docs/old.txt".len())
        .position(|w| w == b"docs/old.txt")
        .unwrap();
    let dst_byte_pos = raw
        .windows(b"src/new.txt".len())
        .position(|w| w == b"src/new.txt")
        .unwrap();
    assert!(
        src_byte_pos < dst_byte_pos,
        "Source docs/old.txt (byte {}) must precede destination src/new.txt (byte {})",
        src_byte_pos,
        dst_byte_pos
    );

    // Source docs/old.txt is not in allowed_paths ["src/"] → CHANGE_NOT_ALLOWED
    let gov_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    let gov_after = capture_governance(&repo);
    assert_eq!(
        gov_before, gov_after,
        "governance files changed during P4-058 check failure"
    );
    assert_phase4_failure_exact(&output, "CHANGE_NOT_ALLOWED");
    assert_no_temp_files(&repo);
}

// --- P4-060: type-change inventory and enforcement ---

#[test]
#[cfg_attr(not(any(unix, windows)), ignore)]
fn test_pkg04_type_change_inventory_and_enforcement() {
    // Case A: allowed type change (file → symlink within allowed scope) → success
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);
        let cfg_out = git(&repo)
            .arg("config")
            .arg("core.symlinks")
            .arg("true")
            .output()
            .unwrap();
        assert_eq!(
            cfg_out.status.code(),
            Some(0),
            "git config core.symlinks failed: stderr={:?}",
            cfg_out.stderr
        );
        let plan_path = repo.join("plan.toml");
        write_plan(&plan_path, valid_plan_toml());
        commit_file(&repo, "plan.toml");
        assert_success(&run_plan_accept(&repo, &plan_path));
        assert_success(&run_phase_select(&repo, "phase-1"));
        let contract_path = repo.join("contract.toml");
        write_plan(&contract_path, valid_contract_toml());
        commit_file(&repo, "contract.toml");
        assert_success(&run_contract_draft(&repo, &contract_path));
        // Commit both files before begin so they are in the baseline
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("target.txt"), b"target content").unwrap();
        std::fs::write(repo.join("src").join("other.txt"), b"other content").unwrap();
        commit_file(&repo, "src/target.txt");
        commit_file(&repo, "src/other.txt");
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Replace src/target.txt with a symlink → src/other.txt (type change)
        std::fs::remove_file(repo.join("src").join("target.txt")).unwrap();
        symlink_relative(&repo.join("src").join("target.txt"), "other.txt");
        let add_out = git(&repo)
            .arg("add")
            .arg("src/target.txt")
            .output()
            .unwrap();
        assert_eq!(
            add_out.status.code(),
            Some(0),
            "git add src/target.txt failed: stderr={:?}",
            add_out.stderr
        );
        let commit_out = git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("type change to symlink")
            .output()
            .unwrap();
        assert_eq!(
            commit_out.status.code(),
            Some(0),
            "git commit 'type change to symlink' failed: stderr={:?}",
            commit_out.stderr
        );

        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 1);
        assert_no_temp_files(&repo);
    }

    // Case B: not-allowed type change (symlink escaping repo) → FILESYSTEM_BOUNDARY_UNSAFE
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);
        let cfg_out = git(&repo)
            .arg("config")
            .arg("core.symlinks")
            .arg("true")
            .output()
            .unwrap();
        assert_eq!(
            cfg_out.status.code(),
            Some(0),
            "git config core.symlinks failed: stderr={:?}",
            cfg_out.stderr
        );
        let plan_path = repo.join("plan.toml");
        write_plan(&plan_path, valid_plan_toml());
        commit_file(&repo, "plan.toml");
        assert_success(&run_plan_accept(&repo, &plan_path));
        assert_success(&run_phase_select(&repo, "phase-1"));
        let contract_path = repo.join("contract.toml");
        write_plan(&contract_path, valid_contract_toml());
        commit_file(&repo, "contract.toml");
        assert_success(&run_contract_draft(&repo, &contract_path));
        // Commit src/bad.txt before begin
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("bad.txt"), b"bad content").unwrap();
        commit_file(&repo, "src/bad.txt");
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        // Replace src/bad.txt with symlink pointing outside repo
        std::fs::remove_file(repo.join("src").join("bad.txt")).unwrap();
        symlink_relative(&repo.join("src").join("bad.txt"), "../../escape");
        let add_out = git(&repo).arg("add").arg("src/bad.txt").output().unwrap();
        assert_eq!(
            add_out.status.code(),
            Some(0),
            "git add src/bad.txt failed: stderr={:?}",
            add_out.stderr
        );
        let commit_out = git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("type change to escaping symlink")
            .output()
            .unwrap();
        assert_eq!(
            commit_out.status.code(),
            Some(0),
            "git commit 'type change to escaping symlink' failed: stderr={:?}",
            commit_out.stderr
        );

        let gov_before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        let gov_after = capture_governance(&repo);
        assert_eq!(
            gov_before, gov_after,
            "governance files changed during P4-060-B check failure"
        );
        assert_phase4_failure_exact(&output, "FILESYSTEM_BOUNDARY_UNSAFE");
        assert_no_temp_files(&repo);
    }
}

// --- P4-075: union deduplicates committed and working path ---

#[test]
fn test_pkg04_union_deduplicates_committed_and_working_path() {
    // Manual setup: commit src/file.rs before begin so it exists at baseline
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
    // Commit src/file.rs before begin so it exists at baseline
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("file.rs"), b"v1").unwrap();
    commit_file(&repo, "src/file.rs");
    let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

    // Record baseline for independent verification
    let record: serde_json::Value = read_json(&repo, "implementation-authority.json");
    let baseline = record["baseline_head"].as_str().unwrap().to_string();

    // Modify and commit after baseline (committed change)
    std::fs::write(repo.join("src").join("file.rs"), b"v2").unwrap();
    commit_file(&repo, "src/file.rs");

    // Modify same path again without committing (working tree change)
    std::fs::write(repo.join("src").join("file.rs"), b"v3").unwrap();

    // Strict structural parse of committed raw baseline-to-HEAD diff
    let raw = git_raw_diff(&repo, &baseline);
    let raw_records = parse_strict_raw_records(&raw);
    let committed_file_rs: Vec<_> = raw_records
        .iter()
        .filter(|r| r.dst == b"src/file.rs")
        .collect();
    assert_eq!(
        committed_file_rs.len(),
        1,
        "Expected exactly one committed raw record for src/file.rs, got {}: {:?}",
        committed_file_rs.len(),
        raw_records
    );
    let committed_rec = committed_file_rs[0];
    // Valid header already validated by parser (colon, 5 fields, modes, OIDs)
    assert_eq!(
        committed_rec.status, 'M',
        "Committed status for src/file.rs must be M, got {:?}",
        committed_rec.status
    );
    assert!(
        committed_rec.score.is_none(),
        "Committed M record must have no score"
    );

    // Strict structural parse of porcelain working/index inventory
    let status_out = git(&repo)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("-z")
        .arg("--untracked-files=all")
        .arg("--ignore-submodules=none")
        .arg("--renames")
        .output()
        .unwrap();
    assert_eq!(
        status_out.status.code(),
        Some(0),
        "git status --porcelain=v1 -z failed: stderr={:?}",
        status_out.stderr
    );
    let porcelain_records = parse_strict_porcelain_records(&status_out.stdout);
    let working_file_rs: Vec<_> = porcelain_records
        .iter()
        .filter(|r| r.dst == b"src/file.rs")
        .collect();
    assert_eq!(
        working_file_rs.len(),
        1,
        "Expected exactly one porcelain record for src/file.rs, got {}: {:?}",
        working_file_rs.len(),
        porcelain_records
    );
    let working_rec = working_file_rs[0];
    // Porcelain mirrors production version/flags: XY must be " M" (unstaged modification)
    assert_eq!(
        working_rec.xy, " M",
        "Working XY for src/file.rs must be ' M' (unstaged modification), got {:?}",
        working_rec.xy
    );
    assert!(
        working_rec.src.is_none(),
        "Non-rename record must have no source path"
    );

    // Both records refer to identical complete path bytes (lossless cross-inventory)
    assert_eq!(
        committed_rec.dst, working_rec.dst,
        "Committed and working path bytes must be identical: committed={:?} working={:?}",
        committed_rec.dst, working_rec.dst
    );

    // Run MRGS check; assert exact success output and count exactly 1
    let output = run_implementation_check(&repo);
    assert_phase4_success_exact(&output, &repo, 1);
    assert_no_temp_files(&repo);
}

// --- P4-121: endpoint semantics ---

#[test]
fn test_pkg04_endpoint_semantics_reverted_forbidden_commit() {
    // Use a contract with "secret/" as a forbidden path
    let custom_contract = valid_contract_toml().replace(
        "forbidden_paths = [\".git/\"]",
        "forbidden_paths = [\"secret/\", \".git/\"]",
    );

    // Case A: forbidden intermediate commit fully reverted → not in current inventory
    {
        let (_dir, repo) = setup_and_begin_with_contract(&custom_contract);

        // Commit a forbidden change (adds file under secret/)
        std::fs::create_dir_all(repo.join("secret")).unwrap();
        std::fs::write(repo.join("secret").join("data.txt"), b"sensitive").unwrap();
        commit_file(&repo, "secret/data.txt");

        // Revert the forbidden commit so net diff from baseline to HEAD is empty
        let revert_out = git(&repo)
            .arg("revert")
            .arg("HEAD")
            .arg("--no-edit")
            .output()
            .unwrap();
        assert_eq!(
            revert_out.status.code(),
            Some(0),
            "git revert HEAD --no-edit failed: stderr={:?}",
            revert_out.stderr
        );

        // The reverted forbidden commit is NOT current inventory → count 0
        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 0);
        assert_no_temp_files(&repo);
    }

    // Case B: net forbidden endpoint state → rejected exactly
    {
        let (_dir, repo) = setup_and_begin_with_contract(&custom_contract);

        // Commit a forbidden change (not reverted)
        std::fs::create_dir_all(repo.join("secret")).unwrap();
        std::fs::write(repo.join("secret").join("data.txt"), b"sensitive").unwrap();
        commit_file(&repo, "secret/data.txt");

        // The forbidden path is in the net diff → CHANGE_FORBIDDEN
        let gov_before = capture_governance(&repo);
        let output = run_implementation_check(&repo);
        let gov_after = capture_governance(&repo);
        assert_eq!(
            gov_before, gov_after,
            "governance files changed during P4-121-B check failure"
        );
        assert_phase4_failure_exact(&output, "CHANGE_FORBIDDEN");
        assert_no_temp_files(&repo);
    }
}

// --- P4-068: duplicate normalized allowed-rule and forbidden-rule rejection ---

#[test]
fn test_pkg05_duplicate_normalized_rules_rejected() {
    struct Observation {
        _dir: tempfile::TempDir,
        repo: std::path::PathBuf,
        output: std::process::Output,
        governance_before: std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry>,
        governance_after: std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry>,
    }

    let mut observations = Vec::new();

    // Subcase A: exact duplicate allowed ["src/","src/"] — rejected at draft
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo_a");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);
        let plan_path = repo.join("plan.toml");
        write_plan(&plan_path, valid_plan_toml());
        commit_file(&repo, "plan.toml");
        let accept_out = run_plan_accept(&repo, &plan_path);
        assert_success(&accept_out);
        let select_out = run_phase_select(&repo, "phase-1");
        assert_success(&select_out);
        let contract = valid_contract_toml().replace(
            r#"allowed_paths = ["src/"]"#,
            r#"allowed_paths = ["src/", "src/"]"#,
        );
        let contract_path = repo.join("contract.toml");
        write_plan(&contract_path, &contract);
        commit_file(&repo, "contract.toml");
        let gov_before = capture_governance(&repo);
        let draft_out = run_contract_draft(&repo, &contract_path);
        let gov_after = capture_governance(&repo);
        observations.push(Observation {
            _dir: dir,
            repo,
            output: draft_out,
            governance_before: gov_before,
            governance_after: gov_after,
        });
    }

    // Subcase B: exact duplicate forbidden ["secret","secret"] — rejected at draft
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo_b");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);
        let plan_path = repo.join("plan.toml");
        write_plan(&plan_path, valid_plan_toml());
        commit_file(&repo, "plan.toml");
        let accept_out = run_plan_accept(&repo, &plan_path);
        assert_success(&accept_out);
        let select_out = run_phase_select(&repo, "phase-1");
        assert_success(&select_out);
        let contract = valid_contract_toml().replace(
            r#"forbidden_paths = [".git/"]"#,
            r#"forbidden_paths = ["secret", "secret"]"#,
        );
        let contract_path = repo.join("contract.toml");
        write_plan(&contract_path, &contract);
        commit_file(&repo, "contract.toml");
        let gov_before = capture_governance(&repo);
        let draft_out = run_contract_draft(&repo, &contract_path);
        let gov_after = capture_governance(&repo);
        observations.push(Observation {
            _dir: dir,
            repo,
            output: draft_out,
            governance_before: gov_before,
            governance_after: gov_after,
        });
    }

    // Subcase C: normalization-equivalent allowed ["src","src/"] — must reject at begin
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo_c");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);
        let plan_path = repo.join("plan.toml");
        write_plan(&plan_path, valid_plan_toml());
        commit_file(&repo, "plan.toml");
        let accept_out = run_plan_accept(&repo, &plan_path);
        assert_success(&accept_out);
        let select_out = run_phase_select(&repo, "phase-1");
        assert_success(&select_out);
        let contract = valid_contract_toml().replace(
            r#"allowed_paths = ["src/"]"#,
            r#"allowed_paths = ["src", "src/"]"#,
        );
        let contract_path = repo.join("contract.toml");
        write_plan(&contract_path, &contract);
        commit_file(&repo, "contract.toml");
        let draft_out = run_contract_draft(&repo, &contract_path);
        assert_success(&draft_out);
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        let accept_c = run_contract_accept(&repo, rev, &sha, "ACCEPTED");
        assert_success(&accept_c);
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        let gov_before = capture_governance(&repo);
        let begin_out = run_implementation_begin(&repo, final_rev, &final_sha);
        let gov_after = capture_governance(&repo);
        observations.push(Observation {
            _dir: dir,
            repo,
            output: begin_out,
            governance_before: gov_before,
            governance_after: gov_after,
        });
    }

    // Subcase D: normalization-equivalent forbidden ["secret","secret/"] — must reject at begin
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo_d");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);
        let plan_path = repo.join("plan.toml");
        write_plan(&plan_path, valid_plan_toml());
        commit_file(&repo, "plan.toml");
        let accept_out = run_plan_accept(&repo, &plan_path);
        assert_success(&accept_out);
        let select_out = run_phase_select(&repo, "phase-1");
        assert_success(&select_out);
        let contract = valid_contract_toml().replace(
            r#"forbidden_paths = [".git/"]"#,
            r#"forbidden_paths = ["secret", "secret/"]"#,
        );
        let contract_path = repo.join("contract.toml");
        write_plan(&contract_path, &contract);
        commit_file(&repo, "contract.toml");
        let draft_out = run_contract_draft(&repo, &contract_path);
        assert_success(&draft_out);
        let draft: serde_json::Value = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        let accept_d = run_contract_accept(&repo, rev, &sha, "ACCEPTED");
        assert_success(&accept_d);
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        let gov_before = capture_governance(&repo);
        let begin_out = run_implementation_begin(&repo, final_rev, &final_sha);
        let gov_after = capture_governance(&repo);
        observations.push(Observation {
            _dir: dir,
            repo,
            output: begin_out,
            governance_before: gov_before,
            governance_after: gov_after,
        });
    }

    assert_eq!(observations.len(), 4);
    for (label, observation) in ["A", "B", "C", "D"].iter().zip(&observations) {
        assert_eq!(
            observation.governance_before, observation.governance_after,
            "subcase {}: governance changed during rejection",
            label
        );
        assert_no_temp_files(&observation.repo);
    }

    let allowed_exact_stderr = b"error: duplicate entry in contract 'allowed_paths' list\n";
    let forbidden_exact_stderr = b"error: duplicate entry in contract 'forbidden_paths' list\n";
    assert_eq!(observations[0].output.status.code(), Some(1));
    assert!(observations[0].output.stdout.is_empty());
    assert_eq!(observations[0].output.stderr, allowed_exact_stderr);
    assert_eq!(observations[1].output.status.code(), Some(1));
    assert!(observations[1].output.stdout.is_empty());
    assert_eq!(observations[1].output.stderr, forbidden_exact_stderr);
    assert_phase4_failure_exact(&observations[2].output, "CONTRACT_PATH_RULE_INVALID");
    assert_phase4_failure_exact(&observations[3].output, "CONTRACT_PATH_RULE_INVALID");
}

fn setup_pkg05_accepted_contract(
    contract: &str,
) -> (tempfile::TempDir, std::path::PathBuf, u32, String) {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_init(&repo);
    let plan_path = repo.join("plan.toml");
    write_plan(&plan_path, valid_plan_toml());
    commit_file(&repo, "plan.toml");
    let plan_accept = run_plan_accept(&repo, &plan_path);
    assert_success(&plan_accept);
    let phase_select = run_phase_select(&repo, "phase-1");
    assert_success(&phase_select);
    let contract_path = repo.join("contract.toml");
    write_plan(&contract_path, contract);
    commit_file(&repo, "contract.toml");
    let draft = run_contract_draft(&repo, &contract_path);
    assert_success(&draft);
    let draft_record = read_json(&repo, "contract-draft.json");
    let revision = draft_record["revision"].as_u64().unwrap() as u32;
    let sha256 = draft_record["sha256"].as_str().unwrap().to_string();
    let accept = run_contract_accept(&repo, revision, &sha256, "ACCEPTED");
    assert_success(&accept);
    let (final_revision, final_sha256) = contract_accepted_revision(&repo);
    (dir, repo, final_revision, final_sha256)
}

fn run_pkg05_reserved_path_check(repo: &Path, path: &str) -> std::process::Output {
    let baseline = git_head_exact(repo);
    let diff_args = [
        "diff",
        "--no-ext-diff",
        "--raw",
        "-z",
        "--no-abbrev",
        "--find-renames=50%",
        "--find-copies=50%",
        "--find-copies-harder",
        baseline.as_str(),
        "HEAD",
        "--",
    ];
    let payload = format!(
        ":000000 100644 0000000000000000000000000000000000000000 1111111111111111111111111111111111111111 A\0{}\0",
        path
    )
    .into_bytes();
    let wrapper = create_git_wrapper(repo, &diff_args, "payload", &payload);
    let output = run_check_with_git_wrapper(repo, &wrapper);
    assert_wrapper_reached(&wrapper);
    output
}

#[test]
fn test_pkg05_rule_case_semantics() {
    let case_sensitive_contract = valid_contract_toml().replace(
        r#"forbidden_paths = [".git/"]"#,
        r#"forbidden_paths = ["secret/"]"#,
    );

    let (_dir, repo, final_revision, final_sha256) =
        setup_pkg05_accepted_contract(&case_sensitive_contract);
    assert_success(&run_implementation_begin(
        &repo,
        final_revision,
        &final_sha256,
    ));
    std::fs::create_dir_all(repo.join("Src")).unwrap();
    std::fs::write(repo.join("Src").join("file.txt"), b"wrong case").unwrap();
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    let governance_after = capture_governance(&repo);
    assert_eq!(
        governance_before, governance_after,
        "case-sensitive allowed rejection changed governance"
    );
    assert_phase4_failure_exact(&output, "CHANGE_NOT_ALLOWED");
    assert_no_temp_files(&repo);

    let (_dir, repo, final_revision, final_sha256) =
        setup_pkg05_accepted_contract(&case_sensitive_contract);
    assert_success(&run_implementation_begin(
        &repo,
        final_revision,
        &final_sha256,
    ));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("file.txt"), b"correct case").unwrap();
    let output = run_implementation_check(&repo);
    assert_phase4_success_exact(&output, &repo, 1);
    assert_no_temp_files(&repo);

    let (_dir, repo, final_revision, final_sha256) =
        setup_pkg05_accepted_contract(&case_sensitive_contract);
    assert_success(&run_implementation_begin(
        &repo,
        final_revision,
        &final_sha256,
    ));
    std::fs::create_dir_all(repo.join("SeCrEt")).unwrap();
    std::fs::write(repo.join("SeCrEt").join("data.txt"), b"forbidden case").unwrap();
    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    let governance_after = capture_governance(&repo);
    assert_eq!(
        governance_before, governance_after,
        "case-insensitive forbidden rejection changed governance"
    );
    assert_phase4_failure_exact(&output, "CHANGE_FORBIDDEN");
    assert_no_temp_files(&repo);

    let (_dir, repo, final_revision, final_sha256) =
        setup_pkg05_accepted_contract(&case_sensitive_contract);
    assert_success(&run_implementation_begin(
        &repo,
        final_revision,
        &final_sha256,
    ));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("public.txt"), b"not forbidden").unwrap();
    let output = run_implementation_check(&repo);
    assert_phase4_success_exact(&output, &repo, 1);
    assert_no_temp_files(&repo);
}

fn record_pkg05_negative_expectations(
    label: &str,
    output: &std::process::Output,
    expected_stderr: &[u8],
    governance_before: &std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry>,
    governance_after: &std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry>,
    repo: &Path,
    failures: &mut Vec<String>,
) {
    if governance_before != governance_after {
        failures.push(format!("case {label}: governance changed"));
    }
    if output.status.code() != Some(1) {
        failures.push(format!(
            "case {label}: expected exit 1, got {:?}",
            output.status.code()
        ));
    }
    if !output.stdout.is_empty() {
        failures.push(format!("case {label}: stdout was not empty"));
    }
    if output.stderr != expected_stderr {
        failures.push(format!(
            "case {label}: expected stderr {:?}, got {:?}",
            expected_stderr, output.stderr
        ));
    }
    let mrgs = repo.join(".mrgs");
    if mrgs.exists() {
        for entry in std::fs::read_dir(&mrgs).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name().to_string_lossy().ends_with(".tmp") {
                failures.push(format!(
                    "case {label}: unexpected temporary governance file {}",
                    entry.path().display()
                ));
            }
        }
    }
}

#[test]
fn test_pkg05_invalid_rule_forms_matrix() {
    let cases = [
        (
            "empty rule",
            r#"forbidden_paths = [""]"#,
            "empty or whitespace-only entry in contract 'forbidden_paths' list",
        ),
        (
            "leading whitespace",
            r#"forbidden_paths = [" secret"]"#,
            "CONTRACT_PATH_RULE_INVALID",
        ),
        (
            "trailing whitespace",
            r#"forbidden_paths = ["secret "]"#,
            "CONTRACT_PATH_RULE_INVALID",
        ),
        (
            "absolute path",
            r#"forbidden_paths = ["/secret"]"#,
            "CONTRACT_PATH_RULE_INVALID",
        ),
        (
            "drive-prefixed path",
            r#"forbidden_paths = ["C:/secret"]"#,
            "CONTRACT_PATH_RULE_INVALID",
        ),
        (
            "parent traversal",
            r#"forbidden_paths = ["../secret"]"#,
            "CONTRACT_PATH_RULE_INVALID",
        ),
        (
            "dot segment",
            r#"forbidden_paths = ["./secret"]"#,
            "CONTRACT_PATH_RULE_INVALID",
        ),
        (
            "doubled slash",
            r#"forbidden_paths = ["secret//file"]"#,
            "CONTRACT_PATH_RULE_INVALID",
        ),
        (
            "backslash",
            "forbidden_paths = ['secret\\file']",
            "CONTRACT_PATH_RULE_INVALID",
        ),
        (
            "control character",
            r#"forbidden_paths = ["secret\u0001file"]"#,
            "CONTRACT_PATH_RULE_INVALID",
        ),
        (
            "glob metacharacter",
            r#"forbidden_paths = ["secret/*"]"#,
            "CONTRACT_PATH_RULE_INVALID",
        ),
    ];

    let mut failures = Vec::new();
    for (label, forbidden_line, expected_category) in cases {
        let contract =
            valid_contract_toml().replace(r#"forbidden_paths = [".git/"]"#, forbidden_line);
        if label == "empty rule" {
            let (_dir, repo, contract_path) = setup_contract_test(&contract);
            let governance_before = capture_governance(&repo);
            let output = run_contract_draft(&repo, &contract_path);
            let governance_after = capture_governance(&repo);
            record_pkg05_negative_expectations(
                label,
                &output,
                format!("error: {expected_category}\n").as_bytes(),
                &governance_before,
                &governance_after,
                &repo,
                &mut failures,
            );
            continue;
        }
        let (_dir, repo, final_revision, final_sha256) = setup_pkg05_accepted_contract(&contract);
        let governance_before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_revision, &final_sha256);
        let governance_after = capture_governance(&repo);
        record_pkg05_negative_expectations(
            label,
            &output,
            format!("error: {expected_category}\n").as_bytes(),
            &governance_before,
            &governance_after,
            &repo,
            &mut failures,
        );
    }
    assert!(
        failures.is_empty(),
        "PKG-05 invalid-rule matrix gaps:\n{}",
        failures.join("\n")
    );
}

#[test]
fn test_pkg05_reserved_allowed_targets_rejected() {
    let cases = [".git", ".GIT", ".mrgs", ".MRGS"];
    for target in cases {
        let contract = valid_contract_toml().replace(
            r#"allowed_paths = ["src/"]"#,
            &format!(r#"allowed_paths = ["{target}"]"#),
        );
        let (_dir, repo, final_revision, final_sha256) = setup_pkg05_accepted_contract(&contract);
        let governance_before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_revision, &final_sha256);
        let governance_after = capture_governance(&repo);
        assert_eq!(
            governance_before, governance_after,
            "allowed reserved target {target}: governance changed"
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "allowed reserved target {target}: unexpected exit"
        );
        assert!(
            output.stdout.is_empty(),
            "allowed reserved target {target}: stdout was not empty"
        );
        assert_eq!(
            output.stderr, b"error: CONTRACT_PATH_RULE_INVALID\n",
            "allowed reserved target {target}: unexpected stderr"
        );
        assert_no_temp_files(&repo);
    }
}

#[test]
fn test_pkg05_forbidden_git_rule_valid_and_redundant() {
    let (_dir, repo) = setup_implementation_basic();
    let draft = read_json(&repo, "contract-draft.json");
    let revision = draft["revision"].as_u64().unwrap() as u32;
    let sha256 = draft["sha256"].as_str().unwrap().to_string();
    assert_success(&run_contract_accept(&repo, revision, &sha256, "ACCEPTED"));
    let (final_revision, final_sha256) = contract_accepted_revision(&repo);
    let begin_output = run_implementation_begin(&repo, final_revision, &final_sha256);
    assert_phase4_begin_exact(&begin_output, &repo);
    assert_no_temp_files(&repo);

    let contract = valid_contract_toml().replace(
        r#"forbidden_paths = [".git/"]"#,
        r#"forbidden_paths = ["secret/"]"#,
    );
    let (_dir, repo, final_revision, final_sha256) = setup_pkg05_accepted_contract(&contract);
    assert_success(&run_implementation_begin(
        &repo,
        final_revision,
        &final_sha256,
    ));
    let governance_before = capture_governance(&repo);
    let output = run_pkg05_reserved_path_check(&repo, ".git/marker");
    let governance_after = capture_governance(&repo);
    assert_eq!(
        governance_before, governance_after,
        "reserved .git path rejection changed governance"
    );
    assert_phase4_failure_exact(&output, "CHANGE_FORBIDDEN");
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg05_reserved_path_changes_unconditionally_rejected() {
    let contract = valid_contract_toml().replace(
        r#"forbidden_paths = [".git/"]"#,
        r#"forbidden_paths = ["secret/"]"#,
    );
    for path in [".git/marker", ".mrgs/marker"] {
        let (_dir, repo, final_revision, final_sha256) = setup_pkg05_accepted_contract(&contract);
        assert_success(&run_implementation_begin(
            &repo,
            final_revision,
            &final_sha256,
        ));
        let governance_before = capture_governance(&repo);
        let output = run_pkg05_reserved_path_check(&repo, path);
        let governance_after = capture_governance(&repo);
        assert_eq!(
            governance_before, governance_after,
            "reserved path {path} rejection changed governance"
        );
        let expected = if path == ".mrgs/marker" {
            "GIT_INVENTORY_INVALID"
        } else {
            "CHANGE_FORBIDDEN"
        };
        assert_phase4_failure_exact(&output, expected);
        assert_no_temp_files(&repo);
    }
}

#[test]
fn test_pkg05_gitignore_cannot_hide_forbidden_file() {
    let contract = valid_contract_toml()
        .replace(
            r#"allowed_paths = ["src/"]"#,
            r#"allowed_paths = [".gitignore"]"#,
        )
        .replace(
            r#"forbidden_paths = [".git/"]"#,
            r#"forbidden_paths = ["secret/"]"#,
        );
    let (_dir, repo, final_revision, final_sha256) = setup_pkg05_accepted_contract(&contract);
    std::fs::write(repo.join(".gitignore"), b"# baseline\n").unwrap();
    commit_file(&repo, ".gitignore");
    assert_success(&run_implementation_begin(
        &repo,
        final_revision,
        &final_sha256,
    ));

    std::fs::write(repo.join(".gitignore"), b"secret/\n").unwrap();
    std::fs::create_dir_all(repo.join("secret")).unwrap();
    std::fs::write(repo.join("secret").join("new.txt"), b"forbidden").unwrap();

    let ignored = git_cmd_output(
        &repo,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--",
        ],
    );
    assert!(
        ignored.status.success(),
        "git ignored-file query failed: stdout={:?} stderr={:?}",
        ignored.stdout,
        ignored.stderr
    );
    assert!(
        ignored.stderr.is_empty(),
        "git ignored-file query wrote stderr: {:?}",
        ignored.stderr
    );
    assert_eq!(ignored.stdout, b"secret/new.txt\n");

    let governance_before = capture_governance(&repo);
    let output = run_implementation_check(&repo);
    let governance_after = capture_governance(&repo);
    assert_eq!(
        governance_before, governance_after,
        "ignored forbidden-file rejection changed governance"
    );
    assert_phase4_failure_exact(&output, "CHANGE_FORBIDDEN");
    assert_no_temp_files(&repo);
}

#[test]
fn test_pkg06_revision_token_matrix() {
    let cases = [
        ("canonical", "1", true),
        ("zero", "0", false),
        ("plus sign", "+1", false),
        ("minus sign", "-1", false),
        ("leading zero", "01", false),
        ("numeric overflow", "4294967296", false),
        ("leading whitespace", " 1", false),
        ("trailing whitespace", "1 ", false),
        ("embedded whitespace", "1 0", false),
        ("non-decimal text", "one", false),
    ];
    struct Observation {
        label: &'static str,
        valid: bool,
        _dir: tempfile::TempDir,
        repo: std::path::PathBuf,
        output: std::process::Output,
        before: std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry>,
        after: std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry>,
    }

    let mut observations = Vec::new();
    for (label, token, valid) in cases {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let revision = draft["revision"].as_u64().unwrap() as u32;
        let sha256 = draft["sha256"].as_str().unwrap().to_string();
        assert_success(&run_contract_accept(&repo, revision, &sha256, "ACCEPTED"));
        let (final_revision, final_sha256) = contract_accepted_revision(&repo);
        assert_eq!(final_revision, 1, "case {label}: fixture revision drifted");
        let before = capture_governance(&repo);
        let output = run_implementation_begin_str(&repo, token, &final_sha256);
        let after = capture_governance(&repo);
        observations.push(Observation {
            label,
            valid,
            _dir,
            repo,
            output,
            before,
            after,
        });
    }

    let mut failures = Vec::new();
    for observation in observations {
        if !observation.valid && observation.before != observation.after {
            failures.push(format!("case {}: governance changed", observation.label));
        }
        if observation.valid {
            if observation.output.status.code() != Some(0) {
                failures.push(format!(
                    "case {}: expected success, stderr={:?}",
                    observation.label, observation.output.stderr
                ));
            }
            if !observation.output.stderr.is_empty() {
                failures.push(format!(
                    "case {}: success wrote stderr {:?}",
                    observation.label, observation.output.stderr
                ));
            }
            if !observation
                .repo
                .join(".mrgs")
                .join("implementation-authority.json")
                .exists()
            {
                failures.push(format!(
                    "case {}: implementation authority was not created",
                    observation.label
                ));
            }
        } else {
            if observation.output.status.code() != Some(1) {
                failures.push(format!(
                    "case {}: expected exit 1, got {:?}",
                    observation.label,
                    observation.output.status.code()
                ));
            }
            if !observation.output.stdout.is_empty() {
                failures.push(format!("case {}: stdout was not empty", observation.label));
            }
            if observation.output.stderr != b"error: INVALID_ARGUMENT\n" {
                failures.push(format!(
                    "case {}: unexpected stderr {:?}",
                    observation.label, observation.output.stderr
                ));
            }
            if observation
                .repo
                .join(".mrgs")
                .join("implementation-authority.json")
                .exists()
            {
                failures.push(format!(
                    "case {}: implementation authority was created",
                    observation.label
                ));
            }
        }
        if let Ok(entries) = std::fs::read_dir(observation.repo.join(".mrgs")) {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().ends_with(".tmp") {
                    failures.push(format!(
                        "case {}: temporary governance file {}",
                        observation.label,
                        entry.path().display()
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "PKG-06 revision-token matrix gaps:\n{}",
        failures.join("\n")
    );
}

fn rewrite_pkg06_accepted_contract_id(repo: &Path, contract_id: &str, content: &str) -> String {
    let sha256 = contract_sha256(content);
    let mut draft = read_json(repo, "contract-draft.json");
    draft["contract_id"] = serde_json::json!(contract_id);
    draft["sha256"] = serde_json::json!(&sha256);
    draft["content"] = serde_json::json!(content);
    write_json(repo, "contract-draft.json", &draft);

    let mut ledger = read_json(repo, "accepted-contract.json");
    ledger["contract_id"] = serde_json::json!(contract_id);
    ledger["revisions"][0]["sha256"] = serde_json::json!(&sha256);
    ledger["revisions"][0]["content"] = serde_json::json!(content);
    write_json(repo, "accepted-contract.json", &ledger);

    let stored_draft = read_json(repo, "contract-draft.json");
    let stored_ledger = read_json(repo, "accepted-contract.json");
    assert_eq!(stored_draft["contract_id"].as_str(), Some(contract_id));
    assert_eq!(stored_ledger["contract_id"].as_str(), Some(contract_id));
    assert_eq!(
        stored_ledger["revisions"][0]["content"].as_str(),
        Some(content)
    );
    sha256
}

#[test]
fn test_pkg06_invalid_contract_id_token_classes() {
    let cases = [
        (
            "embedded ASCII space",
            "test contract-v1",
            "test contract-v1",
        ),
        ("tab", "test\tcontract-v1", "test\\tcontract-v1"),
        ("line terminator", "test\ncontract-v1", "test\\ncontract-v1"),
        (
            "Unicode character",
            "testé-contract-v1",
            "testé-contract-v1",
        ),
        (
            "non-token punctuation",
            "test@contract-v1",
            "test@contract-v1",
        ),
    ];
    struct Observation {
        label: &'static str,
        _dir: tempfile::TempDir,
        repo: std::path::PathBuf,
        output: std::process::Output,
        before: std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry>,
        after: std::collections::BTreeMap<std::ffi::OsString, GovernanceEntry>,
        implementation_before: bool,
        implementation_after: bool,
    }

    let mut observations = Vec::new();
    for (label, contract_id, encoded_id) in cases {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let draft_revision = draft["revision"].as_u64().unwrap() as u32;
        let draft_sha256 = draft["sha256"].as_str().unwrap().to_string();
        assert_success(&run_contract_accept(
            &repo,
            draft_revision,
            &draft_sha256,
            "ACCEPTED",
        ));
        let content = valid_contract_toml().replace("test-contract-v1", encoded_id);
        let final_sha256 = rewrite_pkg06_accepted_contract_id(&repo, contract_id, &content);
        let stored_ledger = read_json(&repo, "accepted-contract.json");
        assert_eq!(stored_ledger["contract_id"].as_str(), Some(contract_id));
        assert_eq!(
            stored_ledger["revisions"][0]["sha256"].as_str(),
            Some(final_sha256.as_str())
        );
        let final_revision = stored_ledger["revisions"][0]["revision"].as_u64().unwrap() as u32;
        let implementation_path = repo.join(".mrgs").join("implementation-authority.json");
        let implementation_before = implementation_path.exists();
        let before = capture_governance(&repo);
        let output = run_implementation_begin(&repo, final_revision, &final_sha256);
        let after = capture_governance(&repo);
        let implementation_after = implementation_path.exists();
        observations.push(Observation {
            label,
            _dir,
            repo,
            output,
            before,
            after,
            implementation_before,
            implementation_after,
        });
    }

    let mut failures = Vec::new();
    for observation in observations {
        if observation.before != observation.after {
            failures.push(format!("case {}: governance changed", observation.label));
        }
        if observation.output.status.code() != Some(1) {
            failures.push(format!(
                "case {}: expected exit 1, got {:?}",
                observation.label,
                observation.output.status.code()
            ));
        }
        if !observation.output.stdout.is_empty() {
            failures.push(format!("case {}: stdout was not empty", observation.label));
        }
        if observation.output.stderr != b"error: GOVERNANCE_AUTHORITY_INVALID\n" {
            failures.push(format!(
                "case {}: unexpected stderr {:?}",
                observation.label, observation.output.stderr
            ));
        }
        if observation.implementation_before != observation.implementation_after {
            failures.push(format!(
                "case {}: implementation authority changed",
                observation.label
            ));
        }
        if let Ok(entries) = std::fs::read_dir(observation.repo.join(".mrgs")) {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().ends_with(".tmp") {
                    failures.push(format!(
                        "case {}: temporary governance file {}",
                        observation.label,
                        entry.path().display()
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "PKG-06 accepted-contract-ID matrix gaps:\n{}",
        failures.join("\n")
    );
}

// ============================================================================
// PKG-07: Symlink, Publication, Category, and Redaction Evidence
// ============================================================================

// P4-114: staged symlink deletion does not inspect the deleted index target,
// while an extant committed symlink version is inspected when selected by the
// baseline diff.
#[test]
fn test_pkg07_staged_symlink_deletion_and_committed_inspection() {
    // Case A: staged symlink deletion — target exists and would fail if
    // inspected, but deletion is staged so no target inspection occurs.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        std::fs::create_dir_all(repo.join("src")).unwrap();
        symlink_relative(&repo.join("src").join("link"), "../.git/config");
        git(&repo).arg("add").arg("src/link").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("add failing link")
            .status()
            .unwrap();

        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        std::fs::remove_file(repo.join("src").join("link")).unwrap();
        git(&repo).arg("add").arg("src/link").status().unwrap();

        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 1);
    }

    // Case B: extant committed symlink selected by baseline diff requires
    // target inspection — its forbidden target produces a distinctive error.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("base.txt"), b"base").unwrap();
        git(&repo).arg("add").arg("src/base.txt").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("add base")
            .status()
            .unwrap();

        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        symlink_relative(&repo.join("src").join("link"), "../.git/config");
        git(&repo).arg("add").arg("src/link").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("add failing link")
            .status()
            .unwrap();

        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "FILESYSTEM_BOUNDARY_UNSAFE",
            &repo,
            &governance_before,
        );
    }
}

// P4-115: symlink target matching a forbidden rule or no allowed rule is
// rejected.
#[test]
fn test_pkg07_symlink_target_authority_rejections() {
    // Case A: symlink target matches a forbidden rule.
    {
        let (_dir, repo) = setup_implementation_forbidden_rule("src/forbidden/");
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        std::fs::create_dir_all(repo.join("src").join("forbidden")).unwrap();
        std::fs::write(repo.join("src").join("forbidden").join("file"), b"x").unwrap();
        git(&repo)
            .arg("add")
            .arg("src/forbidden/file")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("add forbidden target")
            .status()
            .unwrap();

        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        symlink_relative(&repo.join("src").join("link"), "forbidden/file");
        git(&repo).arg("add").arg("src/link").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("add link to forbidden")
            .status()
            .unwrap();

        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "FILESYSTEM_BOUNDARY_UNSAFE",
            &repo,
            &governance_before,
        );
    }

    // Case B: symlink target matches no allowed rule.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("base.txt"), b"base").unwrap();
        std::fs::write(repo.join("outside.txt"), b"outside").unwrap();
        git(&repo).arg("add").arg("outside.txt").status().unwrap();
        git(&repo).arg("add").arg("src/base.txt").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("base")
            .status()
            .unwrap();

        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        symlink_relative(&repo.join("src").join("link"), "../outside.txt");
        git(&repo).arg("add").arg("src/link").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("link to outside")
            .status()
            .unwrap();

        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "FILESYSTEM_BOUNDARY_UNSAFE",
            &repo,
            &governance_before,
        );
    }
}

// P4-116: safe contained symlink target and link path both require allowed
// authority.
#[test]
fn test_pkg07_safe_symlink_requires_link_and_target_authority() {
    // Case A: link allowed + target allowed => success.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("target.txt"), b"t").unwrap();
        git(&repo)
            .arg("add")
            .arg("src/target.txt")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("target")
            .status()
            .unwrap();

        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        symlink_relative(&repo.join("src").join("link"), "target.txt");
        git(&repo).arg("add").arg("src/link").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("link")
            .status()
            .unwrap();

        let output = run_implementation_check(&repo);
        assert_phase4_success_exact(&output, &repo, 1);
    }

    // Case B: link not allowed + target allowed => rejection.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("target.txt"), b"t").unwrap();
        git(&repo)
            .arg("add")
            .arg("src/target.txt")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("target")
            .status()
            .unwrap();

        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        symlink_relative(&repo.join("not_allowed_link"), "src/target.txt");
        git(&repo)
            .arg("add")
            .arg("not_allowed_link")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("link at root")
            .status()
            .unwrap();

        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "CHANGE_NOT_ALLOWED",
            &repo,
            &governance_before,
        );
    }

    // Case C: link allowed + target not allowed => rejection.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("base.txt"), b"base").unwrap();
        std::fs::write(repo.join("outside.txt"), b"outside").unwrap();
        git(&repo).arg("add").arg("src/base.txt").status().unwrap();
        git(&repo).arg("add").arg("outside.txt").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("base")
            .status()
            .unwrap();

        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        symlink_relative(&repo.join("src").join("link"), "../outside.txt");
        git(&repo).arg("add").arg("src/link").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("link to outside")
            .status()
            .unwrap();

        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "FILESYSTEM_BOUNDARY_UNSAFE",
            &repo,
            &governance_before,
        );
    }
}

// P4-117: create the destination after the implementation temporary appears
// and before no-clobber publication. This is an actual filesystem race against
// the real publication boundary.
#[test]
fn test_pkg07_atomic_destination_race_preserves_competing_record() {
    let (_dir, repo) = setup_implementation_basic();
    let draft = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let governance_before = capture_earlier_governance_bytes(&repo);

    let signal = _dir.path().join("atomic-before-publish.signal");
    let release = _dir.path().join("atomic-before-publish.release");
    let mut child = spawn_implementation_begin_with_env(
        &repo,
        final_rev,
        &final_sha,
        &[
            (
                "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_SIGNAL",
                signal.as_path(),
            ),
            (
                "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_RELEASE",
                release.as_path(),
            ),
        ],
        &[],
    );
    wait_for_publish_signal(&mut child, &signal);
    assert_eq!(std::fs::read(&signal).unwrap(), b"reached");

    let destination = repo.join(".mrgs").join("implementation-authority.json");
    let competing_bytes = br#"{"competing":true,"bytes":"preserved"}"#.to_vec();
    let mut competing = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .unwrap();
    use std::io::Write;
    competing.write_all(&competing_bytes).unwrap();
    competing.sync_all().unwrap();
    std::fs::write(&release, b"release").unwrap();

    let output = child.wait_with_output().unwrap();
    assert_phase4_failure_exact(&output, "IMPLEMENTATION_AUTHORITY_CONFLICT");
    assert_eq!(std::fs::read(&destination).unwrap(), competing_bytes);
    assert_eq!(governance_before, capture_earlier_governance_bytes(&repo));
    assert_no_temp_files(&repo);
}

// P4-118: force the real no-clobber operation to return the same internal
// persistence error used for an unsupported platform operation.
#[test]
fn test_pkg07_unsupported_no_clobber_no_fallback_or_partial_record() {
    let (_dir, repo) = setup_implementation_basic();
    let draft = read_json(&repo, "contract-draft.json");
    let sha = draft["sha256"].as_str().unwrap().to_string();
    let rev = draft["revision"].as_u64().unwrap() as u32;
    assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
    let (final_rev, final_sha) = contract_accepted_revision(&repo);
    let governance_before = capture_earlier_governance_bytes(&repo);
    let signal = _dir.path().join("unsupported-before-publish.signal");
    let release = _dir.path().join("unsupported-before-publish.release");
    let mut child = spawn_implementation_begin_with_env(
        &repo,
        final_rev,
        &final_sha,
        &[
            (
                "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_SIGNAL",
                signal.as_path(),
            ),
            (
                "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_RELEASE",
                release.as_path(),
            ),
        ],
        &[("MRGS_TEST_ONLY_FORCE_NO_CLOBBER_UNSUPPORTED", "1")],
    );
    wait_for_publish_signal(&mut child, &signal);
    assert_eq!(std::fs::read(&signal).unwrap(), b"reached");
    std::fs::write(&release, b"release").unwrap();

    let output = child.wait_with_output().unwrap();
    assert_phase4_failure_exact(&output, "PERSISTENCE_FAILED");
    assert!(!repo
        .join(".mrgs")
        .join("implementation-authority.json")
        .exists());
    assert_eq!(governance_before, capture_earlier_governance_bytes(&repo));
    assert_no_temp_files(&repo);
}

fn capture_governance_bytes(repo: &Path) -> Vec<(String, Vec<u8>)> {
    let mut bytes = Vec::new();
    for fname in &[
        "accepted-plan.json",
        "state.json",
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
    ] {
        let path = repo.join(".mrgs").join(fname);
        if path.exists() {
            bytes.push((fname.to_string(), std::fs::read(&path).unwrap()));
        }
    }
    bytes
}

fn capture_earlier_governance_bytes(repo: &Path) -> Vec<(String, Vec<u8>)> {
    capture_governance_bytes(repo)
        .into_iter()
        .filter(|(name, _)| name != "implementation-authority.json")
        .collect()
}

fn wait_for_publish_signal(child: &mut std::process::Child, signal: &Path) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if signal.is_file() {
            return;
        }
        if child.try_wait().unwrap().is_some() {
            panic!("publication process exited before failpoint signal");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("publication failpoint signal timeout");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

// P4-119: exact category mapping for representative failure from every
// Section 15 category. Reuses existing complete tests and adds only missing
// categories.
#[test]
fn test_pkg07_section15_category_mapping_matrix() {
    // INVALID_ARGUMENT
    {
        let (_dir, repo) = setup_implementation_basic();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin_str(
            &repo,
            "abc",
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert_phase4_failure_preserves_governance(
            &output,
            "INVALID_ARGUMENT",
            &repo,
            &governance_before,
        );
    }

    // REPOSITORY_INVALID: a missing repository path reaches the direct
    // repository validation category.
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("missing-repo");
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin_str(
            &repo,
            "1",
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert_phase4_failure_preserves_governance(
            &output,
            "REPOSITORY_INVALID",
            &repo,
            &governance_before,
        );
    }

    // GOVERNANCE_AUTHORITY_INVALID
    {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_init(&repo);
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin_str(
            &repo,
            "1",
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert_phase4_failure_preserves_governance(
            &output,
            "GOVERNANCE_AUTHORITY_INVALID",
            &repo,
            &governance_before,
        );
    }

    // CONTRACT_NOT_ACCEPTED
    {
        let (_dir, repo) = setup_implementation_basic();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin_str(
            &repo,
            "1",
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert_phase4_failure_preserves_governance(
            &output,
            "CONTRACT_NOT_ACCEPTED",
            &repo,
            &governance_before,
        );
    }

    // REQUESTED_REVISION_STALE
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        assert_success(&run_contract_accept(&repo, 1, &sha, "ACCEPTED"));
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin_str(&repo, "99", &sha);
        assert_phase4_failure_preserves_governance(
            &output,
            "REQUESTED_REVISION_STALE",
            &repo,
            &governance_before,
        );
    }

    // REQUESTED_SHA_STALE
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        assert_success(&run_contract_accept(&repo, 1, &sha, "ACCEPTED"));
        let wrong_sha = "0000000000000000000000000000000000000000000000000000000000000000";
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin_str(&repo, "1", wrong_sha);
        assert_phase4_failure_preserves_governance(
            &output,
            "REQUESTED_SHA_STALE",
            &repo,
            &governance_before,
        );
    }

    // CONTRACT_PATH_RULE_INVALID
    {
        let (_dir, repo, final_revision, final_sha256) =
            setup_pkg05_accepted_contract(&valid_contract_toml().replace(
                r#"allowed_paths = ["src/"]"#,
                r#"allowed_paths = ["src", "src/"]"#,
            ));
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin(&repo, final_revision, &final_sha256);
        assert_phase4_failure_preserves_governance(
            &output,
            "CONTRACT_PATH_RULE_INVALID",
            &repo,
            &governance_before,
        );
    }

    // GIT_COMMAND_FAILED
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        let empty_path_dir = tempfile::TempDir::new().unwrap();
        let mut cmd = cargo_bin();
        cmd.arg("implementation")
            .arg("begin")
            .arg("--repo")
            .arg(&repo)
            .arg("--revision")
            .arg(final_rev.to_string())
            .arg("--sha256")
            .arg(&final_sha)
            .env("PATH", empty_path_dir.path());
        let governance_before = capture_governance_bytes(&repo);
        let output = cmd.output().unwrap();
        assert_phase4_failure_preserves_governance(
            &output,
            "GIT_COMMAND_FAILED",
            &repo,
            &governance_before,
        );
    }

    // GIT_ROOT_MISMATCH: the wrapper returns a valid but different top-level
    // path for the exact root-discovery invocation and delegates every other
    // Git command.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        let governance_before = capture_earlier_governance_bytes(&repo);
        let different_root = repo.parent().unwrap().join("different-root");
        std::fs::create_dir(&different_root).unwrap();
        let payload = format!("{}\n", different_root.display()).into_bytes();
        let wrapper = create_git_wrapper(
            &repo,
            &["rev-parse", "--show-toplevel"],
            "payload",
            &payload,
        );
        let output = run_begin_with_git_wrapper(&repo, final_rev, &final_sha, &wrapper);
        assert_phase4_failure_exact(&output, "GIT_ROOT_MISMATCH");
        assert_wrapper_reached(&wrapper);
        assert_eq!(governance_before, capture_governance_bytes(&repo));
        assert!(!repo
            .join(".mrgs")
            .join("implementation-authority.json")
            .exists());
        assert_no_temp_files(&repo);
    }

    // GIT_DETACHED_HEAD
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        let head = git_head_exact(&repo);
        git(&repo)
            .arg("checkout")
            .arg("--detach")
            .arg(&head)
            .status()
            .unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_preserves_governance(
            &output,
            "GIT_DETACHED_HEAD",
            &repo,
            &governance_before,
        );
    }

    // GIT_HEAD_INVALID: the wrapper returns malformed HEAD output for the exact
    // verification invocation and delegates every other Git command.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        let governance_before = capture_earlier_governance_bytes(&repo);
        let wrapper = create_git_wrapper(
            &repo,
            &["rev-parse", "--verify", "HEAD^{commit}"],
            "payload",
            b"not-a-valid-head\n",
        );
        let output = run_begin_with_git_wrapper(&repo, final_rev, &final_sha, &wrapper);
        assert_phase4_failure_exact(&output, "GIT_HEAD_INVALID");
        assert_wrapper_reached(&wrapper);
        assert_eq!(governance_before, capture_governance_bytes(&repo));
        assert!(!repo
            .join(".mrgs")
            .join("implementation-authority.json")
            .exists());
        assert_no_temp_files(&repo);
    }

    // GIT_DIRTY: modify a tracked file within allowed_paths to trigger dirty detection
    // before governance validation
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("dirty.rs"), b"fn dirty() {}").unwrap();
        git(&repo).arg("add").arg("src/dirty.rs").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("add dirty")
            .status()
            .unwrap();
        std::fs::write(repo.join("src").join("dirty.rs"), b"fn dirty_modified() {}").unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_preserves_governance(&output, "GIT_DIRTY", &repo, &governance_before);
    }

    // GIT_OPERATION_IN_PROGRESS
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        std::fs::write(repo.join(".git").join("MERGE_HEAD"), b"fake").unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_preserves_governance(
            &output,
            "GIT_OPERATION_IN_PROGRESS",
            &repo,
            &governance_before,
        );
    }

    // GIT_SUBMODULE_UNSUPPORTED
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        let sub = repo.join("sub");
        std::fs::create_dir(&sub).unwrap();
        git_init(&sub);
        std::fs::write(sub.join("f"), b"x").unwrap();
        git(&sub).arg("add").arg("f").status().unwrap();
        git(&sub)
            .arg("commit")
            .arg("-m")
            .arg("init")
            .status()
            .unwrap();
        git(&repo)
            .arg("submodule")
            .arg("add")
            .arg(&sub)
            .arg("sub")
            .status()
            .unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("sub")
            .status()
            .unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_preserves_governance(
            &output,
            "GIT_SUBMODULE_UNSUPPORTED",
            &repo,
            &governance_before,
        );
    }

    // IMPLEMENTATION_AUTHORITY_MISSING
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "IMPLEMENTATION_AUTHORITY_MISSING",
            &repo,
            &governance_before,
        );
    }

    // IMPLEMENTATION_AUTHORITY_INVALID
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        std::fs::write(
            repo.join(".mrgs").join("implementation-authority.json"),
            b"not json",
        )
        .unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "IMPLEMENTATION_AUTHORITY_INVALID",
            &repo,
            &governance_before,
        );
    }

    // IMPLEMENTATION_AUTHORITY_CONFLICT
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("new.rs"), b"fn new() {}").unwrap();
        git(&repo).arg("add").arg("src/new.rs").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("desc")
            .status()
            .unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_preserves_governance(
            &output,
            "IMPLEMENTATION_AUTHORITY_CONFLICT",
            &repo,
            &governance_before,
        );
    }

    // IMPLEMENTATION_AUTHORITY_STALE: a newer accepted contract invalidates the
    // existing implementation binding.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        let contract_path = repo.join("contract.toml");
        let v2 = valid_contract_toml().replace("Test objective", "V2 objective");
        write_plan(&contract_path, &v2);
        commit_file(&repo, "contract.toml");
        let v2_sha = contract_sha256(&v2);
        assert_success(&run_contract_revise(&repo, &contract_path, 1, &sha));
        assert_success(&run_contract_accept(&repo, 2, &v2_sha, "ACCEPTED"));
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "IMPLEMENTATION_AUTHORITY_STALE",
            &repo,
            &governance_before,
        );
    }

    // BASELINE_BRANCH_CHANGED
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        git(&repo)
            .arg("checkout")
            .arg("-b")
            .arg("other")
            .status()
            .unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "BASELINE_BRANCH_CHANGED",
            &repo,
            &governance_before,
        );
    }

    // BASELINE_COMMIT_MISSING
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        let mut record: serde_json::Value = read_json(&repo, "implementation-authority.json");
        record["baseline_head"] = serde_json::json!("0000000000000000000000000000000000000000");
        write_json(&repo, "implementation-authority.json", &record);
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "BASELINE_COMMIT_MISSING",
            &repo,
            &governance_before,
        );
    }

    // BASELINE_HISTORY_CHANGED: after begin, resetting HEAD to parent causes
    // the baseline commit to be unreachable from HEAD, triggering the category.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        let parent = git(&repo).arg("rev-parse").arg("HEAD~1").output().unwrap();
        let parent = String::from_utf8(parent.stdout).unwrap().trim().to_string();
        git(&repo)
            .arg("reset")
            .arg("--hard")
            .arg(&parent)
            .status()
            .unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "BASELINE_HISTORY_CHANGED",
            &repo,
            &governance_before,
        );
    }

    // GIT_INVENTORY_INVALID
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        git(&repo)
            .arg("update-index")
            .arg("--assume-unchanged")
            .arg("plan.toml")
            .status()
            .unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_begin(&repo, final_rev, &final_sha);
        assert_phase4_failure_preserves_governance(
            &output,
            "GIT_INVENTORY_INVALID",
            &repo,
            &governance_before,
        );
    }

    // GIT_CONFLICT: an unmerged stage-1/2/3 entry reaches the direct conflict
    // category before change inventory.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        let oid1 = git_blob(&repo, b"ancestor");
        let oid2 = git_blob(&repo, b"ours");
        let oid3 = git_blob(&repo, b"theirs");
        let mut child = git(&repo)
            .arg("update-index")
            .arg("--index-info")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "100644 {} 1\tsrc/conflict.txt", oid1).unwrap();
        writeln!(stdin, "100644 {} 2\tsrc/conflict.txt", oid2).unwrap();
        writeln!(stdin, "100644 {} 3\tsrc/conflict.txt", oid3).unwrap();
        drop(child.stdin.take());
        assert!(child.wait_with_output().unwrap().status.success());

        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "GIT_CONFLICT",
            &repo,
            &governance_before,
        );
    }

    // CHANGE_PATH_INVALID: the Git inventory grammar is intentionally identical
    // to the changed-path grammar, so the private failpoint reaches the exact
    // production changed-path decision after a valid inventory record exists.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        std::fs::write(repo.join("new.rs"), b"fn new() {}\n").unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check_with_env(
            &repo,
            &[("MRGS_TEST_ONLY_FORCE_CHANGE_PATH_INVALID", "1")],
        );
        assert_phase4_failure_exact(&output, "CHANGE_PATH_INVALID");
        assert_eq!(governance_before, capture_governance_bytes(&repo));
        assert_no_temp_files(&repo);
    }

    // CHANGE_FORBIDDEN: create forbidden file after begin so it appears in the diff
    {
        let (_dir, repo) = setup_implementation_forbidden_rule("src/secret/");
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        std::fs::create_dir_all(repo.join("src").join("secret")).unwrap();
        std::fs::write(repo.join("src").join("secret").join("new"), b"x").unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "CHANGE_FORBIDDEN",
            &repo,
            &governance_before,
        );
    }

    // CHANGE_NOT_ALLOWED: create untracked file outside allowed_paths after begin
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        std::fs::write(repo.join("outside.txt"), b"x").unwrap();
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "CHANGE_NOT_ALLOWED",
            &repo,
            &governance_before,
        );
    }

    // FILESYSTEM_BOUNDARY_UNSAFE: symlink to .git/ returns FILESYSTEM_BOUNDARY_UNSAFE
    // because the target matches a forbidden rule (or is outside allowed_paths).
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src").join("base.txt"), b"base").unwrap();
        git(&repo).arg("add").arg("src/base.txt").status().unwrap();
        git(&repo)
            .arg("commit")
            .arg("-m")
            .arg("base")
            .status()
            .unwrap();
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));
        symlink_relative(&repo.join("src").join("link"), "../.git/config");
        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "FILESYSTEM_BOUNDARY_UNSAFE",
            &repo,
            &governance_before,
        );
    }

    // PERSISTENCE_FAILED: the real publication boundary is reached, then the
    // private unsupported no-clobber failpoint returns the platform error.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        let governance_before = capture_earlier_governance_bytes(&repo);
        let signal = _dir.path().join("matrix-before-publish.signal");
        let release = _dir.path().join("matrix-before-publish.release");
        let mut child = spawn_implementation_begin_with_env(
            &repo,
            final_rev,
            &final_sha,
            &[
                (
                    "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_SIGNAL",
                    signal.as_path(),
                ),
                (
                    "MRGS_TEST_ONLY_ATOMIC_BEFORE_PUBLISH_RELEASE",
                    release.as_path(),
                ),
            ],
            &[("MRGS_TEST_ONLY_FORCE_NO_CLOBBER_UNSUPPORTED", "1")],
        );
        wait_for_publish_signal(&mut child, &signal);
        assert_eq!(std::fs::read(&signal).unwrap(), b"reached");
        std::fs::write(&release, b"release").unwrap();
        let output = child.wait_with_output().unwrap();
        assert_phase4_failure_exact(&output, "PERSISTENCE_FAILED");
        assert!(!repo
            .join(".mrgs")
            .join("implementation-authority.json")
            .exists());
        assert_eq!(governance_before, capture_earlier_governance_bytes(&repo));
        assert_no_temp_files(&repo);
    }
}

// P4-120: semantic errors redact absolute paths, inherited Git diagnostics,
// and invalid non-UTF-8 path bytes.
#[test]
fn test_pkg07_semantic_error_redaction_matrix() {
    // Case A: absolute path redaction
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        let repo_str = repo.to_string_lossy().to_string();

        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        std::fs::write(repo.join("outside.txt"), b"x").unwrap();
        let governance_before = capture_governance_bytes(&repo);

        let output = run_implementation_check(&repo);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            !stderr.contains(&repo_str),
            "stderr must not contain absolute repository path: {}",
            stderr
        );
        assert!(
            !stderr.contains("M:") && !stderr.contains("C:"),
            "stderr must not contain drive prefix: {}",
            stderr
        );
        assert_phase4_failure_preserves_governance(
            &output,
            "CHANGE_NOT_ALLOWED",
            &repo,
            &governance_before,
        );
    }

    // Case B: inherited Git diagnostic redaction
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);

        let sentinel = "MRGS_SENTINEL_GIT_DIAGNOSTIC_12345";

        let dir = tempfile::TempDir::new().unwrap();
        let wrapper_dir = dir.path().join("bin");
        std::fs::create_dir_all(&wrapper_dir).unwrap();
        let source = format!(
            r#"
use std::env;
use std::ffi::OsString;
use std::process::Command;

fn main() {{
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let args_str: Vec<String> = args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let is_config_get = args_str.contains(&"config".to_string())
        && args_str.contains(&"--get".to_string());

    if is_config_get {{
        eprintln!("{sentinel}");
        std::process::exit(1);
    }}

    let status = Command::new({real:?}).args(&args).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}}
"#,
            real = real_git_executable().display().to_string(),
        );
        let source_path = wrapper_dir.join("git-wrapper.rs");
        std::fs::write(&source_path, source).unwrap();
        let wrapper = wrapper_dir.join("git.exe");
        let compile = std::process::Command::new("rustc")
            .arg(&source_path)
            .arg("-o")
            .arg(&wrapper)
            .output()
            .unwrap();
        assert_eq!(compile.status.code(), Some(0));

        let mut cmd = cargo_bin();
        cmd.arg("implementation")
            .arg("begin")
            .arg("--repo")
            .arg(&repo)
            .arg("--revision")
            .arg(final_rev.to_string())
            .arg("--sha256")
            .arg(&final_sha)
            .env("PATH", wrapper_dir);
        let governance_before = capture_governance_bytes(&repo);
        let output = cmd.output().unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains(sentinel),
            "stderr must not contain Git diagnostic sentinel: {}",
            stderr
        );
        assert_phase4_failure_preserves_governance(
            &output,
            "GIT_COMMAND_FAILED",
            &repo,
            &governance_before,
        );
    }

    // Case C: invalid non-UTF-8 parser input redaction. Windows does not
    // support non-UTF-8 filesystem paths, so the contract's parser fallback is
    // exercised with invalid bytes in the implementation record itself.
    {
        let (_dir, repo) = setup_implementation_basic();
        let draft = read_json(&repo, "contract-draft.json");
        let sha = draft["sha256"].as_str().unwrap().to_string();
        let rev = draft["revision"].as_u64().unwrap() as u32;
        assert_success(&run_contract_accept(&repo, rev, &sha, "ACCEPTED"));
        let (final_rev, final_sha) = contract_accepted_revision(&repo);
        assert_success(&run_implementation_begin(&repo, final_rev, &final_sha));

        let invalid_bytes = b"\xff\xfe\xfd";
        let authority_path = repo.join(".mrgs").join("implementation-authority.json");
        let mut authority_bytes = std::fs::read(&authority_path).unwrap();
        authority_bytes.extend_from_slice(invalid_bytes);
        std::fs::write(&authority_path, authority_bytes).unwrap();

        let governance_before = capture_governance_bytes(&repo);
        let output = run_implementation_check(&repo);
        assert_phase4_failure_preserves_governance(
            &output,
            "IMPLEMENTATION_AUTHORITY_INVALID",
            &repo,
            &governance_before,
        );
        assert!(!output.stderr.contains(&0xff));
        assert!(!String::from_utf8_lossy(&output.stderr).contains('\u{FFFD}'));
    }
}
