# Multi-Repository Governance System

Phase 1 — Accepted Plan Authority and Phase Selection
Phase 2 — Active-Phase Contract Draft Registration
Phase 3 — Contract Acceptance, Revision, and Lifecycle Transitions

## CLI

```
mrgs plan accept --repo <REPOSITORY_PATH> --plan <PLAN_PATH>
mrgs phase select --repo <REPOSITORY_PATH> --phase <PHASE_ID>
mrgs contract draft --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH>
mrgs contract accept --repo <REPOSITORY_PATH> --revision <REVISION> --sha256 <SHA256> --decision <DECISION>
mrgs contract revise --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH> --expected-revision <REVISION> --expected-sha256 <SHA256>
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
