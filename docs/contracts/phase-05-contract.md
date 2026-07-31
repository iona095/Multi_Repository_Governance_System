# Phase 5 Contract — Independent Audit and Bounded Repair Routing

Contract version: 1
Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

## 1. Objective

Extend the Phase 1–4 governance foundation with deterministic audit-subject capture, exact independent-audit evidence ingestion, append-only audit history, and a bounded repair route that can be checked before re-audit.

Phase 5 implements only:

1. capture of an exact implementation snapshot after the Phase 4 boundary check succeeds;
2. registration of one independent auditor identity for each audit round;
3. strict ingestion and preservation of an audit report bound to that exact snapshot;
4. deterministic `PASS` or `FAIL` validation;
5. exact-path repair routing for failed findings;
6. repair-delta enforcement against the failed audit snapshot;
7. re-audit after a checked repair;
8. a maximum of two repair attempts;
9. terminal success after audit `PASS`, or terminal failure after the final failed re-audit.

Phase 5 does not invoke models, execute implementation work, execute repairs, decide whether an independence declaration is truthful, close a phase, create a final manifest, commit, push, merge, tag, mutate Git refs, use the network, or automatically chain into Phase 6.

A Phase 4 `IMPLEMENTATION_OK` result proves only that the implementation change boundary is valid. Phase 5 audit evidence determines correctness.

## 2. Controlling authority and lifecycle

All Phase 1–4 authority remains controlling:

- accepted plan;
- active phase;
- current accepted contract revision;
- accepted contract content and exact SHA-256;
- implementation authority;
- Git object format;
- implementation baseline branch and commit;
- Phase 4 path rules and repository-safety requirements.

Phase 5 adds exactly one governance file:

```text
<repo>/.mrgs/audit-ledger.json
```

The audit lifecycle is inferred from the fully validated ledger:

- `NOT_STARTED`: no audit ledger exists;
- `PENDING`: the final audit round has no report;
- `REPAIR_ROUTED`: the final round is `FAIL` and its repair route has not been checked;
- `REPAIR_CHECKED`: the final round is `FAIL` and its repair route has been checked;
- `PASSED`: the final round is `PASS`;
- `FAILED_FINAL`: the final round is `FAIL`, two repair attempts have already been checked, and no further repair route exists.

The lifecycle is not stored separately.

No audit round may be opened after `PASSED` or `FAILED_FINAL`.

## 3. CLI surface

Preserve all existing commands and add exactly:

```text
mrgs audit begin --repo <REPOSITORY_PATH> --auditor <AUDITOR_ID>
mrgs audit record --repo <REPOSITORY_PATH> --report <REPORT_PATH>
mrgs repair check --repo <REPOSITORY_PATH>
```

No other new command is authorized.

The complete new command families are:

```text
mrgs audit begin ...
mrgs audit record ...
mrgs repair check ...
```

Phase 5 does not add `audit run`, `audit accept`, `repair begin`, `repair apply`, `repair record`, `repair retry`, or automatic execution commands.

## 4. Common Phase 5 validation order

Every Phase 5 command must fail closed and validate in this order before any write:

1. CLI token grammar;
2. canonical repository path;
3. safe `.mrgs` directory and fixed governance-file topology;
4. accepted plan, state, plan bytes, active phase, draft, and accepted-contract ledger;
5. accepted contract exact bytes and SHA-256;
6. implementation authority structure and exact contextual binding;
7. Git root, object format, current branch, current `HEAD`, operation state, index structure, index flags, sparse state, and promisor/no-lazy-fetch requirements;
8. current Phase 4 implementation boundary check;
9. audit-ledger file topology, parse, structure, hashes, history, and contextual binding when it exists;
10. command-specific preconditions;
11. current audit-subject construction when required;
12. source-report path and exact report validation when required;
13. deterministic transition construction;
14. atomic publication.

A failure before publication must preserve every existing governance byte exactly and must leave no temporary file.

## 5. Reuse of Phase 4 authority

Phase 5 must reuse the Phase 4 authority and Git adapters rather than creating a weaker parallel implementation.

The implementation may refactor Phase 4 internals into reusable `pub(crate)` functions or structures only when:

- public CLI behavior remains byte-for-byte compatible;
- Phase 4 error categories remain unchanged;
- all existing Phase 4 tests remain green;
- the refactor does not broaden Git commands, environment inheritance, network access, or mutable scope.

Every successful Phase 5 command requires the equivalent of a successful current:

```text
mrgs implementation check --repo <REPOSITORY_PATH>
```

The check must be performed from current authority and repository state. A stale earlier command result is not evidence.

## 6. Auditor identity

`--auditor` is required for `audit begin`.

An auditor ID must:

- be strict UTF-8;
- contain 1–128 bytes;
- equal its own trimmed form;
- begin with an ASCII alphanumeric character;
- contain only ASCII alphanumeric characters, `.`, `_`, `-`, `@`, or `:`;
- contain no whitespace or control character.

The auditor ID is preserved exactly.

MRGS records an independence declaration but cannot prove organizational or model independence. The audit report must contain exactly:

```json
"independence_declaration": "INDEPENDENT"
```

Any other token is invalid.

## 7. Audit subject

An audit round is bound to an exact deterministic `AuditSubject`.

The subject contains exactly:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-hex",
  "phase_id": "active-phase",
  "contract_id": "accepted-contract-id",
  "contract_revision": 1,
  "contract_source_path": "repo/relative/path",
  "contract_sha256": "lowercase-hex",
  "implementation_baseline_head": "git-object-id",
  "implementation_baseline_branch": "branch-name",
  "git_object_format": "sha1-or-sha256",
  "current_head": "git-object-id",
  "current_branch": "branch-name",
  "entries": []
}
```

All fields are required. Unknown fields are rejected.

`entries` contains exactly one entry for every path in the current Phase 4 change inventory, sorted by raw UTF-8 byte order of `path`, with no duplicates.

Each entry has exactly:

```json
{
  "path": "repo/relative/path",
  "baseline": null,
  "head": null,
  "index": null,
  "worktree": {
    "kind": "ABSENT",
    "sha256": null
  }
}
```

`baseline`, `head`, and `index` are either `null` or:

```json
{
  "mode": "six-octal-digit-mode",
  "oid": "lowercase-git-object-id"
}
```

`worktree.kind` is exactly one of:

- `ABSENT`;
- `REGULAR`;
- `SYMLINK`.

For `ABSENT`, `worktree.sha256` is `null`.

For `REGULAR`, `worktree.sha256` is lowercase SHA-256 over the exact current file bytes.

For `SYMLINK`, `worktree.sha256` is lowercase SHA-256 over the exact strict-UTF-8 symlink-target bytes returned without following the link.

Directories, devices, sockets, FIFOs, unsafe reparse points, non-UTF-8 paths, non-UTF-8 symlink targets, ambiguous Git records, conflicts, and malformed modes or object IDs are rejected.

## 8. Snapshot layer semantics

For each inventory path:

- `baseline` is the exact stage-zero tree entry at the implementation baseline commit;
- `head` is the exact tree entry at current `HEAD`;
- `index` is the exact stage-zero index entry;
- `worktree` is the exact live leaf state.

An absent layer is represented by JSON `null`, never by omitted fields, empty strings, zero object IDs, or synthetic modes.

The object-ID length is determined by the validated Git object format:

- SHA-1: 40 lowercase hexadecimal characters;
- SHA-256: 64 lowercase hexadecimal characters.

Git output must be parsed with NUL-delimited plumbing commands and strict complete-record validation. Human-readable, quoted, locale-dependent, or line-oriented filename output is not authoritative.

The subject must include ignored changed paths when Phase 4 includes them in its inventory. Ignored status never bypasses the accepted contract path rules.

## 9. Subject hash

The `subject_sha256` is lowercase SHA-256 over the exact compact UTF-8 JSON encoding of the `AuditSubject` object:

- struct field order shown in Section 7;
- entry field order shown in Section 7;
- no insignificant whitespace;
- no trailing newline;
- entries already sorted;
- no `subject_sha256` field inside the hashed object.

The same subject must always produce the same hash.

A hash match without structural and contextual equality is insufficient. Validation must recompute the hash and compare all subject fields.

## 10. Audit ID

An audit ID is deterministic.

Create this exact identity seed:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-hex",
  "phase_id": "active-phase",
  "contract_id": "accepted-contract-id",
  "contract_revision": 1,
  "contract_sha256": "lowercase-hex",
  "round": 1,
  "subject_sha256": "lowercase-hex",
  "auditor_id": "exact-auditor-id"
}
```

Encode it as compact UTF-8 JSON with the shown field order and no trailing newline.

The audit ID is lowercase SHA-256 over those exact bytes.

## 11. Audit report source

`audit record` accepts a report file through `--report`.

The report is external evidence and must not alter the repository snapshot after `audit begin`. The report source may be outside the governed repository.

The report source must:

- exist;
- resolve to a regular file;
- have a leaf that is not a symlink, junction, directory, device, or unsafe reparse-point object;
- canonicalize successfully;
- have a canonical path representable as strict UTF-8;
- be read-only input to MRGS;
- remain byte-identical through read, hash, parse, validation, and publication.

MRGS stores the canonical absolute report path, exact source bytes, and lowercase SHA-256 in the ledger. It never writes to, renames, deletes, or otherwise mutates the report source.

A report placed inside the governed repository is legal only when it already belongs to the exact pending audit subject. Creating or modifying a report inside the repository after `audit begin` changes the subject and therefore causes `AUDIT_SUBJECT_STALE`. The intended workflow is to generate the report outside the governed repository.

The report is strict JSON. Unknown or missing fields are rejected.

## 12. Audit report format

The report object contains exactly:

```json
{
  "schema_version": 1,
  "audit_id": "lowercase-hex",
  "subject_sha256": "lowercase-hex",
  "auditor_id": "exact-auditor-id",
  "independence_declaration": "INDEPENDENT",
  "verdict": "PASS",
  "summary": "nonblank exact text",
  "requirement_results": [],
  "verification_results": [],
  "findings": []
}
```

All fields are required.

`summary` must be nonempty, equal its own trimmed form, and contain no NUL.

### 12.1 Requirement results

There must be exactly one result for every accepted-contract `requirements` entry, in the same order.

Each result contains exactly:

```json
{
  "requirement": "exact accepted-contract requirement",
  "status": "PASS",
  "evidence": "nonblank exact evidence"
}
```

`requirement` must equal the corresponding accepted-contract string exactly.

`status` is exactly one of:

- `PASS`;
- `FAIL`;
- `BLOCKED`.

`evidence` must be nonempty, equal its own trimmed form, and contain no NUL.

### 12.2 Verification results

There must be exactly one result for every accepted-contract `verification_commands` entry, in the same order.

Each result contains exactly:

```json
{
  "command": "exact accepted-contract verification command",
  "status": "PASS",
  "evidence": "nonblank exact evidence"
}
```

`command` must equal the corresponding accepted-contract command exactly.

The status and evidence rules are the same as requirement results.

MRGS records the evidence. It does not execute the command or infer success from prose.

### 12.3 Findings

Each finding contains exactly:

```json
{
  "id": "F-001",
  "severity": "BLOCKER",
  "claim_kind": "REQUIREMENT",
  "claim_index": 1,
  "summary": "nonblank exact text",
  "evidence": "nonblank exact evidence",
  "repair_paths": ["src/example.rs"]
}
```

Finding IDs:

- contain 1–64 bytes;
- equal their trimmed form;
- begin with an ASCII alphanumeric character;
- contain only ASCII alphanumeric characters, `.`, `_`, or `-`;
- are unique within the report.

`severity` is exactly one of:

- `BLOCKER`;
- `MAJOR`;
- `MINOR`.

`claim_kind` is exactly one of:

- `REQUIREMENT`;
- `VERIFICATION`.

`claim_index` is one-based and must identify an existing result of the selected kind whose status is not `PASS`.

`summary` and `evidence` follow the nonblank rules.

`repair_paths`:

- contains at least one entry;
- contains no duplicates;
- is sorted by raw UTF-8 byte order;
- contains exact file paths only, never directory-prefix rules;
- uses strict normalized repository-relative `/` syntax;
- contains no `.git` or `.mrgs` path;
- contains no glob, wildcard, trailing slash, absolute path, drive prefix, backslash, empty component, `.` component, `..` component, or control character;
- must be permitted by the accepted contract rule set;
- may name a currently absent file when creation of that exact file is required.

## 13. Verdict consistency

`verdict` is exactly `PASS` or `FAIL`.

A `PASS` report is valid only when:

- every requirement result is `PASS`;
- every verification result is `PASS`;
- `findings` is empty.

A `FAIL` report is valid only when:

- at least one requirement or verification result is `FAIL` or `BLOCKED`;
- `findings` is nonempty;
- every finding references a non-`PASS` claim;
- every non-`PASS` claim is referenced by at least one finding.

MRGS must reject internally inconsistent reports. It must not normalize, downgrade, aggregate, vote, or infer a verdict.

## 14. Audit ledger

`audit-ledger.json` is a strict deterministic JSON object:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-hex",
  "phase_id": "active-phase",
  "contract_id": "accepted-contract-id",
  "contract_revision": 1,
  "contract_source_path": "repo/relative/path",
  "contract_sha256": "lowercase-hex",
  "implementation_baseline_head": "git-object-id",
  "implementation_baseline_branch": "branch-name",
  "git_object_format": "sha1-or-sha256",
  "max_repair_attempts": 2,
  "rounds": []
}
```

All fields are required. Unknown fields are rejected.

The authority tuple must exactly match current validated authority.

`max_repair_attempts` is exactly `2`.

`rounds` is nonempty after first publication and uses contiguous one-based round numbers.

## 15. Audit round record

Each round contains exactly:

```json
{
  "round": 1,
  "audit_id": "lowercase-hex",
  "auditor_id": "exact-auditor-id",
  "subject_sha256": "lowercase-hex",
  "subject": {},
  "status": "PENDING",
  "report_source_path": null,
  "report_sha256": null,
  "report_content": null,
  "repair": null
}
```

`status` is exactly:

- `PENDING`;
- `PASS`;
- `FAIL`.

For `PENDING`:

- all report fields are `null`;
- `repair` is `null`.

For `PASS`:

- all report fields are populated;
- the exact report validates as `PASS`;
- `repair` is `null`.

For `FAIL`:

- all report fields are populated;
- the exact report validates as `FAIL`;
- `repair` is either a valid repair route or `null` only for terminal final failure.

Earlier rounds are immutable after their repair reaches `CHECKED`.

Only the final round may be `PENDING` or contain a `ROUTED` repair.

## 16. Repair route record

A repair route contains exactly:

```json
{
  "attempt": 1,
  "status": "ROUTED",
  "finding_ids": ["F-001"],
  "allowed_paths": ["src/example.rs"],
  "pre_subject_sha256": "lowercase-hex",
  "post_subject_sha256": null,
  "post_subject": null,
  "changed_paths": []
}
```

`attempt` is `1` or `2`.

`status` is exactly:

- `ROUTED`;
- `CHECKED`.

`finding_ids` preserves report finding order and contains every report finding ID exactly once.

`allowed_paths` is the sorted unique union of all report finding `repair_paths`.

`pre_subject_sha256` equals the failed round subject hash.

For `ROUTED`:

- `post_subject_sha256` is `null`;
- `post_subject` is `null`;
- `changed_paths` is empty.

For `CHECKED`:

- `post_subject_sha256` and `post_subject` are populated and validate;
- `changed_paths` is nonempty, sorted, unique, and equals the exact subject-entry delta;
- every changed path belongs to `allowed_paths`;
- every finding has at least one changed path in its own `repair_paths`.

## 17. `audit begin`

Command:

```text
mrgs audit begin --repo <REPOSITORY_PATH> --auditor <AUDITOR_ID>
```

It must:

1. perform the common validation order;
2. require current Phase 4 implementation check success;
3. build the exact current audit subject;
4. compute the subject hash;
5. validate any existing audit ledger;
6. enforce lifecycle preconditions;
7. compute the next round and audit ID;
8. create or append one `PENDING` round;
9. atomically write `audit-ledger.json`;
10. print:

```text
AUDIT_OPEN <audit_id> <round> <subject_sha256>
```

### 17.1 First audit

When no ledger exists, create a ledger and round `1`.

### 17.2 Audit after repair

A new round is allowed only when the previous round is `FAIL` with a `CHECKED` repair.

Before opening the new round, the freshly recomputed subject must equal the prior repair `post_subject` structurally and by hash.

### 17.3 Idempotent begin

If the final round is `PENDING` and the supplied auditor ID and freshly computed subject exactly equal that round, return the same `AUDIT_OPEN` output without writing.

A different auditor ID or subject while a round is pending is `AUDIT_PENDING_CONFLICT`.

## 18. `audit record`

Command:

```text
mrgs audit record --repo <REPOSITORY_PATH> --report <REPORT_PATH>
```

It must:

1. perform the common validation order;
2. require an existing final `PENDING` round;
3. recompute the current audit subject;
4. require exact equality with the pending subject and hash;
5. read and hash the exact report source;
6. parse and validate the strict report;
7. require matching audit ID, subject hash, and auditor ID;
8. validate complete requirement and verification coverage;
9. validate verdict consistency;
10. transition only the final pending round;
11. atomically replace the ledger;
12. preserve exact report bytes.

A changed repository snapshot between `audit begin` and `audit record` is `AUDIT_SUBJECT_STALE`.

### 18.1 PASS output

For a valid `PASS` report, print:

```text
AUDIT_PASS <audit_id> <round> <subject_sha256>
```

The audit lifecycle becomes `PASSED`.

### 18.2 FAIL with repair available

Count earlier `CHECKED` repair routes.

When fewer than two exist:

1. create the next repair route;
2. set `attempt` to checked-count plus one;
3. derive finding IDs and allowed paths exactly;
4. print:

```text
REPAIR_ROUTED <audit_id> <round> <attempt> <allowed_path_count>
```

The lifecycle becomes `REPAIR_ROUTED`.

### 18.3 Final FAIL

When two repair attempts have already been checked, record the valid `FAIL` report with `repair: null` and print:

```text
AUDIT_FAIL_FINAL <audit_id> <round> <subject_sha256>
```

The lifecycle becomes `FAILED_FINAL`.

No further audit or repair command may succeed.

### 18.4 Idempotent record

Re-recording the exact same source path, SHA-256, bytes, parsed report, and resulting transition returns the same output without writing.

A different report after the round is no longer pending is `AUDIT_REPORT_CONFLICT`.

## 19. Repair execution boundary

MRGS does not execute a repair.

After `REPAIR_ROUTED`, an external repair process may edit only exact routed paths and must not commit, change branches, rewrite history, alter Git configuration, mutate `.git`, or edit `.mrgs`.

The accepted contract path rules remain controlling. A routed path never overrides a forbidden path or expands the accepted implementation boundary.

## 20. `repair check`

Command:

```text
mrgs repair check --repo <REPOSITORY_PATH>
```

It must:

1. perform the common validation order;
2. require the final round to be `FAIL` with a `ROUTED` repair;
3. require unchanged accepted authority, implementation baseline, current branch, and current `HEAD`;
4. recompute the current audit subject;
5. compare it with the failed pre-repair subject;
6. derive the exact sorted subject-entry delta;
7. require at least one changed path;
8. require every changed path in the route `allowed_paths`;
9. require every finding to have at least one changed path within that finding’s own `repair_paths`;
10. require current Phase 4 implementation check success;
11. set the repair to `CHECKED`;
12. store the exact post-repair subject, hash, and changed paths;
13. atomically replace the ledger;
14. print:

```text
REPAIR_OK <audit_id> <round> <attempt> <post_subject_sha256> <changed_path_count>
```

The command proves only that the repair delta stayed inside its routed exact paths and the overall implementation boundary still passes. It is not a correctness verdict. A new audit round is required.

### 20.1 Idempotent repair check

When the final repair is already `CHECKED`, and the freshly recomputed subject exactly equals its stored post subject, return the same `REPAIR_OK` output without writing.

Any drift after a checked repair is `REPAIR_SUBJECT_STALE`.

## 21. Subject delta

The repair delta is computed over the union of pre- and post-subject entry paths.

A path is changed when:

- it exists in only one subject; or
- any baseline, head, index, worktree kind, or worktree hash field differs.

Authority-level or Git-context differences are not repair-path changes. They are stale authority and must fail before delta evaluation.

A content-hash collision is not treated as sufficient equality when other recorded fields differ.

## 22. Ledger history validation

A valid ledger must satisfy all of the following:

1. authority tuple exactly matches current authority;
2. round numbers are contiguous from `1`;
3. each audit ID recomputes exactly;
4. each subject hash recomputes exactly;
5. each stored report SHA recomputes from exact stored report bytes;
6. each stored report parses and revalidates against its round;
7. no round follows `PASS`;
8. no round follows terminal final `FAIL`;
9. every round after round `1` follows a `FAIL` round with a `CHECKED` repair;
10. each later subject exactly equals the preceding repair post subject;
11. repair attempts are contiguous `1`, then `2`, with no duplicates;
12. no more than two repair routes exist;
13. only the final round may be pending;
14. only the final round may contain a routed unchecked repair;
15. all earlier rounds are complete and immutable;
16. all nullable fields obey their state-specific rules;
17. arrays are ordered, unique where required, and complete.

Malformed, contradictory, truncated, stale, or impossible history is `AUDIT_LEDGER_INVALID` or `AUDIT_LEDGER_STALE`. It is never repaired silently.

## 23. Governance-file topology

`audit-ledger.json` must be:

- exactly `<repo>/.mrgs/audit-ledger.json`;
- a regular file when present;
- not a symlink, junction, directory, device, or other reparse-point object;
- reached through safe existing `.mrgs` topology.

The governance filename allowlist must add exactly:

```text
audit-ledger.json
```

No user input may choose a governance destination filename.

Tracked `.mrgs` entries remain prohibited according to the existing Phase 4 rules.

## 24. Git subprocess requirements

All Phase 5 Git calls must use the hardened Phase 4 Git runner.

Requirements include:

- no shell invocation;
- standard input closed;
- no network;
- `--no-replace-objects`;
- `--no-lazy-fetch`;
- `--literal-pathspecs`;
- `GIT_NO_LAZY_FETCH=1`;
- `GIT_OPTIONAL_LOCKS=0`;
- system Git configuration disabled;
- external diff and fsmonitor disabled;
- inherited Git environment removed;
- strict exit-status handling;
- strict UTF-8 only where textual output is required;
- NUL-delimited path records.

Phase 5 must not run Git commands that mutate the worktree, index, refs, configuration, object database, remotes, submodules, stash, or hooks.

## 25. Persistence

Use deterministic human-readable JSON for `audit-ledger.json`.

Publication must use a same-directory unique temporary file, complete write, file sync, and atomic replacement.

Requirements:

- temporary creation uses no-clobber semantics;
- a collision retries with a different generated name;
- a failed write or replace preserves the prior ledger exactly;
- temporary files are removed after failure where safely possible;
- no partial ledger is visible;
- idempotent operations preserve ledger bytes exactly;
- no command writes before all validation succeeds.

No multi-file transaction is introduced in Phase 5.

## 26. Error model

Successful Phase 5 commands exit `0`.

Failures exit nonzero and print exactly:

```text
error: <CATEGORY>
```

Phase 5 preserves applicable Phase 4 categories and adds:

```text
AUDITOR_ID_INVALID
AUDIT_LEDGER_MISSING
AUDIT_LEDGER_INVALID
AUDIT_LEDGER_STALE
AUDIT_PENDING_CONFLICT
AUDIT_NOT_PENDING
AUDIT_TERMINAL
AUDIT_SUBJECT_STALE
AUDIT_REPORT_INVALID
AUDIT_REPORT_MISMATCH
AUDIT_REPORT_CONFLICT
REPAIR_NOT_ROUTED
REPAIR_SCOPE_VIOLATION
REPAIR_NO_CHANGE
REPAIR_SUBJECT_STALE
```

Invalid report source paths, unsafe topology, malformed JSON, unknown fields, missing fields, invalid hashes, invalid status tokens, incomplete claim coverage, inconsistent verdicts, invalid finding references, and invalid repair paths map to `AUDIT_REPORT_INVALID` unless a more specific safety or authority category already controls.

Ledger parse or structural defects map to `AUDIT_LEDGER_INVALID`.

A structurally valid ledger bound to older accepted or implementation authority maps to `AUDIT_LEDGER_STALE`.

Persistence failures map to `PERSISTENCE_FAILED`.

No failure prints report contents, filesystem secrets, raw Git stderr, or a backtrace during normal operation.

## 27. Dependencies

No new production or development dependency is authorized.

Continue using only the dependencies already present in `Cargo.toml`.

No async runtime, Git library, database, HTTP client, UUID library, timestamp library, logging framework, plugin framework, or model SDK.

## 28. Required tests

Add focused Phase 5 tests in:

```text
tests/phase5.rs
```

Do not add a Phase 5 obligation registry or another generated test matrix.

Required coverage includes at least:

### 28.1 CLI and first audit

1. exact CLI parsing for all three commands;
2. valid first `audit begin`;
3. deterministic audit ID;
4. deterministic subject hash;
5. sorted unique subject entries;
6. exact `AUDIT_OPEN` output;
7. repeated identical begin is byte-preserving idempotent;
8. pending begin with different auditor rejects;
9. pending begin after subject drift rejects.

### 28.2 Subject layers

10. baseline-only entry;
11. HEAD-only entry;
12. staged-only entry;
13. unstaged entry;
14. untracked entry;
15. ignored inventory entry where permitted;
16. deletion represented as absent worktree;
17. regular-file exact-byte hash;
18. symlink-target exact-byte hash on supported platforms;
19. executable/index mode preservation where supported;
20. SHA-1 object IDs;
21. SHA-256 object IDs when test Git supports them;
22. malformed Git layer records reject;
23. conflicts reject;
24. unsafe filesystem types reject;
25. non-UTF-8 evidence rejects where constructible.

### 28.3 PASS report

26. valid complete PASS report;
27. exact report bytes and SHA preserved;
28. exact `AUDIT_PASS` output;
29. missing requirement result rejects;
30. duplicate or reordered requirement result rejects;
31. missing verification result rejects;
32. mismatched command rejects;
33. PASS with non-PASS claim rejects;
34. PASS with findings rejects;
35. wrong auditor rejects;
36. wrong audit ID rejects;
37. wrong subject hash rejects;
38. changed subject before record rejects;
39. unknown or missing report field rejects;
40. invalid independence declaration rejects.

### 28.4 FAIL and routing

41. valid FAIL creates attempt 1;
42. repair paths are sorted unique union;
43. exact `REPAIR_ROUTED` output;
44. FAIL without a non-PASS claim rejects;
45. FAIL without findings rejects;
46. unreferenced non-PASS claim rejects;
47. finding referencing PASS claim rejects;
48. invalid finding ID rejects;
49. invalid severity rejects;
50. invalid exact repair path rejects;
51. repair path outside accepted rules rejects;
52. duplicate repair path rejects;
53. unsorted repair path list rejects;
54. exact absent new-file repair path is accepted when contract-allowed.

### 28.5 Repair check and re-audit

55. valid attempt-1 repair check;
56. no-change repair rejects;
57. out-of-route delta rejects;
58. every finding requires an intersecting changed path;
59. changed branch rejects;
60. changed HEAD rejects;
61. stale authority rejects;
62. Phase 4 boundary failure rejects;
63. exact `REPAIR_OK` output;
64. repeated identical repair check is byte-preserving idempotent;
65. drift after checked repair rejects;
66. second audit begins from exact checked post subject;
67. second FAIL creates attempt 2;
68. attempt-2 repair check succeeds;
69. third audit PASS terminates success;
70. third audit FAIL becomes final failure;
71. no third repair route is created;
72. commands after terminal PASS reject;
73. commands after terminal final FAIL reject.

### 28.6 Ledger corruption and persistence

74. unknown ledger field rejects;
75. missing ledger field rejects;
76. wrong authority tuple is stale;
77. noncontiguous rounds reject;
78. recomputed audit ID mismatch rejects;
79. recomputed subject hash mismatch rejects;
80. stored report hash or bytes mismatch rejects;
81. impossible nullable-field combination rejects;
82. round after PASS rejects;
83. round after final FAIL rejects;
84. duplicate or skipped repair attempt rejects;
85. later subject not equal prior checked post subject rejects;
86. unsafe `audit-ledger.json` topology rejects;
87. tracked governance-file bypass rejects;
88. first-publication failure leaves no ledger;
89. replacement failure preserves old ledger bytes;
90. temporary collision does not truncate another file;
91. failed command leaves no new temporary file.

### 28.7 Regression and subprocess boundaries

92. all Phase 1–4 tests remain green;
93. existing Phase 4 output and category behavior remains unchanged;
94. Git invocation retains no-network and sanitized-environment controls;
95. no Git mutation command is introduced;
96. no new dependency is introduced.

Tests must prove behavior directly. Test names, comments, aggregate suite success, or source resemblance are not substitutes for executable assertions.

## 29. Allowed implementation paths

Only these repository paths may change for Phase 5:

```text
README.md
src/audit.rs
src/cli.rs
src/error.rs
src/git.rs
src/implementation.rs
src/main.rs
src/state.rs
tests/phase5.rs
docs/contracts/phase-05-contract.md
```

`src/audit.rs` and `tests/phase5.rs` may be created.

No other source, test, manifest, lockfile, contract, plan, agent configuration, or generated artifact is authorized.

## 30. Forbidden paths and operations

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
tests/integration.rs
tests/phase4_obligations.json
tests/phase4_obligations.rs
graphify-out/**
.git/**
.mrgs/**
target/**
```

Generated `graphify-out/**` files may be refreshed as ignored advisory output by the external development workflow, but they are never Phase 5 implementation evidence and must not enter the Git diff.

Do not commit, push, tag, branch, merge, rebase, amend, reset, restore, stash, clean, install global software, edit Git configuration, use the network except Cargo’s already-resolved local behavior, or add future-phase scaffolding.

## 31. Verification

Run in this order:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --test phase5
cargo test
git diff --check
```

Also verify:

```text
git status --short
git diff --name-only
git diff --stat
git diff -- README.md src/audit.rs src/cli.rs src/error.rs src/git.rs src/implementation.rs src/main.rs src/state.rs tests/phase5.rs docs/contracts/phase-05-contract.md
git ls-files graphify-out
```

`git ls-files graphify-out` must produce no tracked Phase 5 artifact.

No verification command may stage or commit files.

## 32. Handoff evidence

The final Phase 5 handoff must contain:

```text
PHASE=5
CONTRACT_VERSION=1
BASELINE_BRANCH=<observed>
BASELINE_HEAD=<observed>
FINAL_HEAD=<observed>
CHANGED_PATHS=<exact>
FORBIDDEN_PATH_CHANGES=<NONE or exact>
TARGETED_TEST=<command and exact result>
FULL_TEST=<command and exact result>
FMT=<exact result>
CHECK=<exact result>
CLIPPY=<exact result>
DIFF_CHECK=<exact result>
AUDIT_CYCLES=<count and verdicts>
REPAIR_CYCLES=<count and exact findings>
GRAPH_RECONNAISSANCE=<SUCCESS|FAILED|SKIPPED>
GRAPH_REFRESH=<SUCCESS|FAILED|SKIPPED>
FINAL_GIT_STATUS=<exact>
BLOCKERS=<NONE or exact>
VERDICT=PASS|FAIL
```

Graph results are advisory and reported separately. A graph failure never changes the implementation verdict and never starts a repair cycle.

`PASS` requires:

- every contract requirement implemented;
- targeted and full verification green;
- independent audit `PASS`;
- no unresolved finding;
- no forbidden path change;
- no commit or push;
- exact final evidence.

## 33. Contract boundary

This contract authorizes Phase 5 implementation only.

It does not authorize Phase 6 closeout, final manifests, completion receipts, phase closure, state recovery, security/adversarial Phase 9 work, adoption readiness, Git integration, commit, or push.

Human review remains required before any Git boundary.
