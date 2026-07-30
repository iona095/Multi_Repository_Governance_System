# Multi-Repository Governance System

## Graphify reconnaissance

Use Graphify for repository reconnaissance when a task touches governance routing, agent selection, contracts, evidence gates, recovery paths, OpenCode/OMO configuration, or the tests surrounding those components.

Use the existing graph when available to inspect:

- architecture and ownership boundaries;
- affected nodes and reverse dependencies;
- paths between controllers, contracts, evidence, recovery, and tests;
- suspicious or unexpected relationships after a change.

Useful commands include:

```text
graphify query "<question>"
graphify path "<node>" "<node>"
graphify explain "<node>"
graphify affected "<node>"
```

After a successful code-change workflow, refresh an existing graph with:

```text
graphify update .
```

Graphify is an orientation and impact-analysis aid, not completion evidence. Never treat its graph, inferred edges, report, or refresh result as proof of correctness, test success, contract satisfaction, or audit completion. Required source inspection, exact tests, evidence gates, and auditor verdicts remain authoritative.

Graph refresh is non-gating. A Graphify warning, timeout, stale graph, or command failure must not change the implementation or audit verdict and must not trigger repair by itself.

Generated Graphify output belongs under `graphify-out/`, which is ignored by Git. Do not commit generated graph artifacts unless a separate task explicitly requires them.
