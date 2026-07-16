# Phase 4 Contract — Contract-Bound Implementation Enforcement

Contract version: 2
Project: Multi-Repository Governance System
Implementation language: Rust
Binary name: `mrgs`

Revision note: Version 2 preserves every non-superseded Version 1 requirement and resolves four Git-inspection blockers identified by independent GPT-5.6 Terra audits: unchanged tracked governance paths are now rejected from complete sparse-preserving index inspection; inherited `GIT_CONFIG_PARAMETERS` and `GIT_SHALLOW_FILE` are removed; every Git child disables lazy promisor-object fetching; and sparse-checkout and sparse-index states are rejected from both effective configuration and structural evidence.

## 1. Objective

Extend the complete Phase 1–3 governance foundation with deterministic implementation-boundary authorization for one exact accepted contract and one exact Git baseline.

The implementation authority answers only:

> Which exact accepted contract authorizes which repository changes from which exact Git baseline?

Phase 4 implements only:

1. exact binding to the validated accepted plan, active phase, final accepted contract revision, accepted source path, accepted SHA-256, accepted exact stored content, Git baseline commit, and Git baseline branch;
2. deterministic interpretation and enforcement of accepted `allowed_paths` and `forbidden_paths`;
3. complete inventory of Git-visible changes from the bound baseline through the current repository state;
4. rejection of stale, malformed, inconsistent, unsafe, or out-of-bound implementation authority;
5. read-only reporting that the current change boundary is valid.

Phase 4 does not execute implementation work and does not determine whether an implementation is correct. A successful boundary check is not an audit verdict and is never an audit `PASS`. Correctness audit belongs to Phase 5.

## 2. Controlling authority and lifecycle

Phase 4 preserves every non-superseded Phase 1–3 requirement. The controlling governance files are:

```text
<repo>/.mrgs/accepted-plan.json
<repo>/.mrgs/state.json
<repo>/.mrgs/contract-draft.json
<repo>/.mrgs/accepted-contract.json
```

Every Phase 4 command must reuse the complete Phase 3 authority-validation path. It must validate the accepted plan, recorded plan source, state, active phase, contract draft, draft preimage when required, accepted-contract ledger, every accepted revision, all exact-content hashes, all cross-record identities, and the inferred lifecycle before performing Phase 4-specific work.

The lifecycle rules are explicit:

1. `DRAFT` cannot authorize implementation. A valid draft without `accepted-contract.json` must be rejected.
2. `ACCEPTED` can authorize implementation. The final accepted ledger entry is the only possible authority and exactly equals the current draft under the Phase 3 lifecycle rules.
3. `REVISION_DRAFT` can authorize implementation, but only from the final accepted ledger entry. The newer unaccepted draft is never implementation authority.
4. In `REVISION_DRAFT`, the requested revision and SHA-256 must identify the final accepted ledger entry, not the current draft.
5. Further valid unaccepted draft revisions do not replace or stale an implementation binding while the same final accepted ledger entry remains authoritative.
6. Acceptance of any later revision immediately makes a binding to an older final accepted revision stale.
7. No historical accepted entry other than the current final accepted entry can authorize a new or continuing Phase 4 binding.

The accepted ledger's final entry supplies the authoritative `revision`, `source_path`, `sha256`, and exact `content`. The original contract source file need not exist and is never reloaded for Phase 4 authority. The ledger-stored content is strictly parsed as the Phase 2 contract model and validated again for enforcement use.

## 3. CLI surface

Preserve all existing commands and add exactly:

```text
mrgs implementation begin --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256>
mrgs implementation check --repo <REPOSITORY_PATH>
```

No other Phase 4 command is authorized. In particular, there is no execute, run, apply, reset, rebind, repair, audit, close, commit, or push command.

The complete CLI after Phase 4 is:

```text
mrgs plan accept --repo <REPOSITORY_PATH> --plan <PLAN_PATH>
mrgs phase select --repo <REPOSITORY_PATH> --phase <PHASE_ID>
mrgs contract draft --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH>
mrgs contract accept --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256> --decision <DECISION>
mrgs contract revise --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH> --expected-revision <REVISION> --expected-sha256 <SHA256>
mrgs implementation begin --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256>
mrgs implementation check --repo <REPOSITORY_PATH>
```

`REVISION` is canonical unsigned decimal text in the range `1` through `4294967295`: ASCII digits only, with no sign, whitespace, separator, or leading zero. Parsing must retain and validate the original token before conversion to `u32`. `SHA256` is exactly 64 lowercase ASCII hexadecimal characters. No trimming, case folding, Unicode normalization, abbreviation, alternate digest syntax, or revision coercion is permitted.

## 4. Common Phase 4 validation order

After successful CLI argument parsing, both Phase 4 commands must validate in this order and stop at the first failure:

1. canonicalize `--repo` as an existing directory without lossy path conversion;
2. validate the existing direct-child `.mrgs` directory and reject a symlink, junction, reparse point, non-directory, or canonical escape;
3. require the Phase 1 authority pair and reject incomplete accepted-plan/state authority;
4. strictly load and validate accepted-plan and state authority according to all existing rules;
5. safely reload the recorded plan, decode strict UTF-8, parse and validate it, recompute its exact SHA-256, and validate every plan/state cross-record relation;
6. require a valid non-closed active phase with all dependencies closed;
7. require `contract-draft.json`, validate its exact Phase 3 shape and content, and reject an orphaned accepted ledger or any incomplete contract authority;
8. require `accepted-contract.json`, strictly validate the entire ledger and all entries, and infer `ACCEPTED` or `REVISION_DRAFT`; a `DRAFT` lifecycle fails here;
9. identify the final accepted ledger entry and strictly parse and validate its exact stored contract content;
10. validate the final accepted contract's enforcement path lists under Section 10 and validate all other Phase 2 contract fields, including `verification_commands` and `handoff_fields`;
11. validate the Git root, attached `HEAD`, object format, and full current commit under Sections 6.1 and 6.2;
12. reject every in-progress Git operation under Section 6.3;
13. validate the complete index structure, reject tracked governance content, gitlinks, conflicts, sparse-directory entries, and unsupported index flags under Section 6.4;
14. validate both effective sparse configuration signals under Section 6.4.

Common Section 4 validation ends at step 14. Sections 7 and 8 define the command-specific continuation; they do not recursively form part of the already-completed common validation. The complete ordering is normative: existing Phase 1–3 authority validation remains first; Git root and attached-`HEAD` validation follows authority; then operation-state, index structure, sparse configuration, command-specific implementation-record handling, begin cleanliness or check baseline/inventory, path enforcement, and persistence. Within index structural validation, conflict classification precedes other malformed-inventory classifications; tracked `.mrgs`, sparse-directory, unsupported-index-flag, and malformed sparse evidence map to `GIT_INVENTORY_INVALID`; gitlinks retain `GIT_SUBMODULE_UNSUPPORTED`.

Every governance authority file must be a direct-child regular file of the validated `.mrgs` directory. A governance authority file that is itself a symlink, junction, reparse point, directory, or other non-regular object is invalid. Existing Phase 1–3 field compatibility remains controlling. Phase 4 may additionally reject an otherwise valid accepted value when it cannot be represented safely in a Phase 4 interface; this is enforcement suitability, not mutation of the earlier record. Phase 4-specific records are strict and reject unknown fields.

For deterministic one-line output, the accepted `contract_id` used by Phase 4 must match the ASCII grammar `[A-Za-z0-9][A-Za-z0-9._-]*`. A valid Phase 1–3 contract ID outside that grammar remains valid historical authority but cannot authorize Phase 4 implementation. It is rejected without normalization or rewriting.

No command may succeed when any existing authority is malformed, incomplete, inconsistent, stale, or unsafe. Validation never repairs, rewrites, deletes, truncates, normalizes, or substitutes authority.

## 5. Implementation authority record

Phase 4 adds exactly one governance record:

```text
<repo>/.mrgs/implementation-authority.json
```

It is a direct child of the already validated `.mrgs` directory. Its destination filename is fixed in code. No CLI argument, contract field, Git value, or other externally controlled value may select or alter the destination filename.

The strict JSON schema has exactly these fields in exactly this serialization order:

```json
{
  "schema_version": 1,
  "accepted_plan_sha256": "lowercase-hex",
  "phase_id": "phase-4",
  "contract_id": "phase-4-contract-v1",
  "contract_revision": 1,
  "contract_source_path": "docs/contracts/runtime-phase-4.toml",
  "contract_sha256": "lowercase-hex",
  "contract_content": "exact accepted UTF-8 contract content",
  "git_object_format": "sha1",
  "baseline_head": "full-lowercase-git-commit-sha",
  "baseline_branch": "main"
}
```

Record rules:

1. `schema_version` is exactly `1`.
2. Unknown fields and missing fields are rejected.
3. `accepted_plan_sha256` exactly equals the completely validated accepted-plan SHA-256.
4. `phase_id` exactly equals the validated active phase, ledger phase, final accepted content phase, and draft phase.
5. `contract_id` exactly equals the ledger ID, final accepted content ID, and draft ID.
6. `contract_revision` is positive and exactly equals the final accepted ledger revision.
7. `contract_source_path` exactly equals the final accepted ledger source path and satisfies the Phase 3 strict normalized source-path rules.
8. `contract_sha256` is lowercase 64-character hexadecimal, exactly equals the final accepted ledger SHA-256, and equals SHA-256 over `contract_content.as_bytes()`.
9. `contract_content.as_bytes()` exactly equals the final accepted ledger content bytes represented by its JSON string.
10. `contract_content` strictly parses and validates as the Phase 2 contract model, including enforcement-specific path validation.
11. `git_object_format` is structurally exactly `sha1` or `sha256`. Command-specific contextual validation separately requires it to equal validated `git rev-parse --show-object-format` output.
12. `baseline_head` is the full lowercase commit ID captured from Git at begin time: 40 hexadecimal characters for `sha1` and 64 for `sha256`. Abbreviated IDs and lengths inconsistent with `git_object_format` are invalid.
13. `baseline_branch` is the exact strict UTF-8 attached branch name captured from Git at begin time. It is non-empty and contains no line terminator.

The exact accepted content and source path are persisted because they are part of the accepted revision tuple and make the authorization self-contained and exact-byte comparable. `allowed_paths`, `forbidden_paths`, and `verification_commands` are not duplicated as separate record fields. They are deterministically derived by strictly parsing `contract_content` on every validation, avoiding two representations that could diverge.

Serialization uses UTF-8 JSON, two-space indentation, the field order shown above, JSON escaping, `\n` line endings between JSON lines, no trailing spaces, and no final newline. Serialization must complete in memory before destination mutation.

The record contains no timestamps, usernames, hostnames, model names, random IDs, absolute paths, environment-specific metadata, decision-maker identities, signatures, or nondeterministic values. Temporary filenames used only during atomic persistence are not record content.

Add `implementation-authority.json` to the fixed governance destination-filename allowlist alongside the four existing governance filenames.

## 6. Git baseline authority

Phase 4 may invoke the installed `git` executable only for read-only inspection. It must never mutate Git, the index, refs, configuration, worktree, object database, or repository metadata.

### 6.1 Git subprocess requirements

Every Git invocation must:

1. invoke `git` directly through `std::process::Command`, never through a shell;
2. clear the inherited child environment before constructing the final child environment, then restore only the operating-system variables required to start the resolved executable;
3. explicitly remove inherited `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_NAMESPACE`, `GIT_PREFIX`, `GIT_QUARANTINE_PATH`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_PARAMETERS`, `GIT_CONFIG_COUNT`, `GIT_SHALLOW_FILE`, and every inherited `GIT_CONFIG_KEY_*` and `GIT_CONFIG_VALUE_*` variable before adding approved Git variables;
4. set exactly the approved controls `GIT_OPTIONAL_LOCKS=0`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_ATTR_NOSYSTEM=1`, and `GIT_NO_LAZY_FETCH=1`;
5. use this fixed global-option order before every subcommand:

```text
git --no-replace-objects --no-lazy-fetch --literal-pathspecs -c core.fsmonitor=false -c core.untrackedCache=false -c diff.external= -C <canonical-repo> <subcommand> <arguments>
```

6. provide null stdin and capture stdout, stderr, and exit status;
7. pass `--no-ext-diff` to diff commands, never request text conversion, and never invoke an external diff, pager, editor, hook, credential helper, filesystem monitor, remote helper, or network command;
8. use `-z` output for path-bearing commands;
9. treat spawn failure, signal termination, non-zero exit where success is required, malformed output, unexpected output, or non-UTF-8 textual metadata as fatal;
10. never run a command supplied by contract content;
11. never change local, global, system, or worktree Git configuration;
12. never retry a failed command with replacement objects, lazy fetching, inherited configuration injection, inherited shallow boundaries, external execution, or optional locks enabled.

The executable is resolved as `git` from the MRGS process's existing executable search path. Environment removal occurs before the final allowlisted environment is built. `GIT_CONFIG_PARAMETERS` cannot inject higher-precedence configuration and `GIT_SHALLOW_FILE` cannot replace the repository's own shallow-boundary metadata. The repository's ordinary local and worktree Git metadata remains authoritative after inherited overrides are removed. Any failure caused by that actual metadata is handled under the deterministic error model.

`GIT_CEILING_DIRECTORIES`, `GIT_DISCOVERY_ACROSS_FILESYSTEM`, `GIT_ATTR_SOURCE`, `GIT_REPLACE_REF_BASE`, and `GIT_INDEX_VERSION` require no separate semantic rule: canonical `-C` fixes discovery, attributes cannot enable prohibited external execution, replacement objects are disabled, and index structure is parsed directly. The environment allowlist omits unapproved inherited Git variables without creating an alternate authority path.

Path-bearing `-z` output is parsed as bytes before strict UTF-8 validation. Locale-dependent prose is never parsed. No Git stderr is forwarded into the Phase 4 semantic error.

### 6.1.1 Absolute no-network and promisor-object rule

Every commit, tree, and blob needed by an authorized Git command must already exist locally. `GIT_NO_LAZY_FETCH=1` and `--no-lazy-fetch` are mandatory on every Git child, including `rev-parse`, `merge-base`, `diff`, `cat-file`, `ls-tree`, `ls-files`, `status`, and `config` invocations. MRGS must never invoke `fetch`, `fetch-pack`, a remote helper, a credential helper, or any networking command.

A partial-clone or promisor repository is not rejected merely because promisor configuration exists. It may be inspected only when every required object is local. A missing promised commit, tree, or blob must fail locally without demand-fetch, retry, helper execution, or fallback. Missing promised objects, including objects required by ancestry, raw diff, copy/rename detection, or symlink-target inspection, map to `GIT_COMMAND_FAILED`. The semantic stderr remains exactly the category line and contains no remote name, URL, helper output, credential text, or Git environment-specific diagnostic.

For deterministic baseline error categorization, check must run `git config --get extensions.partialClone` and `git config --get-all extensions.partialClone` under the same isolated Git-child template before resolving `baseline_head`. Unset means no promisor marker; one non-empty strict UTF-8 line consistently returned by both commands means promisor-enabled; empty, malformed, multi-line, multi-valued, disagreeing, or failed configuration evidence is fatal under the ordinary configuration-evidence rules. If `baseline_head^{commit}` cannot be resolved while that marker is present, the result is `GIT_COMMAND_FAILED`; the same unresolved recorded commit without that marker is `BASELINE_COMMIT_MISSING`. The marker's presence never rejects a repository whose required objects are local. Any later required tree or blob lookup failure remains `GIT_COMMAND_FAILED` regardless of marker state because inventory or content inspection did not complete.

### 6.2 Repository and branch validation

Both commands must prove:

1. `git rev-parse --is-inside-work-tree` returns exactly `true` plus an optional single line terminator;
2. `git rev-parse --show-toplevel` yields a path whose canonical form exactly equals canonical `--repo`;
3. `git symbolic-ref --quiet --short HEAD` succeeds and returns one non-empty strict UTF-8 branch name without embedded CR or LF;
4. `git rev-parse --show-object-format` returns exactly `sha1` or `sha256` plus an optional single line terminator;
5. `git rev-parse --verify HEAD^{commit}` returns exactly one full lowercase commit ID of the length required by that object format;
6. strict parsing of `git ls-files --sparse --stage -z` completes without conflict, tracked governance path, gitlink, sparse-directory entry, malformed entry, or unsupported mode under Section 6.4;
7. strict parsing of `git ls-files -v -z` finds no lowercase assume-unchanged tag and no uppercase `S` skip-worktree tag;
8. effective `core.sparseCheckout` and `index.sparse` validation under Section 6.4 proves neither sparse mode is active.

Repositories containing a tracked submodule gitlink are unsupported and rejected by both commands. Phase 4 does not recurse into submodules, ignore submodule dirtiness, or treat a submodule as one ordinary file. An untracked nested repository is handled as untracked content by the parent repository's Git status.

Assume-unchanged, skip-worktree, sparse-checkout, sparse-index, and tracked `.mrgs` repository content are unsupported because they can conceal or confuse implementation authority. Detection is required during both begin and check and does not depend on whether a tracked path is changed relative to the baseline.

### 6.3 In-progress operation rejection

Both commands must resolve Git paths through `git rev-parse --git-path <marker>` and reject if any of these operation markers exists:

```text
MERGE_HEAD
CHERRY_PICK_HEAD
REVERT_HEAD
BISECT_LOG
BISECT_START
rebase-apply
rebase-merge
sequencer
```

Existence of any marker is sufficient; its content is not interpreted. Failure to resolve or inspect a marker is fatal. This covers merge, cherry-pick, revert, bisect, rebase, `git am`, and sequencer-driven operations. Phase 4 never removes a marker or attempts recovery.

### 6.4 Index structure and sparse-state validation

Both begin and check must perform this validation after Git root, attached `HEAD`, and operation-state validation and before any untracked or ignored governance-path exemption.

#### 6.4.1 Sparse-preserving index inspection

Run and strictly parse:

```text
git ls-files --sparse --stage -z
```

`--sparse` is mandatory. An invocation that can silently expand a sparse index before its structure is inspected is not an authorized substitute.

For every record, in output order:

1. validate the complete record structure, six-digit octal mode, full lowercase object ID of the repository's object-format length, stage token, strict UTF-8 path encoding, and repository-relative component safety; defer ordinary-file trailing-slash rejection until after mode classification;
2. classify stage `1`, `2`, or `3` as `GIT_CONFLICT`; classify any stage other than `0`, `1`, `2`, or `3` as `GIT_INVENTORY_INVALID`;
3. classify stage-0 mode `160000` as `GIT_SUBMODULE_UNSUPPORTED`;
4. for stage-0 mode `040000`, require the sparse-directory path form to be non-empty, repository-relative, `/`-separated, component-safe, and terminated by exactly one `/`, then classify it as structural sparse-directory evidence and `GIT_INVENTORY_INVALID`; malformed sparse-directory path form is also `GIT_INVENTORY_INVALID`;
5. accept only ordinary stage-0 modes `100644`, `100755`, and `120000` after the preceding classifications and require their paths to satisfy the ordinary no-trailing-slash Git path syntax;
6. for a valid ordinary stage-0 entry, before any governance exemption, reject every tracked path whose first segment is `.mrgs` under ASCII case-insensitive comparison and map it to `GIT_INVENTORY_INVALID`;
7. apply tracked-governance rejection to all five fixed governance paths, unknown governance paths, temporary-file-shaped paths, nested paths, clean unchanged paths, paths already present in the baseline, paths added after the baseline, and every host-expressible case alias such as `.MRGS/state.json`;
8. never exempt a tracked governance path because its bytes form a valid governance record.

All tracked `.mrgs` repository content is prohibited, not only tracked changes beneath `.mrgs`. An index entry proves tracked status even when status, ignored-file enumeration, and baseline-to-`HEAD` diff are empty. The complete index inspection is therefore authoritative for unchanged tracked governance detection.

Strict parsing of `git ls-files -v -z` follows the same structural pass. Uppercase `S` rejects skip-worktree state; any lowercase tag rejects assume-unchanged state; malformed tag output rejects. These map to `GIT_INVENTORY_INVALID`.

#### 6.4.2 Effective sparse-checkout configuration

Run:

```text
git config --type=bool --get core.sparseCheckout
git config --type=bool --get-all core.sparseCheckout
```

The `--get` result is interpreted exactly:

1. exit `0` with exact strict UTF-8 `true` plus one optional line terminator means sparse checkout is active and fails with `GIT_INVENTORY_INVALID`;
2. exit `0` with exact strict UTF-8 `false` plus one optional line terminator does not reject by itself;
3. exit `1` with empty stdout and stderr means unset and does not reject by itself;
4. malformed boolean output, multiple lines, non-UTF-8 output, unexpected stdout or stderr, or any other exit is deterministic failure;
5. spawn, signal, or command-execution failure maps to `GIT_COMMAND_FAILED`; successfully produced but malformed or multi-valued configuration evidence maps to `GIT_INVENTORY_INVALID`.

The `--get-all` companion must obey the same byte and exit discipline and proves cardinality. More than one value is rejected as multi-valued `GIT_INVENTORY_INVALID`, even when the final effective value would be `false`. Zero values must agree with unset `--get`; one value must exactly agree with `--get`.

#### 6.4.3 Effective sparse-index configuration

Run:

```text
git config --type=bool --get index.sparse
git config --type=bool --get-all index.sparse
```

Apply exactly the Section 6.4.2 exit, output, cardinality, agreement, and category rules. Effective `true` rejects with `GIT_INVENTORY_INVALID`; exact `false` or unset does not reject by itself.

Configuration and structural evidence are independent. Mode `040000` rejects even when `index.sparse` is unset or false. Active sparse configuration rejects even when the worktree is temporarily expanded or all skip-worktree bits are cleared. Sparse rejection never relies solely on uppercase `S` output.

### 6.5 Begin-time cleanliness

`implementation begin` must run:

```text
git status --porcelain=v1 -z --untracked-files=all --ignore-submodules=none --renames
git ls-files --others --ignored --exclude-standard -z --
```

The exact five fixed governance paths are:

```text
.mrgs/accepted-plan.json
.mrgs/state.json
.mrgs/contract-draft.json
.mrgs/accepted-contract.json
.mrgs/implementation-authority.json
```

Strictly parse all status records before applying an exemption or generic dirty classification. Any tracked porcelain `XY` record whose source or destination path has `.mrgs` as its first segment under ASCII case-insensitive comparison maps to `GIT_INVENTORY_INVALID`; this includes a staged deletion whose path no longer has a stage-0 index entry. Conflict status retains `GIT_CONFLICT` under the earlier conflict precheck and internal index precedence. After their corresponding files have been completely validated, the exact fixed paths are removed from `??` status and ignored-untracked consideration only when Section 6.4 proved that no tracked index entry exists for the path. Governance authority is not implementation content, but tracked governance is always invalid repository content. At first begin, the implementation path is absent; during idempotent begin it must already have passed complete record validation. No other `.mrgs` path is exempt. A tracked governance path, unknown governance path, temporary file, directory summary, or malformed status record is not exempt and fails.

Any remaining status or ignored-untracked path means the repository is dirty and begin fails. Therefore begin rejects staged changes, unstaged changes, tracked deletions, type changes, conflicts, all non-governance untracked files, and all non-governance ignored untracked files. This rule does not depend on `.mrgs` being listed in `.gitignore` and prevents an ignore-rule change from hiding implementation content.

The validated current `HEAD` and attached branch become `baseline_head` and `baseline_branch`. They are captured only after complete authority, path-rule, Git-root, submodule, operation-state, and cleanliness validation.

### 6.6 Check-time baseline relation

`implementation check` must require:

1. the current Git object format exactly equals `git_object_format` before interpreting `baseline_head`;
2. the current branch is attached and exactly equals `baseline_branch` using case-sensitive byte equality;
3. `baseline_head^{commit}` still resolves to the exact recorded commit object;
4. current `HEAD` is a full lowercase commit SHA of the current object format;
5. `git merge-base --is-ancestor <baseline_head> HEAD` exits successfully;
6. no Git operation is in progress and Section 6.4 index and sparse-state validation remains satisfied.

Current `HEAD` may equal the baseline or may be a descendant created by an external workflow. A branch-name change fails even when it points to the same commit. Detached `HEAD` fails. A missing baseline object, rewritten branch history in which the baseline is no longer an ancestor, or replacement with a different baseline fails. Phase 4 performs no silent rebase, baseline update, reset, checkout, or record replacement.

## 7. `implementation begin`

The command is:

```text
mrgs implementation begin --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256>
```

After completing the common validation in Section 4, begin must proceed in this order:

1. require the lifecycle to be `ACCEPTED` or `REVISION_DRAFT`;
2. select only the final accepted ledger entry;
3. require supplied `revision >= 1`;
4. require the supplied revision to exactly equal the final accepted revision;
5. require the supplied SHA-256 to be exactly 64 lowercase ASCII hexadecimal characters;
6. require the supplied SHA-256 to exactly equal the final accepted SHA-256;
7. compare exact final accepted content, source path, identity, phase, plan SHA, and recomputed content SHA;
8. validate `allowed_paths` and `forbidden_paths` for enforcement use;
9. inspect the fixed implementation-record path; if it exists, strictly parse it, validate its complete accepted-authority tuple, validate all record fields structurally including internal object-format/ID-length consistency, and make that validated result available to the governance-path exemption in Section 6.5; do not yet compare its baseline fields with current Git values;
10. complete Section 6.5, exempting an existing implementation record only when step 9 completely validated it and Section 6.4 proved it is not tracked;
11. capture the exact clean `HEAD`, object format, and branch, then contextually compare them with an existing record under Section 7.1;
12. construct the exact record from validated authority and Git values;
13. create only `implementation-authority.json` when it does not exist, or apply Section 7.1 when step 9 found it;
14. preserve all existing governance files byte-for-byte and modify no source or Git file.

A stale requested revision fails even if it identifies an earlier accepted ledger entry. A stale requested SHA fails even if it belongs to an earlier accepted revision or the current unaccepted draft.

On first success, stdout is exactly one line:

```text
IMPLEMENTATION_BOUND <contract_id> <revision> <sha256> <baseline_head>
```

The line ends with one platform newline emitted by the CLI. There is no additional stdout.

### 7.1 Existing record and idempotency

If `implementation-authority.json` already exists, step 9 above must strictly parse and completely validate it before cleanliness validation or any idempotency decision. There is no circular dependency: accepted-authority equality and record syntax are validated first; current branch/HEAD equality is validated after Section 6.5 captures the clean current Git values.

Idempotent success is allowed only when:

1. the existing record is completely valid;
2. every record authority field exactly equals the currently final accepted ledger entry and validated plan/phase authority;
3. the supplied revision and SHA exactly equal that same final accepted entry;
4. the current repository is clean under Section 6.5;
5. current branch exactly equals `baseline_branch`;
6. current `HEAD` exactly equals `baseline_head`;
7. the newly derived deterministic record is value-for-value identical to the existing record.

Idempotent success performs no write, preserves the existing record byte-for-byte even if its insignificant JSON whitespace differs from newly generated serialization, preserves every other governance file byte-for-byte, and prints the same success line.

Any malformed, stale, inconsistent, or different existing binding is rejected. A current descendant commit is not identical begin-time authority and is rejected by begin even if check would allow it. Phase 4 provides no overwrite, reset, delete, rebind, unconditional replacement, or guessed recovery. A later phase may define an explicit compare-and-swap rebind protocol; Phase 4 does not.

## 8. `implementation check`

The command is:

```text
mrgs implementation check --repo <REPOSITORY_PATH>
```

It is read-only. After completing the common validation in Section 4, check must validate in this order:

1. require `implementation-authority.json` to exist as a direct-child regular file;
2. strictly parse the record and reject missing, unknown, malformed, or invalid fields;
3. validate exact accepted plan SHA, active phase, contract ID, final accepted revision, accepted source path, accepted SHA, and exact accepted content equality;
4. require the record to bind the current final accepted ledger entry, including in `REVISION_DRAFT`;
5. parse and validate the record's exact accepted contract content and require it to produce exactly the same contract values as the final ledger content;
6. complete Section 6.6 using the now-validated `baseline_branch`, `git_object_format`, and `baseline_head` fields;
7. rerun the unmerged precheck in Section 11.1 to detect any conflict race after common index validation;
8. build the complete deterministic change inventory under Section 11;
9. strictly validate every inventory path;
10. perform required filesystem and symlink checks under Section 12;
11. reject a path matching any forbidden rule;
12. reject a path that matches no allowed rule;
13. compute the unique changed-path count.

Paths are checked in ascending order of their strict UTF-8 byte sequences after inventory union. For a rename or copy, both source and destination are independently inserted into that ordered set and independently enforced.

On success, stdout is exactly:

```text
IMPLEMENTATION_OK <contract_id> <revision> <sha256> <changed_path_count>
```

`changed_path_count` is the decimal count of unique enforced repository-relative paths. Zero changed paths is valid and prints `0`.

Success proves only that the complete validated Git-visible change inventory is within the accepted path boundary from the bound baseline. It does not execute verification commands, inspect semantic correctness, produce an audit result, authorize closeout, or imply completion.

## 9. Stale authority rejection

An implementation record is stale and check must fail when any of these is true:

1. the accepted plan SHA changes or recorded plan bytes drift;
2. the active phase changes, disappears, becomes closed, or becomes inconsistent;
3. the ledger phase or contract ID differs;
4. the final accepted contract revision differs from `contract_revision`;
5. the final accepted contract SHA differs from `contract_sha256`;
6. the final accepted source path differs from `contract_source_path`;
7. the final accepted exact content differs from `contract_content`;
8. the recomputed content SHA differs from either stored SHA;
9. any accepted authority, draft authority, preimage receipt, or cross-record relation becomes malformed or inconsistent;
10. the implementation record itself is malformed or inconsistent;
11. the final accepted revision is no longer the record's exact binding;
12. the baseline branch changes;
13. the baseline commit is missing or no longer an ancestor of current `HEAD`.

A valid newer unaccepted draft does not silently replace the bound accepted revision and does not by itself stale the binding. It is validated as required Phase 3 authority, then ignored as implementation authority. Once that draft or any later draft is accepted and becomes the final ledger entry, the older binding is stale.

There is no fallback to an earlier accepted revision, no search for a convenient historical contract, and no inference from the current draft.

## 10. Path-rule model

Phase 4 gives the Phase 2 contract strings one exact enforcement grammar. Rules are not globs and are never passed to Git, a shell, or the host filesystem as patterns.

### 10.1 Strict rule syntax

Every `allowed_paths` and `forbidden_paths` entry must be strict UTF-8 and satisfy all of these rules:

1. it is non-empty and has no leading or trailing whitespace;
2. it is repository-relative and does not begin with `/`, `//`, or an ASCII drive prefix such as `C:`;
3. it contains `/` separators only and no backslash;
4. it contains no empty segment, `.` segment, or `..` segment;
5. it contains no NUL, ASCII control character, or DEL;
6. it contains none of the glob metacharacters `*`, `?`, `[`, or `]`;
7. it is already normalized: no leading `./`, doubled slash, or lexical rewrite is permitted;
8. entries are unique by exact UTF-8 byte equality within their own list after validation;
9. no lossy conversion, case folding, separator conversion, Unicode normalization, percent decoding, environment expansion, or filesystem canonicalization is performed.

An entry ending in `/` is a directory-prefix rule. Its prefix before the trailing slash must contain at least one valid segment. An entry not ending in `/` is an exact file-path rule.

Duplicate normalized rules are rejected. Because no normalization is performed, this means exact duplicates in one list. The same exact rule may appear once in each list; that is an intentional overlap resolved by forbidden precedence.

An allowed rule may not be `.git`, `.git/`, `.mrgs`, `.mrgs/`, or lie below either reserved first segment. Reserved first-segment comparison is ASCII case-insensitive so aliases such as `.GIT/` and `.MRGS/` are also rejected as allowed implementation targets. A forbidden rule may name these reserved scopes; the Phase 2 example `.git/` remains valid and is redundant with the unconditional reserved-path rejection.

Malformed rules invalidate the accepted contract for Phase 4. They are not silently reinterpreted.

### 10.2 Matching

Allowed matching is exact, case-sensitive UTF-8 byte comparison with no Unicode normalization. Forbidden matching applies both exact comparison and ASCII case-insensitive comparison; non-ASCII bytes remain exact. This conservative forbidden check is deterministic on every host and prevents ASCII case aliases from bypassing a forbidden scope on case-insensitive filesystems:

1. file rule `src/main.rs` matches only `src/main.rs`;
2. file rule `src/main.rs` does not match `src/main.rs.bak`;
3. directory rule `src/` matches paths beginning with the exact segment prefix `src/`;
4. directory rule `src/` does not match `src`, `src-old/file.rs`, or `Src/file.rs`;
5. a changed path matches allowed scope when at least one allowed rule matches it;
6. a changed path is forbidden when at least one forbidden rule matches it exactly or under ASCII case-insensitive comparison;
7. forbidden matching is evaluated first and always wins over allowed matching.

The host filesystem's case behavior does not alter matching. On a case-insensitive host, Git-reported path bytes remain the authority. Unicode normalization is never performed; names that differ only by non-ASCII normalization remain distinct Git paths and are outside Phase 4's physical-alias claim. Reserved `.git` and `.mrgs` rejection remains ASCII case-insensitive.

### 10.3 Changed-path syntax

Every ordinary changed or inventory path admitted after Section 6.4 mode classification must independently satisfy the strict normalized repository-relative syntax above, except that ordinary filename characters `*`, `?`, `[`, and `]` are permitted in Git paths and are compared literally against rules. Such ordinary paths must not end in `/`. The sole parser-level trailing-slash exception is a mode-`040000` sparse-directory record: Section 6.4 validates its sparse-directory path form and then rejects it as `GIT_INVENTORY_INVALID`; it never enters the ordinary changed-path inventory or contract matching. An ordinary path whose first segment is `.git` or `.mrgs` under ASCII case-insensitive comparison is always rejected before contract matching.

## 11. Git change inventory

Check must inventory the net committed tree difference from the baseline to current `HEAD` and every current index/worktree/untracked difference. It must not rely on `git diff --name-only`. Phase 4 governs the current repository delta, not historical activity that was fully reverted before the checked `HEAD`; audit of intermediate commit history is outside this phase.

### 11.1 Conflict precheck

Run and strictly parse:

```text
git ls-files --unmerged -z
```

Any output rejects the check as conflicted. A later unmerged status discovered during inventory also rejects the check, covering a race between commands.

### 11.2 Baseline-to-HEAD inventory

Run:

```text
git diff --no-ext-diff --raw -z --no-abbrev --find-renames=50% --find-copies=50% --find-copies-harder <baseline_head> HEAD --
```

Strictly parse raw diff records, including old mode, new mode, full object IDs, status letter, optional similarity score, and one or two NUL-terminated paths. Accept only documented statuses `A`, `C`, `D`, `M`, `R`, and `T`. Status `U`, combined diff, malformed metadata, invalid modes, wrong object-ID length, missing path fields, unexpected extra fields, or any other status is fatal. `R` and `C` contribute both source and destination paths. Other statuses contribute their one path.

This inventory includes externally committed additions, copies where Git detects them, deletions, modifications, renames, and type changes between the bound baseline and current `HEAD`.

### 11.3 Index/worktree/untracked inventory

Run:

```text
git status --porcelain=v1 -z --untracked-files=all --ignore-submodules=none --renames
git ls-files --others --ignored --exclude-standard -z --
```

Strictly parse each porcelain-v1 `XY` record and its NUL-terminated path data. This includes staged and unstaged additions, copies where Git reports them, deletions, modifications, renames, type changes, and non-ignored untracked paths. Rename/copy records in `-z` form provide destination then source; both are enforced regardless of display order. `??` contributes its untracked path. `!!` is unexpected because ignored paths were not requested and is fatal.

Any unmerged combination, including `DD`, `AU`, `UD`, `UA`, `DU`, `AA`, `UU`, or any status containing `U`, rejects the check. Unsupported or malformed status bytes reject the check. Ordinary staged and unstaged states are allowed only when their paths pass the boundary rules.

Strictly parse the ignored-untracked output as NUL-terminated byte paths and add every path to the inventory. An ignored build artifact is still repository content for Phase 4 boundary purposes and must be authorized by the accepted contract or absent. This prevents current ignore rules, repository-local excludes, or an implementation change to `.gitignore` from concealing an out-of-bound path.

The exact five fixed governance paths in Section 6.5 are excluded from `??` status and ignored-untracked output only after the corresponding existing files have passed complete governance validation and Section 6.4 proved there is no tracked index entry for that path. A tracked governance path is never exempt. No other `.mrgs` path, directory summary, or temporary file is excluded, and any `.git` path is always rejected.

Any raw-diff path with `.mrgs` as its first segment under ASCII case-insensitive comparison and a tracked mode on either side, and any porcelain tracked `XY` record for such a path, maps to `GIT_INVENTORY_INVALID`. This includes a tracked governance path added after the baseline or deleted from the index/worktree after the baseline even when no current index entry remains. The index-wide Section 6.4 check separately catches clean unchanged tracked governance paths already present in the baseline.

### 11.4 Union and enforcement

The final inventory is the set union of paths from Sections 11.2 and 11.3. A path appearing in multiple states is counted once but every parsed record remains subject to structural and mode validation. Rename and copy source and destination paths are separate set members.

For each unique path, in ascending UTF-8 byte order:

1. require strict UTF-8 without lossy conversion;
2. validate strict repository-relative changed-path syntax;
3. reject `.git` or `.mrgs` scope;
4. perform applicable filesystem and symlink validation;
5. reject any forbidden match;
6. require at least one allowed match.

A deleted source need not exist on the filesystem. A rename source and destination are both enforced even when only the destination exists. Zero records produce an empty set and are valid.

### 11.5 Index and record parser invariants

`git ls-files --sparse --stage -z` records must have exactly `<mode> SP <object-id> SP <stage> TAB <path> NUL`. Mode is exactly six octal digits. Object-ID length must match `git_object_format` and characters must be lowercase hexadecimal. Path bytes must first be strict UTF-8 and repository-relative with safe components. Stages `1`, `2`, and `3` are conflicts; any other non-zero stage is malformed inventory. Stage-0 mode `160000` is the separately categorized unsupported submodule. Stage-0 mode `040000` must have the exact sparse-directory trailing-slash form in Section 6.4 and is then always sparse-directory structural evidence and `GIT_INVENTORY_INVALID`, not an ordinary tree, file, or harmless unknown mode. Accepted ordinary stage-0 modes are exactly `100644`, `100755`, and `120000`, and only these apply the ordinary no-trailing-slash path rule. Every other mode is invalid.

After stage and mode classification and ordinary-path syntax validation, but before any governance exemption, an ordinary stage-0 path whose first segment equals `.mrgs` under ASCII case-insensitive comparison is `GIT_INVENTORY_INVALID`. This applies regardless of record bytes, baseline presence, or current change status. No fixed governance filename bypasses this index rule. A compound conflict-stage or gitlink record naming `.mrgs` retains the earlier `GIT_CONFLICT` or `GIT_SUBMODULE_UNSUPPORTED` category; sparse-directory and unsupported-mode records likewise retain their earlier structural category.

Raw diff metadata must use those modes or `000000` for an absent side. Additions require absent old mode and all-zero old object ID; deletions require absent new mode and all-zero new object ID; non-add/delete records require present modes and non-zero IDs. Similarity scores are decimal `0` through `100` and are present only for `R` or `C`; `R` and `C` require exactly two paths, while all other statuses require exactly one. Intent-to-add, combined diff, and inconsistent status/mode/object tuples are rejected.

After rejecting unmerged states, porcelain `XY` accepts exactly these two-byte codes: ` M`, ` T`, ` D`, `M `, `MM`, `MT`, `MD`, `T `, `TM`, `TT`, `TD`, `A `, `AM`, `AT`, `AD`, `D `, `R `, `RM`, `RT`, `RD`, `C `, `CM`, `CT`, `CD`, and `??`. The blank inside each inline code is significant. No Cartesian combination outside this list is accepted. Only codes beginning `R` or `C` require exactly two paths in porcelain `-z` destination-then-source order; every other accepted code requires one. Missing NUL terminators, empty paths, extra path fields, or trailing bytes are malformed.

`git ls-files -v -z` records must be exactly `H SP <path> NUL` for ordinary cached entries. Uppercase `S` is the specifically rejected skip-worktree tag. Any lowercase tag is the specifically rejected assume-unchanged form. Any other tag, missing space, empty path, missing terminator, or trailing bytes is malformed; the prior conflict precheck ensures an unmerged tag cannot be valid here.

## 12. Symlinks and filesystem safety

Git path authority and filesystem containment are separate checks. Lexically safe Git paths do not prove safe live filesystem topology.

### 12.1 Git metadata proof

Raw diff modes and index modes identify Git symlinks with mode `120000`. Version selection is exact:

1. for a baseline-to-`HEAD` record whose new mode is `120000` and status is not `D`, inspect the new object ID with direct `git cat-file blob <object-id>`;
2. for a stage-0 index entry whose current mode is `120000`, inspect that exact index object ID with the same plumbing command;
3. for an unstaged or untracked live symlink, use `symlink_metadata` and `read_link` without following the link;
4. do not inspect an old-side or deleted symlink blob merely because the old mode was `120000`; its source path remains boundary-enforced;
5. if different current layers contain different symlink versions, inspect every extant version selected by rules 1–3.

Every selected target blob or live target must be strict UTF-8. `git cat-file` failure, unexpected size/output, or an object whose type is not `blob` is fatal.

Target resolution must remain in the version's own layer:

1. a `HEAD` symlink target is resolved only against the `HEAD` tree through strict `git ls-tree -z HEAD -- <path>` lookups;
2. an index symlink target is resolved only against stage-0 index entries through strict `git ls-files --sparse --stage -z -- <path>` lookups after Section 6.4 has rejected sparse state;
3. a live symlink target is resolved only with live `symlink_metadata` and canonicalization;
4. no Git-object target may be proved or rejected from a different live, index, or tree layer;
5. for each layer, inspect every existing target-path prefix and the target leaf; if any is a symlink in that same layer, reject the target rather than following a chain;
6. malformed lookup output, a lookup race, or inability to prove the required same-layer topology is fatal.

Rejecting all symlink-to-symlink chains avoids layer mixing, cycles, and aliases that could hide a forbidden final target. A missing target in its own layer is treated as broken and may proceed only under the lexical broken-target rule below.

A symlink target is valid only when:

1. it is non-empty;
2. it is relative, not absolute, UNC, device-prefixed, or drive-prefixed;
3. resolving its `.` and `..` components lexically from the symlink's repository-relative parent does not escape the repository root;
4. every existing ancestor and the target leaf in the applicable layer are free of symlinks; live ancestors and leaves are also free of junctions and non-symlink reparse points;
5. when the target exists and can be canonicalized, its canonical path remains within the canonical repository;
6. the normalized repository-relative lexical target does not match a forbidden rule and matches at least one allowed rule, using the same precedence and segment semantics as a changed path;
7. when a live target exists, its canonical target after the no-chain proof is converted without loss to a normalized repository-relative path and independently passes the same forbidden-first and allowed-required matching; a Git-tree or index target instead uses its proved same-layer repository-relative target path for this second match.

A broken symlink is permitted only when its target is lexically contained, its lexical target passes contract scope, and all existing target ancestors are safe in its own layer. A symlink whose target bytes are not strict UTF-8 is rejected. Deleted symlinks have no current target to inspect; their deleted Git path is still boundary-enforced. Every symlink chain is rejected, including a chain whose lexical alias appears allowed.

### 12.2 Live path topology

For every changed path that currently exists in the worktree, inspect each existing path component below the canonical repository using `symlink_metadata`:

1. a symlink or reparse-point ancestor is rejected;
2. a directory junction is rejected;
3. a non-symlink reparse-point leaf is rejected;
4. a symlink leaf is allowed only under Section 12.1;
5. an inspection or canonicalization failure is fatal rather than treated as absence.

For deleted paths and missing rename sources, Git metadata and lexical path validation are the available proof; filesystem existence is not required. For rename destinations and additions that exist, live topology inspection is required.

Phase 4 does not follow directory links to discover changes outside the repository. It rejects unsafe topology instead. Hard-link alias detection and mount/bind-mount boundary detection are not portable through the existing dependency set and are explicitly outside Phase 4; a successful check makes no claim about those aliases. Protection against a hostile concurrent process changing filesystem topology or Git state between validated system calls remains outside Phase 4. Ordinary detected races that produce malformed, inconsistent, or failed Git/filesystem reads are fatal and never treated as success.

## 13. Verification commands in accepted contracts

The final accepted contract's `verification_commands` must remain present and well-formed under the strict Phase 2 contract model. They remain part of the exact accepted `contract_content` persisted in the implementation record and therefore part of the bound authority.

Neither Phase 4 command executes, expands, parses as shell syntax, or separately prints these commands. Execution belongs to the external implementation workflow. Phase 4 must not invoke a shell, PowerShell, Cargo, a test runner, a model, or any arbitrary command obtained from contract content.

## 14. Persistence and byte preservation

1. `implementation begin` writes at most one governance file: `implementation-authority.json`.
2. `implementation check` writes nothing.
3. Begin must serialize the complete record before opening a temporary file.
4. First persistence reuses the established unique same-directory temporary-file, full-write, and sync steps, then uses an atomic create-if-absent publication primitive that cannot replace an existing destination.
5. Temporary-file creation must not truncate or reuse an existing file.
6. An existing implementation record is never replaced in Phase 4. If the destination appears after validation but before publication, publication must fail with `IMPLEMENTATION_AUTHORITY_CONFLICT`; the competing destination is preserved byte-for-byte and the invocation removes only its own temporary file.
7. Idempotent begin performs no write and preserves the existing record byte-for-byte.
8. Every handled persistence failure removes only the temporary file created by that invocation when it exists.
9. No handled success or failure leaves a Phase 4 temporary file.
10. Every failure preserves byte-for-byte every existing governance file, including malformed files.
11. No existing accepted-plan, state, draft, accepted-ledger, or implementation-authority file is rewritten as a side effect of validation.
12. No source, documentation, test, manifest, lockfile, Git file, index entry, ref, configuration file, or worktree path is modified by either command.

The existing replacing rename helper is not sufficient for first publication because it can overwrite a concurrently created destination. Implementation must use an operating-system atomic no-clobber rename/link primitive and fail with `PERSISTENCE_FAILED` when the filesystem cannot provide one; no non-atomic check-then-replace fallback is authorized. The existing protection against hostile concurrent filesystem topology replacement remains outside scope. A detected destination conflict or failed atomic publication is a complete failure, not partial success.

## 15. Error model

Success exits with code `0`. Every failure exits non-zero. A semantic command failure emits no success stdout and emits one concise deterministic stderr line beginning `error: ` with one of these categories:

```text
INVALID_ARGUMENT
REPOSITORY_INVALID
GOVERNANCE_AUTHORITY_INVALID
CONTRACT_NOT_ACCEPTED
REQUESTED_REVISION_STALE
REQUESTED_SHA_STALE
CONTRACT_PATH_RULE_INVALID
GIT_COMMAND_FAILED
GIT_ROOT_MISMATCH
GIT_DETACHED_HEAD
GIT_HEAD_INVALID
GIT_DIRTY
GIT_OPERATION_IN_PROGRESS
GIT_SUBMODULE_UNSUPPORTED
IMPLEMENTATION_AUTHORITY_MISSING
IMPLEMENTATION_AUTHORITY_INVALID
IMPLEMENTATION_AUTHORITY_CONFLICT
IMPLEMENTATION_AUTHORITY_STALE
BASELINE_BRANCH_CHANGED
BASELINE_COMMIT_MISSING
BASELINE_HISTORY_CHANGED
GIT_INVENTORY_INVALID
GIT_CONFLICT
CHANGE_PATH_INVALID
CHANGE_FORBIDDEN
CHANGE_NOT_ALLOWED
FILESYSTEM_BOUNDARY_UNSAFE
PERSISTENCE_FAILED
```

For Phase 4 commands, stderr is exactly `error: <CATEGORY>` followed by one platform newline; no suffix is permitted. Category mapping is normative:

1. CLI token syntax, including non-canonical revision or SHA text: `INVALID_ARGUMENT`.
2. Missing/non-directory/non-Git `--repo`: `REPOSITORY_INVALID`.
3. Canonical Git top-level mismatch: `GIT_ROOT_MISMATCH`.
4. Any malformed, incomplete, unsafe, or inconsistent Phase 1–3 authority other than a valid `DRAFT` lifecycle: `GOVERNANCE_AUTHORITY_INVALID`.
5. Valid `DRAFT` lifecycle with no accepted ledger: `CONTRACT_NOT_ACCEPTED`.
6. Requested revision or SHA differs from the final accepted entry: `REQUESTED_REVISION_STALE` or `REQUESTED_SHA_STALE`, with revision checked first.
7. Malformed enforcement rule: `CONTRACT_PATH_RULE_INVALID`; a Phase 4-unsafe accepted contract ID: `GOVERNANCE_AUTHORITY_INVALID`.
8. Git spawn failure, signal termination, unexpected execution failure, or missing promised commit/tree/blob under no-lazy-fetch: `GIT_COMMAND_FAILED`. No retry or Git stderr forwarding is permitted.
9. Detached or unborn `HEAD`: `GIT_DETACHED_HEAD`; malformed object format or `HEAD` ID: `GIT_HEAD_INVALID`.
10. Begin-time remaining status/ignored path: `GIT_DIRTY`.
11. Operation marker: `GIT_OPERATION_IN_PROGRESS`; stage-0 gitlink mode `160000`: `GIT_SUBMODULE_UNSUPPORTED`.
12. Missing implementation record on check: `IMPLEMENTATION_AUTHORITY_MISSING`; malformed record: `IMPLEMENTATION_AUTHORITY_INVALID`.
13. Structurally valid record whose plan/phase/final accepted tuple changed: `IMPLEMENTATION_AUTHORITY_STALE` for both begin and check. On check, contextual current Git object-format mismatch is also `IMPLEMENTATION_AUTHORITY_STALE`. Accepted-authority comparison occurs before deriving or comparing a new baseline.
14. On begin, an existing structurally valid record whose accepted tuple is current but whose contextual current Git object format, branch, or `HEAD` differs; destination publication race; or prohibited replacement attempt: `IMPLEMENTATION_AUTHORITY_CONFLICT`.
15. Current branch mismatch: `BASELINE_BRANCH_CHANGED`; unresolved recorded commit without an effective `extensions.partialClone` marker: `BASELINE_COMMIT_MISSING`; unresolved recorded commit with that marker: `GIT_COMMAND_FAILED`; non-ancestor baseline: `BASELINE_HISTORY_CHANGED`.
16. Malformed raw/index/status/ignored output, invalid index flags, tracked `.mrgs` index or tracked-diff/status path, active `core.sparseCheckout`, active `index.sparse`, sparse-directory mode `040000`, successfully returned malformed or multi-valued sparse configuration evidence, or inconsistent mode/object/status tuple: `GIT_INVENTORY_INVALID`; stage `1`–`3` or any other unmerged entry: `GIT_CONFLICT`.
17. Malformed/non-UTF-8/reserved changed path: `CHANGE_PATH_INVALID`; forbidden match: `CHANGE_FORBIDDEN`; no allowed match: `CHANGE_NOT_ALLOWED`.
18. Unsafe symlink target, target-scope failure, junction, reparse point, topology, or required filesystem-inspection failure: `FILESYSTEM_BOUNDARY_UNSAFE`.
19. Serialization, temporary-write, sync, or no-clobber publication failure other than a destination conflict: `PERSISTENCE_FAILED`.

When multiple paths fail, deterministic sorted-path validation determines the first category. Absolute paths, environment-specific Git diagnostics, lossy path text, backtraces, and nondeterministic values are never printed by a Phase 4 semantic error.

Phase 4 failure precedence is exact:

1. common Phase 1–3 authority validation;
2. Git root, attached `HEAD`, object-format, and current-commit validation;
3. in-progress Git operation validation;
4. Git index structural validation, in this internal order: malformed record structure or path encoding/component safety, conflict stage, gitlink, sparse-directory mode and sparse path form, unsupported mode, ordinary path syntax, tracked `.mrgs`, then assume-unchanged or skip-worktree evidence;
5. effective sparse-checkout then sparse-index configuration validation;
6. begin cleanliness or check baseline relation and complete change inventory;
7. changed-path and filesystem enforcement;
8. persistence.

A Git child that cannot be spawned or executed interrupts the applicable step with `GIT_COMMAND_FAILED`. A successfully executed sparse-config command that returns malformed evidence maps to `GIT_INVENTORY_INVALID`. A missing promised object at any Git step maps to `GIT_COMMAND_FAILED`, even when the object would otherwise have been used by inventory or symlink enforcement. Section 6.1.1's effective `extensions.partialClone` distinction governs the only overlap between that rule and `BASELINE_COMMIT_MISSING`.

There is no normal-operation backtrace, silent repair, partial success, invalid-authority normalization, fallback to stale accepted contracts, guessing of Git state, or success after a failed required subprocess.

## 16. Dependencies

No production or development dependency may be added. Production dependencies remain:

- `clap` with derive;
- `serde`;
- `serde_json`;
- `toml`;
- `sha2`;
- `thiserror`.

Development dependencies remain:

- `tempfile`;
- `assert_cmd`;
- `predicates`.

Use `std::process::Command` and the installed Git executable for inspection. A Git library, shell library, glob library, filesystem-watching library, random library, time library, async runtime, logging framework, or platform-abstraction dependency is not authorized. `Cargo.toml` and `Cargo.lock` must remain unchanged.

## 17. Phase boundary

Phase 4 must not implement:

- source-code generation;
- patch creation or application;
- shell-command execution from a contract;
- automatic implementation;
- model invocation;
- implementation correctness audit;
- audit `PASS` or `FAIL`;
- repair ticket generation or repair routing;
- phase closeout or active-phase clearing;
- final manifests or completion receipts;
- Git commit, push, fetch, pull, reset, checkout, switch, branch creation, merge, rebase, cherry-pick, revert, stash, clean, tag, worktree mutation, or configuration mutation;
- multi-repository continuity metadata;
- corruption recovery;
- activation or rollback drills;
- background services, networking, or automatic command chaining.

MRGS observes implementation changes; it does not create them. External tools and humans remain responsible for editing files and, if separately authorized outside MRGS, creating descendant commits.

## 18. Required tests

Add at least 120 distinct executed Phase 4 test scenarios, in addition to retaining the prior 251 non-superseded Phase 1–3 tests. Tests must use temporary real Git repositories where Git behavior is under test and must inspect exact bytes, exact persisted JSON fields, stdout, stderr category, exit status, Git status, and preservation invariants rather than only exit codes.

Meaningfully cover at least:

1. valid first implementation begin in `ACCEPTED`;
2. valid first implementation begin in `REVISION_DRAFT` binds the final accepted revision;
3. `DRAFT` lifecycle rejection without an accepted ledger;
4. newer unaccepted draft is never selected as authority;
5. exact requested accepted revision binding;
6. exact requested accepted SHA binding;
7. stale requested revision rejection;
8. stale requested SHA rejection;
9. uppercase and malformed requested SHA rejection;
10. deterministic record field order and serialization bytes;
11. literal baseline SHA persistence from an independently asserted Git commit ID;
12. exact baseline branch persistence;
13. exact accepted source-path persistence;
14. exact accepted content persistence including final newline and LF/CRLF distinction;
15. no duplicated path-list fields in the implementation record;
16. unknown implementation-record field rejection;
17. missing implementation-record field rejection;
18. malformed implementation-record JSON rejection;
19. unsupported implementation-record schema rejection;
20. implementation-record content/hash mismatch rejection;
21. implementation-record contract identity, phase, plan SHA, source path, revision, and SHA mismatch rejection;
22. valid idempotent begin;
23. idempotent begin preserves the implementation record's exact bytes;
24. idempotent begin preserves all Phase 1–3 governance bytes;
25. idempotent begin rejects current descendant `HEAD` rather than replacing the baseline;
26. different existing binding rejection;
27. stale existing binding rejection after a later accepted revision;
28. no overwrite or rebind of an existing record;
29. dirty unstaged tracked file rejection at begin;
30. staged change rejection at begin;
31. non-governance untracked and ignored-untracked file rejection at begin;
32. tracked deletion and type change rejection at begin;
33. detached `HEAD` rejection;
34. wrong Git top-level rejection, including a subdirectory passed as `--repo`;
35. non-Git repository rejection;
36. malformed or failed Git command output rejection;
37. merge, rebase, cherry-pick, revert, bisect, `git am`, and sequencer operation-marker rejection;
38. tracked submodule gitlink rejection at begin and check;
39. malformed accepted-plan, state, draft, preimage, ledger, accepted entry, and cross-record authority rejection;
40. symlinked or reparse-point governance file rejection;
41. missing implementation record rejection on check;
42. stale accepted plan after begin;
43. changed active phase after begin;
44. newly accepted later contract revision stales the record;
45. valid newer unaccepted draft leaves the final accepted binding authoritative;
46. changed branch after begin, including same commit on another branch;
47. missing baseline commit without an effective `extensions.partialClone` marker emits exactly `BASELINE_COMMIT_MISSING`;
48. rewritten history where baseline is not an ancestor rejection;
49. descendant `HEAD` on the same branch is accepted for inventory;
50. zero-change check prints exact count `0`;
51. allowed unstaged modified file;
52. allowed staged modified file;
53. allowed committed modified file after baseline;
54. allowed added file in staged, committed, and untracked states;
55. allowed deleted file in staged, unstaged, and committed states;
56. allowed rename with both source and destination in scope;
57. rename rejected when only source is allowed;
58. rename rejected when only destination is allowed;
59. copy source and destination enforcement where Git reports a copy;
60. type-change inventory and enforcement;
61. forbidden path rejection;
62. path outside all allowed rules rejection;
63. forbidden-over-allowed precedence;
64. exact file rule does not match a suffix extension;
65. directory-prefix rule respects segment boundaries;
66. case-sensitive allowed matching and conservative ASCII case-insensitive forbidden matching;
67. empty, whitespace-padded, absolute, drive-prefixed, traversal, dot-segment, doubled-slash, backslash, control-character, and glob rule rejection;
68. duplicate normalized allowed-rule and forbidden-rule rejection;
69. `.git`, case-alias `.GIT`, `.mrgs`, and case-alias `.MRGS` allowed-target rejection;
70. forbidden `.git/` rule remains valid and redundant;
71. changed path under `.git` or `.mrgs` is unconditionally rejected;
72. non-UTF-8 Git path rejection on a platform that supports constructing it;
73. malformed raw-diff and porcelain records rejection through parser-level tests;
74. unmerged/conflicted index rejection;
75. union deduplicates a path present in committed and working-state inventories;
76. rename source deletion does not require filesystem existence;
77. symlink addition with a safe contained relative target;
78. symlink modification target validation for committed, staged, and live states;
79. absolute, drive-prefixed, lexically escaping, canonical outside-repository, and non-UTF-8 symlink target rejection;
80. deleted symlink path enforcement without target inspection;
81. directory junction, unsafe reparse point, and symlink ancestor rejection;
82. implementation check writes nothing on success and failure;
83. every failed begin preserves all existing governance files byte-for-byte;
84. every failed check preserves all existing governance files byte-for-byte;
85. temporary-file absence after first success, idempotent success, and every handled failure;
86. pre-existing temporary files are not truncated, reused, or removed;
87. implementation begin creates only `implementation-authority.json`;
88. Git refs, index, config, and worktree bytes remain unchanged by begin and check except for the authorized governance-record creation by begin;
89. verification commands remain validated and bound in exact content;
90. a contract verification command with an observable side effect is not executed by begin or check;
91. no shell, Cargo, test runner, model, or arbitrary contract command is launched;
92. exact deterministic success output for begin and check;
93. deterministic first-failure ordering for multiple invalid changed paths;
94. all failures are non-zero with no success stdout and no backtrace;
95. exact recognized governance files are excluded after validation even when `.mrgs` is not ignored;
96. immediate check succeeds after first begin when `.mrgs` is not ignored and there are zero implementation changes;
97. unknown, temporary, directory-summary, and tracked `.mrgs` paths are not exempt;
98. ignored out-of-bound files are inventoried and rejected;
99. an allowed `.gitignore` change cannot hide a newly ignored forbidden file;
100. inherited alternate index, worktree, Git directory, object directory, namespace, and config environment variables do not alter inspection;
101. replacement refs are disabled for ancestry and diff inspection;
102. configured filesystem monitor, external diff, pager, editor, and hook are not executed;
103. assume-unchanged entry rejection;
104. skip-worktree, sparse-checkout, and sparse-index rejection;
105. SHA-1 object-format persistence and validation;
106. SHA-256 object-format persistence and validation when the installed Git supports SHA-256 repositories, with parser-level coverage otherwise;
107. canonical revision token acceptance and rejection of zero, sign, leading zero, overflow, whitespace, and non-decimal text;
108. accepted contract ID with embedded whitespace, line terminator, Unicode, or other non-token character cannot authorize Phase 4;
109. `REVISION_DRAFT` enforcement uses accepted rules: an accepted path remains allowed when a newer draft removes it;
110. `REVISION_DRAFT` enforcement ignores draft rules: a path added only by the newer draft remains not allowed;
111. exact index-stage parser coverage for valid modes, object lengths, stage values, gitlinks, and malformed records;
112. raw-diff zero-object, mode, score, path-count, and status consistency coverage;
113. ignored-output parser malformed-record and non-UTF-8 coverage;
114. staged symlink deletion does not inspect the deleted index target, while an extant committed symlink version is inspected when selected by the baseline diff;
115. symlink target matching a forbidden rule or no allowed rule is rejected;
116. safe contained symlink target and link path both require allowed authority;
117. atomic destination-appearance race preserves the competing implementation record and all earlier authority;
118. unsupported no-clobber publication fails without fallback or partial record;
119. exact category mapping for representative failure from every Section 15 category;
120. semantic errors redact absolute paths, inherited Git diagnostics, and invalid non-UTF-8 path bytes;
121. endpoint semantics: an intermediate forbidden commit fully reverted before current `HEAD` is not current inventory, while any net forbidden path is rejected;
122. every symlink-to-symlink chain is rejected, including an allowed lexical alias to a forbidden or not-allowed target;
123. committed, staged, and live symlink target topology is resolved only in its own layer, including fixtures where the other two layers diverge;
124. all non-superseded Phase 1, Phase 2, and Phase 3 tests continue to pass;
125. begin rejects a clean unchanged tracked `.mrgs/accepted-plan.json`;
126. check rejects a clean unchanged tracked `.mrgs/accepted-plan.json`;
127. begin rejects a clean unchanged tracked `.mrgs/state.json`;
128. check rejects a clean unchanged tracked `.mrgs/state.json`;
129. begin rejects a clean unchanged tracked `.mrgs/contract-draft.json`;
130. check rejects a clean unchanged tracked `.mrgs/contract-draft.json`;
131. begin rejects a clean unchanged tracked `.mrgs/accepted-contract.json`;
132. check rejects a clean unchanged tracked `.mrgs/accepted-contract.json`;
133. begin rejects a clean unchanged tracked `.mrgs/implementation-authority.json`;
134. check rejects a clean unchanged tracked `.mrgs/implementation-authority.json`;
135. begin and check reject an unchanged tracked unknown `.mrgs/extra.json`;
136. begin and check reject a tracked Phase 4 temporary-file-shaped `.mrgs` path;
137. begin and check reject tracked `.MRGS/state.json` ASCII case alias where the host permits the fixture, with platform-neutral parser coverage everywhere;
138. a tracked governance path added after baseline is `GIT_INVENTORY_INVALID`;
139. a tracked governance path deleted after baseline is `GIT_INVENTORY_INVALID` even when absent from the current index;
140. otherwise valid governance bytes never exempt a tracked governance path;
141. every ordinary stage-0, raw-diff, or non-conflict tracked-status governance rejection emits exactly `error: GIT_INVENTORY_INVALID`;
142. every tracked-governance failure preserves all governance files byte-for-byte;
143. every tracked-governance failure preserves Git refs, index, configuration, object database, and worktree byte-for-byte;
144. inherited `GIT_CONFIG_PARAMETERS` is absent from every Git child;
145. attempted `GIT_CONFIG_PARAMETERS` injection of `core.fsmonitor`, `diff.external`, pager, editor, hooks path, or related external behavior cannot alter any child and executes no helper;
146. inherited `GIT_SHALLOW_FILE` is absent from every Git child;
147. an injected `GIT_SHALLOW_FILE` cannot change baseline object availability, merge-base, or ancestry interpretation;
148. child-environment inspection proves both `GIT_CONFIG_PARAMETERS` and `GIT_SHALLOW_FILE` are absent after final environment construction;
149. environment-isolation results and categories are deterministic and no inherited value or Git diagnostic leaks into stderr;
150. every Git child receives exact `GIT_NO_LAZY_FETCH=1`;
151. every Git invocation includes `--no-lazy-fetch` in the fixed global-option position before the subcommand;
152. a promisor repository with every required object local can be inspected without network or helper execution;
153. a missing promised blob fails locally with exactly `GIT_COMMAND_FAILED`;
154. a missing promised tree fails locally with exactly `GIT_COMMAND_FAILED`;
155. a missing promised commit required for baseline verification or ancestry in a repository with effective `extensions.partialClone` fails locally with exactly `GIT_COMMAND_FAILED`;
156. symlink blob inspection cannot trigger lazy fetch and a missing promised symlink blob fails locally;
157. raw diff copy/rename detection cannot trigger lazy fetch and missing required objects fail locally;
158. no remote helper, credential helper, fetch process, or fetch-pack process is launched by begin or check;
159. an observable fake remote helper and observable fake credential helper are never invoked;
160. every missing-promisor-object case emits exactly `error: GIT_COMMAND_FAILED`;
161. no remote URL, helper output, credential text, or network-derived stderr is surfaced;
162. effective `core.sparseCheckout=true` rejects with `GIT_INVENTORY_INVALID`;
163. effective `core.sparseCheckout=false` does not reject by that signal alone;
164. unset `core.sparseCheckout` with exit `1` and empty output does not reject by that signal alone;
165. malformed, non-UTF-8, unexpected, or multi-line sparse-checkout configuration output rejects deterministically;
166. multiple `core.sparseCheckout` values reject even when the final effective value is false;
167. effective `index.sparse=true` rejects with `GIT_INVENTORY_INVALID`;
168. effective `index.sparse=false` does not reject by that signal alone;
169. unset `index.sparse` with exit `1` and empty output does not reject by that signal alone;
170. malformed, non-UTF-8, unexpected, or multi-line sparse-index configuration output rejects deterministically;
171. multiple `index.sparse` values reject even when the final effective value is false;
172. an actual cone-mode sparse-checkout repository is rejected;
173. an actual non-cone sparse-checkout repository is rejected;
174. an actual sparse-index repository is rejected without silently expanding its index;
175. parser-level mode `040000` sparse-directory fixture with its required trailing slash passes sparse-path structural parsing and then maps exactly to `GIT_INVENTORY_INVALID` rather than ordinary `CHANGE_PATH_INVALID`;
176. structural sparse-directory evidence rejects when `index.sparse` is unset or false;
177. active sparse checkout rejects when skip-worktree bits are temporarily cleared;
178. uppercase `S` skip-worktree rejection remains independently enforced;
179. begin rejects each active sparse-checkout, active sparse-index, and structural sparse-directory state;
180. check rejects each active sparse-checkout, active sparse-index, and structural sparse-directory state;
181. malformed successfully returned sparse-state evidence maps exactly to `GIT_INVENTORY_INVALID`, while spawn, signal, or command-execution failure maps exactly to `GIT_COMMAND_FAILED`;
182. every sparse-state failure preserves all governance files byte-for-byte;
183. every sparse-state failure preserves Git refs, index, configuration, object database, and worktree byte-for-byte;
184. all prior 251 non-superseded Phase 1, Phase 2, and Phase 3 tests continue to pass;
185. a conflict-stage index record beneath `.mrgs` emits exactly `error: GIT_CONFLICT` under structural precedence;
186. a stage-0 gitlink beneath `.mrgs` emits exactly `error: GIT_SUBMODULE_UNSUPPORTED` under structural precedence;
187. child-process argument recording proves begin executes all four exact sparse-state commands from Sections 6.4.2 and 6.4.3, including `--get` and `--get-all` for each key;
188. child-process argument recording proves check executes all four exact sparse-state commands from Sections 6.4.2 and 6.4.3, including `--get` and `--get-all` for each key.

The list above contains exactly 188 numbered Phase 4 obligations. Multiple assertions may share a fixture, but the implementation handoff must map every obligation to executed test evidence. Platform-specific symlink, non-UTF-8, junction, reparse-point, and case-alias tests may be conditionally compiled only where the platform cannot express the fixture. Parser-level and platform-neutral negative coverage must still run everywhere. The reported Phase 4 scenario count must not include ignored tests as executed passes.

## 19. Allowed implementation paths

Future implementation of this Phase 4 contract may modify only:

- `.gitignore`;
- `README.md`;
- `src/**`;
- `tests/**`.

Before any Phase 4 implementation edit, `docs/contracts/phase-04-contract.md` must be tracked in the implementation baseline and its baseline blob must equal the accepted authoring result byte-for-byte. If it is untracked, missing, staged, or already modified, implementation must stop without editing. Contract authoring and its later human-authorized commit are separate from Phase 4 implementation.

`Cargo.toml` and `Cargo.lock` must remain unchanged because Section 16 authorizes no dependency addition.

These authority documents are read-only during implementation:

- `docs/master-plan.md`;
- `docs/contracts/phase-01-contract.md`;
- `docs/contracts/phase-02-contract.md`;
- `docs/contracts/phase-03-contract.md`;
- `docs/contracts/phase-04-contract.md`.

No generated artifact, fixture outside `tests/**`, example, benchmark, script, workflow, or future-phase scaffold is authorized.

## 20. Forbidden paths and operations

Do not modify `.github/**`, `.git/**`, `.mrgs/**` as repository implementation content, `scripts/**`, `examples/**`, `benches/**`, any authority document, any dependency manifest or lockfile, or anything outside the repository.

Runtime creation of `<target-repo>/.mrgs/implementation-authority.json` by the implemented begin command is the sole Phase 4 governance-write exception. It does not authorize editing a checked-in `.mrgs` path during Phase 4 implementation.

Do not commit, push, tag, fetch, pull, create or switch branches, merge, rebase, cherry-pick, revert, reset, stash, clean, mutate worktrees, change Git configuration, install global software, use networking, or add later-phase behavior.

## 21. Verification

Run during future Phase 4 implementation:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo test --all -- --list
git diff --check
git diff --cached --check
git status --short --untracked-files=all
git ls-files --error-unmatch docs/contracts/phase-04-contract.md
git diff --name-only
git diff --cached --name-only
git diff --stat
git diff --cached --stat
git diff -- Cargo.toml Cargo.lock
git diff --cached -- Cargo.toml Cargo.lock
git diff -- docs/master-plan.md docs/contracts/phase-01-contract.md docs/contracts/phase-02-contract.md docs/contracts/phase-03-contract.md docs/contracts/phase-04-contract.md
git diff --cached -- docs/master-plan.md docs/contracts/phase-01-contract.md docs/contracts/phase-02-contract.md docs/contracts/phase-03-contract.md docs/contracts/phase-04-contract.md
```

All Rust checks and both diff checks must pass. Dependencies and every authority document must have no staged or unstaged diff, and the Phase 4 contract must be tracked. `git status` is the controlling inventory for staged and untracked files; every listed path must be reconciled against the allowed implementation paths, and every untracked allowed file must be inspected directly because ordinary `git diff` omits it. Report unit, integration, platform-skipped, Phase 4 scenario, and total test counts separately. Do not describe the integration count or Phase 4 scenario count as the total.

## 22. Handoff evidence

Report:

- phase;
- model;
- repository;
- branch;
- baseline HEAD and final HEAD;
- remote and remote HEAD;
- pre-status and post-status;
- exact changed files;
- CLI result;
- implementation-record schema and exact-byte persistence result;
- accepted binding and lifecycle result;
- Git baseline and operation-state result;
- sparse-preserving index-structure and tracked-governance rejection result;
- `GIT_CONFIG_PARAMETERS` and `GIT_SHALLOW_FILE` removal result;
- no-lazy-fetch option/environment and promisor-object local-failure result;
- sparse-checkout configuration, sparse-index configuration, sparse-directory, and skip-worktree result;
- exact begin/check child-argument evidence for all four required sparse-state config commands;
- begin and idempotency result;
- check and zero-change result;
- stale-authority result;
- path-rule and forbidden-precedence result;
- Git inventory and rename/copy result;
- symlink, junction, reparse-point, and containment result;
- persistence and byte-preservation result;
- verification-command non-execution result;
- unit, integration, platform-skipped, Phase 4 scenario, prior Phase 1–3 regression, and total test counts;
- dependency-diff result;
- authority-document-diff result;
- every verification command result;
- forbidden-path and forbidden-operation result;
- unresolved issues or `None`;
- recommendation `PASS` or `FAIL`.

`PASS` requires every contract requirement, negative behavior, preservation invariant, scope audit, and verification item. Passing tests alone is insufficient.

If no local symbolic remote-HEAD ref exists, report remote HEAD exactly as `UNAVAILABLE: no local remote HEAD; network forbidden`. Do not fetch or query the network to populate handoff evidence.

## 23. Contract boundary

This contract authorizes future Phase 4 implementation only within Sections 19 and 20.

It authorizes implementation-boundary enforcement, not implementation execution or correctness. It does not authorize Phase 5 audit behavior, Phase 6 closeout behavior, any later-phase behavior, commit, or push.
