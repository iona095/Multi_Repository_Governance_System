# Phase 9 Contract — Adversarial, Security, Resource, and Regression Validation

Contract version: 3
Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

## Revision 3 — Concurrent Audit Record Compare-and-Swap Repair

Revision 3 preserves every requirement of Phase 9 contract versions 1 and 2
except for the exact bounded repair authority defined here.

### Reason for revision

Phase 9 obligation 38 exposed a production concurrency defect in
`cmd_audit_record` (`src/audit.rs`). The operation reads the audit ledger
once at flow start, validates and transitions the round in memory, then
publishes via temporary file plus atomic replace with no compare-and-swap:
two concurrent conflicting callers can commit from the same stale ledger
preimage, and a conflicting loser reports transient success for its own
payload while the durable round belongs to another caller. This violates
obligation 38 ("losers fail with the existing conflict/stale category") and
Section 10 ("a loser process must fail or return the exact idempotent
result"). The defect is accepted as production-side evidence; the failing
obligation must remain preserved.

### Supersession

After separate human acceptance of this revision, and only then, the
following provisions are overridden for the authorized paths below and
only for them: Section 4 (validation-only implementation boundary),
Section 18 (allowed implementation paths), Section 19 (forbidden changes,
including `src/**` and the frozen-contract rule), and Section 20
(source-defect escalation rule, whose escalation outcome this revision
is). All other sections remain fully controlling. Revision 2's
authorized-path authority is revoked: `src/implementation.rs` is no longer
authorized to change under any Phase 9 revision and returns to the
forbidden-change set. This draft authorizes nothing by itself; no repair
may begin until this contract revision is accepted by a separate human
decision.

### Authorized paths

Revision 3 authorizes changes only to:

```text
docs/contracts/phase-09-contract.md
src/audit.rs
tests/phase9.rs
```

`docs/contracts/phase-09-contract.md` may change only to record contract
version 3 and this exact repair authorization. No other source, test,
manifest, dependency, lockfile, README, schema, CLI, recovery, closeout,
continuity, or implementation file may change.

### Required repair semantics

The audit-record repair must make conflicting publication linearizable. The
protected critical operation must include, within one coordinated interval
bounded to the canonical repository:

1. reading the current audit ledger;
2. validating the expected audit state and round;
3. classifying replay, conflict, stale authority, or permitted transition;
4. constructing the next ledger;
5. durably publishing that ledger;
6. completing while no conflicting writer can commit from the same stale
   preimage.

A post-write re-read alone is explicitly insufficient evidence of
compare-and-swap correctness. The implementation may use an existing
repository coordination or persistence primitive if one already exists and
preserves the current durability guarantees.

### Required concurrent result

For eight genuinely simultaneous callers containing conflicting valid PASS
and FAIL payloads for the same pending round:

1. one conflicting payload may become the durable winner;
2. an exactly identical caller may return the exact idempotent result;
3. every caller whose payload conflicts with the durable winner must fail
   with the existing contract-authorized audit conflict or stale category;
4. no conflicting loser may report transient success;
5. exactly one canonical round may survive;
6. no temporary file may remain;
7. audit-ledger bytes and receipts must remain valid;
8. supplied source repositories and unrelated paths must remain unchanged.

Do not weaken the existing obligation-38 synchronization or loser
assertions.

### Private synchronization mechanism constraints

Any private synchronization mechanism must:

- be process-shared and work across independent `mrgs` processes;
- be bounded to the supplied repository;
- preserve crash safety (automatic release when a participating process
exits or crashes) and avoid stale permanent lock state;
- remain inert outside the audit-record mutation;
- preserve exact replay behavior.

### Forbidden additions and repairs

Do not add a public command or flag; a new error category; a new governance
schema or schema version; a dependency; a background service; network
behavior; a permanent lock or authority file; success-output changes;
weakened atomic persistence; or a test-only success bypass. Do not rely on
a post-write re-read as the compare-and-swap mechanism. Do not accept a
conflicting loser's transient success as legal. Do not weaken, delete,
rename, ignore, or condition away obligation 38 or its loser assertions.

### Sequential compatibility

Revision 3 must preserve:

- ordinary single-writer audit record;
- exact replay;
- PASS terminal behavior;
- FAIL repair routing;
- malformed-report validation order;
- existing stdout and stderr;
- existing ledger bytes, schema, hashes, and receipts;
- existing persistence-failure behavior.

### Verification authorized after human acceptance

The later implementation mission must run only:

1. obligation 38 repeatedly enough to establish deterministic concurrency
   behavior;
2. exact obligations 37–40;
3. the smallest existing Phase 5 audit/repair regression set affected by
   `cmd_audit_record`;
4. compile and diff checks for the changed surface.

Do not rerun obligations 01–36 or 41–51 unless the `src/audit.rs` change
can plausibly invalidate a specific test. After the Revision 3 repair gate
passes, resume at obligation 52. No staging, commit, or push is authorized
by Revision 3.

## Revision 2 — Concurrent Implementation Publication Repair

Revision 2 preserves every requirement of Phase 9 contract version 1 except
for the exact bounded repair authority defined here.

### Reason for revision

Phase 9 obligation 36 exposed a production concurrency defect in
`implementation begin`. During concurrent first publication, one process
may create its legitimate in-flight `.mrgs_impl_tmp_<pid>_<attempt>_<nanos>.tmp`
file while another process is still performing begin-time Git cleanliness
validation, causing the second process to report `GIT_DIRTY` instead of
reaching the canonical publication race. The defect is accepted as
production-side evidence; the failing obligation must remain preserved.

### Authorized paths

Revision 2 authorizes changes only to:

```text
src/implementation.rs
tests/phase9.rs
docs/contracts/phase-09-contract.md
```

`docs/contracts/phase-09-contract.md` may change only to record contract
version 2 and this exact repair authorization.

### Required production behavior

Concurrent `implementation begin` callers targeting the same repository and
same eligible accepted contract must satisfy all of the following:

1. At most one canonical `implementation-authority.json` publication occurs.
2. Every other caller returns either the exact idempotent
   `IMPLEMENTATION_BOUND ...` result or `IMPLEMENTATION_AUTHORITY_CONFLICT`.
3. No caller returns `GIT_DIRTY` solely because another live caller created
   the implementation publisher's own in-flight temporary file.
4. No partial authority file is accepted.
5. No duplicate authority is created.
6. No publisher temporary file remains after all callers exit.
7. Existing strict cleanliness behavior remains unchanged for pre-existing
   temporary files, stale producer temporary files, malformed near-match
   temporary names, unknown `.mrgs` paths, symlinks, junctions or reparse
   points, and non-regular filesystem objects.
8. A pre-existing temp-shaped path must not become exempt merely because
   its name matches producer grammar.
9. No new public command, flag, success token, error category, durable
   governance file, dependency, or configuration is introduced.
10. The coordination mechanism must release automatically if a participating
    process exits or crashes.
11. The repair must not write a lock or coordination file into the
    repository, `.git`, another repository, or an external temporary
    directory.
12. Git remains read-only.

### Required repair architecture

The production operation must coordinate concurrent `implementation begin`
processes per canonical repository before the race-sensitive
implementation-authority existence and cleanliness decision. The coordinated
interval must cover implementation-authority existence classification,
begin-time cleanliness validation, first publication or existing-record
validation, and idempotent/conflict resolution. An operating-system
coordination primitive that creates no durable filesystem artifact is
permitted. The existing debug-only pre-publication synchronization hook may
be repositioned so eight test callers synchronize before coordination
acquisition, but moving the hook alone is explicitly insufficient.

### Forbidden repairs

Do not merely move the test hook and leave unhooked production behavior
unchanged; broadly exempt `.mrgs_impl_tmp_*` from Git cleanliness; delete
another process's temporary file; infer ownership only from filename
grammar; introduce sleeps as correctness; introduce an unbounded retry
loop; create a repository lock file; create an external lock file; weaken
obligation 36; accept `GIT_DIRTY` as a valid loser result; or modify Phase 4
regression expectations to hide the defect.

### Required direct tests and regression set

Revision 2 requires the preserved obligation 36 with exactly eight
synchronized callers, one canonical durable publication, no `GIT_DIRTY`
caller, every loser classified as idempotent success or
`IMPLEMENTATION_AUTHORITY_CONFLICT`, no leftover temporary file, valid
final authority bytes, unchanged fixture Git state, repeated execution, an
unhooked concurrent supplemental case, a pre-existing canonical
producer-temp case that continues to return `GIT_DIRTY`, malformed,
unknown, symlinked, and non-regular temp cases that remain rejected, and
unchanged `IMPLEMENTATION_AUTHORITY_CONFLICT` atomic destination-race
behavior. The affected regression set and full Phase 4 obligation test
binary must pass, and the unfinished Phase 9 obligations plus the full
Phase 9 and repository verification ladders remain required before
completion. No staging, commit, or push is authorized by revision 2.

## 1. Objective

Validate the complete Phase 1–8 governance implementation under adversarial input, hostile filesystem topology, authority corruption, interrupted persistence, replay and concurrency pressure, privacy boundaries, deterministic resource stress, and end-to-end regression.

Phase 9 is validation-only. It adds no production capability, command, governance file, authority record, output token, error category, dependency, runtime configuration, network integration, background service, or Git operation.

Phase 9 implements only one new test binary:

```text
tests/phase9.rs
```

The Phase 9 test binary must exercise public CLI behavior and durable repository effects. It may independently define test fixtures and helpers, but it may not duplicate production algorithms merely to prove that equivalent test code agrees with itself.

## 2. Authority and non-gating rule

All Phase 1–8 authority remains controlling:

- accepted plan and exact plan bytes;
- phase selection and dependency ordering;
- contract draft, acceptance, and revision authority;
- implementation authority and path enforcement;
- audit and bounded repair history;
- closeout manifests, receipts, and completion ordering;
- continuity manifests, receipts, and cross-repository proof;
- recovery diagnosis, plans, journal entries, actions, and receipts.

Phase 9 creates no authority that can replace, repair, normalize, waive, or reinterpret any Phase 1–8 record.

Phase 9 evidence is test evidence only. A passing Phase 9 result does not authorize Phase 10, activation, deployment, Git integration, commit, push, or a production-source change.

## 3. No new runtime surface

Preserve the exact existing command surface:

```text
mrgs plan accept --repo <REPOSITORY_PATH> --plan <PLAN_PATH>
mrgs phase select --repo <REPOSITORY_PATH> --phase <PHASE_ID>
mrgs contract draft --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH>
mrgs contract accept --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256> --decision <DECISION>
mrgs contract revise --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH> --expected-revision <REVISION> --expected-sha256 <SHA256>
mrgs implementation begin --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256>
mrgs implementation check --repo <REPOSITORY_PATH>
mrgs audit begin --repo <REPOSITORY_PATH> --auditor <AUDITOR_ID>
mrgs audit record --repo <REPOSITORY_PATH> --report <REPORT_PATH>
mrgs repair check --repo <REPOSITORY_PATH>
mrgs phase close --repo <REPOSITORY_PATH> --phase <PHASE_ID>
mrgs continuity record --repo <REPOSITORY_PATH> --metadata <METADATA_PATH> [--source-repo <SOURCE_REPOSITORY_PATH>]...
mrgs recovery inspect --repo <REPOSITORY_PATH>
mrgs recovery apply --repo <REPOSITORY_PATH> --recovery-id <RECOVERY_ID> --subject-sha256 <SUBJECT_SHA256> --decision <DECISION>
```

Do not add any Phase 9 CLI command or flag. Do not change Clap grammar, success stdout, error stderr, exit behavior, governance filenames, schema versions, or accepted token casing.

## 4. Validation-only implementation boundary

The initial Phase 9 implementation authority is exactly:

```text
tests/phase9.rs
```

No production source file and no existing test file may change under contract version 1.

A test may invoke the compiled `mrgs` binary, Git, `rustc`, and operating-system filesystem primitives required to construct isolated test repositories. A test may use existing test-only environment hooks already implemented by Phase 1–8. An ephemeral Git recorder or fault wrapper compiled under a temporary fixture directory is allowed when it delegates to the resolved real Git binary and is deleted with the fixture. Phase 9 may not add a repository executable, production hook, feature flag, dependency, runtime helper, or external service.

Every test repository must be isolated under a temporary directory. Tests must not use the real MRGS working repository as a runtime target.

## 5. Public-surface coverage

The required test set must reach every command listed in Section 3 through the compiled public binary.

For each rejection case, the test must assert the applicable combination of:

- non-zero process exit;
- exact existing error category or exact Clap rejection class;
- empty success stdout;
- unchanged pre-existing governance bytes;
- absence of newly published governance files;
- absence or contractually valid disposition of temporary files;
- unchanged Git `HEAD`, index, branch, configuration, and worktree except for fixture changes made directly by the test;
- no mutation outside the isolated repository and explicitly supplied source paths.

Source-presence checks, helper-only checks, comments, test names, and aggregate test counts are not direct evidence.

## 6. Adversarial input model

Input validation must cover, where representable by the host process API and applicable to the command:

- missing required arguments;
- duplicate non-repeatable arguments;
- unknown arguments and unknown subcommands;
- wrong token casing;
- empty and whitespace-only values;
- leading or trailing whitespace;
- ASCII control characters;
- embedded NUL rejected by the process API or application boundary;
- malformed, uppercase, and mixed-case SHA-256 values;
- zero, overflow, and malformed revision values;
- one-byte, exact-boundary, and one-over-boundary identifiers;
- Unicode scalar and normalization edge cases without silently rewriting submitted bytes;
- unknown, missing, duplicate-semantic, or reordered authority fields as governed by the existing schema;
- trailing data and malformed UTF-8 where representable;
- source bytes that parse but are semantically false.

The test suite must preserve existing validation order. It may not accept a later error merely because the command eventually failed.

## 7. Filesystem threat model

Filesystem validation must cover:

- repository escape through `.` or `..` components;
- absolute, drive-prefixed, UNC, and device-prefixed persisted paths where prohibited;
- doubled separators and backslash normalization attacks;
- symlink traversal on systems that support symlinks;
- Windows junction and non-symlink reparse-point handling;
- directories, FIFOs, sockets, devices, and other non-regular objects where the host supports their creation;
- unsafe `.git` and `.mrgs` topology;
- nested unexpected governance objects;
- case aliases where the filesystem permits them;
- temporary-file ambiguity and duplicate candidates;
- destination replacement failure and no-truncate behavior;
- external source-path arguments and cross-repository source roots.

No test may silently follow an unsafe object and then claim safety from the final canonical path alone.

## 8. Governance-authority corruption model

The Phase 9 test suite must tamper with the complete Phase 1–8 authority chain:

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

Tampering must include representative instances of:

- unknown raw keys;
- missing required keys;
- incorrect schema versions;
- valid-format but false hashes;
- broken previous-receipt links;
- stale phase, revision, plan, contract, subject, or repository bindings;
- non-contiguous sequence values;
- invalid ordering and duplicate semantic identifiers;
- parse-valid but semantically false content;
- disagreement between persisted records and archived exact bytes.

The command under test must fail closed before mutation whenever its existing validation order requires the affected authority to be validated first.

## 9. Persistence and interruption model

Persistence evidence must be deterministic. Use existing test-only synchronization hooks, preconstructed interrupted durable states, create-new collisions, safe permission failures, or controlled destination topology. Do not use random process termination.

The suite must distinguish these interruption points when the relevant Phase 1–8 operation supports them:

- before temporary-file creation;
- after temporary-file creation;
- immediately before atomic replacement;
- after target replacement but before journal advancement;
- during closeout cleanup;
- during recovery action execution;
- during ledger publication;
- during replay of an incomplete durable operation.

Each persistence test must assert the exact applicable combination of prior-byte preservation, temporary-file disposition, journal state, resumability, idempotency, error category, and absence of unrelated mutation.

## 10. Replay and concurrency model

Concurrency tests must use deterministic synchronization primitives: existing signal/release files, process barriers, channels, or a preconstructed durable state. Arbitrary sleeps are forbidden as the proof mechanism.

Required concurrency and replay conditions include:

- exact replay;
- conflicting replay;
- stale authorization;
- concurrent first publication;
- concurrent duplicate publication;
- concurrent conflicting publication;
- interrupted operation followed by retry;
- action completed before journal advancement;
- journal advancement before the caller observes success;
- fixed-point replay after completion.

At most one canonical durable result may be accepted. A loser process must fail or return the exact idempotent result without corrupting, duplicating, or rewriting accepted authority.

## 11. Privacy and external-effects model

Tests must prove that the Phase 1–8 commands do not unexpectedly:

- access the network;
- invoke a shell;
- spawn an executable other than the governed Git child processes already required by MRGS;
- inspect or persist environment secrets;
- persist usernames or host-discovered identity;
- persist prohibited absolute repository or source paths;
- expose source contents in success output;
- expose credentials or sentinel environment values in errors;
- mutate Git state;
- modify another repository except for explicitly authorized read-only source proof;
- write outside the canonical target repository and expressly supplied source/destination files.

Use sentinel environment values, sentinel files, isolated `PATH`, and test-controlled Git wrappers where the current test architecture permits. Do not require real credentials, live providers, internet access, or external services.

## 12. Deterministic resource-validation model

Resource validation must use exact bounded fixtures and structural assertions rather than machine-specific timing thresholds.

The required fixture sizes are:

- plan fixture: exactly 128 phases in one valid linear dependency chain;
- contract fixture: exactly 256 requirements, 64 allowed paths, 64 forbidden paths, 64 verification commands, and 64 handoff fields;
- audit fixture: exactly 3 audit rounds with the existing maximum 2 checked repair attempts and one terminal result;
- completion fixture: exactly 32 contiguous completion entries;
- continuity fixture: exactly 32 contiguous continuity entries and exactly 16 resolved predecessor links in the final entry;
- recovery fixture: exactly 32 applied recovery entries and one separately constructed pending entry with the maximum action count produced by a deterministic fixture;
- governance inventory fixture: exactly 256 ordinary tracked files plus every valid Phase 1–8 governance file;
- recognized temporary-candidate fixture: exactly 15 occupied candidate names followed by the sixteenth available candidate, matching the existing bounded collision search;
- replay fixture: exactly 64 repeated exact replays;
- read-only fixture: exactly 64 repeated `recovery inspect` operations;
- concurrency fixture: exactly 8 synchronized callers;
- scalar boundary fixture: one exact maximum and one one-over-maximum value for every existing bounded scalar exercised by the selected command.

These are test fixture counts, not new runtime limits. A test must not assert that an unbounded production field is invalid merely because the fixture uses a finite count.

Resource tests must prove deterministic output or deterministic rejection, no panic, no deadlock, no unbounded recursion, no partial publication, no mutation amplification, no duplicate ledger entry, and complete cleanup or resumability after an injected failure.

A timeout is failure evidence, never passing evidence.

## 13. Cross-platform evidence model

Every platform-sensitive obligation must report one exact branch:

```text
CAPABILITY_EXECUTED
CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION
```

`CAPABILITY_EXECUTED` requires the real topology or behavior to be created and tested.

`CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION` requires:

- an explicit assertion that the host cannot create or expose the capability;
- a concrete production-facing fallback safety assertion for the same trust boundary;
- no silent return, ignored result, or unconditional pass.

A skipped or ignored test is not evidence.

## 14. Determinism requirements

Acceptance evidence may not depend solely on:

- random fuzz seeds;
- nondeterministic directory or map ordering;
- current time;
- process ID as an asserted value;
- machine hostname;
- internet or provider availability;
- arbitrary sleeps;
- aggregate-only test counts;
- skipped tests;
- manual visual inspection;
- Graphify output.

Randomized property cases, if used without a new dependency, must use fixed documented seeds and bounded case counts. Every failure must print the seed and minimized concrete case. Deterministic table-driven cases are preferred.

## 15. Test architecture and evidence discipline

`tests/phase9.rs` must contain an obligation map from `1` through `64` at the top of the file.

Each obligation has exactly one primary function named:

```text
test_obligation_NN_<descriptive_name>
```

Every primary function must contain or call direct executable assertions against the compiled `mrgs` binary and observed durable effects. Shared helpers are allowed, but a primary function consisting only of source inspection, string search, test enumeration, or an unconditional helper call with no obligation-specific assertion is weak evidence.

Supplemental tests may be added in `tests/phase9.rs`, but must not use the `test_obligation_` prefix and must be reported separately.

The Phase 9 binary must not recursively invoke `cargo test`, `cargo clippy`, `cargo check`, or itself.

## 16. Required tests

Exactly 64 numbered obligations are required. Each validation family contains exactly eight obligations.

### 16.1 CLI and adversarial input validation
1. `test_obligation_01_plan_and_phase_cli_rejection_matrix` — Exercise `plan accept` and `phase select` with missing, duplicate, and unknown arguments; empty, whitespace, control-character, Unicode-edge, and boundary-length phase or path values; assert exact Clap/application rejection, empty success stdout, and zero governance or Git mutation.
2. `test_obligation_02_contract_cli_and_source_adversarial_matrix` — Exercise `contract draft`, `contract accept`, and `contract revise` with wrong token casing, zero/overflow/malformed revisions, malformed/uppercase/mixed-case SHA-256 values, missing or duplicate arguments, and strict TOML cases containing unknown fields, missing fields, duplicate list entries, trailing data, and parse-valid semantic falsehoods; assert validation-order-correct failure and byte preservation.
3. `test_obligation_03_implementation_cli_rejection_matrix` — Exercise `implementation begin` and `implementation check` with missing, duplicated, unknown, malformed revision/SHA, stale authorization, whitespace/control values, and injected Git-control environment values; assert exact existing category, no success stdout, no authority publication, and unchanged Git state.
4. `test_obligation_04_audit_and_repair_cli_rejection_matrix` — Exercise `audit begin`, `audit record`, and `repair check` with malformed auditor IDs, malformed report bytes, unknown/missing report fields, false subject hashes, duplicate semantic identifiers, stale rounds, missing/duplicate/unknown CLI arguments, and exact-boundary/one-over-boundary identifiers; assert fail-closed behavior and no unrelated mutation.
5. `test_obligation_05_closeout_cli_rejection_matrix` — Exercise `phase close` with wrong/missing/duplicate/unknown arguments, empty or malformed phase IDs, wrong casing, already-closed or non-active phases, incomplete authority, and semantically false but parse-valid final audit authority; assert exact error category, no success stdout, no completion publication, and preservation of phase-scoped bytes.
6. `test_obligation_06_continuity_cli_and_metadata_adversarial_matrix` — Exercise `continuity record` with missing/duplicate/unknown arguments, empty and unsafe source repositories, malformed metadata UTF-8 where representable, unknown/missing nested fields, unsorted or duplicate models/hosts/links, uppercase hashes, stale completion binding, control characters, and exact scalar boundaries; assert strict rejection and no ledger/source mutation.
7. `test_obligation_07_recovery_cli_rejection_matrix` — Exercise `recovery inspect` and `recovery apply` with missing/duplicate/unknown arguments, malformed or uppercase hashes, wrong decision casing, stale subject/recovery IDs, embedded control values, healthy/unrecoverable/pending subjects, and parse-valid corrupt journal content; assert the exact recovery category, exact stdout emptiness or required read-only output, and zero unauthorized mutation.
8. `test_obligation_08_global_error_and_no_mutation_invariants` — Run a table covering all fourteen command surfaces and at least one representative adversarial rejection per surface; for every case assert non-zero exit, exact existing stderr form or Clap class, no success stdout, unchanged governance byte inventory, unchanged Git `HEAD`/index/branch/config, no new temporary file, and no write outside the isolated repository.

### 16.2 Filesystem and path-topology security
9. `test_obligation_09_repository_root_and_escape_topology` — Validate repository-root arguments containing `.`, `..`, doubled separators, trailing separators, relative escapes, absolute aliases, drive/UNC/device prefixes where applicable, and roots whose ancestors are unsafe; assert canonical-boundary rejection without creating or changing `.mrgs`.
10. `test_obligation_10_source_path_normalization_and_external_boundaries` — Across plan, contract, audit-report, and continuity-metadata source arguments, validate absolute/external paths, `.git`/`.mrgs` paths, backslashes, doubled separators, empty components, `.`/`..`, control characters, and path aliases; assert only contractually authorized source locations are accepted and persisted paths are exact normalized repository-relative values.
11. `test_obligation_11_governance_directory_and_unknown_child_topology` — Construct `.mrgs` as a file, symlink, junction/reparse point, or directory with nested unexpected objects, case aliases, and non-regular children where supported; assert each affected public command fails with the existing filesystem/governance category and never deletes or rewrites the hostile object.
12. `test_obligation_12_symlink_traversal_capability_branch` — Create source, governance, ancestor, leaf, and Git-layer symlink attacks on supported systems and assert rejection before unsafe traversal; otherwise report `CAPABILITY_UNAVAILABLE_WITH_FALLBACK_ASSERTION` and prove the lexical and ordinary-directory fallback rejects the equivalent escape.
13. `test_obligation_13_windows_reparse_and_junction_capability_branch` — On Windows, create or detect a non-symlink reparse point/junction at repository, `.mrgs`, source ancestor, and destination boundaries and assert rejection; when unavailable or on non-Windows, explicitly assert capability absence and execute a concrete non-regular/ancestor fallback safety case.
14. `test_obligation_14_nonregular_file_and_external_source_objects` — Use every host-creatable non-regular object class—directory, FIFO, socket, device substitute, dangling link, locked/unreadable file—and external source roots; assert safe deterministic rejection, no follow-through mutation, and a capability/fallback result for unsupported object classes.
15. `test_obligation_15_temporary_ambiguity_and_destination_replacement` — Construct recognized and unknown temporary names, duplicate candidates for one target, target-absent and target-present variants, occupied create-new slots, and a controlled replacement failure; assert no truncation, deterministic classification, prior-byte preservation, and only contractually resumable leftovers.
16. `test_obligation_16_cross_repository_path_and_isolation_boundary` — Exercise continuity source-repository arguments with duplicate canonical roots, target-equals-source, symlink/reparse aliases, unsafe source governance topology, unreferenced roots, and two distinct repositories containing sentinels; assert one-to-one resolution, read-only source proof, and no mutation of either unrelated repository.

### 16.3 Governance-authority corruption and stale-state handling
17. `test_obligation_17_accepted_plan_corruption_matrix` — Tamper `accepted-plan.json` with unknown/missing keys, schema drift, false lowercase hash, stale plan path/count/ID, malformed exact source bytes, and parse-valid plan disagreement; exercise commands whose validation order consumes accepted-plan authority and assert fail-closed behavior before mutation.
18. `test_obligation_18_state_corruption_and_plan_relation_matrix` — Tamper `state.json` with unknown/missing keys, schema drift, false accepted-plan hash, duplicate/unknown/unsorted closed phases, invalid active phase, closed-active conflict, and dependency/order violations; assert exact state/governance rejection and preservation of all authority bytes.
19. `test_obligation_19_contract_authority_corruption_matrix` — Tamper `contract-draft.json` and `accepted-contract.json` with raw-key changes, revision gaps, false content hashes, stale phase/plan binding, draft/accepted disagreement, reordered or duplicate revisions, and archived-byte mismatch; assert contract, implementation, audit, closeout, and recovery consumers fail at the correct boundary.
20. `test_obligation_20_implementation_authority_corruption_matrix` — Tamper `implementation-authority.json` with schema/key changes, false plan/contract/revision/baseline bindings, unsafe or duplicate rule paths, inconsistent rule hashes, and stale repository identity; assert implementation/audit/closeout/recovery commands reject before target publication or Git mutation.
21. `test_obligation_21_audit_ledger_corruption_matrix` — Tamper `audit-ledger.json` with broken round ordering, duplicate or skipped repair attempts, false subject/report hashes, stale auditor/contract/implementation binding, terminal-state inconsistency, unknown fields, and archived report disagreement; assert audit, repair, closeout, and recovery consumers fail closed.
22. `test_obligation_22_completion_ledger_and_receipt_corruption_matrix` — Tamper `completion-ledger.json` with non-contiguous sequence, reordered phases, false manifest/receipt hashes, broken previous receipt, stale before/after state, archived authority byte mismatch, duplicate phase, and plan disagreement; assert closeout, continuity, recovery, and later phase selection reject without partial cleanup.
23. `test_obligation_23_continuity_ledger_corruption_matrix` — Tamper `continuity-ledger.json` with unknown/missing keys, immutable repository-ID drift, non-contiguous sequence, duplicate phase/continuity ID, false manifest/receipt or predecessor proof, broken chain, reordered entries, and exact metadata-byte disagreement; assert continuity and recovery behavior follows existing non-gating versus authoritative validation rules exactly.
24. `test_obligation_24_recovery_ledger_and_cross_chain_corruption_matrix` — Tamper `recovery-ledger.json` with raw-key/schema changes, false plan/prefix/action/receipt hashes, invalid `next_action`, noncanonical action targets, broken prior receipt, stale accepted-plan/Git/subject binding, and disagreement with completion/continuity authority; assert inspect/apply and relevant consumers reject before mutation with the exact recovery category.

### 16.4 Persistence, interruption, and fault-injection safety
25. `test_obligation_25_failure_before_temp_creation_preserves_absence` — For representative first publications in plan, contract, implementation, audit, closeout, continuity, and recovery, inject or construct a validation failure before temporary creation; assert no target, no command-created temporary file, unchanged prior authority, and exact error output.
26. `test_obligation_26_failure_after_temp_creation_disposes_safely` — Use existing deterministic hooks or a controlled write/replacement boundary to reach a state after a command-created temporary file exists but before publication; assert handled failure removes only its own temporary file, never truncates a collision sentinel, and preserves every durable target byte.
27. `test_obligation_27_failure_before_atomic_replace_preserves_target` — Create a pre-existing valid target and deterministically force the final replace step to fail before replacement for each available ledger-publication path; assert old bytes and hash remain exact, no fallback truncate/write occurs, and the command returns the existing persistence/filesystem category.
28. `test_obligation_28_target_replaced_before_journal_advance_resumes` — Construct or use the existing recovery synchronization hook for the exact state in which an action target already equals the next prefix but `next_action` was not advanced; assert retry recognizes the completed action, advances once, produces one receipt, and performs no duplicate target mutation.
29. `test_obligation_29_interrupted_closeout_cleanup_resumes_exactly` — Construct each deterministic Phase 6 incomplete-closeout cleanup prefix using archived exact bytes and final receipt state; assert recovery resumes the fixed cleanup order, promotes or reconstructs state as required, publishes no second completion entry, and rejects any byte mismatch before deletion.
30. `test_obligation_30_interrupted_recovery_action_and_ledger_publish` — Exercise existing recovery failpoints at journal publication, before action, after action, before advancement, and finalization; assert prefix validation, resumability, receipt uniqueness, prior-byte preservation on failed replacement, and contractually valid recovery-owned temporary disposition.
31. `test_obligation_31_interrupted_audit_continuity_and_completion_publication` — Use deterministic collision/replacement failures or preconstructed durable states for audit, continuity, and completion ledgers; assert no partial JSON authority is accepted, previous ledger bytes remain exact, sequences do not skip, and a clean retry publishes exactly one canonical entry.
32. `test_obligation_32_incomplete_durable_operation_replay_fixed_point` — For one interrupted operation from implementation publication, closeout, continuity, and recovery, perform the exact retry twice after completion; assert the first retry completes or returns the canonical result, the second is byte-preserving idempotent replay, and no temporary or duplicate authority remains.

### 16.5 Idempotency, replay, conflict, and concurrency behavior
33. `test_obligation_33_exact_replay_matrix_all_publishers` — For plan acceptance, contract draft/accept/revise where applicable, implementation begin, audit begin/record, repair check, phase close, continuity record, and recovery apply, repeat an exact successful request and assert exact canonical output plus byte-identical durable authority.
34. `test_obligation_34_conflicting_replay_matrix_all_publishers` — After each representative publication, change one semantically binding input while preserving superficial validity and replay; assert deterministic conflict/stale rejection, no new sequence, no overwritten bytes, and no success stdout.
35. `test_obligation_35_stale_authorization_and_compare_and_swap` — Exercise stale revision/SHA, stale implementation baseline, stale audit subject, stale closeout phase, stale continuity completion receipt, and stale recovery ID/subject; assert every stale request is rejected before publication and the accepted authority remains exact.
36. `test_obligation_36_concurrent_first_publication_eight_callers` — Launch exactly 8 synchronized callers against one eligible first-publication operation using an existing pre-publish barrier; assert at most one canonical creation, all other results are exact idempotent success or conflict as specified, and the final file is valid with no temporary leftovers.
37. `test_obligation_37_concurrent_duplicate_publication_eight_callers` — Launch exactly 8 synchronized identical callers against an already-published or simultaneously publishing operation; assert every success output is byte-identical, one durable semantic entry exists, sequence remains contiguous, and no caller rewrites accepted bytes.
38. `test_obligation_38_concurrent_conflicting_publication_eight_callers` — Launch exactly 8 synchronized callers split between two valid but conflicting payloads for the same authority slot; assert exactly one canonical payload wins, the other payload never appears in durable bytes, and losers fail with the existing conflict/stale category.
39. `test_obligation_39_journal_advance_and_caller_observation_races` — Exercise both recovery states: action complete before journal advancement and journal finalized before the original caller observes success; assert retry/fixed-point behavior returns one receipt, one action history, one final subject, and no replay misclassification.
40. `test_obligation_40_replay_and_concurrency_cross_repository_isolation` — Run synchronized continuity proof against two source repositories and simultaneous unrelated mutations in a third sentinel repository; assert target publication resolves exactly the supplied proofs, source repositories remain read-only, and unrelated repository bytes and Git state remain unchanged.

### 16.6 Privacy, process, network, environment, and output security
41. `test_obligation_41_network_and_shell_nonuse` — Run representative Phase 1–8 commands under a test-controlled environment that would record network-helper or shell invocation; assert no network endpoint is contacted, no shell is invoked, and only the expected MRGS process plus governed Git children execute.
42. `test_obligation_42_git_child_process_sanitization` — Install a recording Git wrapper and inject Git control variables, lazy-fetch/promisor settings, alternate object directories, hooks path, pager/editor variables, and credential helpers; assert every required Git child receives the existing sanitized controls, no lazy object fetch occurs, and no unauthorized executable runs.
43. `test_obligation_43_environment_secret_nonobservation` — Set distinctive sentinel values in credential-, token-, username-, host-, CI-, provider-, and proxy-like environment variables; run representative success and failure commands and assert no sentinel appears in governance bytes, stdout, stderr, source archives, receipts, or manifests.
44. `test_obligation_44_path_and_identity_privacy` — Use target and source repositories whose absolute paths and parent usernames contain unique sentinels; assert success output and durable records persist only the path forms explicitly authorized by Phase 1–8 and never persist canonical roots, user-profile prefixes, remote URLs, or automatically discovered host identity.
45. `test_obligation_45_source_content_and_error_redaction` — Place unique secret sentinels in plan, contract, audit-report, continuity-metadata, malformed authority, and external source files; assert success output never echoes source contents and failure stderr contains only the existing category/format without secret or environment leakage.
46. `test_obligation_46_git_nonmutation_all_commands` — Snapshot `HEAD`, branch, index bytes, refs, configuration, hooks, remotes, and tracked worktree before representative success and failure paths for every command family; assert MRGS performs no add, commit, checkout, reset, clean, merge, rebase, tag, config mutation, or remote write.
47. `test_obligation_47_repository_and_external_write_confinement` — Place sentinel trees before, beside, and outside the target repository plus read-only source repositories; trace resulting filesystem changes structurally and assert writes are confined to the target `.mrgs` files and expressly supplied source/destination files permitted by the existing command.
48. `test_obligation_48_output_contract_regression_and_secret_safety` — Exercise every success token and every new Phase 4–8 error family plus representative Phase 1–3 errors; assert exact stdout/stderr framing, no path/secret leakage, no mixed success and error output, and no changed token, field order, or casing.

### 16.7 Deterministic resource-bound robustness
49. `test_obligation_49_large_plan_and_phase_selection_fixture` — Accept the exact 128-phase linear-chain plan fixture, select valid boundary phases, reject an unmet dependency deterministically, and assert stable accepted-plan/state bytes, no recursion failure, no duplicate phase, and byte-identical exact replay.
50. `test_obligation_50_large_contract_and_audit_fixture` — Use the exact contract fixture of 256 requirements and 64 entries in each remaining list; draft/accept/begin implementation, create reports with exact one-to-one result counts, exercise the existing 3-round/2-repair maximum, and assert deterministic validation without panic or partial ledger publication.
51. `test_obligation_51_long_completion_history_fixture` — Build exactly 32 valid contiguous phase completions through public behavior or byte-exact validated fixtures, then exercise phase selection, closeout replay, continuity binding, and recovery inspection; assert contiguous ordering, exact chain validation, deterministic output, and no duplicate completion.
52. `test_obligation_52_long_continuity_and_cross_link_fixture` — Validate exactly 32 continuity entries with 16 resolved predecessor links in the final entry; assert sorted unique links, exact archived proof, contiguous receipt chain, byte-identical replay without source availability, and deterministic rejection of one altered link.
53. `test_obligation_53_long_recovery_history_and_pending_fixture` — Validate exactly 32 applied recovery entries plus one separately constructed pending entry whose action list is the deterministic fixture maximum; assert complete chain validation, bounded resume, exact sequence, fixed-point replay, and no stack growth or duplicate action.
54. `test_obligation_54_large_inventory_and_temp_candidate_fixture` — Use exactly 256 ordinary tracked files plus all valid governance files, then occupy exactly 15 recognized create-new candidate names so the sixteenth is selected; assert sorted deterministic inventory, bounded collision search, sentinel preservation, and no mutation amplification.
55. `test_obligation_55_scalar_boundaries_and_one_over_limits` — For every existing bounded scalar exercised by plan, contract, audit, closeout, continuity, and recovery, test one exact-maximum value and one one-over value; assert exact acceptance/rejection with no truncation, normalization, panic, or partial publication.
56. `test_obligation_56_repeated_replay_inspection_and_bounded_callers` — Execute exactly 64 exact replays, 64 read-only recovery inspections, and 8 synchronized callers on bounded fixtures; assert byte-identical deterministic outputs, stable file counts and sizes, no duplicate ledger entry, no deadlock, and no residual temporary file.

### 16.8 Phase 1–8 regression and cross-platform compatibility
57. `test_obligation_57_phase1_plan_and_selection_regression` — Exercise Phase 1 acceptance, exact replay, conflict, dependency ordering, state replacement, and representative filesystem failure through the public CLI; assert existing output, error category, files, and Git boundary remain unchanged.
58. `test_obligation_58_phase2_contract_draft_regression` — Exercise Phase 2 strict contract parsing, exact byte/hash preservation, first draft, exact replay, conflict, path rules, and failure cleanup; assert no behavior or output drift.
59. `test_obligation_59_phase3_acceptance_and_revision_regression` — Exercise exact `ACCEPTED`, revision compare-and-swap, revision-draft lifecycle, stale and casing rejection, history ordering, and idempotent acceptance; assert existing authority bytes and outputs remain unchanged.
60. `test_obligation_60_phase4_implementation_enforcement_regression` — Exercise implementation begin/check over allowed, forbidden, symlink, index, HEAD, worktree, and Git-sanitization boundaries including one platform capability branch; assert existing path enforcement and error categories remain unchanged.
61. `test_obligation_61_phase5_audit_and_repair_regression` — Exercise audit begin/record, PASS, bounded FAIL/repair routing, terminal failure, exact replay, collision, and subject drift; assert existing maximum attempts, ledger chain, outputs, and failure preservation.
62. `test_obligation_62_phase6_closeout_regression` — Exercise closeout readiness, exact manifest/archive/receipt hashes, cleanup order, state transition, chained second completion, replay, collision, and interrupted-closeout classification; assert existing output and durable format remain exact.
63. `test_obligation_63_phase7_continuity_and_phase8_recovery_regression` — Exercise continuity first publication/cross-link/replay/privacy plus recovery healthy/recoverable/unrecoverable/pending/apply/replay paths, including a platform capability/fallback branch; assert exact Phase 7–8 outputs, chains, and non-gating behavior.
64. `test_obligation_64_complete_public_cli_lifecycle_and_test_discipline` — Execute one complete public lifecycle from plan acceptance through phase selection, contract draft/accept, implementation begin/check, audit PASS, closeout, continuity record, induced recoverable state, recovery inspect/apply, and final healthy inspection; assert every intermediate authority and output, then assert the Phase 9 binary contains exactly 64 discoverable primary obligations, no ignored test, no recursive Cargo invocation, and no dependency/configuration change.

Every numbered obligation requires a direct executable assertion against production behavior. Silent platform skips, ignored tests, source-presence-only checks, helper-only checks where public command routing is required, aggregate-only assertions, and recursive full-suite invocation are weak evidence.

## 17. Verification ladder

Before the final ladder, independently discover and count the primary tests:

```powershell
$Primary = Select-String `
    -Path tests/phase9.rs `
    -Pattern '^fn test_obligation_[0-9]{2}_[a-z0-9_]+\(\)'

if ($Primary.Count -ne 64) {
    throw "Expected 64 primary Phase 9 obligations; found $($Primary.Count)."
}
```

Run the final ladder directly and inspect each real exit code:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo test --test phase9
cargo clippy --all-targets -- -D warnings
cargo test
git diff --check
```

The handoff must distinguish:

```text
PHASE9_OBLIGATION_TESTS
PHASE9_SUPPLEMENTAL_TESTS
FULL_SUITE_TESTS
```

Do not substitute the full-suite aggregate for the Phase 9 count. Do not invoke the full suite from inside `tests/phase9.rs`.

A timeout, killed process, truncated output, skipped command, masked exit code, or unexecuted capability branch is not a pass.

## 18. Allowed implementation paths

The only path that may change during initial Phase 9 implementation is:

```text
tests/phase9.rs
```

Create only:

```text
tests/phase9.rs
```

The supplied Phase 9 contract is frozen and must remain byte-identical after implementation begins.

## 19. Forbidden changes

Do not modify:

```text
AGENTS.md
README.md
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
docs/contracts/phase-08-contract.md
docs/contracts/phase-09-contract.md
src/**
tests/integration.rs
tests/phase4_obligations.json
tests/phase4_obligations.rs
tests/phase5.rs
tests/phase6.rs
tests/phase7.rs
tests/phase8.rs
.git/**
.mrgs/**
graphify-out/**
target/**
```

The `.mrgs/**` prohibition applies to repository implementation changes, not runtime files created inside isolated temporary test repositories.

Do not weaken, delete, rename, ignore, replace, or mark an existing test skipped.

Do not add dependencies, features, build scripts, examples, benchmarks, fuzz targets, generated registries, external fixtures, hidden configuration, telemetry, network clients, or new test executables.

Do not run Graphify during Phase 9 implementation or verification. Graphify is neither required evidence nor authorized workspace mutation for this phase.

## 20. Source-defect escalation rule

If a direct, deterministic, contract-valid Phase 9 test exposes a production defect:

- preserve the smallest valid failing test in `tests/phase9.rs`;
- confirm the failure with the narrowest exact test invocation;
- do not weaken, delete, ignore, condition away, or rewrite the test to match defective behavior;
- do not edit production source under contract version 1;
- stop the implementation mission;
- report `SOURCE_CHANGE_REQUIRED=YES`;
- identify the exact production path or smallest plausible path set;
- report the expected behavior, actual behavior, error/output, mutation evidence, and reproduction command;
- require a human-reviewed Phase 9 contract revision that authorizes only the exact repair surface and affected regression set.

A missing test hook is not by itself permission to edit source. First use an existing public behavior, deterministic fixture, existing hook, or preconstructed durable state. If direct proof remains impossible, report the precise evidence gap as a blocker rather than inventing test-only production authority.

## 21. Final evidence

The final implementation handoff must include at least:

```text
VERDICT
PHASE
CONTRACT_VERSION
BASELINE_HEAD
FINAL_HEAD
CONTRACT_SHA256
PHASE9_OBLIGATION_COUNT
PHASE9_OBLIGATION_TESTS
PHASE9_SUPPLEMENTAL_TESTS
VALIDATION_FAMILY_COUNTS
MISSING_OBLIGATIONS
WEAK_OBLIGATIONS
CAPABILITY_EXECUTED_COUNT
CAPABILITY_FALLBACK_COUNT
SOURCE_CHANGE_REQUIRED
SOURCE_DEFECT_EVIDENCE
CHANGED_PATHS
STAGED_PATHS
FORBIDDEN_PATH_RESULT
FMT_RESULT
CHECK_RESULT
PHASE9_TEST_RESULT
CLIPPY_RESULT
FULL_SUITE_RESULT
DIFF_CHECK_RESULT
BLOCKERS
RECOMMENDATION
COMMIT_PERFORMED
PUSH_PERFORMED
```

The required successful values include:

```text
PHASE=9
CONTRACT_VERSION=1
PHASE9_OBLIGATION_COUNT=64
VALIDATION_FAMILY_COUNTS=8,8,8,8,8,8,8,8
MISSING_OBLIGATIONS=NONE
WEAK_OBLIGATIONS=NONE
SOURCE_CHANGE_REQUIRED=NO
CHANGED_PATHS=tests/phase9.rs
STAGED_PATHS=NONE
FORBIDDEN_PATH_RESULT=PASS
COMMIT_PERFORMED=NO
PUSH_PERFORMED=NO
```

Report exact per-binary full-suite counts, not only an aggregate. Report any killed or timed-out command as failure unless the same unmodified source is rerun successfully under an adequate external harness cap and the final handoff clearly distinguishes the failed invocation from the authoritative successful invocation.

## 22. Completion rule

Phase 9 is complete only when:

- all 64 numbered obligations have direct, non-vacuous executable evidence;
- every family count is exactly eight;
- no obligation is missing or weak;
- every Phase 9 primary and supplemental test passes;
- the complete Phase 1–8 suite passes without weakening an earlier test;
- formatting, check, clippy, and diff-check pass;
- every platform-sensitive obligation executes the real capability or an explicit fallback assertion;
- only `tests/phase9.rs` changed;
- the frozen contract bytes remain unchanged;
- no valid test exposes an unresolved production defect;
- no file is staged;
- no commit or push occurred without separate human authorization;
- a final read-only obligation audit returns `64/64`, zero missing, and zero weak.

Phase 9 authorizes adversarial, security, resource, and regression validation only. It does not authorize Phase 10 activation readiness, deployment, source repair without contract revision, Git integration, commit, or push.
