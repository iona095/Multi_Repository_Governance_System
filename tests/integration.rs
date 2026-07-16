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
