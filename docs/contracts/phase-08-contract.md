# Phase 8 Contract — State Recovery and Corruption Handling

Contract version: 1
Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

## 1. Objective

Extend the complete Phase 1–7 governance foundation with deterministic diagnosis and narrowly bounded recovery of governance state after an interrupted publication or corruption event.

Phase 8 implements only:

1. read-only classification of the exact current governance subject;
2. reconstruction of `accepted-plan.json` only from a valid self-contained completion ledger and the exact plan source;
3. deterministic reconstruction of `state.json` from validated plan, completion, and phase-scoped authority;
4. resumption of a Phase 6 closeout whose completion ledger was already published;
5. removal of strictly redundant producer temporary files;
6. human-authorized application bound to an exact recovery ID and exact subject hash;
7. an append-only, resumable recovery journal and chained recovery receipts.

It does not invent authority, repair arbitrary contract/audit/completion/continuity corruption, restore deleted source files, modify Git, access a network, discover credentials, or automatically continue into another governance command.

## 2. Authority and fail-closed rule

Recovery authority is derived only from surviving records that fully validate under their original Phase 1–7 rules.

The following records are never synthesized from guesses:

```text
contract-draft.json
accepted-contract.json
implementation-authority.json
audit-ledger.json
completion-ledger.json
continuity-ledger.json
recovery-ledger.json
```

Only `accepted-plan.json` and `state.json` are reconstructible, and only under the exact rules in this contract.

An ambiguity, conflicting survivor, unsupported filesystem object, invalid ledger, unknown `.mrgs` child, or missing proof is `RECOVERY_UNRECOVERABLE`. Recovery must preserve all bytes and fail closed.

## 3. CLI surface

Add exactly:

```text
mrgs recovery inspect --repo <REPOSITORY_PATH>
mrgs recovery apply --repo <REPOSITORY_PATH> --recovery-id <RECOVERY_ID> --subject-sha256 <SUBJECT_SHA256> --decision <DECISION>
```

`RECOVERY_ID` and `SUBJECT_SHA256` are lowercase 64-character hexadecimal strings.

The only accepted decision is exact uppercase:

```text
RECOVER
```

No trimming, case folding, or normalization is allowed.

## 4. Common validation order

Every recovery command must, before any target mutation:

1. resolve the repository as the exact Git worktree root;
2. reject detached, unborn, unreadable, or changed repository identity;
3. validate `.mrgs` topology without following links;
4. validate an existing recovery ledger before trusting any pending or applied entry;
5. capture the complete recovery subject;
6. classify every permanent and temporary governance entry;
7. derive surviving accepted-plan authority or prove exact reconstruction;
8. validate the plan source bytes and plan semantics;
9. validate completion and continuity ledgers when present;
10. validate or derive state and phase-scoped authority;
11. derive one deterministic classification and action list;
12. recompute all hashes and exact output.

`recovery apply` additionally recomputes the plan and compares caller arguments before publishing a pending entry.

## 5. Recovery subject

The recovery subject is a canonical JSON object with this exact field order:

```text
schema_version
repository_git_object_format
repository_head
repository_branch
governance_entries
plan_source
```

`schema_version` is `1`.

`governance_entries` inventories every direct child of `.mrgs` except the permanent `recovery-ledger.json`, which is validated separately as the recovery journal. Entries are sorted by exact UTF-8 filename bytes and contain:

```text
filename
kind
byte_length
sha256
```

`kind` is one of `REGULAR`, `SYMLINK`, `DIRECTORY`, or `OTHER`. `byte_length` and `sha256` are present only for regular files. Absence of a permanent filename is represented by a fixed `ABSENT` inventory entry so identical missing-state subjects hash identically.

`plan_source` is either `null` or an object containing exact normalized path, topology, byte length, and SHA-256.

The subject SHA-256 is computed over compact canonical JSON with no trailing newline.

A dirty worktree is allowed. Recovery subject capture must not run implementation enforcement or treat ordinary source changes as governance corruption.

## 6. Governance inventory

Permanent recognized filenames are exactly:

```text
accepted-plan.json
state.json
contract-draft.json
accepted-contract.json
implementation-authority.json
audit-ledger.json
completion-ledger.json
continuity-ledger.json
recovery-ledger.json
```

No nested `.mrgs` directory and no unknown direct child is allowed.

Recognized temporary filenames are limited to exact producer grammars already used by the repository plus the deterministic Phase 8 recovery grammar. Each recognized temp maps to exactly one permanent target.

Unknown names, malformed names, aliases, non-UTF-8 names, nested children, or ambiguous mappings are unrecoverable.

## 7. Health classification

Inspection returns exactly one classification:

- `HEALTHY`: all surviving authority validates, state and completion relation are exact, no closeout is incomplete, no redundant temp exists, and no pending recovery exists;
- `RECOVERABLE`: one deterministic ordered action list can reach `HEALTHY` without inventing authority;
- `PENDING`: one valid recovery journal entry is incomplete and can be resumed only by its exact recovery ID;
- `UNRECOVERABLE`: evidence is missing, conflicting, unsafe, unsupported, or corrupt.

The classification algorithm must be deterministic and may not prefer one of multiple plausible states.

## 8. Accepted-plan recovery

An existing accepted-plan record must pass strict raw-key validation, schema validation, normalized path validation, exact plan-source hashing, plan parsing, and plan consistency.

A missing or malformed regular `accepted-plan.json` is recoverable only when:

1. `completion-ledger.json` is a safe regular file;
2. the complete completion ledger validates using its own stored plan identity;
3. every final manifest agrees on exact plan ID, plan path, plan content, accepted-plan SHA-256, and phase inventory;
4. the plan source currently exists as a safe regular file at the exact stored path;
5. current plan-source bytes exactly equal the archived plan content and SHA-256;
6. the deterministic reconstructed record validates.

The reconstructed record contains exactly:

```text
schema_version
plan_id
plan_path
sha256
phase_count
```

It is serialized using the repository's canonical governance JSON format.

A symlink, junction, directory, device, conflicting completion manifest, missing plan source, or source drift is unrecoverable and must not be replaced.

## 9. State recovery

An existing state record must pass strict raw-key validation and the complete Phase 1 state validator.

A missing or malformed regular `state.json` is recoverable only when one exact state can be derived.

Closed phases are:

- the final valid completion receipt's exact `closed_phases_after`; or
- an empty list when no completion ledger exists.

The active phase is:

- `null` when no phase-scoped authority exists and no closeout is incomplete;
- the exact phase inferred under Section 10; or
- the final receipt's exact `active_phase_before` while resuming an incomplete closeout.

The prospective state must validate against the accepted plan. A valid selected phase with no draft is healthy and must never be erased merely because phase-scoped files are absent.

## 10. Active-phase inference

Phase-scoped authority is a contiguous prefix in this exact order:

```text
contract-draft.json
accepted-contract.json
implementation-authority.json
audit-ledger.json
```

If any later file exists, every predecessor must exist.

The draft supplies the candidate active phase. Every present successor must fully validate and bind to the same accepted plan, phase, contract, revision, content, implementation baseline, and audit history under the original Phase 2–5 rules.

The inferred phase must exist in the plan, must not be closed, and all dependencies must already be closed.

Subject drift of a nonterminal audit is not itself governance-file corruption; recovery validates stored structure and bindings, not whether the current worktree still equals an old audit subject.

## 11. Completion and state relation

A present completion ledger must pass full Phase 6 raw-key, schema, manifest, archive, receipt, hash, ordering, and chain validation.

The final completion receipt is authoritative for its before/after state transition.

A healthy completed state must equal the final receipt's `closed_phases_after` and `active_phase_after` exactly.

A valid state that represents a later selected phase is allowed only when the final completion state is its exact closed-phase prefix and the selected phase is valid under the plan.

Any other disagreement is recoverable only when Sections 9–12 derive one exact correction; otherwise it is unrecoverable.

## 12. Incomplete closeout recovery

A closeout is incomplete when a valid final completion entry has been published but one or more of these remain true:

1. the final phase is still active;
2. state does not yet equal the receipt's exact after-state;
3. one or more phase-scoped files remain.

Before resumption:

- every remaining phase-scoped file must be a safe regular file;
- its exact bytes and SHA-256 must equal the final manifest's archived copy;
- no later-phase authority may coexist;
- the final completion entry must remain the unique applicable entry.

Recovery must reuse or narrowly expose the existing Phase 6 resumable finalizer. It may not implement a second cleanup order or create a second completion entry.

## 13. Temporary-file handling

A pre-existing producer temp is recoverable only when its mapping is unambiguous and its bytes are exactly redundant with an existing valid target. The action is:

```text
REMOVE_REDUNDANT_TEMP
```

A differing temp, multiple candidates, target-absent pre-Phase-8 temp, malformed filename, or unsafe topology is unrecoverable.

Phase 8 recovery-owned temps use a deterministic name bound to recovery ID and action index. A valid pending journal may promote or remove only the exact recorded temp whose bytes match the exact action content. No name-only trust is allowed.

## 14. Recovery actions

The closed action enum is:

```text
REMOVE_REDUNDANT_TEMP
RESTORE_ACCEPTED_PLAN
RESTORE_STATE
RESUME_CLOSEOUT
```

Every action is strict JSON with `deny_unknown_fields` behavior and exact required fields.

Actions are sorted in this exact order:

1. redundant temp removal by target/path order;
2. accepted-plan restoration;
3. state restoration;
4. closeout resumption.

No action may write outside `.mrgs` or mutate a Git object, ref, index, config, hook, worktree source file, plan source, contract source, report source, or continuity source repository.

## 15. Recovery plan and ID

A recovery plan seed contains exactly:

```text
schema_version
accepted_plan_sha256
plan_id
pre_subject_sha256
actions
prefix_subject_sha256
```

`prefix_subject_sha256` contains `actions.len() + 1` entries. Entry zero equals `pre_subject_sha256`; each later entry is the deterministic expected subject after the corresponding action prefix.

`recovery_id` is the SHA-256 of the compact canonical plan seed.

The plan may contain exact replacement JSON bytes for accepted-plan and state, but those bytes must be recomputed by `apply`; caller input never supplies action content.

## 16. Inspect output

Healthy output is exactly:

```text
RECOVERY_NOT_REQUIRED <subject_sha256>
```

Recoverable output is exactly:

```text
RECOVERY_REQUIRED <recovery_id> <subject_sha256> <action_count>
RECOVERY_ACTION 1 <kind> <target>
...
```

Pending output is exactly:

```text
RECOVERY_PENDING <recovery_id> <next_action> <action_count>
```

Action lines use one-based contiguous sequence and deterministic target labels.

Unrecoverable inspection emits no success stdout and returns the exact error category.

## 17. Apply authorization and stale protection

`recovery apply` must:

1. validate exact `RECOVER`;
2. validate both lowercase hashes;
3. read and validate any recovery ledger;
4. recompute current subject, classification, plan, and recovery ID;
5. require exact equality with caller recovery ID and subject SHA-256;
6. require current Git object format, branch, and `HEAD` to equal the inspected subject;
7. reject a different request while a pending entry exists.

No mutation occurs before all checks pass.

## 18. Recovery ledger

The only new governance file is:

```text
.mrgs/recovery-ledger.json
```

It is append-only and contains exactly:

```text
schema_version
accepted_plan_sha256
plan_id
recoveries
```

Each entry contains exactly:

```text
recovery_id
plan
next_action
status
post_subject_sha256
recovery_receipt
recovery_receipt_sha256
```

`status` is `PENDING` or `APPLIED`.

For `PENDING`, post-subject and receipt fields are explicit `null`. For `APPLIED`, all are non-null. Missing keys and impossible null combinations are invalid.

At most one final entry may be pending, and no entry may follow a pending entry.

## 19. Pending journal publication

The pending entry is atomically appended before the first target action.

First publication and replacement use same-directory, create-new temporary files, bounded collision handling, flush/sync, no-clobber semantics, and atomic replacement. No truncate or copy fallback is allowed.

Failure preserves the previous recovery-ledger bytes and removes only the temp created by the current command.

## 20. Resumable action execution

After each action, `next_action` is atomically advanced.

On restart:

- current subject equal to prefix `next_action` means execute that action;
- current subject equal to prefix `next_action + 1` means the action completed before journal advancement, so advance without repeating;
- for `RESUME_CLOSEOUT`, an exact valid intermediate Phase 6 cleanup state may call the same resumable finalizer again;
- any unrelated subject is `RECOVERY_SUBJECT_STALE`.

Every action must be idempotent under these rules.

## 21. Recovery receipt

An applied recovery receipt contains exactly:

```text
schema_version
accepted_plan_sha256
plan_id
recovery_sequence
recovery_id
pre_subject_sha256
post_subject_sha256
action_count
actions_sha256
previous_recovery_receipt_sha256
```

Sequence starts at one and is contiguous. `actions_sha256` is the SHA-256 of compact canonical action-array JSON.

Receipt hash is the SHA-256 of compact canonical receipt JSON.

After publication, the complete recovery ledger, receipt chain, plan hashes, prefix hashes, and final healthy subject are re-read and validated.

## 22. Idempotency and conflicts

Applying an already `APPLIED` recovery ID is idempotent only when current subject equals its stored post-subject and current accepted-plan authority equals the stored authority. It returns the original output and writes nothing.

Success output is exactly:

```text
RECOVERY_APPLIED <recovery_sequence> <recovery_id> <pre_subject_sha256> <post_subject_sha256> <recovery_receipt_sha256>
```

A reused ID with different authority or subject is a conflict, not an idempotent replay.

## 23. Coexistence and non-gating rule

A valid recovery ledger is evidence of exceptional state repair. It does not replace plan, contract, implementation, audit, completion, or continuity authority.

Existing Phase 1–7 commands must preserve their outputs and behavior.

Where later implementation/audit inventory needs to ignore governance-generated untracked files, only exact `.mrgs/recovery-ledger.json` may receive the same topology-safe, untracked-only exemption as other recognized ledgers. No child path or alias is exempt.

## 24. Filesystem safety

All repository, `.mrgs`, permanent, temporary, and source paths are inspected with non-following metadata.

Reject symlinks, junctions, reparse points, directories where files are required, devices, sockets, FIFOs, sparse directory index entries, case aliases, path traversal, backslashes in normalized stored paths, and any escape from the repository.

Platform-dependent tests must execute the supported branch. A capability-unavailable branch must assert the missing capability and a concrete fail-closed fallback. Silent skips are not coverage.

## 25. Git, privacy, and network boundary

Recovery may run only read-only Git queries needed for repository identity and object format.

Every Git child must use the existing sanitized runner, remove inherited Git control variables, disable external helpers and replacement refs, and disable lazy promisor fetching.

Recovery must not inspect remotes, credentials, user identity, hostname, hardware, environment secrets, model provider, network state, or unrelated files.

No network access is permitted.

## 26. Persistence safety

All Phase 8 writes are limited to exact authorized `.mrgs` targets and use no-clobber temporary creation plus atomic replacement.

Handled failure leaves no new temp. Publication failure preserves prior destination bytes. Pre-existing temp files are never truncated or overwritten.

Restored accepted-plan and state bytes are re-read and fully validated before the next action.

## 27. Error categories

Add exact categories:

```text
RECOVERY_ID_INVALID
RECOVERY_DECISION_INVALID
RECOVERY_NOT_REQUIRED
RECOVERY_UNRECOVERABLE
RECOVERY_LEDGER_INVALID
RECOVERY_LEDGER_STALE
RECOVERY_PENDING_CONFLICT
RECOVERY_SUBJECT_STALE
RECOVERY_ACTION_FAILED
RECOVERY_POSTCONDITION_FAILED
```

Existing `FILESYSTEM_BOUNDARY_UNSAFE`, `REPOSITORY_INVALID`, `PERSISTENCE_FAILED`, and governance-authority categories remain controlling where specified.

Errors print through the existing exact format and produce no success stdout.

## 28. Dependencies

Do not add or change dependencies, features, build scripts, lockfile content, environment configuration, or runtime services.

Use the existing standard library plus current `clap`, `serde`, `serde_json`, `toml`, `sha2`, and `thiserror` dependencies.

## 29. Required tests

Create `tests/phase8.rs` with exactly 88 numbered obligation tests, one for each item below. Supplemental regression tests are allowed but must be reported separately.

1. Exact CLI parsing for `recovery inspect --repo <REPOSITORY_PATH>`.
2. Exact CLI parsing for `recovery apply` with `--recovery-id`, `--subject-sha256`, and `--decision`.
3. Missing, duplicated, unknown, or malformed recovery arguments are rejected without writes.
4. `recovery inspect` is read-only and preserves every repository, Git, and governance byte.
5. A healthy repository returns the exact `RECOVERY_NOT_REQUIRED` output.
6. The recovery subject SHA-256 is deterministic across repeated identical inspections.
7. Recovery-subject inventory entries are complete, sorted, unique, and use exact byte hashes.
8. The subject binds the exact Git object format, current branch, and current `HEAD` without requiring a clean worktree.
9. Detached `HEAD`, unborn `HEAD`, or an unreadable Git repository is rejected without mutation.
10. The `.mrgs` directory must be a real in-repository directory and may not be a symlink, junction, or other reparse point.
11. An unknown direct child of `.mrgs` is unrecoverable and is never silently ignored or deleted.
12. A nested directory, non-UTF-8 child name, device, socket, FIFO, or other unsupported governance object is rejected.
13. A symlink at any permanent governance filename is rejected before any recovery publication.
14. The supported Windows reparse-point branch executes, or a capability-unavailable branch proves the fallback safety assertion.
15. A valid accepted-plan record and exact plan source are recognized as authoritative.
16. Plan-source absence, unsafe topology, parse failure, or hash drift is unrecoverable.
17. A missing accepted-plan with no valid completion ledger is unrecoverable.
18. A malformed accepted-plan with no valid completion ledger is unrecoverable.
19. A missing or malformed accepted-plan is reconstructible from a valid self-contained completion ledger and exact plan source.
20. Reconstructed accepted-plan JSON has exact required fields, values, deterministic bytes, and SHA-256.
21. Disagreement among completion manifests about plan identity, path, bytes, or phase count is unrecoverable.
22. A completion ledger with invalid hashes, receipt chain, schema, or raw required fields is unrecoverable.
23. A valid existing state record and accepted-plan relation are recognized without rewriting state.
24. Missing state before any phase selection is reconstructed as active `null` and an empty closed-phase list.
25. Malformed state before any phase selection is reconstructed identically and atomically.
26. Closed phases are reconstructed exactly from the final valid completion receipt's `closed_phases_after`.
27. An active phase is inferred only from a valid phase-scoped authority prefix rooted at `contract-draft.json`.
28. A present accepted-contract ledger is fully validated and bound to the inferred draft phase and contract.
29. A present implementation authority is fully validated and bound to the inferred accepted contract.
30. A present audit ledger is structurally and historically validated and bound to the inferred implementation authority.
31. A later phase-scoped file without every required predecessor file is unrecoverable.
32. Disagreement among phase-scoped records about plan, phase, contract, revision, or authority is unrecoverable.
33. An inferred active phase may not already be closed and must have every plan dependency closed.
34. A valid selected phase with no contract draft remains healthy and is not erased or guessed away.
35. A valid completion ledger and state must satisfy the exact completion-to-state relation.
36. A published final completion with unfinished cleanup or state transition is classified as recoverable incomplete closeout.
37. When state is missing during incomplete closeout, the exact pre-closeout state is reconstructed from the final receipt before resumption.
38. Closeout resumption accepts only remaining phase-scoped files whose bytes exactly equal the archived manifest bytes.
39. Closeout resumption removes phase-scoped files only in the existing fixed order and reaches the exact closed state.
40. A phase-scoped byte mismatch during incomplete closeout is unrecoverable and no file is removed.
41. A valid continuity ledger is validated against the reconstructed or existing accepted-plan and completion authority.
42. A malformed, stale, or topology-unsafe continuity ledger is unrecoverable and is never regenerated.
43. A valid absent recovery ledger is accepted; an existing recovery ledger must pass strict raw-key, schema, hash, and chain validation.
44. A corrupt, stale, or topology-unsafe recovery ledger blocks all recovery mutation.
45. Recognized producer temporary filenames map to one exact permanent target; unknown temp names are unrecoverable.
46. A regular producer temp identical to an existing valid target is classified as `REMOVE_REDUNDANT_TEMP`.
47. A differing temp, target-absent non-recovery temp, duplicate candidate, unsafe temp topology, or ambiguous mapping is unrecoverable.
48. Recovery-owned deterministic temp files are bound to a pending journal entry and can be safely promoted or removed after interruption.
49. Recovery actions use a closed enum, exact required fields, strict normalized paths, and no unknown fields.
50. Action order is deterministic: redundant-temp cleanup, accepted-plan restoration, state restoration, then closeout resumption.
51. The recovery plan contains exact prefix subject hashes of length `actions + 1`.
52. The recovery ID is the SHA-256 of the canonical plan seed and is deterministic for identical subjects and actions.
53. `recovery inspect` prints exact ordered `RECOVERY_REQUIRED` and `RECOVERY_ACTION` lines for a recoverable subject.
54. Repeated inspection of an unchanged recoverable subject returns byte-identical output and performs no writes.
55. An unrecoverable subject produces the exact recovery error category, no success output, and no mutation.
56. `recovery apply` accepts only lowercase 64-hex recovery and subject hashes and exact uppercase `RECOVER`.
57. A wrong recovery ID, stale subject hash, wrong decision token, or changed Git identity is rejected before publication.
58. Apply recomputes the entire plan rather than trusting caller-supplied action content.
59. A pending recovery entry is durably published before the first target mutation.
60. Pending entries contain the exact plan, prefix hashes, action list, and `next_action = 0`.
61. Each completed action advances `next_action` atomically and preserves the prior recovery-ledger bytes on failure.
62. A crash before the first action resumes from the same pending recovery ID.
63. A crash after a target action but before journal advancement is detected from the next prefix subject and resumes without duplicate mutation.
64. A crash during Phase 6 closeout cleanup resumes through the existing closeout finalizer and does not create a second completion.
65. A different recovery request while one entry is pending is rejected as a pending conflict.
66. After all actions, the post-recovery subject is recomputed, must be healthy, and must equal the final prefix subject hash.
67. Successful recovery finalizes the pending entry as `APPLIED` with no nullable-field ambiguity.
68. The recovery receipt has exact required fields, contiguous sequence, action count/hash, pre/post subjects, and prior-receipt link.
69. Recovery receipt and stored receipt SHA-256 values recompute exactly from canonical JSON.
70. Recovery-ledger append order, recovery IDs, receipt chain, and accepted-plan binding are fully revalidated after publication.
71. Successful apply prints the exact `RECOVERY_APPLIED` output.
72. Exact replay of an applied recovery returns the original output and preserves every byte.
73. Replay with post-recovery subject drift or mismatched authority is rejected rather than treated as idempotent.
74. A second independent recovery appends sequence two and links to the first receipt without rewriting entry one.
75. No recovery action writes outside `.mrgs`, edits plan/contract source files, or changes source-repository bytes.
76. Recovery executes no Git mutation, networking, credential discovery, host discovery, telemetry, or external helper command.
77. All Git child processes use the existing sanitized environment, disable lazy object fetching, and reject injected Git control variables.
78. All existing Phase 1–7 command outputs and error categories remain unchanged.
79. `.mrgs/recovery-ledger.json` receives only the exact topology-safe untracked governance exemption required by later implementation checks.
80. A tracked recovery ledger, case alias, child path, symlink, or arbitrary `.mrgs` path is not exempt.
81. Recovery publication uses create-new temporary files, bounded collision handling, same-directory replacement, and no truncate fallback.
82. A pre-existing recovery temp collision is never truncated, and a failed replacement preserves the previous recovery ledger.
83. Handled failure leaves no newly created recovery temp; interruption leftovers remain recoverable only through the journal rules.
84. All platform-sensitive file replacement, share-mode, symlink, and reparse branches execute or contain explicit capability and fallback assertions.
85. Applying to a healthy subject returns `RECOVERY_NOT_REQUIRED`, creates no recovery ledger, and changes no byte.
86. Every new recovery error uses the exact existing stderr format, has no success stdout, and maps to its specified category.
87. No new dependency, feature, build script, generated registry, background service, or hidden configuration is introduced.
88. The Phase 8 test binary never recursively invokes the full `cargo test` suite and every obligation has a direct executable assertion.

Every numbered obligation requires a direct executable assertion against production behavior. Name-only mappings, source-presence-only checks, helper-only checks when command routing is required, silent platform skips, and recursive full-suite invocation are weak evidence.

## 30. Verification

Required final ladder:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo test --test phase8
cargo clippy --all-targets -- -D warnings
cargo test
git diff --check
```

Run each command directly and inspect its real exit code. Do not use a pipeline or wrapper that can mask failure.

Do not invoke full `cargo test` from inside `tests/phase8.rs`.

A timeout, truncated output, skipped command, or unexecuted platform branch is not a pass.

## 31. Allowed implementation paths

Only these paths may change:

```text
README.md
src/cli.rs
src/closeout.rs
src/continuity.rs
src/error.rs
src/implementation.rs
src/main.rs
src/recovery.rs
src/state.rs
tests/phase8.rs
docs/contracts/phase-08-contract.md
```

Create only:

```text
src/recovery.rs
tests/phase8.rs
```

The supplied contract is frozen and may only remain as the exact supplied file.

## 32. Forbidden changes

Do not modify:

```text
AGENTS.md
Cargo.toml
Cargo.lock
.gitignore
docs/master-plan.md
docs/contracts/phase-01-contract.md
docs/contracts/phase-02-contract.md
docs/contracts/phase-03-contract.md
docs/contracts/phase-04-contract.md
docs/contracts/phase-05-contract.md
docs/contracts/phase-06-contract.md
docs/contracts/phase-07-contract.md
src/path.rs
src/git.rs
src/plan.rs
src/contract.rs
src/rules.rs
src/audit.rs
tests/integration.rs
tests/phase4_obligations.json
tests/phase4_obligations.rs
tests/phase5.rs
tests/phase6.rs
tests/phase7.rs
graphify-out/**
.git/**
.mrgs/**
target/**
```

The `.mrgs/**` prohibition applies to repository implementation changes and fixtures, not runtime files created inside isolated test repositories.

Do not weaken, delete, rename, ignore, or replace an existing test.

## 33. Final evidence

The final handoff must include:

- baseline branch and exact baseline/final `HEAD`;
- frozen contract SHA-256;
- exact changed and created paths;
- forbidden-path result;
- exact Phase 8 targeted count and supplemental count;
- exact full-suite per-binary counts;
- formatting, check, clippy, and diff-check exit results;
- final read-only mapping `88/88`, zero missing, zero weak;
- recovery audit and bounded repair-cycle counts;
- Graphify reconnaissance/refresh as advisory evidence only;
- final Git status and staged paths;
- confirmation that no commit or push occurred;
- exact blockers or `NONE`;
- recommendation for the human Git boundary.

## 34. Completion rule

Phase 8 is complete only when:

1. all contract requirements are implemented;
2. all 88 numbered tests contain direct, non-vacuous executable assertions;
3. every required verification command exits zero;
4. all Phase 1–7 tests remain green;
5. no forbidden path changed;
6. the frozen contract bytes are unchanged;
7. no file is staged;
8. no commit or push occurred;
9. a final independent read-only audit maps `88/88`, with zero missing and zero weak.

Phase 8 authorizes state recovery and corruption handling only. It does not authorize Phase 9 adversarial expansion, Phase 10 activation readiness, Git integration, commit, or push.
