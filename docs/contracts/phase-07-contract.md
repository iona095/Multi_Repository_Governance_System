# Phase 7 Contract — Model, Host, and Cross-Repository Continuity Metadata

Contract version: 1
Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

## 1. Objective

Extend the Phase 1–6 governance foundation with deterministic, privacy-minimal continuity metadata for completed phases.

Phase 7 implements only:

1. explicit user-supplied model metadata;
2. explicit user-supplied host and execution-surface metadata;
3. local verification of optional cross-repository predecessor links;
4. exact preservation of the submitted continuity source bytes;
5. deterministic continuity manifests and chained continuity receipts;
6. one append-only continuity ledger;
7. byte-preserving idempotent replay;
8. safe coexistence with all Phase 1–6 governance commands and records.

Phase 7 does not execute models, discover providers, inspect model APIs, collect telemetry, read environment variables, infer host identity, persist usernames, persist canonical filesystem paths, read Git remotes, use the network, modify another repository, replace audit or completion authority, recover corruption, stage, commit, push, merge, tag, or automatically advance a phase.

Continuity metadata is descriptive evidence. It is not correctness authority and cannot substitute for an accepted contract, implementation authority, audit `PASS`, final manifest, completion receipt, or closed-phase state.

## 2. Authority and non-gating rule

All Phase 1–6 authority remains controlling:

- accepted plan and exact plan bytes;
- validated state;
- accepted contract and implementation authority for active work;
- Phase 4 implementation boundaries;
- Phase 5 audit history;
- Phase 6 completion ledger, manifests, receipts, and closed-phase ordering.

Phase 7 adds one governance file:

```text
<repo>/.mrgs/continuity-ledger.json
```

The absence of `continuity-ledger.json` must not block any Phase 1–6 command.

A present safe regular continuity ledger is preserved by Phase 1–6 commands and is exempt from Phase 4/5 Git dirtiness only at the exact untracked path `.mrgs/continuity-ledger.json`.

A present unsafe filesystem object at that path is never exempt. Existing commands must fail closed with the existing filesystem-boundary category when they encounter an unsafe continuity-ledger topology through the implementation/audit boundary.

Malformed continuity-ledger content is authoritative only to `continuity record`. Other Phase 1–6 commands do not interpret it and must preserve it byte-for-byte.

## 3. CLI surface

Preserve every existing command and add exactly:

```text
mrgs continuity record --repo <REPOSITORY_PATH> --metadata <METADATA_PATH> [--source-repo <SOURCE_REPOSITORY_PATH>]...
```

`--source-repo` is repeatable.

No other Phase 7 command is authorized. Do not add `continuity begin`, `continuity check`, `continuity import`, `continuity export`, `continuity repair`, `continuity delete`, `host detect`, or `model detect`.

Exact success output:

```text
CONTINUITY_RECORDED <repository_id> <phase_id> <continuity_sequence> <continuity_manifest_sha256> <continuity_receipt_sha256>
```

No success output contains a filesystem path, source-repository path, host username, environment value, model credential, remote URL, or metadata content.

## 4. Common validation and publication order

For a first publication, `continuity record` must fail closed and validate in this order:

1. CLI grammar;
2. canonical target repository path;
3. safe target `.mrgs` directory;
4. accepted plan record, exact plan source bytes, plan SHA-256, and plan structure;
5. state structure and accepted-plan relation;
6. completion-ledger topology, structure, hashes, chain, and state relation;
7. metadata source path and safe regular-file topology;
8. exact metadata bytes, UTF-8, TOML structure, strict fields, and scalar grammar;
9. target completed-phase and completion-receipt binding;
10. existing continuity-ledger topology and complete validation when present;
11. exact replay or conflict detection;
12. source-repository argument set;
13. first-publication cross-repository proof resolution;
14. deterministic continuity-manifest construction;
15. deterministic continuity-receipt construction;
16. atomic continuity-ledger publication;
17. complete post-publication validation;
18. exact output.

Before publication, any failure preserves every target governance byte and leaves no temporary file.

Publication changes only `.mrgs/continuity-ledger.json`.

An exact replay validates the durable target entry and returns its original output without requiring any source repository to remain available.

## 5. Metadata source path

`--metadata` must resolve to an existing regular file:

- inside the canonical target repository;
- outside `.git` and `.mrgs`;
- through ordinary-directory ancestors only;
- with no symlink, junction, reparse-point, device, socket, FIFO, or other unsafe topology;
- with a normalized repository-relative persisted path using `/` separators;
- with no absolute, drive-prefixed, UNC, device-prefixed, backslash, empty, `.`, `..`, doubled-separator, control-character, or non-UTF-8 path form.

The source file may be tracked or untracked. Phase 7 does not modify, delete, stage, or commit it.

Persist its exact bytes, exact normalized repository-relative path, and SHA-256.

## 6. Continuity metadata format

The metadata source is strict TOML with exactly this schema:

```toml
schema_version = 1
repository_id = "mrgs"
continuity_id = "phase-06-primary"
phase_id = "phase-06"
completion_receipt_sha256 = "<lowercase-64-hex>"
note = "Primary governed execution continuity record"

models = [
  {
    role = "implementer",
    provider = "openai",
    model_id = "gpt-5.6",
    execution_mode = "hosted",
    session_label = "phase-06-implementation"
  }
]

hosts = [
  {
    host_id = "main-workstation",
    platform = "windows",
    architecture = "x86_64",
    execution_surface = "opencode"
  }
]

links = [
  {
    relation = "continues_from",
    repository_id = "source-repository",
    accepted_plan_sha256 = "<lowercase-64-hex>",
    phase_id = "source-phase",
    completion_receipt_sha256 = "<lowercase-64-hex>",
    source_continuity_receipt_sha256 = "<optional-lowercase-64-hex>"
  }
]
```

`links = []` is valid.

`source_continuity_receipt_sha256` may be omitted. No other field may be omitted. Unknown fields are rejected at every level.

The exact submitted bytes are preserved. Semantic validation does not normalize or rewrite the source.

## 7. Scalar grammar

All strings must be valid UTF-8, equal their own trimmed form, contain no control character, and respect these limits:

- `repository_id`: 1–128 bytes;
- `continuity_id`: 1–128 bytes;
- `phase_id`: 1–128 bytes;
- `note`: 1–1024 bytes;
- model and host scalar fields: 1–256 bytes;
- cross-repository `repository_id` and `phase_id`: 1–128 bytes.

`repository_id`, `continuity_id`, `role`, `execution_mode`, `session_label`, `host_id`, `platform`, and `architecture` must:

- begin with an ASCII alphanumeric character;
- contain only ASCII alphanumeric characters, `.`, `_`, or `-`;
- contain no slash, backslash, colon, whitespace, shell metacharacter, or path syntax.

`provider`, `model_id`, and `execution_surface` may additionally contain `/`, `:`, `@`, and single internal ASCII spaces, but may not begin or end with whitespace and may not contain repeated whitespace.

All SHA-256 values must be exactly 64 lowercase hexadecimal characters.

The metadata `repository_id` becomes immutable target-repository continuity identity at first publication. Later records must use the same value.

## 8. Model metadata

`models` must contain at least one entry.

Each model entry has exactly:

```text
role
provider
model_id
execution_mode
session_label
```

Entries must be strictly sorted and unique by this tuple:

```text
(role, provider, model_id, execution_mode, session_label)
```

Phase 7 does not validate whether a provider or model exists. It records only the explicit submitted labels.

No model token, API key, endpoint, pricing value, prompt content, response content, or hidden reasoning field is authorized.

## 9. Host metadata

`hosts` must contain at least one entry.

Each host entry has exactly:

```text
host_id
platform
architecture
execution_surface
```

Entries must be strictly sorted by `host_id`. `host_id` must be unique.

Host fields are user-assigned labels. Phase 7 must not call hostname APIs, enumerate hardware, inspect environment variables, inspect users, inspect IP addresses, or infer location.

## 10. Cross-repository links

`links` may be empty.

Each link has exactly:

```text
relation
repository_id
accepted_plan_sha256
phase_id
completion_receipt_sha256
source_continuity_receipt_sha256  # optional
```

`relation` must equal exact lowercase `continues_from`.

Links must be strictly sorted and unique by:

```text
(repository_id, phase_id, completion_receipt_sha256)
```

A link may not name the target `repository_id` and may not resolve to the canonical target repository root.

No source filesystem path is persisted.

## 11. Target completion binding

The target repository must have a valid Phase 6 completion ledger.

The metadata `phase_id` must:

- exist in the accepted plan;
- identify exactly one completion entry;
- appear in `state.closed_phases` in the completion-ledger-consistent order;
- not be merely active or unclosed.

`completion_receipt_sha256` must exactly equal the stored receipt hash for that phase.

The referenced target completion entry must pass complete Phase 6 validation, including:

- final-manifest hash;
- completion-receipt hash;
- previous-receipt chain;
- accepted-plan binding;
- phase identity and completion sequence;
- closed-phase before/after arrays.

The continuity manifest archives the exact target completion receipt object, its hash, and the target final-manifest hash.

## 12. Continuity ordering

A completed phase may have at most one continuity entry.

A `continuity_id` may appear at most once.

The first continuity entry may bind any valid completed phase.

Later entries must bind a strictly greater target `completion_sequence` than the preceding continuity entry. Gaps are allowed. Backfilling an earlier or equal completion sequence after later continuity publication is a conflict.

`continuity_sequence` starts at `1` and increases contiguously by one independent of target completion-sequence gaps.

## 13. Source-repository arguments

For a first publication with `links = []`, no `--source-repo` argument is allowed.

For a first publication with nonempty links:

- exactly one canonical source repository must be supplied for each link;
- every source root must be unique;
- every supplied source must resolve exactly one link;
- every link must resolve exactly one source;
- source argument order is non-authoritative;
- a source must differ from the canonical target root.

For exact replay, source repositories are optional. If supplied, they must still match the durable proof.

Phase 7 reads source repositories only. It performs no source write and no source Git mutation.

## 14. Source completion proof

Each source repository must provide:

- a safe `.mrgs` directory;
- a valid accepted plan and exact accepted-plan source relation;
- a valid state record;
- a valid Phase 6 completion ledger and state relation;
- a continuity repository identity when the link requires a source continuity receipt.

The link fields must exactly match the source authority:

- `repository_id`;
- `accepted_plan_sha256`;
- `phase_id`;
- `completion_receipt_sha256`.

The referenced source completion entry must pass complete Phase 6 hash and chain validation.

The resolved proof stored in the target continuity manifest contains:

- relation;
- source repository ID;
- source accepted-plan SHA-256;
- source plan ID;
- source phase ID;
- source completion sequence;
- source final-manifest SHA-256;
- exact source completion receipt object;
- source completion-receipt SHA-256;
- optional source continuity-manifest SHA-256;
- optional exact source continuity receipt object;
- optional source continuity-receipt SHA-256.

It never stores the source path.

## 15. Optional source continuity proof

When `source_continuity_receipt_sha256` is omitted, the verified source completion receipt is sufficient.

When it is present:

- the source must have a valid continuity ledger;
- the source ledger repository ID must equal the link repository ID;
- the receipt hash must identify exactly one source continuity entry;
- that entry must bind the same source phase and completion receipt;
- all source continuity hashes and previous-receipt links must validate.

The target stores the exact source continuity receipt and its continuity-manifest hash, not the source metadata content.

## 16. Continuity manifest

Each ledger entry contains one `continuity_manifest` with fields in exactly this serialization order:

```text
schema_version
accepted_plan_sha256
plan_id
repository_id
continuity_id
phase_id
target_completion_sequence
target_final_manifest_sha256
target_completion_receipt
target_completion_receipt_sha256
metadata_source_path
metadata_sha256
metadata_content
note
models
hosts
resolved_links
```

`schema_version` is `1`.

`models` and `hosts` preserve the exact validated metadata order.

`resolved_links` preserve the metadata link order after exact proof resolution.

No unlisted field is allowed.

## 17. Continuity-manifest hash

`continuity_manifest_sha256` is SHA-256 over compact UTF-8 JSON serialization of the complete continuity manifest:

- struct declaration order;
- no insignificant whitespace;
- no trailing newline;
- lowercase hexadecimal digest.

Recompute and compare this hash whenever a continuity ledger is read.

## 18. Continuity receipt

Each ledger entry contains one `continuity_receipt` with fields in exactly this order:

```text
schema_version
accepted_plan_sha256
plan_id
repository_id
continuity_sequence
continuity_id
phase_id
target_completion_sequence
target_completion_receipt_sha256
continuity_manifest_sha256
previous_continuity_receipt_sha256
```

`schema_version` is `1`.

For the first entry, `previous_continuity_receipt_sha256` is JSON `null`.

Later entries contain the immediately preceding continuity-receipt hash.

## 19. Continuity-receipt hash

`continuity_receipt_sha256` is SHA-256 over compact UTF-8 JSON serialization of the complete continuity receipt under the same deterministic rules as the manifest hash.

Recompute and compare every receipt hash whenever the ledger is read.

## 20. Continuity ledger

`.mrgs/continuity-ledger.json` has exactly:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "<sha256>",
  "plan_id": "<plan-id>",
  "repository_id": "<repository-id>",
  "entries": [
    {
      "continuity_manifest": {},
      "continuity_manifest_sha256": "<sha256>",
      "continuity_receipt": {},
      "continuity_receipt_sha256": "<sha256>"
    }
  ]
}
```

Top-level and entry unknown fields are rejected.

`entries` must be nonempty when the file exists.

The ledger must validate:

- schema version;
- accepted-plan SHA and plan ID;
- immutable repository ID;
- nonempty ordered entries;
- contiguous continuity sequences;
- strict target completion-sequence increase;
- unique phase IDs;
- unique continuity IDs;
- every manifest hash;
- every receipt hash;
- every manifest-to-receipt binding;
- every previous-receipt link;
- every target completion binding against the current valid completion ledger;
- all required fields, including required JSON `null` for the first previous hash.

Malformed, missing, duplicate, reordered, stale, contradictory, or hash-invalid records are terminal failures for `continuity record`.

## 21. First publication

When no continuity ledger exists, build one entry and atomically publish the complete new ledger.

When a valid ledger exists and the request is a new record, validate the whole existing ledger, append exactly one entry in memory, serialize the complete replacement, and atomically replace the file.

No partial entry, side file, sequence file, cache file, or separate identity file is authorized.

## 22. Idempotency and conflicts

An exact replay requires equality of:

- target accepted-plan authority;
- repository ID;
- continuity ID;
- target phase;
- target completion receipt;
- normalized metadata source path;
- exact metadata bytes and SHA-256;
- parsed note, models, hosts, and links;
- durable resolved source proofs.

Exact replay returns the original output and preserves every byte.

The following are conflicts:

- same phase with different metadata;
- same continuity ID with different phase or metadata;
- changed target completion binding;
- changed metadata source path or bytes;
- changed resolved source proof;
- repository-ID mismatch;
- non-increasing target completion sequence.

No update, overwrite, delete, or replacement of an existing logical entry is authorized.

## 23. Filesystem and persistence safety

The target `.mrgs` directory, metadata source, continuity ledger, and all source governance paths must be validated with `symlink_metadata` or an equivalent no-follow proof.

Reject:

- symlink or junction leaves;
- reparse points;
- non-regular ledger or metadata leaves;
- unsafe ancestors;
- canonical escape;
- metadata under `.git` or `.mrgs`;
- tracked `.mrgs/continuity-ledger.json`;
- case-alias governance paths;
- temporary or child paths masquerading as the fixed ledger.

Publication must:

1. serialize completely before opening a file;
2. create a unique same-directory temporary file with create-new semantics;
3. never truncate an existing colliding path;
4. write exact bytes and flush;
5. atomically publish or replace the final file;
6. remove only the temporary file created by this command after a handled failure;
7. preserve the prior ledger bytes on replacement failure;
8. leave no command-created temporary file after success.

Persist pretty JSON without a trailing newline, consistent with existing governance files.

## 24. Privacy and non-observation boundary

Phase 7 must not automatically collect or persist:

- hostname;
- username;
- home directory;
- target or source canonical root;
- source CLI path;
- environment variables;
- API keys or tokens;
- model prompts or responses;
- hidden reasoning;
- IP or MAC addresses;
- geolocation;
- hardware serial numbers;
- Git remote URLs;
- network-derived information.

Only exact fields present in the explicit metadata source and verified governance proofs may enter the continuity ledger.

## 25. Error categories

Add exact stable categories:

- `CONTINUITY_METADATA_INVALID` for source structure, field, ordering, grammar, or semantic errors;
- `CONTINUITY_LEDGER_INVALID` for malformed structure, missing fields, hash failures, receipt-chain failures, or duplicate/reordered entries;
- `CONTINUITY_LEDGER_STALE` for a structurally valid ledger bound to different accepted-plan or completion authority;
- `CONTINUITY_CONFLICT` for phase, identity, replay, ordering, or existing-entry conflicts;
- `CONTINUITY_SOURCE_INVALID` for an invalid or unsafe source repository;
- `CONTINUITY_SOURCE_MISMATCH` for source arguments or link proof mismatches.

Existing path, filesystem, governance-authority, completion-ledger, Git, and persistence categories remain controlling when more specific.

No failure prints metadata content, source paths, report content, environment values, raw Git stderr, or a backtrace.

## 26. Dependencies

No new production or development dependency is authorized.

Continue using only dependencies already present in `Cargo.toml`.

Do not add an async runtime, Git library, database, HTTP client, UUID library, time library, hostname library, system-information library, logging framework, model SDK, or telemetry framework.

## 27. Required tests

Add focused Phase 7 tests in:

```text
tests/phase7.rs
```

Do not add a generated obligation registry. Do not invoke `cargo test` recursively from a test.

Required direct executable coverage includes exactly these 80 obligations.

### 27.1 CLI, source path, and metadata structure

1. exact `continuity record` CLI parsing without source repositories;
2. repeated `--source-repo` parsing preserves arguments;
3. missing or unknown CLI arguments reject;
4. metadata outside target repository rejects;
5. metadata under `.git` or `.mrgs` rejects;
6. symlink, junction, or unsafe metadata topology rejects with an executed platform branch;
7. invalid UTF-8 metadata rejects;
8. malformed TOML rejects;
9. unknown top-level, model, host, and link fields reject;
10. every required top-level and nested field is enforced;
11. unsupported schema version rejects;
12. scalar trimming, control, length, token, whitespace, and SHA grammar are enforced.

### 27.2 Model and host metadata

13. one valid model and host entry is accepted;
14. multiple strictly sorted model and host entries are preserved exactly;
15. zero models rejects;
16. zero hosts rejects;
17. unsorted models reject;
18. duplicate models reject;
19. invalid model role, provider, ID, mode, and session-label forms reject;
20. unsorted hosts reject;
21. duplicate host IDs reject;
22. invalid host ID, platform, architecture, and execution-surface forms reject.

### 27.3 Target completion binding

23. valid closed phase and exact receipt are accepted;
24. unknown phase rejects;
25. active or unclosed phase rejects;
26. wrong target completion-receipt hash rejects;
27. malformed target completion ledger rejects;
28. target completion ledger bound to a different plan rejects as stale;
29. completed phase missing from closed-state relation rejects;
30. target completion-receipt hash mismatch rejects;
31. target final-manifest hash mismatch rejects;
32. invalid target completion ordering or receipt chain rejects.

### 27.4 Cross-repository resolution

33. empty links with zero source arguments is accepted;
34. nonempty links require source repositories on first publication;
35. an unreferenced supplied source repository rejects;
36. duplicate canonical source roots reject;
37. source equal to target rejects;
38. invalid or unsafe source accepted-plan authority rejects;
39. invalid source completion ledger or state relation rejects;
40. source repository-ID mismatch rejects;
41. source accepted-plan SHA mismatch rejects;
42. missing source phase rejects;
43. source completion-receipt mismatch rejects;
44. source final-manifest or receipt-chain mismatch rejects;
45. omitted source continuity receipt is accepted with completion proof only;
46. present valid source continuity receipt is resolved and archived exactly;
47. missing, stale, or mismatched source continuity receipt rejects;
48. link relation, sorting, uniqueness, and one-to-one source resolution are enforced.

### 27.5 Manifest, receipt, and ledger

49. valid continuity manifest has exact required fields and order;
50. metadata source path, exact bytes, and SHA are preserved;
51. note, models, and hosts are preserved exactly;
52. resolved links store exact proof fields and no source path;
53. continuity-manifest bytes and hash are deterministic;
54. first continuity receipt uses JSON null previous hash;
55. later receipt chains to the immediately preceding receipt hash;
56. continuity sequence is contiguous from one;
57. target completion sequence and receipt binding are exact;
58. continuity-receipt bytes and hash are deterministic;
59. ledger top-level fields and immutable repository ID are exact;
60. reordered entries reject;
61. duplicate phase or continuity ID rejects;
62. broken manifest hash, receipt hash, binding, or previous link rejects.

### 27.6 Publication, idempotency, and conflicts

63. successful first record prints exact output;
64. first publication creates only `continuity-ledger.json`;
65. exact replay returns identical output and preserves all bytes;
66. exact replay with cross-links succeeds without source repositories;
67. same continuity ID with changed metadata rejects;
68. same phase with a different continuity ID rejects;
69. earlier or equal target completion sequence after later continuity rejects;
70. temporary collision does not truncate an existing file;
71. replacement failure preserves previous continuity-ledger bytes;
72. success and handled failure leave no command-created temporary file.

### 27.7 Safety and Phase 1–6 regression

73. unsafe target continuity-ledger topology rejects;
74. unsafe source completion or continuity-ledger topology rejects;
75. implementation and audit boundaries exempt only the exact safe untracked continuity-ledger path;
76. no source path, canonical root, Git remote, environment value, or automatically observed host value is persisted;
77. phase selection, contract lifecycle, implementation, audit, repair, and closeout preserve a safe continuity ledger;
78. representative Phase 1–6 success outputs and error categories remain unchanged;
79. no Git mutation, network command, model invocation, hostname query, or environment enumeration is executed;
80. no new dependency and no recursive `cargo test` invocation is required.

Platform-dependent topology tests must execute the supported branch. A capability-unavailable branch must contain an explicit capability assertion and a concrete fallback safety assertion. Silent omission is not coverage.

## 28. Verification

Required targeted and full verification:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo test --test phase7
cargo clippy --all-targets -- -D warnings
cargo test
git diff --check
```

Run the narrowest affected Phase 7 test after each repair before rerunning broader verification.

Do not place a full-suite `cargo test` invocation inside `tests/phase7.rs`; the external verification ladder is authoritative.

A timeout, truncated output, skipped command, masked exit code, or unexecuted command is not a pass.

## 29. Allowed implementation paths

Only these repository paths may change for Phase 7:

```text
README.md
src/cli.rs
src/closeout.rs
src/continuity.rs
src/error.rs
src/implementation.rs
src/main.rs
src/state.rs
tests/phase7.rs
docs/contracts/phase-07-contract.md
```

Create only:

```text
src/continuity.rs
tests/phase7.rs
```

The supplied Phase 7 contract is frozen and may only remain as the exact supplied file.

No other source, test, manifest, lockfile, contract, plan, generated artifact, or configuration is authorized.

## 30. Forbidden changes

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
graphify-out/**
.git/**
.mrgs/**
target/**
```

Do not weaken, delete, rename, ignore, or replace an existing test.

Do not add dependencies, features, build scripts, examples, benchmarks, generated registries, external fixtures, hidden runtime configuration, telemetry, or automatic host/model detection.

## 31. Final evidence

The final handoff must include:

- baseline branch and `HEAD`;
- final `HEAD`, which must remain unchanged;
- exact changed and created paths;
- forbidden-path result;
- exact targeted and full test summaries per binary;
- formatting, check, clippy, and diff-check results;
- exact Phase 7 obligation coverage count `80/80`;
- continuity audit verdict and repair-cycle count;
- Graphify reconnaissance and refresh status as advisory evidence only;
- final Git status and staged-path list;
- confirmation that no commit or push occurred;
- exact blockers or `NONE`;
- recommendation for the human Git boundary.

## 32. Completion rule

Phase 7 implementation is complete only when:

1. all contract requirements are implemented;
2. all 80 required tests contain direct executable assertions;
3. all required verification commands pass;
4. all Phase 1–6 tests remain green;
5. no forbidden path changes;
6. the frozen contract is unchanged;
7. no file is staged;
8. no commit or push occurred;
9. a final read-only audit maps all 80 obligations to direct, non-vacuous evidence and returns `PASS`.

Phase 7 authorizes continuity metadata only. It does not authorize Phase 8 recovery, Phase 9 adversarial expansion, Phase 10 activation readiness, Git integration, commit, or push.
