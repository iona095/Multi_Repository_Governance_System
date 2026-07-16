# Phase 1 Contract — Accepted Plan Authority and Phase Selection

Contract version: 1  
Project: Multi-Repository Governance System  
Implementation language: Rust  
Binary name: `mrgs`

## 1. Objective

Create the smallest usable Rust foundation for a central governance CLI that can operate against multiple target repositories.

Phase 1 implements only:

1. exact accepted-plan authority;
2. validation of a governed plan;
3. safe selection of one eligible phase.

It does not implement contract drafting, implementation execution, audit, repair, closeout, Git mutation, networking, background services, or automatic mode chaining.

## 2. Required repository result

The repository must contain:

- `Cargo.toml`
- `Cargo.lock`
- `.gitignore`
- `README.md`
- `src/main.rs`
- supporting Rust modules under `src/`
- integration tests under `tests/`
- `docs/master-plan.md`
- `docs/contracts/phase-01-contract.md`

The Cargo package and executable must be named `mrgs`.

Use Rust edition 2021.

## 3. Governed plan format

The governed plan is a TOML file with this minimum structure:

```toml
schema_version = 1
plan_id = "example-plan"

[[phases]]
id = "phase-1"
title = "First phase"
depends_on = []

[[phases]]
id = "phase-2"
title = "Second phase"
depends_on = ["phase-1"]
```

Validation must reject unsupported schema versions, empty plan IDs, zero phases, empty phase IDs or titles, duplicate phase IDs, unknown dependencies, self-dependencies, and dependency cycles. Phase order must be preserved.

## 4. CLI surface

Implement exactly:

```text
mrgs plan accept --repo <REPOSITORY_PATH> --plan <PLAN_PATH>
mrgs phase select --repo <REPOSITORY_PATH> --phase <PHASE_ID>
```

No other command is required.

### 4.1 `plan accept`

It must:

1. canonicalize repository and plan paths;
2. require the plan inside the target repository;
3. read exact plan bytes;
4. parse and validate the plan;
5. compute lowercase SHA-256 over exact bytes;
6. create `<repo>/.mrgs/accepted-plan.json`;
7. create `<repo>/.mrgs/state.json`;
8. print the plan ID and SHA-256.

`accepted-plan.json` must contain at least:

```json
{
  "schema_version": 1,
  "plan_id": "example-plan",
  "plan_path": "relative/path/inside/repo.toml",
  "sha256": "lowercase-hex",
  "phase_count": 2
}
```

`state.json` must contain at least:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-hex",
  "active_phase": null,
  "closed_phases": []
}
```

Rules:

- first valid acceptance succeeds;
- repeating the same exact plan is idempotent;
- accepting different bytes when authority exists fails;
- no replacement or revision behavior is allowed.

### 4.2 `phase select`

It must:

1. load accepted plan and state;
2. reload the recorded plan;
3. recompute SHA-256 and reject drift;
4. reject unknown phases;
5. reject selection while another phase is active;
6. require all dependencies in `closed_phases`;
7. set `active_phase`;
8. persist state;
9. print the selected phase ID.

Phase 1 does not implement phase closing. Tests may create state fixtures directly.

## 5. Persistence

- State lives only under `<target-repo>/.mrgs/`.
- JSON is deterministic and human-readable.
- Writes use a same-directory temporary file followed by rename.
- Failed validation creates or changes nothing.
- Failed selection does not modify state.

## 6. Path safety

- `--repo` resolves to an existing directory.
- `--plan` resolves to an existing regular file.
- The canonical plan path must be below the canonical repository path.
- Traversal and symlink escapes are rejected.
- Writes never leave `<target-repo>/.mrgs/`.

## 7. Errors

- Success exit code: `0`.
- All failures: non-zero.
- Errors are concise and name the condition.
- No normal-operation backtraces.
- No silent repair of malformed state.

## 8. Allowed dependencies

Production:

- `clap` with derive;
- `serde`;
- `serde_json`;
- `toml`;
- `sha2`;
- `thiserror`.

Development:

- `tempfile`;
- `assert_cmd`;
- `predicates`.

No async runtime, HTTP client, database, Git library, logging framework, or plugin framework.

## 9. Required tests

Cover:

1. valid first acceptance;
2. exact SHA-256 persistence;
3. same-plan idempotence;
4. rejection of different accepted bytes;
5. unsupported schema;
6. empty plan ID;
7. zero phases;
8. duplicate phase IDs;
9. unknown dependency;
10. self-dependency;
11. dependency cycle;
12. plan outside repository;
13. plan drift;
14. successful unblocked selection;
15. unknown phase rejection;
16. active-phase conflict;
17. blocked dependency rejection;
18. no state mutation after failure.

## 10. Allowed paths

Only:

- `.gitignore`
- `Cargo.toml`
- `Cargo.lock`
- `README.md`
- `src/**`
- `tests/**`
- `docs/master-plan.md`
- `docs/contracts/phase-01-contract.md`

## 11. Forbidden paths and operations

Do not modify `.github/**`, `.git/**`, `scripts/**`, `examples/**`, `benches/**`, or anything outside the repository.

Do not commit, push, tag, branch, merge, rebase, reset, stash, clean, install global software, or add future-phase scaffolding.

Network use is allowed only for Cargo dependency resolution.

## 12. Verification

Run:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
git diff --check
git status --short
git diff --name-only
git diff --stat
```

All Rust checks and `git diff --check` must pass.

## 13. Handoff evidence

Report:

- repository;
- branch;
- baseline HEAD, or `UNBORN`;
- final HEAD;
- remote;
- pre/post status;
- exact changed files;
- every verification result;
- forbidden-path check;
- summary;
- unresolved issues or `None`;
- `PASS` or `FAIL`.

`PASS` requires every contract requirement and verification item.

## 14. Boundary

This authorizes Phase 1 implementation only. It does not authorize commit or push.
