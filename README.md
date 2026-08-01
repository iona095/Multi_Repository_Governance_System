# Multi-Repository Governance System

Phase 1 — Accepted Plan Authority and Phase Selection
Phase 2 — Active-Phase Contract Draft Registration
Phase 3 — Contract Acceptance, Revision, and Lifecycle Transitions
Phase 4 — Contract-Bound Implementation Enforcement
Phase 5 — Independent Audit and Bounded Repair Routing
Phase 6 — Phase Closeout and Completion Ledger
Phase 7 — Model, Host, and Cross-Repository Continuity Metadata
Phase 8 — State Recovery and Corruption Handling

## CLI

```
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

## Commands

### `plan accept`

Accepts a governance plan for a repository. Creates `.mrgs/accepted-plan.json` and `.mrgs/state.json`.

### `phase select`

Selects an active phase from an accepted plan. Requires all phase dependencies to be closed.

### `contract draft`

Registers a contract draft for the currently active phase. Requires an existing accepted plan and an active phase. The contract source must be a strict TOML file inside the repository (outside `.mrgs`). Persists the exact source content and its SHA-256 into `.mrgs/contract-draft.json`. Idempotent — re-drafting the exact same bytes returns success without writing. Initial draft is always revision 1.

### `contract accept`

Accepts the current contract draft. Requires the exact revision, SHA-256, and `ACCEPTED` decision. Creates or appends to `.mrgs/accepted-contract.json`. Prints `ACCEPTED <contract_id> <revision> <sha256>`. Idempotent for the current accepted revision.

### `phase close`

Closes the active phase: validates the full Phase 1-5 authority chain, archives the phase-scoped governance files into a final manifest, builds a chained completion receipt, and atomically publishes `.mrgs/completion-ledger.json`. Prints `PHASE_CLOSED <phase_id> <completion_sequence> <final_manifest_sha256> <completion_receipt_sha256>`. Idempotent for a completed phase.

### `continuity record`

Records deterministic, privacy-minimal continuity metadata for a completed phase into the append-only `.mrgs/continuity-ledger.json`. The `--metadata` file is strict TOML (schema version 1) with explicit `repository_id`, `continuity_id`, `phase_id`, the exact `completion_receipt_sha256` of a closed phase, `note`, `models`, `hosts`, and optional `links`. Each link is a `continues_from` predecessor relation that is verified locally against `--source-repo` repositories (completion proof, and optionally the source continuity receipt). The exact metadata bytes are archived with a deterministic continuity manifest and chained continuity receipt. Exact replay of an identical record returns the original output and preserves every byte; conflicts and ledger corruption fail closed. No host or model discovery, telemetry, network access, or Git mutation is performed. Prints `CONTINUITY_RECORDED <repository_id> <phase_id> <continuity_sequence> <continuity_manifest_sha256> <continuity_receipt_sha256>`.

### `recovery inspect`

Read-only deterministic diagnosis of the exact governance subject: repository identity, a canonical inventory of every `.mrgs` child (except the recovery journal), and the plan source. Returns `RECOVERY_NOT_REQUIRED <subject_sha256>` for a healthy repository, `RECOVERY_REQUIRED <recovery_id> <subject_sha256> <action_count>` plus one `RECOVERY_ACTION <n> <kind> <target>` line per deterministic action for a recoverable subject, or `RECOVERY_PENDING <recovery_id> <next_action> <action_count>` while a journal entry is incomplete. Unrecoverable subjects fail closed with the exact recovery error category and no success output.

### `recovery apply`

Human-authorized application of a recovery plan bound to an exact recovery ID and subject SHA-256 (both lowercase 64-hex; the only accepted decision is exact `RECOVER`). The plan is recomputed from surviving Phase 1-7 authority, never from caller input. The pending entry is durably published to the append-only `.mrgs/recovery-ledger.json` before the first mutation; each action (`REMOVE_REDUNDANT_TEMP`, `RESTORE_ACCEPTED_PLAN`, `RESTORE_STATE`, `RESUME_CLOSEOUT`) is executed resumably against deterministic prefix subject hashes, with `next_action` advanced atomically. Completion writes a chained recovery receipt and prints `RECOVERY_APPLIED <recovery_sequence> <recovery_id> <pre_subject_sha256> <post_subject_sha256> <recovery_receipt_sha256>`. Exact replay of an applied recovery returns the original output; conflicts, stale subjects, and corrupt or stale journals fail closed before any mutation.

### `contract revise`

Creates a new contract draft revision. Uses compare-and-swap: requires `expected-revision` and `expected-sha256` equal to the current draft. The new revision is `expected-revision + 1`. Prints `DRAFT <contract_id> <revision> <sha256>` or `REVISION_DRAFT <contract_id> <revision> <sha256>` depending on whether an accepted contract exists.

## Contract lifecycle

The lifecycle is inferred from validated governance files:

- **DRAFT**: `contract-draft.json` exists, no `accepted-contract.json`
- **ACCEPTED**: `accepted-contract.json` exists, its final revision equals the current draft revision and content
- **REVISION_DRAFT**: `accepted-contract.json` exists, its final revision is lower than the current draft revision

## Contract format

```toml
schema_version = 1
contract_id = "<unique-id>"
phase_id = "<active-phase-id>"
title = "<human-readable title>"
objective = "<one-line objective>"

requirements = ["...", "..."]
allowed_paths = ["src/", ...]
forbidden_paths = [".git/", ...]
verification_commands = ["cargo test", ...]
handoff_fields = ["FIELD1", ...]
```

All top-level fields are required. Unknown top-level fields are rejected. Scalar fields must not be empty or whitespace-only. List fields must have at least one entry with no empty or duplicate entries.
