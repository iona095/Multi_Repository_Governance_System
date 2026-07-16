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
        err.contains("accept different plan")
            || err.contains("authority exists")
            || err.contains("accepted plan ID mismatch"),
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
    assert!(stderr_string(&output).contains("drift"));
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
    assert!(
        stderr_string(&output).contains("plan ID")
            || stderr_string(&output).contains("drift")
            || stderr_string(&output).contains("mismatch")
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
        stderr_string(&output).contains("drift"),
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
