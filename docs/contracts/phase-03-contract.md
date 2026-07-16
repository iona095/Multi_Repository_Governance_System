# Phase 3 Contract — Contract Acceptance, Revision, and Lifecycle Transitions

Contract version: 1
Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

## 1. Objective

Extend the Phase 1 and Phase 2 governance foundation with deterministic human contract acceptance, optimistic-concurrency contract revision, immutable acceptance history, and explicit contract lifecycle transitions.

Phase 3 implements only:

1. exact acceptance of the current contract draft;
2. stale-safe revision of the current contract draft;
3. an append-only accepted-contract revision ledger;
4. deterministic lifecycle states inferred from validated authority;
5. strict preservation and corruption rejection across all contract authority.

Phase 3 does not implement phase closeout, implementation execution, implementation authorization, independent audit, repair routing, Git mutation, networking, model invocation, background services, or automatic command execution.

## 2. Controlling lifecycle model

The contract lifecycle has exactly three valid states:

- `DRAFT`: a valid `contract-draft.json` exists and no accepted-contract ledger exists;
- `ACCEPTED`: the final accepted revision exactly equals the current draft revision and exact content;
- `REVISION_DRAFT`: an accepted revision exists, but the current draft has a greater revision.

The lifecycle state is inferred from validated files. It is not stored in `state.json`.

The existing phase state remains unchanged:

- `active_phase` is not cleared;
- `closed_phases` is not changed;
- no phase is closed;
- no new phase may be selected while the existing Phase 1 active-phase rule blocks selection.

During `REVISION_DRAFT`, the last accepted revision remains authoritative. The newer draft is not accepted authority until an exact acceptance succeeds.

## 3. CLI surface

Preserve all existing commands and add exactly:

```text
mrgs contract accept --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256> --decision <DECISION>
mrgs contract revise --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH> --expected-revision <REVISION> --expected-sha256 <SHA256>
```

The complete CLI after Phase 3 is:

```text
mrgs plan accept --repo <REPOSITORY_PATH> --plan <PLAN_PATH>
mrgs phase select --repo <REPOSITORY_PATH> --phase <PHASE_ID>
mrgs contract draft --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH>
mrgs contract accept --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256> --decision <DECISION>
mrgs contract revise --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH> --expected-revision <REVISION> --expected-sha256 <SHA256>
```

No other new command is authorized.

## 4. Existing draft record evolution

The existing file remains:

```text
<repo>/.mrgs/contract-draft.json
```

Its schema remains:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-hex",
  "phase_id": "phase-3",
  "contract_id": "phase-3-contract-v1",
  "revision": 1,
  "source_path": "docs/contracts/runtime-phase-3.toml",
  "sha256": "lowercase-hex",
  "content": "exact UTF-8 source content"
}
```

Phase 3 intentionally supersedes the Phase 2 restriction that every persisted draft must have `revision == 1`.

After Phase 3:

- initial `contract draft` still creates revision `1`;
- every persisted draft revision must be at least `1`;
- only `contract revise` may create revision `2` or greater;
- revisions increase by exactly one from the current draft preimage;
- revision `0` is always invalid.

`contract draft` must remain exact-byte idempotent for the current valid draft, including a current draft with revision greater than `1`. Different bytes remain rejected and must use `contract revise`.

The Phase 2 test that treated every revision other than `1` as malformed must be updated because that behavior is intentionally superseded. Equivalent or stronger corruption coverage must remain for revision `0`, invalid lifecycle relations, and unauthorized revision transitions.

## 5. Accepted-contract ledger

Add exactly one governance file:

```text
<repo>/.mrgs/accepted-contract.json
```

It is deterministic, strict JSON with this structure:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-hex",
  "phase_id": "phase-3",
  "contract_id": "phase-3-contract-v1",
  "revisions": [
    {
      "revision": 1,
      "source_path": "docs/contracts/runtime-phase-3.toml",
      "sha256": "lowercase-hex",
      "content": "exact UTF-8 source content"
    }
  ]
}
```

Both the ledger and each revision entry must reject unknown JSON fields.

The ledger must contain no timestamps, hostnames, usernames, absolute paths, model names, decision-maker names, signatures, random identifiers, or nondeterministic metadata.

## 6. Accepted ledger validation

Validation must require:

1. ledger `schema_version == 1`;
2. valid lowercase 64-character `accepted_plan_sha256`;
3. ledger plan SHA equals the validated accepted-plan SHA;
4. non-empty `phase_id`;
5. ledger phase equals the current active phase;
6. non-empty `contract_id`;
7. at least one accepted revision;
8. accepted revision numbers are positive and strictly increasing;
9. accepted revision numbers are unique;
10. gaps are allowed because unaccepted intermediate drafts may exist;
11. each `source_path` is strict normalized repository-relative text using `/`;
12. no accepted source path is under `.mrgs`;
13. each `sha256` is lowercase 64-character hexadecimal;
14. each `content` is strict UTF-8 as represented by the JSON string;
15. each stored content parses as the strict Phase 2 contract TOML model;
16. each stored contract validates completely;
17. each stored contract `phase_id` equals the ledger phase;
18. each stored contract `contract_id` equals the ledger contract ID;
19. each stored content SHA-256 equals its recorded SHA;
20. the final accepted revision is not greater than the current draft revision;
21. if the final accepted revision equals the current draft revision, its source path, SHA, and content exactly equal the draft;
22. if the final accepted revision is lower than the current draft revision, the lifecycle is `REVISION_DRAFT`.

The original source file for an accepted revision need not still exist or remain unchanged. The stored content and hash are authority.

Malformed or inconsistent accepted authority must be rejected without repair, deletion, replacement, truncation, or normalization.

## 7. Common contract-command authority validation

Before `contract draft`, `contract accept`, or `contract revise` makes a governance change, it must:

1. canonicalize the repository;
2. require a valid direct-child `.mrgs` directory;
3. load and validate `accepted-plan.json`;
4. load and validate `state.json`;
5. safely reload the recorded plan;
6. strictly decode and validate the plan;
7. recompute the exact plan SHA-256;
8. validate accepted-plan, state, and plan consistency;
9. require a valid active phase;
10. detect incomplete contract authority;
11. strictly validate every existing contract authority record;
12. infer a valid lifecycle state.

Incomplete authority includes:

- `accepted-contract.json` existing without `contract-draft.json`.

If `contract-draft.json` is absent and no accepted ledger exists, initial Phase 2 drafting remains allowed.

A failed precondition creates or changes nothing.

## 8. Exact contract acceptance

The acceptance command is:

```text
mrgs contract accept --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256> --decision <DECISION>
```

Acceptance must require:

1. a valid current draft;
2. complete valid authority;
3. `revision >= 1`;
4. the supplied revision exactly equals the current draft revision;
5. the supplied SHA is lowercase 64-character hexadecimal;
6. the supplied SHA exactly equals the current draft SHA;
7. `decision` is exactly the seven uppercase ASCII characters `ACCEPTED`;
8. no trimming, case folding, Unicode normalization, or alternate token is permitted.

The following examples are invalid:

```text
accepted
Accepted
 ACCEPTED
ACCEPTED
ACCEPT
```

A stale revision or stale SHA must fail even if it identifies a previously accepted revision.

### 8.1 First acceptance

When no accepted ledger exists:

- create a strict ledger containing exactly one revision copied from the current draft;
- write only `accepted-contract.json`;
- preserve every existing governance file byte-for-byte;
- print:

```text
ACCEPTED <contract_id> <revision> <sha256>
```

### 8.2 Later acceptance

When a valid accepted ledger exists and its final revision is lower than the current draft revision:

- append exactly one new accepted revision copied from the current draft;
- preserve every earlier accepted entry byte-for-byte in value and order;
- write only `accepted-contract.json`;
- print the same deterministic success format.

The accepted ledger is append-only through valid commands. An earlier entry is never removed, edited, reordered, or replaced.

### 8.3 Idempotent acceptance

When the final accepted revision already exactly equals the current draft:

- validate the complete ledger and draft first;
- return success;
- preserve `accepted-contract.json` byte-for-byte;
- preserve every other governance file byte-for-byte;
- perform no write;
- print the existing deterministic success output.

## 9. Stale-safe contract revision

The revision command is:

```text
mrgs contract revise --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH> --expected-revision <REVISION> --expected-sha256 <SHA256>
```

The submitted source contract uses the exact Phase 2 strict TOML format and validation rules.

Revision must:

1. validate complete existing authority;
2. require an existing current draft;
3. require `expected-revision >= 1`;
4. require lowercase valid `expected-sha256`;
5. canonicalize and validate the new source path;
6. require the new source to be a regular file strictly inside the repository and outside `.mrgs`;
7. reject traversal, symlink, junction, and reparse-point escape;
8. read and hash exact source bytes;
9. decode strict UTF-8;
10. strictly parse and validate the contract;
11. require the new contract phase to equal the active phase;
12. require the new contract ID to exactly equal the current draft contract ID;
13. reject a new contract with exact bytes equal to the current draft;
14. reject revision overflow;
15. leave the accepted ledger unchanged.

### 9.1 Compare-and-swap transition

A new revision may be written only when:

```text
expected-revision == current draft revision
expected-sha256 == current draft sha256
```

The new draft must have:

```text
revision = expected-revision + 1
```

All other draft fields are deterministically derived from validated current authority and exact new source bytes.

The operation writes only `contract-draft.json`.

### 9.2 Idempotent revision replay

A repeated invocation after a successful revision is idempotent only when all are true:

```text
current draft revision == expected-revision + 1
current draft sha256 == newly submitted exact source sha256
current draft content bytes == newly submitted exact source bytes
current draft phase and contract ID equal the submitted contract
```

In that case:

- return success;
- perform no write;
- preserve every governance file byte-for-byte;
- report the lifecycle implied by the accepted ledger.

Every other stale preimage is rejected.

### 9.3 Revision output

When no accepted ledger exists, successful revision output is:

```text
DRAFT <contract_id> <revision> <sha256>
```

When an accepted ledger exists and its final revision is lower than the new draft, successful revision output is:

```text
REVISION_DRAFT <contract_id> <revision> <sha256>
```

## 10. Lifecycle transition table

Only these transitions are legal:

```text
No contract authority -> DRAFT
DRAFT -> DRAFT by valid revision
DRAFT -> ACCEPTED by exact acceptance
ACCEPTED -> REVISION_DRAFT by valid revision
REVISION_DRAFT -> REVISION_DRAFT by valid revision
REVISION_DRAFT -> ACCEPTED by exact acceptance
ACCEPTED -> ACCEPTED by idempotent acceptance
DRAFT -> DRAFT by idempotent draft or revision replay
REVISION_DRAFT -> REVISION_DRAFT by idempotent revision replay
```

Forbidden transitions include:

- acceptance without a draft;
- acceptance of a stale draft;
- revision without a draft;
- revision without the exact current preimage;
- revision zero;
- revision jumps;
- accepted ledger rollback;
- accepted ledger truncation;
- accepted revision mutation;
- phase closeout;
- active-phase clearing;
- implementation authorization.

## 11. Persistence and failure preservation

- Add `accepted-contract.json` to the explicit governance filename allowlist.
- Every command writes at most one governance file.
- Reuse the established unique same-directory temporary-write and replacement mechanism.
- Serialize completely before touching the destination.
- No handled failure may leave a temporary governance file.
- Every failed contract operation preserves byte-for-byte:
  - `accepted-plan.json`;
  - `state.json`;
  - an existing `contract-draft.json`;
  - an existing `accepted-contract.json`.
- Acceptance never writes the draft or state.
- Revision never writes the accepted ledger or state.
- Initial draft never creates accepted authority.
- No contract operation changes `active_phase` or `closed_phases`.

## 12. Path safety

All Phase 2 path requirements remain controlling.

Additionally:

- accepted ledger paths must use strict normalized `/` form;
- source paths are metadata only and never become governance destination filenames;
- accepted source files need not remain present;
- strict path conversion must not use lossy conversion;
- no contract-controlled string may choose a governance filename;
- governance writes remain direct children of the validated `.mrgs`.

Protection against hostile concurrent filesystem topology changes between system calls remains outside this phase.

## 13. Errors

- Success exit code: `0`.
- Every failure: non-zero.
- Errors are concise and identify the failed condition.
- No normal-operation backtrace.
- No silent repair.
- No invalid-input normalization.
- No acceptance-token normalization.
- No stale-preimage recovery by guessing.

## 14. Dependencies

Production dependencies remain limited to:

- `clap` with derive;
- `serde`;
- `serde_json`;
- `toml`;
- `sha2`;
- `thiserror`.

Development dependencies remain limited to:

- `tempfile`;
- `assert_cmd`;
- `predicates`.

No dependency may be added.

## 15. Required tests

Meaningfully cover at least:

1. valid first acceptance;
2. exact `ACCEPTED` token;
3. lowercase token rejection;
4. mixed-case token rejection;
5. leading acceptance whitespace rejection;
6. trailing acceptance whitespace rejection;
7. wrong acceptance token rejection;
8. stale acceptance revision rejection;
9. stale acceptance SHA rejection;
10. uppercase acceptance SHA rejection;
11. invalid acceptance SHA rejection;
12. acceptance without draft;
13. first acceptance exact ledger persistence;
14. accepted content exact-byte persistence;
15. accepted literal SHA verification;
16. accepted ledger unknown-field rejection;
17. accepted revision unknown-field rejection;
18. malformed accepted ledger rejection;
19. empty accepted revisions rejection;
20. accepted plan SHA mismatch;
21. accepted phase mismatch;
22. accepted contract ID mismatch;
23. accepted revision zero rejection;
24. duplicate accepted revision rejection;
25. non-increasing accepted revisions rejection;
26. accepted source path normalization rejection;
27. accepted stored-content parse rejection;
28. accepted stored-content phase mismatch;
29. accepted stored-content contract-ID mismatch;
30. accepted stored-content hash mismatch;
31. accepted final revision greater than draft rejection;
32. equal revision with different content rejection;
33. valid idempotent acceptance;
34. idempotent acceptance preserves every governance file;
35. valid acceptance append after revision;
36. append preserves earlier accepted entries and order;
37. accepted ledger remains append-only;
38. valid revision from unaccepted draft;
39. valid revision from accepted state;
40. chained pending revisions;
41. revision increments exactly one;
42. revision expected-number mismatch;
43. revision expected-hash mismatch;
44. revision uppercase expected SHA rejection;
45. revision zero preimage rejection;
46. revision overflow rejection;
47. revision same-byte no-op rejection;
48. revision contract-ID change rejection;
49. revision phase mismatch rejection;
50. revision invalid UTF-8 rejection;
51. revision malformed TOML rejection;
52. revision source outside repository rejection;
53. revision source under `.mrgs` rejection;
54. revision symlink escape rejection;
55. exact revised source-byte persistence;
56. revised LF and CRLF distinction;
57. normalized revised source path;
58. valid idempotent revision replay;
59. replay preserves every governance file;
60. stale replay with different source rejection;
61. accepted ledger preserved during revision;
62. state preserved during acceptance;
63. state preserved during revision;
64. draft preserved during acceptance;
65. accepted ledger preserved on failed acceptance;
66. draft preserved on failed revision;
67. orphaned accepted ledger rejection;
68. temporary files absent after acceptance success;
69. temporary files absent after revision success;
70. temporary files absent after handled failures;
71. `contract draft` remains idempotent for a valid revision greater than one;
72. `contract draft` validates existing lifecycle authority;
73. initial draft creates no accepted ledger;
74. all non-superseded Phase 1 and Phase 2 tests continue to pass;
75. the superseded revision-equals-one test is replaced by stronger revision-zero and lifecycle-consistency tests.

Tests must inspect actual bytes and JSON fields, not only exit status.

## 16. Allowed implementation paths

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
- `docs/contracts/phase-03-contract.md`

The four authoritative planning and contract documents must remain unchanged during implementation.

## 17. Forbidden paths and operations

Do not modify `.github/**`, `.git/**`, `scripts/**`, `examples/**`, `benches/**`, or anything outside the repository.

Do not commit, push, tag, create or switch branches, merge, rebase, reset, stash, clean, install global software, or add future-phase scaffolding.

Network use is allowed only for existing Cargo dependency resolution.

## 18. Verification

Run:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo test --all -- --list
git diff --check
git status --short --untracked-files=all
git diff --name-only
git diff --stat
git diff -- Cargo.toml Cargo.lock
git diff -- docs/master-plan.md docs/contracts/phase-01-contract.md docs/contracts/phase-02-contract.md docs/contracts/phase-03-contract.md
```

All Rust checks and `git diff --check` must pass.

Report unit, integration, and total test counts separately. Do not describe the integration count as the total.

## 19. Handoff evidence

Report:

- phase;
- repository;
- branch;
- baseline and final HEAD;
- remote;
- pre-status and post-status;
- exact changed files;
- CLI result;
- lifecycle model result;
- accepted-ledger result;
- exact acceptance result;
- revision CAS result;
- idempotency result;
- append-only history result;
- state-preservation result;
- path-containment result;
- unit, integration, and total test counts;
- verification results;
- forbidden-path result;
- unresolved issues or `None`;
- `PASS` or `FAIL`.

`PASS` requires every Phase 3 requirement and verification item.

## 20. Boundary

This authorizes Phase 3 implementation only.

It does not authorize phase closeout, implementation execution, implementation authorization, audit execution, repair routing, final manifests, Git mutation, commit, or push.
