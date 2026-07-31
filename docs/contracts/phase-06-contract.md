# Phase 6 Contract — Closeout, Final Manifests, and Completion Receipts

Contract version: 1
Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

## 1. Objective

Extend the Phase 1–5 governance foundation with deterministic phase closeout, self-contained final manifests, chained completion receipts, resumable publication, and exact cleanup of phase-scoped governance authority.

Phase 6 implements only:

1. validation that the active phase has a terminal Phase 5 `PASS` bound to the current repository subject;
2. construction of one deterministic self-contained final manifest for that phase;
3. construction of one deterministic chained completion receipt;
4. durable registration of both objects in one append-only completion ledger;
5. exact archival of the phase-scoped governance-file bytes inside the final manifest;
6. resumable removal of the archived phase-scoped governance files;
7. transition of `state.json` from the active phase to the closed phase;
8. byte-preserving idempotent replay after successful or interrupted closeout;
9. preservation of all Phase 1–5 behavior and authority.

Phase 6 does not execute implementation work, execute verification commands from a contract, run an audit, route or apply repairs, infer correctness without a Phase 5 `PASS`, modify Git refs, stage, commit, push, merge, tag, use the network, invoke models, collect host or model metadata, recover arbitrary corruption, or automatically select the next phase.

A Phase 5 `PASS` is necessary but not sufficient for closeout. The passed audit subject must still equal the current subject at closeout time.

## 2. Controlling authority and lifecycle

All Phase 1–5 authority remains controlling:

- accepted plan and exact plan bytes;
- validated state and active phase;
- accepted contract ledger and exact final accepted revision;
- current contract draft and lifecycle consistency;
- implementation authority;
- Git object format, baseline branch, and baseline commit;
- current Phase 4 implementation boundary;
- complete Phase 5 audit ledger and terminal passed audit subject.

Phase 6 adds exactly one governance file:

```text
<repo>/.mrgs/completion-ledger.json
```

The closeout lifecycle is inferred from validated state, phase-scoped governance files, and the completion ledger:

- `OPEN`: the requested phase is the active phase and has no completion entry;
- `ARCHIVED_PENDING_FINALIZATION`: the completion ledger has the phase entry, state still names the phase as active, and zero or more archived phase-scoped governance files remain;
- `CLOSED`: the completion ledger has the phase entry, state has `active_phase: null`, `closed_phases` ends with the phase, and all archived phase-scoped governance files are absent;
- `CONFLICT`: the records cannot form one of the three legal states.

The lifecycle is not stored separately.

Closeout is durable once the completion entry has been atomically published. Cleanup and the state transition are deterministic finalization steps that may be resumed by replaying the same command.

## 3. CLI surface

Preserve every existing command and extend `phase` with exactly:

```text
mrgs phase close --repo <REPOSITORY_PATH> --phase <PHASE_ID>
```

The explicit phase ID is required so an already completed closeout can be replayed idempotently after `active_phase` has been cleared.

No other new command is authorized.

Phase 6 does not add `closeout begin`, `closeout finalize`, `manifest create`, `receipt accept`, `phase reopen`, `phase rollback`, or automatic next-phase selection.

## 4. Common validation order

`phase close` must fail closed and validate in this order before any first publication:

1. CLI token grammar;
2. canonical repository path;
3. safe `.mrgs` directory and fixed governance-file topology;
4. accepted plan record, exact plan source bytes, plan SHA-256, and plan structure;
5. state structure, dependencies, active phase, and closed-phase consistency;
6. requested phase existence and exact relation to state;
7. completion-ledger topology, parse, hashes, chain, ordering, and state relation when present;
8. idempotent-complete or resumable-closeout detection;
9. for a first closeout only: contract draft, accepted contract, implementation authority, Git context, current Phase 4 boundary, audit ledger, and terminal current-subject Phase 5 `PASS`;
10. deterministic final-manifest construction;
11. deterministic completion-receipt construction;
12. atomic completion-ledger publication;
13. exact resumable cleanup of phase-scoped governance files;
14. atomic state transition;
15. final cross-file validation;
16. output.

Before the first completion-ledger publication, any failure must preserve every governance byte exactly and leave no temporary file.

After the first completion-ledger publication, any failure must preserve that exact completion entry and leave the repository in a replayable `ARCHIVED_PENDING_FINALIZATION` state. A later replay must never regenerate different manifest or receipt bytes.

## 5. Phase ID validation

`--phase` must:

- be strict UTF-8;
- contain 1–128 bytes;
- equal its own trimmed form;
- contain no control character;
- exactly equal a phase ID in the accepted plan.

For a first closeout, it must exactly equal `state.active_phase`.

For idempotent replay of a completed closeout, it must exactly identify the final completion entry and the final element of `state.closed_phases`.

A different active or completed phase is a closeout conflict.

## 6. Phase-scoped governance files

The phase-scoped governance files are exactly:

```text
.mrgs/contract-draft.json
.mrgs/accepted-contract.json
.mrgs/implementation-authority.json
.mrgs/audit-ledger.json
```

All four are required for a first closeout.

They must be safe regular files reached through safe `.mrgs` topology. They must not be symlinks, junctions, directories, devices, or other unsafe reparse-point objects.

The exact bytes of all four files must be strict UTF-8 and are archived in the final manifest before any is removed.

No other governance file is archived or removed.

The accepted plan, plan source, `state.json`, and `completion-ledger.json` remain after closeout.

## 7. Closeout readiness

A first closeout is ready only when all of the following are true:

1. the requested phase is the active phase;
2. the phase is not already in `closed_phases`;
3. all dependencies of the phase are already closed;
4. the current draft and final accepted contract revision are exact and lifecycle-consistent;
5. implementation authority is exact and current;
6. the current branch equals the implementation baseline branch, the implementation baseline commit remains a valid ancestor, and current `HEAD` satisfies the Phase 4 authority relation;
7. the current Phase 4 implementation check succeeds;
8. the audit ledger is structurally and contextually valid;
9. the inferred Phase 5 lifecycle is `PASSED`;
10. the final audit round is `PASS` and has no repair route;
11. the final report bytes and SHA-256 revalidate;
12. every requirement and verification result in the final report is `PASS`;
13. the current recomputed audit subject exactly equals the passed round subject;
14. the current subject hash exactly equals the passed round subject hash;
15. no completion entry already exists for the phase.

A stale, pending, failed, routed, checked-but-not-reaudited, or final-failed audit is not closeout-ready.

Closeout does not rerun the contract's verification commands. It relies on the exact verification results already accepted by the terminal Phase 5 report and proves that the bound repository subject has not changed.

## 8. Final manifest

Each completion entry contains one `FinalManifest` object with exactly these fields in this order:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-sha256",
  "plan_id": "plan-id",
  "plan_source_path": "repo/relative/path",
  "plan_content": "exact accepted plan bytes",
  "phase_id": "phase-id",
  "phase_title": "phase-title",
  "phase_dependencies": [],
  "plan_phase_index": 0,
  "completion_sequence": 1,
  "contract_id": "contract-id",
  "contract_revision": 1,
  "contract_source_path": "repo/relative/path",
  "contract_sha256": "lowercase-sha256",
  "contract_content": "exact accepted contract bytes",
  "implementation_baseline_head": "git-object-id",
  "implementation_baseline_branch": "branch-name",
  "git_object_format": "sha1-or-sha256",
  "final_head": "git-object-id",
  "final_branch": "branch-name",
  "final_audit_id": "lowercase-sha256",
  "final_audit_round": 1,
  "final_auditor_id": "auditor-id",
  "final_subject_sha256": "lowercase-sha256",
  "final_subject": {},
  "final_report_source_path": "canonical/strict-utf8/path",
  "final_report_sha256": "lowercase-sha256",
  "final_report_content": "exact report bytes",
  "archived_governance": {}
}
```

All fields are required. Unknown fields are rejected.

`plan_content` is the exact accepted plan source content whose SHA-256 equals `accepted_plan_sha256`.

`phase_dependencies` is copied from the accepted plan in declared order.

`plan_phase_index` is the zero-based index of the phase in the accepted plan.

`completion_sequence` starts at `1` and increments by exactly one for each completion entry.

`contract_content` is the exact accepted contract content preserved by accepted authority.

`final_subject` is the exact passed Phase 5 audit subject.

`final_report_content` is the exact passed report content preserved in the audit ledger.

`final_head` and `final_branch` must equal the corresponding fields in `final_subject` and the current validated Git context.

## 9. Archived governance object

`archived_governance` contains exactly these fields in this order:

```json
{
  "contract_draft_sha256": "lowercase-sha256",
  "contract_draft_content": "exact file bytes",
  "accepted_contract_sha256": "lowercase-sha256",
  "accepted_contract_content": "exact file bytes",
  "implementation_authority_sha256": "lowercase-sha256",
  "implementation_authority_content": "exact file bytes",
  "audit_ledger_sha256": "lowercase-sha256",
  "audit_ledger_content": "exact file bytes"
}
```

Each SHA-256 is computed over the exact UTF-8 bytes stored in the corresponding content field and must also equal the SHA-256 of the on-disk file at first publication.

The archived bytes must parse and revalidate as the exact records used to construct the manifest.

A hash match without exact byte equality is insufficient during cleanup and replay.

## 10. Final-manifest hash

`final_manifest_sha256` is lowercase SHA-256 over the exact compact UTF-8 JSON encoding of the `FinalManifest` object:

- struct field order shown in Section 8;
- nested field order shown in Sections 8 and 9 and the existing Phase 5 subject schema;
- no insignificant whitespace;
- no trailing newline;
- no hash field inside the hashed manifest.

The same authority and evidence must always produce the same manifest hash.

## 11. Completion receipt

Each completion entry contains one `CompletionReceipt` object with exactly these fields in this order:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-sha256",
  "plan_id": "plan-id",
  "phase_id": "phase-id",
  "phase_title": "phase-title",
  "plan_phase_index": 0,
  "completion_sequence": 1,
  "final_manifest_sha256": "lowercase-sha256",
  "previous_completion_receipt_sha256": null,
  "closed_phases_before": [],
  "closed_phases_after": ["phase-id"],
  "active_phase_before": "phase-id",
  "active_phase_after": null
}
```

All fields are required. Unknown fields are rejected.

For the first entry, `previous_completion_receipt_sha256` is JSON `null`.

For later entries, it is the exact `completion_receipt_sha256` of the preceding entry.

`closed_phases_before` must exactly equal validated state before first publication.

`closed_phases_after` must equal `closed_phases_before` with the requested phase appended exactly once.

`active_phase_after` is explicit JSON `null`, never omitted.

## 12. Completion-receipt hash

`completion_receipt_sha256` is lowercase SHA-256 over the exact compact UTF-8 JSON encoding of the `CompletionReceipt` object using the field order shown in Section 11, with no insignificant whitespace or trailing newline.

The receipt hash forms the chain anchor for the next completion entry.

## 13. Completion ledger

`completion-ledger.json` contains exactly:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-sha256",
  "plan_id": "plan-id",
  "completions": []
}
```

All fields are required. Unknown fields are rejected.

Each `completions` element contains exactly:

```json
{
  "final_manifest": {},
  "final_manifest_sha256": "lowercase-sha256",
  "completion_receipt": {},
  "completion_receipt_sha256": "lowercase-sha256"
}
```

The ledger must satisfy all of the following:

1. authority fields exactly match current accepted plan authority;
2. completion sequences are contiguous from `1`;
3. plan phase indexes and phase metadata match the accepted plan;
4. each phase appears at most once;
5. each completed phase's dependencies appear earlier in the completion chain;
6. each manifest hash recomputes exactly;
7. each receipt hash recomputes exactly;
8. each receipt names its own manifest hash;
9. each receipt points to the preceding receipt hash, or `null` for the first;
10. `closed_phases_before` equals preceding receipt `closed_phases_after`, or `[]` for the first;
11. `closed_phases_after` appends exactly the receipt phase;
12. final manifest and receipt phase identity, sequence, title, and plan index agree;
13. final subject hash and report hash recompute exactly;
14. every archived governance hash recomputes from exact archived bytes;
15. every archived governance record parses and revalidates against the manifest;
16. no entry follows contradictory or malformed history;
17. arrays are ordered and unique where required.

Malformed, contradictory, truncated, reordered, stale, or impossible history is never repaired silently.

## 14. State-to-ledger relation

Before first closeout publication, a missing completion ledger is legal only when `state.closed_phases` is empty.

A nonempty `closed_phases` list without a completion ledger is `CLOSEOUT_LEDGER_STALE` because Phase 1–5 contain no legal command that closes a phase.

After the completion ledger exists, exactly one of these relations is legal:

### 14.1 Stable relation

```text
state.closed_phases == ledger completion phase IDs
```

and either:

- `state.active_phase` names a different open phase not yet in the ledger; or
- `state.active_phase` is null.

### 14.2 In-progress finalization relation

```text
ledger completion phase IDs == state.closed_phases + [state.active_phase]
```

The extra ledger phase must be the final entry and the requested phase.

No other difference in count, order, or identity is legal.

## 15. First closeout publication

For an `OPEN` phase, `phase close` must:

1. complete all common and readiness validation;
2. read and preserve the exact four phase-scoped governance-file byte strings;
3. construct the final manifest deterministically;
4. compute `final_manifest_sha256`;
5. construct the completion receipt deterministically;
6. compute `completion_receipt_sha256`;
7. create or append the completion entry in memory;
8. fully validate the prospective ledger;
9. atomically publish `completion-ledger.json` using same-directory no-clobber temporary-file semantics;
10. enter resumable finalization;
11. continue with Section 16;
12. print success only after final cross-file validation.

No phase-scoped governance file or state byte may change before the completion ledger is durably published.

## 16. Resumable finalization

For `ARCHIVED_PENDING_FINALIZATION`, including immediately after first publication, `phase close` must:

1. load the final completion entry;
2. revalidate the complete completion ledger;
3. verify the requested phase, manifest, receipt, and state relation;
4. for each phase-scoped governance file that still exists, require its exact bytes to equal the corresponding archived content and its hash to recompute;
5. reject any changed, replaced, unsafe, malformed, or unarchived phase-scoped file;
6. remove each still-existing exact archived phase-scoped file;
7. tolerate an already absent archived phase-scoped file;
8. require all four phase-scoped files to be absent after cleanup;
9. construct new state bytes by setting `active_phase` to `null` and appending the phase once to `closed_phases`;
10. validate the prospective state against the accepted plan;
11. atomically replace `state.json`;
12. re-read and validate completion ledger and state relation;
13. require all four phase-scoped files to remain absent;
14. return the exact success output.

Removal order is fixed:

1. `audit-ledger.json`;
2. `implementation-authority.json`;
3. `accepted-contract.json`;
4. `contract-draft.json`.

A failure during finalization leaves the completion ledger unchanged and is replayable. Existing exact archived files may be removed before a later failure; their absence is legal on replay.

## 17. Completed idempotency

For `CLOSED`, replaying the exact command must:

1. fully validate accepted plan, state, and completion ledger;
2. require `state.active_phase` to be `null`;
3. require the requested phase to be the final closed phase and final completion entry;
4. require all four phase-scoped governance files to be absent;
5. recompute and validate every manifest, receipt, and chain hash;
6. return the exact original output;
7. write nothing and preserve every file byte exactly.

Closing an earlier completed phase while a later phase is active or completed is not an idempotent replay and must reject.

## 18. Success output

A successful first closeout, resumed closeout, or completed idempotent replay prints exactly:

```text
PHASE_CLOSED <phase_id> <completion_sequence> <final_manifest_sha256> <completion_receipt_sha256>
```

The line contains exactly five ASCII-space-separated tokens and one trailing newline from the CLI.

No additional stdout or stderr is permitted on success.

## 19. Closeout conflicts

Reject without changing completion evidence when any of the following occurs:

- requested phase is unknown;
- requested phase is neither the active phase nor the final exactly completed phase;
- requested phase already appears non-finally in completion history;
- active phase and completion-ledger relation is impossible;
- phase-scoped governance bytes differ from the archived bytes after first publication;
- an archived file is replaced by an unsafe object;
- state already closes the phase but required completion evidence is absent;
- completion ledger records the phase but state names a different active phase before finalization;
- current authority or repository subject drifts before first publication;
- manifest or receipt recomputation differs;
- history or dependency ordering is inconsistent.

No conflict is repaired silently.

## 20. Governance-file topology

`completion-ledger.json` must be:

- exactly `<repo>/.mrgs/completion-ledger.json`;
- a regular file when present;
- not a symlink, junction, directory, device, or unsafe reparse-point object;
- reached through safe existing `.mrgs` topology.

The fixed governance filename allowlist must add exactly:

```text
completion-ledger.json
```

The Phase 4 governance exemption list must add exactly:

```text
.mrgs/completion-ledger.json
```

No user input chooses a governance destination filename.

Tracked `.mrgs` entries remain prohibited.

## 21. Git subprocess requirements

All Phase 6 Git calls must use the hardened Phase 4 Git runner and existing validation boundaries.

Requirements include:

- no shell invocation;
- standard input closed;
- no network or lazy fetch;
- replacement refs disabled;
- literal pathspecs;
- inherited Git environment removed;
- system Git configuration disabled;
- external diff, text conversion, filters, fsmonitor, and hooks not executed;
- strict exit-status and strict-output parsing;
- no Git mutation command.

Phase 6 must not stage, commit, amend, switch, checkout, reset, restore, clean, stash, merge, rebase, tag, push, fetch, modify refs, or write Git configuration.

## 22. Persistence and crash consistency

`completion-ledger.json` and `state.json` use deterministic human-readable JSON.

Every file replacement must use:

- complete serialization before opening the destination;
- same-directory unique temporary files;
- no-clobber temporary creation;
- complete write and file sync;
- atomic replacement;
- cleanup after failure where safely possible.

Phase 6 intentionally introduces a resumable two-file closeout transition:

1. completion ledger first;
2. exact phase-scoped cleanup;
3. state last.

The completion entry is the durable recovery anchor. The command must recognize and resume every prefix of the fixed cleanup sequence.

A failure must never:

- truncate existing completion history;
- create a second entry for the same phase;
- regenerate different manifest or receipt bytes;
- close state before the completion entry exists;
- delete a phase-scoped file whose bytes do not exactly match its archive;
- leave a temporary file after a handled pre-publication failure.

## 23. Error model

Success exits `0`.

Failure exits nonzero and prints exactly:

```text
error: <CATEGORY>
```

Phase 6 preserves all applicable Phase 1–5 categories and adds:

```text
CLOSEOUT_NOT_READY
CLOSEOUT_LEDGER_INVALID
CLOSEOUT_LEDGER_STALE
CLOSEOUT_CONFLICT
CLOSEOUT_ARCHIVE_MISMATCH
CLOSEOUT_STATE_MISMATCH
```

Use:

- `CLOSEOUT_NOT_READY` when current valid authority has not reached an exact current-subject Phase 5 `PASS`;
- `CLOSEOUT_LEDGER_INVALID` for malformed or contradictory completion-ledger structure, hashes, or history;
- `CLOSEOUT_LEDGER_STALE` for structurally valid completion evidence bound to different accepted plan authority or an impossible legacy state relation;
- `CLOSEOUT_CONFLICT` for requested-phase or lifecycle conflicts;
- `CLOSEOUT_ARCHIVE_MISMATCH` when a still-existing phase-scoped file differs from its archived exact bytes or topology;
- `CLOSEOUT_STATE_MISMATCH` when state and completion history cannot form a legal stable or in-progress relation.

Existing path, Git, authority, audit-ledger, and persistence categories remain controlling when more specific.

No failure prints archived content, report content, filesystem secrets, raw Git stderr, or a backtrace.

## 24. Dependencies

No new production or development dependency is authorized.

Continue using only dependencies already present in `Cargo.toml`.

No async runtime, Git library, database, HTTP client, UUID library, time library, logging framework, compression library, model SDK, or transaction framework.

## 25. Required tests

Add focused Phase 6 tests in:

```text
tests/phase6.rs
```

Do not add a generated obligation registry. Do not invoke `cargo test` recursively from a test.

Required direct executable coverage includes exactly these 72 obligations.

### 25.1 CLI and readiness

1. exact `phase close` CLI parsing;
2. unknown phase rejects;
3. requested phase different from active rejects;
4. missing active phase without completed replay rejects;
5. dependency inconsistency rejects;
6. missing contract draft rejects;
7. missing accepted contract rejects;
8. missing implementation authority rejects;
9. missing audit ledger rejects;
10. pending, failed, routed, checked, and final-failed audit states reject.

### 25.2 Current-subject proof

11. valid terminal PASS is closeout-ready;
12. changed worktree after PASS rejects;
13. changed index after PASS rejects;
14. changed `HEAD` after PASS rejects;
15. changed branch after PASS rejects;
16. stale accepted contract rejects;
17. stale implementation authority rejects;
18. malformed passed report rejects;
19. passed report hash mismatch rejects;
20. passed subject hash mismatch rejects.

### 25.3 Final manifest

21. valid manifest has exact required fields;
22. deterministic manifest bytes and hash;
23. exact plan metadata and zero-based phase index;
24. exact phase dependencies in declared order;
25. exact accepted contract content preserved;
26. exact final audit subject preserved;
27. exact report bytes and SHA preserved;
28. all four governance files archived as exact bytes;
29. all four archive hashes recompute;
30. manifest hash mismatch rejects.

### 25.4 Completion receipt and ledger

31. valid first receipt uses null previous hash;
32. exact closed-phases before and after arrays;
33. deterministic receipt bytes and hash;
34. exact manifest-hash binding;
35. second receipt chains to first receipt hash;
36. completion sequence is contiguous;
37. duplicate completed phase rejects;
38. dependency completion ordering enforced;
39. reordered completion entries reject;
40. receipt hash mismatch rejects;
41. broken previous-receipt link rejects;
42. wrong plan authority is stale.

### 25.5 First publication and finalization

43. successful first closeout prints exact output;
44. completion ledger is published before phase-scoped cleanup;
45. successful closeout removes exactly the four phase-scoped files;
46. accepted-plan and completion-ledger bytes remain present;
47. final state clears active phase and appends closed phase once;
48. no unrelated governance file changes;
49. no tracked repository path changes;
50. no temporary files after success.

### 25.6 Resumable and idempotent behavior

51. replay after ledger publication resumes cleanup;
52. replay after one archived file removal resumes;
53. replay after two archived file removals resumes;
54. replay after three archived file removals resumes;
55. replay after all archived files removed finalizes state;
56. completed replay returns identical output;
57. completed replay preserves all bytes;
58. existing archived file with changed bytes rejects;
59. existing archived file with unsafe topology rejects;
60. state closed without completion entry rejects;
61. completion entry with wrong active phase rejects;
62. earlier completed phase replay after later progress rejects.

### 25.7 Corruption, persistence, and regression

63. unknown completion-ledger field rejects;
64. missing completion-ledger field rejects;
65. noncontiguous completion sequence rejects;
66. archived governance content/hash mismatch rejects;
67. first-publication failure leaves no completion ledger;
68. temporary collision does not truncate an existing file;
69. replacement failure preserves previous completion-ledger bytes;
70. Git runner safety and no mutation boundaries remain active;
71. Phase 1–5 representative CLI outputs and error categories remain unchanged;
72. no new production or development dependency is required.

Platform-dependent topology tests must execute the supported branch. A capability-unavailable branch must contain an explicit capability assertion and a concrete fallback safety assertion. Silent omission is not coverage.

## 26. Verification

Required targeted and full verification:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo test --test phase6
cargo clippy --all-targets -- -D warnings
cargo test
git diff --check
```

Run the narrowest affected Phase 6 test after each repair before rerunning broader verification.

Do not place a full-suite `cargo test` invocation inside `tests/phase6.rs`; the external verification ladder is authoritative for regression execution.

A timeout, truncated output, skipped command, or unexecuted command is not a pass.

## 27. Allowed implementation paths

Only these repository paths may change for Phase 6:

```text
README.md
src/audit.rs
src/cli.rs
src/closeout.rs
src/error.rs
src/implementation.rs
src/main.rs
src/state.rs
tests/phase6.rs
docs/contracts/phase-06-contract.md
```

Create only:

```text
src/closeout.rs
tests/phase6.rs
```

The supplied Phase 6 contract is frozen and may only remain as the exact supplied file.

No other source, test, manifest, lockfile, contract, plan, generated artifact, or agent configuration is authorized.

## 28. Forbidden changes

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
src/path.rs
src/git.rs
src/plan.rs
src/contract.rs
src/rules.rs
tests/integration.rs
tests/phase4_obligations.json
tests/phase4_obligations.rs
tests/phase5.rs
graphify-out/**
.git/**
.mrgs/**
target/**
```

Do not weaken, delete, rename, ignore, or replace any existing test.

Do not add dependencies, features, build scripts, examples, benchmarks, fixtures outside `tests/phase6.rs`, generated registries, or hidden runtime configuration.

## 29. Final evidence

The final handoff must include:

- baseline branch and `HEAD`;
- final `HEAD`, which must remain unchanged;
- exact changed and created paths;
- forbidden-path result;
- exact targeted and full test summaries;
- formatting, check, clippy, and diff-check results;
- exact Phase 6 obligation coverage count `72/72`;
- closeout audit verdict and repair-cycle count;
- Graphify reconnaissance and refresh status as advisory evidence only;
- final Git status and staged-path list;
- confirmation that no commit or push occurred;
- exact blockers or `NONE`;
- recommendation for the human Git boundary.

## 30. Completion rule

Phase 6 implementation is complete only when:

1. all contract requirements are implemented;
2. all 72 required tests contain direct executable assertions;
3. all required verification commands pass;
4. all Phase 1–5 tests remain green;
5. no forbidden path changes;
6. the frozen contract is unchanged;
7. no file is staged;
8. no commit or push occurred;
9. an independent read-only audit maps all 72 obligations to direct evidence and returns `PASS`.

Phase 6 authorizes closeout behavior only. It does not authorize Phase 7 continuity metadata, Phase 8 recovery, Phase 9 adversarial expansion, Phase 10 adoption readiness, Git integration, commit, or push.
