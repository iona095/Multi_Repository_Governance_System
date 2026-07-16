# Phase 3 Contract — Contract Acceptance, Revision, and Lifecycle Transitions

Contract version: 2
Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

Revision note: Version 2 resolves the revision-replay preimage contradiction by adding a strict immediately preceding draft receipt. It also makes replay output lifecycle-aware and makes draft idempotency explicitly depend on exact-byte equality. All non-superseded Phase 3 requirements remain controlling.

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
- `ACCEPTED`: the final accepted revision exactly equals the current draft revision, source path, SHA, and content;
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

The deterministic outer object remains strict and rejects unknown fields. A revision-1 draft remains compatible with the Phase 2 record and has exactly this shape, with no `preimage` field:

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

A draft with revision greater than `1` has the `preimage` field immediately after `revision` and before `source_path` in the deterministic record:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-hex",
  "phase_id": "phase-3",
  "contract_id": "phase-3-contract-v1",
  "revision": 2,
  "preimage": {
    "revision": 1,
    "sha256": "lowercase-hex"
  },
  "source_path": "docs/contracts/runtime-phase-3-v2.toml",
  "sha256": "lowercase-hex",
  "content": "exact UTF-8 source content"
}
```

`preimage` is the sole optional outer field, with presence determined strictly by draft revision. The nested `preimage` object is strict and contains exactly `revision` and `sha256`; unknown fields are rejected. The receipt is replay-verification evidence only. It is not accepted authority, is not lifecycle state, is not copied into `accepted-contract.json`, and contains no timestamp, username, hostname, model name, source content, path, or nondeterministic metadata.

Phase 3 intentionally supersedes the Phase 2 restriction that every persisted draft must have `revision == 1`. Version 2 further supersedes the version 1 draft shape for revisions greater than `1` by requiring the preimage receipt while retaining exact compatibility for valid revision-1 Phase 2 drafts.

After Phase 3:

- initial `contract draft` still creates revision `1`;
- every persisted draft revision must be at least `1`;
- only `contract revise` may create revision `2` or greater;
- revisions increase by exactly one from the current draft preimage;
- revision `0` is always invalid;
- revision `1` requires `preimage` to be absent;
- revision greater than `1` requires `preimage` to be present;
- `preimage.revision` must be positive and equal `revision - 1`;
- `preimage.sha256` must be lowercase 64-character hexadecimal;
- JSON `null` is not equivalent to an absent `preimage` field;
- malformed or inconsistent preimage authority is rejected without repair.

An existing valid revision-1 Phase 2 draft without `preimage` remains valid. A revision-1 draft with `preimage`, or a revision-greater-than-1 draft without `preimage`, is invalid.

`contract draft` must remain exact-byte idempotent for the current valid draft, including a current draft with revision greater than `1`. Idempotent success requires complete lifecycle validation, submitted source SHA equality, submitted exact source-byte equality with `draft.content.as_bytes()`, submitted phase equality with the active phase, submitted contract-ID equality with the current draft, and a normalized submitted source path satisfying the existing Phase 2 idempotency rules. Recorded SHA equality alone never authorizes success. Different exact bytes must be rejected even if corrupted or synthetic comparison data presents the same digest, and must use `contract revise`.

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

The accepted-contract schema is unchanged by contract version 2. Accepted revision entries never contain `preimage`. Acceptance copies only the draft revision's `revision`, `source_path`, `sha256`, and `content` fields.

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
11. strictly validate every existing contract authority record, including the outer draft object and any required nested preimage receipt;
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
- copy only `revision`, `source_path`, `sha256`, and `content`; do not copy `preimage`;
- write only `accepted-contract.json`;
- preserve every existing governance file byte-for-byte;
- print:

```text
ACCEPTED <contract_id> <revision> <sha256>
```

### 8.2 Later acceptance

When a valid accepted ledger exists and its final revision is lower than the current draft revision:

- append exactly one new accepted revision copied from the current draft;
- copy only `revision`, `source_path`, `sha256`, and `content`; do not copy `preimage`;
- preserve every earlier accepted entry byte-for-byte in value and order;
- write only `accepted-contract.json`;
- print the same deterministic success format.

The accepted ledger is append-only through valid commands. An earlier entry is never removed, edited, reordered, or replaced.

Acceptance does not delete or alter the current draft preimage receipt. The receipt remains available so a later replay of the revision command can validate the exact preimage and return the lifecycle currently implied by accepted authority.

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
14. reject revision overflow before addition;
15. leave the accepted ledger unchanged.

### 9.1 Compare-and-swap transition

A new revision may be written only when:

```text
expected-revision == current draft revision
expected-sha256 == current draft sha256
```

Both supplied values must be validated exactly before any write.

The new draft must have:

```text
revision = current draft revision + 1
preimage.revision = validated expected-revision
preimage.sha256 = validated expected-sha256
```

The receipt records the exact immediately preceding draft tuple. A later normal revision replaces the receipt with that transition's newly validated immediately preceding tuple. All other draft fields are deterministically derived from validated current authority and exact new source bytes.

The operation writes only `contract-draft.json`.

### 9.2 Idempotent revision replay

A repeated invocation after a successful revision is idempotent only when complete current authority has been validated and all are true:

```text
current draft revision == expected-revision + 1
current draft preimage exists
current draft preimage revision == expected-revision
current draft preimage sha256 == expected-sha256
expected-sha256 is valid lowercase 64-character hexadecimal
current draft sha256 == newly submitted exact source sha256
current draft content bytes == newly submitted exact source bytes
current draft phase and contract ID equal the submitted contract
normalized submitted source_path == current draft source_path
```

The `expected-revision + 1` comparison uses checked arithmetic; overflow is rejected rather than wrapped or normalized.

In that case:

- return success;
- perform no write;
- preserve every governance file byte-for-byte;
- report the lifecycle implied by the accepted ledger.

Every other stale preimage is rejected.

Rejection explicitly includes an arbitrary valid lowercase SHA, an SHA from an older accepted revision, an SHA from an older unaccepted revision, an expected revision older by more than one, a correct expected revision with the wrong SHA, a correct SHA with the wrong expected revision, the same submitted bytes from a different normalized source path, an absent or malformed current preimage receipt, malformed accepted authority, a changed phase, a changed contract ID, or different new-source bytes.

### 9.3 Revision output

When no accepted ledger exists, successful revision output is:

```text
DRAFT <contract_id> <revision> <sha256>
```

When an accepted ledger exists and its final revision is lower than the new draft, successful revision output is:

```text
REVISION_DRAFT <contract_id> <revision> <sha256>
```

Revision replay reports the lifecycle inferred from completely validated current authority. Its exact output is:

```text
DRAFT <contract_id> <revision> <sha256>
```

when no accepted ledger exists;

```text
REVISION_DRAFT <contract_id> <revision> <sha256>
```

when the final accepted revision is lower than the current draft; and

```text
ACCEPTED <contract_id> <revision> <sha256>
```

when the final accepted revision exactly equals the current draft in revision, source path, SHA, and content. Replay after the revised draft has subsequently been accepted therefore returns `ACCEPTED`, not `REVISION_DRAFT`. A normal revision from an accepted current draft still creates a newer pending draft and returns `REVISION_DRAFT`.

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
ACCEPTED -> ACCEPTED by idempotent revision replay
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
- The preimage receipt is stored only inside `contract-draft.json`; no new governance file is introduced.
- Acceptance preserves the current draft, including any preimage receipt, byte-for-byte.
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
75. the superseded revision-equals-one test is replaced by stronger revision-zero, preimage-receipt, and lifecycle-consistency tests;
76. revision-1 draft without preimage is valid;
77. revision-1 draft with preimage is rejected;
78. revision-greater-than-1 draft without preimage is rejected;
79. revision-greater-than-1 draft with a valid immediate preimage is valid;
80. preimage revision zero rejection;
81. preimage revision mismatch rejection;
82. malformed preimage SHA rejection;
83. uppercase preimage SHA rejection;
84. unknown preimage JSON field rejection;
85. null preimage rejection where absence is required or a valid object is required;
86. normal revision stores the exact validated preimage tuple;
87. chained revisions replace the receipt with the immediately preceding tuple;
88. replay with the exact stored preimage succeeds;
89. replay with an arbitrary valid wrong SHA fails;
90. replay with an older accepted revision SHA fails;
91. replay with the correct revision and wrong SHA fails;
92. replay with the correct SHA and wrong revision fails;
93. replay older by more than one revision fails;
94. replay with the same content from a different normalized source path fails;
95. replay after acceptance returns `ACCEPTED`;
96. replay before first acceptance returns `DRAFT`;
97. replay with an older accepted ledger returns `REVISION_DRAFT`;
98. malformed receipt preserves every governance file byte-for-byte;
99. accepted ledger entries contain no preimage field;
100. acceptance preserves the draft preimage receipt byte-for-byte;
101. contract draft proves exact submitted-byte equality in addition to digest equality;
102. an implementation-level comparator regression constructs unequal byte content while presenting equal digest metadata and proves digest equality alone cannot authorize idempotency, without requiring a real SHA-256 collision;
103. all non-superseded Phase 1, Phase 2, and Phase 3 tests continue to pass.

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
