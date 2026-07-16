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

// 37. persisted draft revision other than 1
#[test]
fn test_draft_persisted_revision_not_one() {
    let (_dir, repo, contract_path) = setup_contract_test(valid_contract_toml());
    assert_success(&run_contract_draft(&repo, &contract_path));
    let mut draft: serde_json::Value = read_json(&repo, "contract-draft.json");
    draft["revision"] = serde_json::json!(2);
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
