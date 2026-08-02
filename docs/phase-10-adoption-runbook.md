# Phase 10 — Activation, Rollback Drills, and Adoption Readiness: Operator Runbook

Status: **readiness evidence only**. This runbook describes rehearsals in isolated temporary pilots. It is not an installer, not a deployment guide, and not authorization to activate anything in a real repository.

## 1. Scope and explicit non-goals

Scope:

- execute the full public `mrgs` governance lifecycle as an **activation rehearsal** in an isolated temporary pilot repository;
- model operator-controlled binary selection with a temporary **activation slot** (`active/`, `backup/`, `evidence/`);
- execute **rollback drills** that restore a pilot repository and the activation slot to exact pre-activation bytes and Git state;
- produce deterministic readiness evidence labelled `ACTIVATION_REHEARSAL`.

Explicit non-goals (MRGS provides none of these, and Phase 10 must not imply otherwise):

- no installer, no updater, no uninstaller;
- no automatic rollback command or mechanism (`mrgs` has no rollback command);
- no service, daemon, scheduler entry, or remote-control surface;
- no production deployment automation;
- no network endpoint contact and no remote Git dependency;
- no modification of `PATH`, the registry, a package manager, a service manager, a shell profile, a user profile, or any global configuration;
- no administrator/root privilege requirement;
- no real-repository activation. Real activation, commit, push, and release publication each require **separate human authorization** (see Section 14).

## 2. Prerequisites and version recording

Supported prerequisites:

- a 64-bit Windows or POSIX host with a working shell;
- `git` (any version supporting `init -b`); `git --version` output is recorded;
- a Rust toolchain for the release build; `rustc --version` and `cargo --version` are recorded;
- the Phase 10 implementation repository at the candidate commit.

Record versions before starting:

```text
git --version
rustc --version
cargo --version
```

Pilot repository prerequisite (Windows especially): set local `core.autocrlf=false` in the pilot so worktree bytes and index bytes are identical under both the operator environment and MRGS's sanitized Git environment. Without this, Git may report modified files immediately after committing on hosts configured with global `core.autocrlf=true`. The pilot must also have no remote and no Git identity from the host environment; explicit local identity is set in Section 5.

## 3. Candidate identity checks

Record the candidate identities from the implementation repository before rehearsing:

```text
git rev-parse HEAD
```

- `BASELINE_HEAD` is the repository HEAD before Phase 10 implementation.
- `FINAL_HEAD` remains equal to `BASELINE_HEAD` until a separately authorized commit transaction; uncommitted Phase 10 docs and tests are reported as changed paths.
- `CONTRACT_SHA256` is the SHA-256 of the accepted `docs/contracts/phase-10-contract.md`.
- `CANDIDATE_SOURCE_TREE_SHA256` is computed from the canonical binary manifest in the Phase 10 contract (Section 5): for every tracked path and each authorized untracked Phase 10 path, emit `<mode> NUL <byte_size> NUL <sha256> NUL <path> NUL` in ascending repository-relative UTF-8 path-byte order (tracked modes from `git ls-files -s`, `100644` for new regular files, ASCII decimal byte size without leading zeroes, lowercase 64-hex SHA-256, forward-slash paths, no Unicode or case normalization), then SHA-256 the exact concatenated manifest bytes.

Binary identity:

- build the release candidate offline:

```text
CARGO_NET_OFFLINE=true cargo build --release --locked
```

- record the release binary hash and byte size **from the built file**, never from Cargo output:

```text
sha256sum target/release/mrgs
wc -c target/release/mrgs
```

(`Get-FileHash target\release\mrgs.exe -Algorithm SHA256` and `(Get-Item target\release\mrgs.exe).Length` on PowerShell.)

Every rehearsal must verify that the binary about to be used hashes to the recorded `RELEASE_BINARY_SHA256` before the first command.

## 4. Release build and activation-slot rehearsal

Create the temporary activation slot. The slot models operator-controlled binary selection; a copy into `active/` is **not** a release publication and must not be described as installation:

```text
mkdir -p <SLOT_PATH>/active <SLOT_PATH>/backup <SLOT_PATH>/evidence
```

Place the candidate binary into `active/` using **copy-to-temporary plus atomic replacement**: copy to a temporary file inside `active/`, then replace any previous `active/mrgs` with the temporary file (on Windows, remove the previous file first, then rename). Record the hash before and after placement; both must equal `RELEASE_BINARY_SHA256`:

```text
sha256sum target/release/mrgs
cp target/release/mrgs <SLOT_PATH>/active/.mrgs-candidate.tmp
rm -f <SLOT_PATH>/active/mrgs
mv <SLOT_PATH>/active/.mrgs-candidate.tmp <SLOT_PATH>/active/mrgs
sha256sum <SLOT_PATH>/active/mrgs
```

The pre-activation slot state (typically absent binaries in `active/` and `backup/`) is recorded as an explicit state before placement and is the state restored by a completed-rehearsal rollback drill.

Smoke through the active-slot path only:

```text
<SLOT_PATH>/active/mrgs --help
<SLOT_PATH>/active/mrgs plan accept --repo <REPOSITORY_PATH> --plan <PLAN_PATH>
```

Stop conditions: any nonzero exit code, any `error:` output, or a missing expected success line means STOP. Never continue a rehearsal past a failed boundary.

## 5. Pilot repository prerequisites

Create the pilot **only** in an isolated temporary directory. The real MRGS source repository must never be used as a governed runtime target.

```text
mkdir -p <PILOT_ROOT>
git init -b main <PILOT_ROOT>/<REPOSITORY_NAME>
```

Set explicit local Git identity and line-ending policy (no host identity, no remote):

```text
git -C <REPOSITORY_PATH> config user.name "Phase 10 Pilot"
git -C <REPOSITORY_PATH> config user.email "phase10@example.invalid"
git -C <REPOSITORY_PATH> config core.autocrlf false
```

Commit the pilot source and the `.mrgs/` ignore rule:

```text
printf '.mrgs/\n' > <REPOSITORY_PATH>/.gitignore
```

Create the source tree and commit it before any governance object exists. The pilot worktree must be clean (empty `git status --porcelain`) before activation.

Place the plan and contract TOML files **inside** the pilot and commit them. The plan must be strict TOML (`schema_version = 1`, unique `plan_id`, ordered phases with `depends_on`); the contract must be strict TOML with all required fields (`schema_version`, `contract_id`, `phase_id`, `title`, `objective`, `requirements`, `allowed_paths`, `forbidden_paths`, `verification_commands`, `handoff_fields`).

## 6. Pre-activation backup and evidence-location rules

Before any `.mrgs` object exists:

1. Record a complete pre-activation snapshot of the pilot: every repository-relative path with object kind (regular file, directory, symlink, or reparse-point classification), regular-file SHA-256 and byte size, symlink target bytes, Git `HEAD`, branch, refs, index, configuration, hooks, worktree porcelain, and untracked inventory. Absence of `.mrgs` is recorded as an explicit state.
2. Preserve the snapshot as the **sole validated backup** in the external evidence location. The backup is never deleted or modified during a rollback drill; it is only read.
3. Record the evidence location outside the pilot repository: `<EVIDENCE_DIR>`.

The evidence location must never contain absolute pilot paths, usernames, hostnames, environment secrets, access tokens, or source-file contents — only deterministic identifiers, hashes, byte sizes, command results, and relative paths.

Procedure-level preconditions before the first `mrgs` command (these are governed by this runbook, not by MRGS; a failing precondition aborts the drill before any `mrgs` invocation):

- backup recorded: the pre-activation snapshot exists and validates;
- pilot clean: `git status --porcelain` is empty;
- candidate identity: the binary to be activated hashes to the recorded `RELEASE_BINARY_SHA256`;
- no stale accepted authority: no `.mrgs` objects exist in the pilot.

## 7. Activation rehearsal — exact public CLI sequence

Run every command from the pilot root with the placeholders resolved. `<REPOSITORY_PATH>`, `<PLAN_PATH>`, `<CONTRACT_PATH>`, `<REPORT_PATH>`, `<METADATA_PATH>` are placeholders; they are never literal values.

```text
mrgs plan accept --repo <REPOSITORY_PATH> --plan <PLAN_PATH>
mrgs phase select --repo <REPOSITORY_PATH> --phase <PHASE_ID>
mrgs contract draft --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH>
mrgs contract accept --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256> --decision <DECISION>
mrgs implementation begin --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256>
mrgs implementation check --repo <REPOSITORY_PATH>
mrgs audit begin --repo <REPOSITORY_PATH> --auditor <AUDITOR_ID>
mrgs audit record --repo <REPOSITORY_PATH> --report <REPORT_PATH>
mrgs phase close --repo <REPOSITORY_PATH> --phase <PHASE_ID>
mrgs continuity record --repo <REPOSITORY_PATH> --metadata <METADATA_PATH>
mrgs recovery inspect --repo <REPOSITORY_PATH>
```

Notes on the sequence:

- `<REVISION>` and `<SHA256>` are the revision and SHA-256 printed by the preceding `contract draft` step; `<DECISION>` is exactly `ACCEPTED`.
- The audit report at `<REPORT_PATH>` must be **outside** the pilot repository and must cover every contract requirement and verification command exactly (one `requirement_results`/`verification_results` row per declared requirement/command), declare `independence_declaration = "INDEPENDENT"`, and set the verdict to `PASS`. The audit id and subject SHA-256 come from the `audit begin` output and must be embedded in the report.
- `<METADATA_PATH>` must be a strict-TOML file **inside** the pilot repository (outside `.git` and `.mrgs`) whose schema matches the continuity record requirements (`repository_id`, `continuity_id`, `phase_id`, the exact `completion_receipt_sha256` from `phase close`, `note`, and optional `models`, `hosts`, `links`). The continuity ledger archives the metadata bytes; remove the temporary metadata file after the record so the pilot worktree returns to its baseline.
- Every command is executed through the same compiled public binary — the slot `active/` binary in slot drills, the release candidate elsewhere. No helper-only production calls.

Expected success shapes (stop on any deviation or on any nonzero exit):

```text
test-plan <plan_sha256>
phase-1
test-contract-v1 <draft_sha256>
ACCEPTED test-contract-v1 1 <sha256>
IMPLEMENTATION_BOUND test-contract-v1 1 <sha256> <git_head>
IMPLEMENTATION_OK test-contract-v1 1 <sha256> <checked_count>
AUDIT_OPEN <audit_id> 1 <subject_sha256>
AUDIT_PASS <audit_id> 1 <subject_sha256>
PHASE_CLOSED phase-1 1 <final_manifest_sha256> <completion_receipt_sha256>
CONTINUITY_RECORDED <repository_id> phase-1 1 <continuity_manifest_sha256> <continuity_receipt_sha256>
RECOVERY_NOT_REQUIRED <subject_sha256>
```

Post-rehearsal checks:

- `mrgs recovery inspect` prints exactly one `RECOVERY_NOT_REQUIRED` line; a second inspection must be byte-identical.
- The surviving `.mrgs` objects are exactly `accepted-plan.json`, `state.json`, `completion-ledger.json`, and `continuity-ledger.json`; the phase-scoped ledgers are archived inside the final manifest.
- The pilot's source bytes and Git identity are unchanged; `git status --porcelain` is empty except the removal of the temporary metadata file.
- The rehearsal evidence is labelled `ACTIVATION_REHEARSAL`, never `PRODUCTION_ACTIVATED`.

## 8. Stop conditions

A drill stops immediately, and its failed evidence is preserved before any cleanup, when any of the following occurs:

- a command exits nonzero or prints `error:` to stderr;
- an expected success line is absent, truncated, or malformed;
- a backup fails validation (`BACKUP_MISSING`, `SNAPSHOT_CORRUPT`, `SNAPSHOT_TRUNCATED`, `SNAPSHOT_BIND_MISMATCH`, `RESTORE_VERIFY_MISMATCH`);
- a procedure precondition fails (`BACKUP_MISSING`, `PILOT_DIRTY`, `CANDIDATE_IDENTITY_MISMATCH`, `STALE_ACCEPTED_AUTHORITY`);
- any temporary file, partial ledger entry, or unclassified state is observed.

## 9. Rollback drills

Rollback is an operator-controlled external procedure. MRGS provides no rollback command.

### 9.1 Partial-activation rollback

After a drill that stopped before implementation begins (accepted plan, state, active phase, contract draft, and accepted contract exist):

1. Preserve the failed/partial activation evidence separately in `<EVIDENCE_DIR>` (hashes, labels, and command results only — no file contents, no absolute pilot paths).
2. Load the sole validated pre-activation backup.
3. Rebuild into a **fresh restore destination** (a sibling directory, never the live repository), then validate the destination byte-for-byte against the backup.
4. Replace the live pilot with the validated destination only after validation succeeds.
5. Prove post-restore equality: source, index, `HEAD`, branch, configuration, hooks, refs, tracked files, untracked files, and file kinds equal the pre-activation snapshot; `.mrgs` is absent if it was absent in the snapshot.
6. Repeat the restore once and prove fixed-point idempotency: the second run performs no rebuild and no replacement and yields the same bytes.

### 9.2 Completed-rehearsal rollback

After a complete Section 7 rehearsal:

1. Preserve the completed activation evidence in `<EVIDENCE_DIR>` **before** restoring anything.
2. Restore the pilot to its exact pre-activation snapshot (same procedure as 9.1).
3. Restore the activation slot to its exact pre-activation state (the recorded absent state, or the preserved `backup/` bytes).
4. Prove the restored repository and slot hashes are identical to the pre-activation hashes.
5. Rerun the snapshot comparison and obtain the same result.
6. Prove the evidence copy remains readable and unchanged.
7. Prove rollback does not mutate the MRGS source repository or any unrelated sentinel repository (compare `HEAD`, branch, refs, and porcelain before and after).

## 10. Post-rollback equality checks

After every rollback drill, verify, in order:

- the sole backup file is unchanged (same SHA-256 as when recorded);
- the restored pilot snapshot equals the pre-activation snapshot on every axis from Section 6.1;
- `.mrgs` absence equals the pre-activation state;
- the restored slot equals the pre-activation slot state;
- no residual temporary paths remain (fresh destination removed, markers removed);
- preserved evidence files are readable and unchanged;
- the MRGS source repository and any sentinel repository are untouched.

## 11. Privacy, secret-handling, path, network, and Git boundaries

- No network contact; no remote Git access; everything runs against local fixtures.
- No credentials, environment secrets, usernames, hostnames, home paths, or access tokens in evidence.
- Evidence contains only deterministic identifiers, hashes, byte sizes, command results, and relative paths; no absolute pilot, source, or evidence paths.
- No writes outside the isolated temporary fixtures except the explicitly supplied external evidence directory.
- The real MRGS source repository's Git state is never modified; rollback drills never touch sentinel repositories.
- No shell is invoked by MRGS production behavior; the drill uses the compiled public binary, Git, and operating-system filesystem APIs only.
- No sleeps, no random backoff, no wall-clock ordering, no host identity, and no model output are used as proof.

## 12. Evidence retention and disposal

- Evidence files are written to `<EVIDENCE_DIR>` during the drill and kept until a separately authorized disposal.
- Failed-drill evidence is preserved before fixture cleanup and labelled with the failing boundary.
- Backup artifacts are the sole validated copy of the pre-activation state; they are destroyed only by authorized disposal after the drill closes, never by the drill itself.
- Temporary fixtures (pilot, slot, restore destination, markers) are removed at drill cleanup.
- Evidence rows are emitted with the `ACTIVATION_REHEARSAL` label; `PRODUCTION_ACTIVATED` must never appear.

## 13. Known limitations

- MRGS supplies no installer, updater, rollback command, service, daemon, remote control, or production deployment automation.
- Activation-slot copies are not release publications.
- The snapshot and restore instrument in `tests/phase10.rs` is a test-only tool; it does not duplicate MRGS plan, contract, audit, closeout, continuity, or recovery algorithms.
- Restore of reparse points and other non-regular, non-symlink objects is not supported by the test instrument; pilots used in rollback drills must not contain them.
- Platform capability branches report `CAPABILITY_EXECUTED` or `CAPABILITY_NOT_COMPILED_FOR_TARGET`; silent skips are forbidden.
- Phase 10 readiness evidence is a rehearsal result: it proves the procedure, not a production deployment.

## 14. Separate human approvals

Each of the following requires its own explicit human decision and is never implied by Phase 10:

- real activation of `mrgs` authority in any non-pilot repository;
- committing Phase 10 deliverables to the source repository;
- pushing or publishing any repository or release artifact;
- executing a rollback in any real repository;
- disposing of evidence or backup artifacts.

## 15. PASS/FAIL checklist

Every item is binary. If any item is not PASS, the readiness outcome is FAIL — there is no "mostly ready".

- [ ] Versions recorded: `git --version`, `rustc --version`, `cargo --version`
- [ ] Candidate identities recorded: `BASELINE_HEAD`, `FINAL_HEAD`, `CONTRACT_SHA256`, `CANDIDATE_SOURCE_TREE_SHA256`
- [ ] Release candidate built offline and its file hash and byte size recorded from the file
- [ ] Release binary hash equals the recorded `RELEASE_BINARY_SHA256`
- [ ] Pilot created in an isolated temporary directory with explicit local identity, `core.autocrlf=false`, and no remote
- [ ] Pre-activation snapshot recorded with `.mrgs` absence explicit; sole validated backup preserved in `<EVIDENCE_DIR>`
- [ ] Procedure preconditions pass: backup recorded, pilot clean, candidate identity matches, no stale authority
- [ ] Full rehearsal sequence executed with exact expected outputs at every boundary
- [ ] `recovery inspect` healthy and deterministic (second inspection byte-identical)
- [ ] Surviving `.mrgs` objects exactly the four post-closeout ledgers
- [ ] Evidence labelled `ACTIVATION_REHEARSAL`; no absolute paths, secrets, usernames, hostnames, or source contents
- [ ] Partial-activation rollback restores exact pre-activation bytes and Git state; fixed-point repeat identical
- [ ] Completed-rehearsal rollback restores pilot and slot exactly; evidence readable and unchanged; source and sentinel repositories untouched
- [ ] No residual temporary paths; slot content removed at cleanup
- [ ] Separate human approvals documented for real activation, commit, push, release publication, and rollback execution
