# Phase 2 Contract — Active-Phase Contract Draft Registration

Contract version: 1
Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

## 1. Objective

Extend the Phase 1 governance foundation with the smallest deterministic mechanism for registering one exact contract draft for the currently active phase.

Phase 2 implements only:

1. a strict machine-readable contract format;
2. validation that a draft belongs to the active phase;
3. exact-byte draft hashing;
4. atomic persistence of one draft record;
5. idempotent re-registration of the same exact draft.

Phase 2 does not implement contract acceptance, contract revision, phase closing, implementation execution, audit, repair, Git mutation, networking, background services, automatic command execution, or automatic model invocation.

## 2. Required repository result

The repository must continue to contain the complete Phase 1 implementation and add:

- `docs/contracts/phase-02-contract.md`;
- a contract domain module under `src/`;
- Phase 2 CLI wiring;
- Phase 2 persistence and validation support;
- Phase 2 integration tests;
- README documentation for the Phase 2 command and contract format.

No production dependency may be added.

## 3. CLI surface

Preserve the Phase 1 commands and add exactly:

```text
mrgs contract draft --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH>
```

The complete CLI after Phase 2 is:

```text
mrgs plan accept --repo <REPOSITORY_PATH> --plan <PLAN_PATH>
mrgs phase select --repo <REPOSITORY_PATH> --phase <PHASE_ID>
mrgs contract draft --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH>
```

No other new command is authorized.

## 4. Contract source format

The source contract is strict UTF-8 TOML with this minimum structure:

```toml
schema_version = 1
contract_id = "phase-2-contract-v1"
phase_id = "phase-2"
title = "Active-phase contract draft registration"
objective = "Register one exact contract draft for the active phase."

requirements = [
  "The implementation must remain inside the authorized paths.",
  "All required verification commands must pass."
]

allowed_paths = [
  "src/",
  "tests/",
  "README.md"
]

forbidden_paths = [
  ".git/",
  ".github/"
]

verification_commands = [
  "cargo fmt --all -- --check",
  "cargo test --all"
]

handoff_fields = [
  "BASELINE_HEAD",
  "FINAL_HEAD",
  "CHANGED_FILES",
  "TEST_RESULTS",
  "RECOMMENDATION"
]
```

The parser must reject unknown top-level fields.

## 5. Contract source validation

Validation must reject:

1. unsupported `schema_version`;
2. empty or whitespace-only `contract_id`;
3. empty or whitespace-only `phase_id`;
4. empty or whitespace-only `title`;
5. empty or whitespace-only `objective`;
6. zero `requirements`;
7. zero `allowed_paths`;
8. zero `forbidden_paths`;
9. zero `verification_commands`;
10. zero `handoff_fields`;
11. empty or whitespace-only entries in any list;
12. duplicate entries within any list;
13. a `phase_id` different from the current active phase;
14. an input path outside the canonical repository;
15. an input path under the repository’s `.mrgs` directory;
16. traversal, absolute-path, symlink, junction, or reparse-point escape;
17. invalid UTF-8;
18. malformed TOML.

Whitespace surrounding scalar identifiers is not normalization. Values such as `" phase-2 "` are invalid rather than silently trimmed.

List entry comparison for duplicates is exact and case-sensitive.

Phase order and plan authority remain governed by Phase 1.

## 6. Draft command preconditions

`contract draft` must:

1. canonicalize the repository;
2. require an existing, valid direct-child `.mrgs` directory;
3. load and validate `accepted-plan.json`;
4. load and validate `state.json`;
5. safely resolve and reload the recorded governed plan;
6. strictly decode and validate the plan;
7. recompute and validate the exact plan SHA-256;
8. validate accepted-plan and state cross-record consistency;
9. require `active_phase` to be present;
10. canonicalize and validate the contract source path;
11. require the source to be a regular file inside the repository and outside `.mrgs`;
12. read exact source bytes;
13. decode strict UTF-8;
14. parse and validate the contract source;
15. require source `phase_id` to equal the active phase.

A failed precondition creates or changes nothing.

## 7. Draft record

Persist exactly one file:

```text
<repo>/.mrgs/contract-draft.json
```

The deterministic, human-readable JSON record must contain:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-hex",
  "phase_id": "phase-2",
  "contract_id": "phase-2-contract-v1",
  "revision": 1,
  "source_path": "docs/contracts/runtime-phase-2.toml",
  "sha256": "lowercase-hex",
  "content": "exact UTF-8 source content"
}
```

Requirements:

- `schema_version` is exactly `1`;
- `accepted_plan_sha256` equals the validated accepted plan SHA;
- `phase_id` equals the validated active phase;
- `contract_id` equals the parsed source contract ID;
- `revision` is exactly `1`;
- `source_path` is normalized repository-relative text using `/`;
- `source_path` is safe, remains inside the repository, and is not under `.mrgs`;
- `sha256` is lowercase SHA-256 over the exact original source bytes;
- `content.as_bytes()` reproduces the exact original source bytes.

The record must not contain timestamps, hostnames, usernames, absolute paths, model names, or nondeterministic metadata.

## 8. First draft and idempotency rules

When `contract-draft.json` does not exist:

- the first valid draft succeeds;
- it is written atomically;
- the command prints `<contract_id> <sha256>`.

When `contract-draft.json` exists:

1. load and validate the complete existing record;
2. require record schema version `1`;
3. require lowercase 64-character SHA-256 fields;
4. require `revision == 1`;
5. validate the safe recorded source path;
6. parse and validate the stored `content`;
7. recompute SHA-256 from stored `content.as_bytes()`;
8. require stored content, record fields, active phase, plan SHA, contract ID, and recomputed hash to agree;
9. do not require the original source file to remain present or unchanged;
10. compare the newly submitted exact bytes with the existing draft hash.

If the newly submitted exact bytes match:

- return success;
- print the existing `<contract_id> <sha256>`;
- preserve `contract-draft.json` byte-for-byte;
- preserve `state.json` byte-for-byte;
- perform no write.

If the newly submitted exact bytes differ:

- reject the operation;
- preserve all governance files byte-for-byte;
- do not create a revision.

Contract revision is Phase 3 scope.

## 9. Incomplete and malformed draft authority

If `contract-draft.json` exists but is malformed or inconsistent, reject it.

Do not silently repair, replace, delete, or normalize it.

Phase 2 has only one draft record, so no auxiliary draft file or directory is authorized.

## 10. Persistence and failure preservation

- Governance state remains only under the validated direct-child `.mrgs` directory.
- Add `contract-draft.json` to the explicit governance filename allowlist.
- Reuse the established unique same-directory temporary-write and replace behavior.
- Serialize completely before touching the destination.
- Failed validation creates or changes nothing.
- Every failed draft operation preserves:
  - `accepted-plan.json` byte-for-byte;
  - `state.json` byte-for-byte;
  - an existing `contract-draft.json` byte-for-byte.
- No handled failure may leave a temporary governance file.
- `contract draft` must not change `active_phase` or `closed_phases`.

## 11. Path safety

- `--repo` resolves to an existing directory.
- `--contract` resolves to an existing regular file.
- The canonical contract source must be strictly below the canonical repository.
- The source must not be inside canonical `<repo>/.mrgs`.
- Traversal, symlink, junction, and reparse-point escapes are rejected.
- Persisted `source_path` must be a safe normalized repository-relative path.
- Governance writes remain direct children of the validated `.mrgs`.
- No contract-controlled string may become a governance destination filename.

Protection against a hostile concurrent process changing filesystem topology between system calls is not Phase 2 scope.

## 12. Errors

- Success exit code: `0`.
- All failures: non-zero.
- Errors are concise and identify the failed condition.
- No normal-operation backtraces.
- No silent repair.
- No normalization of invalid identifiers.

## 13. Allowed dependencies

Production dependencies remain exactly within the Phase 1 allowance:

- `clap` with derive;
- `serde`;
- `serde_json`;
- `toml`;
- `sha2`;
- `thiserror`.

Development dependencies remain exactly within the Phase 1 allowance:

- `tempfile`;
- `assert_cmd`;
- `predicates`.

No async runtime, HTTP client, database, Git library, logging framework, plugin framework, UUID library, random-number library, or time library may be added.

## 14. Required tests

Meaningfully cover at least:

1. valid first draft;
2. exact source-byte SHA-256 persistence using a literal independently known digest;
3. exact content persistence including final newline and line-ending differences;
4. normalized repository-relative `source_path`;
5. same exact draft idempotence;
6. idempotent operation preserves draft and state bytes;
7. different draft bytes are rejected;
8. different draft rejection preserves all governance files;
9. missing active phase;
10. phase ID mismatch;
11. unsupported contract schema;
12. empty contract ID;
13. empty phase ID;
14. empty title;
15. empty objective;
16. zero requirements;
17. zero allowed paths;
18. zero forbidden paths;
19. zero verification commands;
20. zero handoff fields;
21. empty list entry;
22. duplicate list entry;
23. unknown TOML top-level field;
24. contract source outside repository;
25. contract source under `.mrgs`;
26. contract source symlink escape;
27. invalid UTF-8;
28. malformed TOML;
29. plan drift;
30. malformed or inconsistent accepted plan;
31. malformed or inconsistent state;
32. malformed existing draft record;
33. uppercase or invalid persisted draft SHA;
34. persisted draft plan SHA mismatch;
35. persisted draft phase mismatch;
36. persisted draft contract ID mismatch;
37. persisted draft revision other than `1`;
38. persisted draft source path unsafe or under `.mrgs`;
39. persisted content/hash mismatch;
40. persisted content/field mismatch;
41. missing original source after successful first draft does not invalidate idempotent registration from another identical source file;
42. no phase-state mutation on success;
43. no phase-state mutation on failure;
44. no temporary files after success or handled failure;
45. all existing Phase 1 tests continue to pass.

Tests must inspect actual file bytes and persisted fields, not only exit codes.

## 15. Allowed implementation paths

Only:

- `.gitignore`
- `Cargo.toml`
- `Cargo.lock`
- `README.md`
- `src/**`
- `tests/**`
- `docs/master-plan.md`
- `docs/contracts/phase-01-contract.md`
- `docs/contracts/phase-02-contract.md`

The three authoritative planning/contract documents must remain unchanged during implementation.

## 16. Forbidden paths and operations

Do not modify `.github/**`, `.git/**`, `scripts/**`, `examples/**`, `benches/**`, or anything outside the repository.

Do not commit, push, tag, create or switch branches, merge, rebase, reset, stash, clean, install global software, or add future-phase scaffolding.

Network use is allowed only for existing Cargo dependency resolution.

## 17. Verification

Run:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
git diff --check
git status --short --untracked-files=all
git diff --name-only
git diff --stat
```

All Rust checks and `git diff --check` must pass.

## 18. Handoff evidence

Report:

- phase;
- repository;
- branch;
- baseline HEAD;
- final HEAD;
- remote;
- pre/post status;
- exact changed files;
- implementation summary;
- test results;
- path-containment result;
- exact-byte persistence result;
- idempotency and preservation result;
- forbidden-path check;
- unresolved issues or `None`;
- `PASS` or `FAIL`.

`PASS` requires every contract requirement and verification item.

## 19. Boundary

This authorizes Phase 2 implementation only.

It does not authorize contract acceptance, contract revision, implementation execution, audit, repair routing, phase closeout, Git mutation, commit, or push.
