# Multi-Repository Governance System

Phase 1 — Accepted Plan Authority and Phase Selection
Phase 2 — Active-Phase Contract Draft Registration

## CLI

```
mrgs plan accept --repo <REPOSITORY_PATH> --plan <PLAN_PATH>
mrgs phase select --repo <REPOSITORY_PATH> --phase <PHASE_ID>
mrgs contract draft --repo <REPOSITORY_PATH> --contract <CONTRACT_PATH>
```

## Commands

### `plan accept`

Accepts a governance plan for a repository. Creates `.mrgs/accepted-plan.json` and `.mrgs/state.json`.

### `phase select`

Selects an active phase from an accepted plan. Requires all phase dependencies to be closed.

### `contract draft`

Registers a contract draft for the currently active phase. Requires an existing accepted plan and an active phase. The contract source must be a strict TOML file inside the repository (outside `.mrgs`). Persists the exact source content and its SHA-256 into `.mrgs/contract-draft.json`. Idempotent — re-drafting the exact same bytes returns success without writing.

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
