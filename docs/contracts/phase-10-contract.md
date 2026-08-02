# Phase 10 Contract — Activation, Rollback Drills, and Adoption Readiness

CONTRACT_VERSION=1
STATUS=DRAFT
HUMAN_ACCEPTANCE_REQUIRED=YES
PHASE=10

Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

The `STATUS=DRAFT` marker describes this file before human decision. Human acceptance binds the exact audited SHA-256 without editing the file; later handoff field `CONTRACT_STATUS=ACCEPTED` records that external decision and does not authorize a status-line mutation.

## 1. Objective

Prove that the completed Phase 1–9 MRGS implementation can be introduced into an isolated pilot repository, operated through one complete public governance lifecycle, rolled back to an exact pre-activation state by an operator-controlled external procedure, and handed to an adopting operator with accurate, executable, privacy-minimal instructions and evidence.

Phase 10 is readiness validation and adoption documentation only. It does not deploy MRGS, activate MRGS in a real repository, introduce a rollback command, create production authority, modify a production source file, contact a network service, or authorize a Git commit or push.

The authoritative master plan provides only the Phase 10 title, “Activation, Rollback Drills, and Adoption Readiness.” This contract therefore adopts the narrowest non-expansive interpretation of that title:

- **activation** means an operator-controlled rehearsal in an isolated temporary pilot repository using the existing public CLI;
- **rollback drill** means an externally controlled restoration of a test repository and activation slot to their exact pre-activation bytes and Git state;
- **adoption readiness** means an accurate runbook, README adoption surface, reproducible evidence, and successful executable rehearsals;
- **activation approval** for any real repository remains a later, separate human decision.

## 2. Controlling authority

All Phase 1–9 authority remains controlling, including:

- accepted plan and phase-selection rules;
- exact contract draft, acceptance, revision, and lifecycle rules;
- contract-bound implementation enforcement;
- independent audit and bounded repair routing;
- closeout manifests and completion receipts;
- continuity metadata and cross-repository proof;
- recovery inspection, authorization, action, journal, and receipt rules;
- Phase 9 adversarial, concurrency, security, resource, and regression evidence;
- the accepted Phase 9 Revision 2 and Revision 3 production repairs.

Phase 10 may exercise those capabilities but may not replace, reinterpret, normalize, waive, repair, or add to their durable authority.

A Phase 10 PASS means only that the current MRGS candidate is ready for a separately approved pilot adoption. It does not itself authorize installation, rollout, migration, production use, repository activation, commit, push, release publication, or deletion of rollback material.

## 3. No new runtime surface

Preserve the existing public command surface exactly:

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

Do not add or change:

- a command, subcommand, flag, token, success line, error category, exit convention, or help grammar;
- a governance file, durable record, field, schema, schema version, receipt, or hash rule;
- a production source module, dependency, feature, build script, installer, service, daemon, network client, telemetry path, or configuration file;
- a runtime activation, rollback, migration, uninstall, or update mechanism.

## 4. Phase 10 implementation boundary

After separate human acceptance of this contract, Phase 10 implementation may change only:

```text
README.md
docs/phase-10-adoption-runbook.md
tests/phase10.rs
```

Required purpose by path:

- `README.md`: list all ten master-plan phases accurately, identify Phase 10 as readiness evidence rather than automatic deployment, and link to the adoption runbook;
- `docs/phase-10-adoption-runbook.md`: provide the complete operator-facing activation rehearsal, rollback drill, evidence checklist, limitations, and separate approval boundaries;
- `tests/phase10.rs`: contain the exact Phase 10 executable readiness obligations defined in Section 15.

The accepted `docs/contracts/phase-10-contract.md` must remain byte-identical after implementation begins.

No other file may change. In particular, no Phase 10 implementation authority exists for `src/**`, `Cargo.toml`, `Cargo.lock`, prior contracts, prior tests, Git metadata, generated output, or real `.mrgs` authority in this repository.

## 5. Readiness candidate identity

The implementation handoff must bind Phase 10 evidence to one exact candidate identity:

```text
BASELINE_HEAD
FINAL_HEAD
CONTRACT_SHA256
CANDIDATE_SOURCE_TREE_SHA256
RELEASE_BINARY_SHA256
RELEASE_BINARY_BYTE_SIZE
RUSTC_VERSION
CARGO_VERSION
TARGET_TRIPLE
```

Requirements:

1. `BASELINE_HEAD` is the repository HEAD before Phase 10 implementation.
2. `FINAL_HEAD` remains equal to `BASELINE_HEAD` until a separately authorized commit transaction; uncommitted Phase 10 docs/tests are reported as changed paths.
3. `CANDIDATE_SOURCE_TREE_SHA256` is computed from a canonical binary manifest. For every tracked path, and for each authorized untracked Phase 10 path, emit `<mode> NUL <byte_size> NUL <sha256> NUL <path> NUL` in ascending repository-relative UTF-8 path-byte order; use Git's recorded mode for tracked paths, `100644` for a new regular file, ASCII decimal byte size without leading zeroes, lowercase 64-hex content SHA-256, forward-slash paths, and no Unicode or case normalization. Hash the exact concatenated manifest bytes with SHA-256.
4. `CARGO_NET_OFFLINE=true cargo build --release --locked` produces the release candidate used for external release-smoke evidence.
5. The release candidate hash and size are recorded from the built file, not inferred from Cargo output.
6. `tests/phase10.rs` uses Cargo's test-provided `CARGO_BIN_EXE_mrgs` binary for in-test activation-slot obligations; it must not assume that `target/release/mrgs` already exists and must not invoke Cargo recursively. The final external harness separately repeats the slot identity/help smoke with the release candidate.
7. Build artifacts are evidence inputs only and remain ignored/uncommitted.
8. Host, model, execution surface, and tool-provider metadata are informational and never acceptance criteria.

## 6. Activation rehearsal

Activation rehearsal must occur only in isolated temporary directories created for the test or verification mission. The real MRGS source repository must never be used as a governed runtime target.

The rehearsal must:

1. create a clean Git pilot repository with explicit local Git identity and no remote;
2. record a complete pre-activation snapshot before any `.mrgs` object exists;
3. use the compiled public `mrgs` binary, not helper-only production calls;
4. accept a strict plan, select a phase, draft and accept a contract, bind implementation, perform implementation check, begin and record an independent PASS audit, close the phase, record continuity, and obtain a healthy recovery inspection;
5. assert exact success framing and validate the complete durable authority chain after each boundary;
6. preserve the pilot repository’s intended source bytes and Git identity;
7. create no network, shell, service, scheduler, credential, or host-discovery dependency;
8. leave no producer temporary file, pending recovery, partial ledger entry, or unclassified state;
9. emit an activation evidence manifest containing only deterministic identifiers, hashes, byte sizes, command results, and relative paths;
10. avoid absolute pilot paths, usernames, hostnames, environment secrets, access tokens, and source-file contents in the adoption evidence.

Phase 10 activation evidence is a rehearsal result. It must be labelled `ACTIVATION_REHEARSAL`, never `PRODUCTION_ACTIVATED` or equivalent.

## 7. Activation slot rehearsal

The runbook and tests must use a temporary activation slot to model operator-controlled binary selection without introducing an installer. In-test slot obligations use `CARGO_BIN_EXE_mrgs`; the final external verification repeats the slot smoke with the offline-built release candidate and records both results separately.

The slot contains:

```text
active/
backup/
evidence/
```

The drill must:

- place the candidate release binary in `active/` using copy-to-temporary plus atomic replacement where the host supports it;
- record the candidate binary hash before and after placement;
- execute `mrgs --help` and one public read/write rehearsal command through the active-slot path;
- preserve a byte-identical backup of the pre-activation slot state, which may be an explicitly recorded absent state;
- never alter `PATH`, a system directory, registry, package manager, service manager, shell profile, user profile, or global configuration;
- remove all slot content at fixture cleanup.

An activation-slot copy is not a release publication and must not be described as installation.

## 8. Rollback drills

Rollback is an operator-controlled external procedure. MRGS provides no rollback command, and Phase 10 must not imply otherwise.

Two executable drills are required.

### 8.1 Partial-activation rollback

From a pre-activation snapshot:

1. create accepted plan, state, active phase, contract draft, and accepted contract authority;
2. stop before implementation begins;
3. preserve the failed/partial activation evidence separately;
4. restore the exact pre-activation repository bytes and Git state;
5. prove `.mrgs` is absent if it was absent in the snapshot;
6. prove source, index, `HEAD`, branch, configuration, hooks, refs, tracked files, untracked files, and file kinds equal the pre-activation snapshot;
7. repeat the restore once and prove fixed-point idempotency.

### 8.2 Completed-rehearsal rollback

After the complete activation rehearsal from Section 6:

1. preserve the completed activation evidence outside the pilot repository;
2. restore the pilot repository to its exact pre-activation snapshot;
3. restore the activation slot to its exact pre-activation state;
4. prove the restored repository and slot hashes are identical to the pre-activation hashes;
5. rerun the snapshot comparison and obtain the same result;
6. prove the evidence copy remains readable and unchanged;
7. prove rollback does not mutate the MRGS source repository or any unrelated sentinel repository.

Rollback must use a fresh restore destination followed by replacement or an equivalently safe external restore sequence. It must not delete the only pre-activation snapshot before post-restore validation succeeds.

## 9. Snapshot and restore evidence

The test-only snapshot model must cover, as applicable:

- repository-relative path;
- object kind: regular file, directory, symlink, or supported reparse-point classification;
- regular-file SHA-256 and byte size;
- symlink target bytes without traversal;
- Git `HEAD`, branch, refs, index, configuration, hooks, worktree porcelain, and untracked inventory;
- activation-slot inventory and binary hashes;
- absence as an explicit state.

Snapshots must be deterministically ordered and must not persist absolute paths or file contents in the final evidence.

The snapshot helper is an independent test instrument, not a replacement for MRGS authority validation. It may compare observed bytes and Git state but may not duplicate MRGS plan, contract, audit, closeout, continuity, or recovery algorithms.

## 10. Adoption runbook requirements

`docs/phase-10-adoption-runbook.md` must be executable by an operator without undocumented repository knowledge. It must include:

1. scope and explicit non-goals;
2. supported prerequisites and how to record versions;
3. candidate commit, contract, source-tree, and binary identity checks;
4. release build and local active-slot rehearsal steps;
5. pilot repository prerequisites;
6. pre-activation backup and evidence-location rules;
7. the exact public CLI sequence for the activation rehearsal;
8. expected success output shapes and stop conditions;
9. partial-activation and completed-rehearsal rollback drills;
10. post-rollback equality checks;
11. privacy, secret-handling, path, network, and Git boundaries;
12. evidence retention and disposal rules;
13. known limitations, including that MRGS supplies no installer, updater, rollback command, service, remote control, or production deployment automation;
14. separate human approvals for real activation, commit, push, release publication, and rollback execution;
15. a concise PASS/FAIL checklist with no ambiguous “mostly ready” outcome.

Commands in the runbook must match the live CLI exactly. Placeholders must be explicit and must not be presented as literal values. The runbook may use PowerShell and POSIX-shell examples only when both express the same governed sequence and limitations.

## 11. README adoption surface

The Phase 10 README update must:

- list Phases 1 through 10 in master-plan order;
- use `docs/master-plan.md` titles exactly and use accepted contracts only to explain scope without renaming a master-plan phase;
- link to `docs/phase-10-adoption-runbook.md`;
- state that Phase 10 produces readiness evidence only;
- state that real activation and Git/release actions require separate human authorization;
- avoid claiming installation support, production deployment, automatic rollback, certification, compliance, security accreditation, or universal platform support.

## 12. Privacy, process, network, and external-effects boundary

Phase 10 implementation and tests must not:

- contact a network endpoint or require remote Git access;
- invoke a shell from MRGS production behavior;
- write credentials, environment secrets, usernames, hostnames, home paths, or access tokens into repository or adoption evidence;
- write outside isolated temporary fixtures except the explicitly supplied external evidence directory;
- modify the real source repository’s Git state;
- modify an unrelated sentinel repository;
- persist absolute pilot, source, or evidence paths in the final readiness manifest;
- install software, edit system/user configuration, register a service, modify `PATH`, or use administrator/root privileges as a correctness requirement.

Test harnesses may invoke the compiled MRGS binary, Git, and operating-system filesystem APIs required to create and compare isolated fixtures. No recursive Cargo invocation is allowed from `tests/phase10.rs`.

## 13. Cross-platform evidence

Platform identity is evidence metadata, not an acceptance criterion.

The final Phase 10 evidence must include:

- a complete primary Phase 10 test-binary run on the primary development platform;
- `cargo check --bin mrgs --test phase10` on Linux;
- one exact Linux activation-and-rollback obligation run;
- real Windows exclusive-file, rename, or reparse capability where required by a Windows-only branch, or an explicit assertion that the branch is not compiled on the current platform;
- real POSIX permission, symlink, and rename behavior where required by a Unix-only branch, or an explicit assertion that the branch is not compiled on the current platform.

Silent platform skips are forbidden. Capability branches must emit test evidence identifying `CAPABILITY_EXECUTED` or `CAPABILITY_NOT_COMPILED_FOR_TARGET`.

A pre-existing failure in an unrelated earlier test target must be reported separately from the changed-surface compile result and may not be relabelled PASS.

## 14. Determinism and evidence discipline

Every Phase 10 obligation must:

- use fixed inputs or values derived from fixed fixture bytes;
- avoid sleeps, random backoff, wall-clock ordering, network availability, host identity, and model output as proof;
- assert exact exit code and success/error framing where invoking MRGS;
- assert repository and external-effects boundaries;
- treat timeout, killed process, truncated output, missing output, or masked exit code as failure or unverified evidence;
- preserve failed evidence before fixture cleanup;
- use one writer per source file;
- edit and verify one obligation at a time;
- run only the smallest affected test until the final ladder.

A documentation substring check is insufficient by itself. Runbook obligations must parse the documented command blocks and compare them to the live Clap command surface or execute the documented governed sequence with placeholders resolved in an isolated fixture.

## 15. Required tests

Create exactly twelve primary tests in `tests/phase10.rs`, divided into three families of four. Test names are contractual.

### 15.1 Activation readiness

1. `test_obligation_01_clean_room_activation_rehearsal` — Execute the complete Section 6 public lifecycle in an isolated Git repository; assert exact outputs, complete authority validation, healthy final recovery inspection, no temporary files, and no unintended Git mutation.
2. `test_obligation_02_activation_slot_binary_identity_and_smoke` — Exercise the temporary activation slot, verify candidate and active binary hashes, execute help plus a public command through the slot path, and prove no system/user installation side effect.
3. `test_obligation_03_activation_preconditions_and_fail_closed_abort` — Exercise missing backup, dirty pilot precondition, mismatched candidate identity, malformed plan/contract, and stale accepted authority; distinguish runbook/harness readiness preconditions from MRGS command validation, assert the correct layer stops before its next boundary, and preserve the precondition snapshot. Do not require an MRGS command to reject a condition that only the adoption procedure governs.
4. `test_obligation_04_activation_evidence_privacy_and_determinism` — Generate the readiness evidence twice from equivalent fixtures; assert semantic and byte identity, relative-path-only records, stable ordering, and absence of sentinel secrets, usernames, hostnames, and source contents.

### 15.2 Rollback readiness

5. `test_obligation_05_partial_activation_rollback_exact_restore` — Execute the Section 8.1 partial activation and restore; assert exact repository/Git equality, `.mrgs` absence, preserved external evidence, and fixed-point second restore.
6. `test_obligation_06_completed_rehearsal_rollback_exact_restore` — Complete the lifecycle and active-slot rehearsal, then execute Section 8.2; assert exact repository and slot restoration, preserved evidence, and no mutation of source or sentinel repositories.
7. `test_obligation_07_rollback_snapshot_integrity_and_stale_rejection` — Corrupt, truncate, cross-bind, and stale-bind independent snapshot metadata or content; assert the test-side drill refuses replacement before deleting the current pilot, preserves both current and backup states, and reports the exact failed precondition.
8. `test_obligation_08_interrupted_restore_resumption_and_cleanup` — Preconstruct interruption before replacement and after replacement but before evidence finalization; assert deterministic resumption, no duplicate restore, no lost sole backup, no residual temporary path, and fixed-point completion.

### 15.3 Adoption readiness

9. `test_obligation_09_runbook_cli_surface_and_sequence` — Parse the runbook command blocks, compare every MRGS command and flag with the live CLI surface, resolve placeholders in an isolated fixture, and execute the documented activation sequence successfully.
10. `test_obligation_10_runbook_rollback_checklist_and_boundaries` — Verify the runbook contains executable pre-backup, stop, restore, post-restore, evidence-retention, privacy, no-network, no-installation, and separate-approval steps; execute the documented rollback sequence in a fixture.
11. `test_obligation_11_readme_master_plan_and_claim_accuracy` — Assert README lists exactly ten phases in order, links the runbook, distinguishes rehearsal from real activation, and contains none of the forbidden unsupported claims from Section 11.
12. `test_obligation_12_two_repository_adoption_rehearsal_and_final_manifest` — Run the documented rehearsal and rollback against two independent pilot repositories with distinct plans and paths; assert no cross-repository authority, identity, temporary-file, evidence, or rollback leakage, then emit and validate the final deterministic Phase 10 readiness manifest.

No primary test may be ignored, dynamically undiscoverable, or replaced by a helper-only assertion. Supplemental tests are permitted only for a direct defect or evidence gap and must be separately counted.

## 16. Release and verification ladder

Before the final ladder, independently discover the exact twelve primary tests:

```powershell
$Primary = Select-String `
    -Path tests/phase10.rs `
    -Pattern '^fn test_obligation_[0-9]{2}_[a-z0-9_]+\(\)'

if ($Primary.Count -ne 12) {
    throw "Expected 12 primary Phase 10 obligations; found $($Primary.Count)."
}
```

Run, once on final bytes, with Cargo network access disabled:

```text
CARGO_NET_OFFLINE=true cargo fmt --all -- --check
CARGO_NET_OFFLINE=true cargo check --all-targets
CARGO_NET_OFFLINE=true cargo test --test phase10 -- --test-threads=1
CARGO_NET_OFFLINE=true cargo clippy --all-targets -- -D warnings
CARGO_NET_OFFLINE=true cargo test
CARGO_NET_OFFLINE=true cargo build --release --locked
target/release/mrgs --help
git diff --check
```

Use the platform-equivalent environment syntax and release binary path on Windows. The external harness must additionally copy the release binary through the Section 7 temporary slot, verify source/slot hashes, execute `--help` and one documented public rehearsal command through that slot, and remove the slot at cleanup. Record this separately from the in-test slot result.

Also run under Linux with a Linux-private target directory:

```text
CARGO_NET_OFFLINE=true cargo check --bin mrgs --test phase10
CARGO_NET_OFFLINE=true cargo test --test phase10 test_obligation_12_two_repository_adoption_rehearsal_and_final_manifest -- --exact --nocapture --test-threads=1
```

Do not recursively invoke Cargo from `tests/phase10.rs`. Do not rerun already-green Phase 1–9 individual tests unless a Phase 10 authorized change can plausibly invalidate them. Because Phase 10 changes no production source, the final complete suite is the only required Phase 1–9 regression execution.

A timed-out aggregate suite may be replaced only by complete per-binary evidence on unchanged final bytes, with the failed aggregate invocation and the authoritative per-binary results both disclosed.

## 17. Allowed dependencies

No new dependency is authorized.

Use only:

- the Rust standard library;
- dependencies already present in `Cargo.toml` and `Cargo.lock`;
- the existing compiled `mrgs` test binary interface;
- local Git;
- operating-system filesystem APIs;
- PowerShell or POSIX shell only as external verification harnesses, not MRGS runtime dependencies.

## 18. Forbidden paths and operations

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
docs/contracts/phase-08-contract.md
docs/contracts/phase-09-contract.md
docs/contracts/phase-10-contract.md
src/**
tests/integration.rs
tests/phase4_obligations.json
tests/phase4_obligations.rs
tests/phase5.rs
tests/phase6.rs
tests/phase7.rs
tests/phase8.rs
tests/phase9.rs
.git/**
.mrgs/**
graphify-out/**
target/**
```

The only implementation changes are the three paths in Section 4. The `.mrgs/**` prohibition applies to this source repository, not isolated temporary pilot repositories.

Do not:

- weaken, delete, rename, ignore, or condition away an earlier test;
- change a production source file to satisfy a readiness test;
- create an installer, release archive, checksum registry, lock file, state migrator, updater, uninstaller, deployment script, service unit, container image, package manifest, or CI workflow;
- modify a real repository for activation evidence;
- contact a network, fetch a dependency, or require a remote;
- stage, commit, push, tag, publish, or create a release without separate human authorization.

Graphify reconnaissance is permitted before drafting or implementation when available. Generated Graphify output is non-gating and must remain ignored. Graphify is not required completion evidence.

## 19. Source-defect and contract-gap escalation

If a direct, deterministic, contract-valid Phase 10 test exposes a production defect:

- preserve the smallest failing test in `tests/phase10.rs`;
- confirm it with one exact invocation;
- do not weaken the test or edit production source;
- stop and report `SOURCE_CHANGE_REQUIRED=YES` with the smallest plausible repair surface;
- require a separately reviewed Phase 10 contract revision before any source edit.

If the master-plan title and controlling Phase 1–9 contracts do not support a proposed activation, rollback, or adoption behavior:

- do not invent runtime authority;
- record `CONTRACT_GAP=YES`;
- preserve the narrow readiness interpretation in Sections 1–3;
- require a master-plan or contract revision before expanding scope.

A test-harness defect or malformed fixture must be repaired only in `tests/phase10.rs` and must not be misclassified as a production defect.

## 20. Implementation handoff

The Phase 10 implementation handoff must include:

```text
VERDICT
PHASE
CONTRACT_VERSION
CONTRACT_STATUS
BASELINE_HEAD
FINAL_HEAD
CONTRACT_SHA256
CANDIDATE_SOURCE_TREE_SHA256
RELEASE_BINARY_SHA256
RELEASE_BINARY_BYTE_SIZE
RUSTC_VERSION
CARGO_VERSION
PRIMARY_PLATFORM
LINUX_TARGET
PHASE10_OBLIGATION_COUNT
PHASE10_OBLIGATION_RESULTS
PHASE10_SUPPLEMENTAL_RESULTS
ACTIVATION_REHEARSAL_RESULT
TEST_BINARY_ACTIVATION_SLOT_RESULT
RELEASE_BINARY_ACTIVATION_SLOT_RESULT
PARTIAL_ROLLBACK_RESULT
COMPLETED_ROLLBACK_RESULT
ROLLBACK_FIXED_POINT_RESULT
RUNBOOK_EXECUTION_RESULT
README_CLAIM_AUDIT
PRIVACY_RESULT
NETWORK_RESULT
EXTERNAL_MUTATION_RESULT
CAPABILITY_EXECUTED_COUNT
CAPABILITY_NOT_COMPILED_COUNT
SOURCE_CHANGE_REQUIRED
CONTRACT_GAP
CHANGED_PATHS
UNAUTHORIZED_CHANGED_PATHS
STAGED_PATHS
FMT_RESULT
CHECK_RESULT
PHASE10_TEST_RESULT
CLIPPY_RESULT
FULL_SUITE_RESULT
RELEASE_BUILD_RESULT
RELEASE_HELP_RESULT
LINUX_CHANGED_SURFACE_COMPILE
LINUX_REHEARSAL_RESULT
DIFF_CHECK_RESULT
BLOCKERS
RECOMMENDATION
COMMIT_PERFORMED
PUSH_PERFORMED
REAL_ACTIVATION_PERFORMED
```

Required successful values include:

```text
PHASE=10
CONTRACT_VERSION=1
CONTRACT_STATUS=ACCEPTED
PHASE10_OBLIGATION_COUNT=12
PHASE10_SUPPLEMENTAL_RESULTS=NONE_OR_PASS
ACTIVATION_REHEARSAL_RESULT=PASS
TEST_BINARY_ACTIVATION_SLOT_RESULT=PASS
RELEASE_BINARY_ACTIVATION_SLOT_RESULT=PASS
PARTIAL_ROLLBACK_RESULT=PASS
COMPLETED_ROLLBACK_RESULT=PASS
ROLLBACK_FIXED_POINT_RESULT=PASS
RUNBOOK_EXECUTION_RESULT=PASS
README_CLAIM_AUDIT=PASS
PRIVACY_RESULT=PASS
NETWORK_RESULT=PASS
EXTERNAL_MUTATION_RESULT=PASS
SOURCE_CHANGE_REQUIRED=NO
CONTRACT_GAP=NO
CHANGED_PATHS=README.md,docs/phase-10-adoption-runbook.md,tests/phase10.rs
UNAUTHORIZED_CHANGED_PATHS=NONE
STAGED_PATHS=NONE
COMMIT_PERFORMED=NO
PUSH_PERFORMED=NO
REAL_ACTIVATION_PERFORMED=NO
```

## 21. Independent audit handoff

After the implementation handoff reports PASS, a separate read-only audit must challenge the contract mapping, direct executable evidence, rollback realism, claim accuracy, path boundary, and final byte identities. The auditor must not edit repository files or substitute implementation assertions for observed evidence. Independence is procedural rather than model- or platform-based: the audit must run from a fresh read-only pass, inspect the final bytes and raw evidence directly, and make no repository mutation; model, host, or execution-surface identity is informational only.

The audit handoff must include:

```text
AUDIT_VERDICT
PHASE
CONTRACT_VERSION
CONTRACT_SHA256
AUDITOR_ID
INDEPENDENCE_DECLARATION
IMPLEMENTATION_HANDOFF_SHA256
CANDIDATE_SOURCE_TREE_SHA256
RELEASE_BINARY_SHA256
OBLIGATION_DISCOVERY_RESULT
OBLIGATION_DIRECT_EVIDENCE_RESULT
ACTIVATION_REHEARSAL_AUDIT
TEST_BINARY_SLOT_AUDIT
RELEASE_BINARY_SLOT_AUDIT
PARTIAL_ROLLBACK_AUDIT
COMPLETED_ROLLBACK_AUDIT
ROLLBACK_FIXED_POINT_AUDIT
RUNBOOK_EXECUTION_AUDIT
README_CLAIM_AUDIT
PRIVACY_AUDIT
NETWORK_AUDIT
EXTERNAL_MUTATION_AUDIT
CROSS_PLATFORM_AUDIT
VERIFICATION_LADDER_AUDIT
CHANGED_PATH_AUDIT
CONTRACT_BYTE_IDENTITY
UNRESOLVED_FINDINGS
RECOMMENDATION
REPOSITORY_FILES_EDITED_BY_AUDIT
COMMIT_PERFORMED
PUSH_PERFORMED
REAL_ACTIVATION_PERFORMED
```

A PASS requires zero unresolved findings, exact contract and candidate identities, direct evidence for all twelve obligations, and no repository mutation by the auditor. After the first PASS, repeat the audit once from the frozen final evidence and final bytes. Phase 10 closeout requires both `PRIMARY_AUDIT=PASS` and `FINAL_VERIFICATION_AUDIT=PASS`. If either audit fails, repair only the authorized evidence/test/document surface implicated by the finding, rerun the smallest affected verification, and repeat both audit stages until both pass.

## 22. Completion rule

Phase 10 is complete only when:

- the accepted contract remains byte-identical;
- exactly twelve primary obligations are discovered and all pass;
- every activation, rollback, and adoption family has exactly four passing obligations;
- the complete activation rehearsal reaches healthy recovery inspection;
- both the test-provided binary slot and the external release-binary slot are proven without installation side effects;
- both rollback drills restore exact pre-activation repository, Git, and slot state;
- rollback is fixed-point and retains the sole validated backup until restoration is proven;
- runbook commands match and execute against the live CLI;
- README accurately represents all ten phases and makes no unsupported claim;
- privacy, no-network, external-mutation, and relative-path evidence checks pass;
- primary-platform Phase 10 tests and the required Linux changed-surface/rehearsal evidence pass;
- formatting, check, clippy, release build, help smoke, complete suite, and diff-check pass;
- only the three authorized implementation paths changed;
- no production source, dependency, schema, CLI, or prior test changed;
- no unresolved production defect or contract gap remains;
- no file is staged and no commit, push, tag, release, or real activation occurred;
- the final audit returns PASS, followed by one independent verification audit that also returns PASS.

Phase 10 completion establishes adoption readiness evidence only. A separate human decision is required before committing, pushing, publishing, installing, or activating MRGS in any real repository.
