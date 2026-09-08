# Architecture guide

## Context map

`runs` is the persistence-backed aggregate. A `Run` owns nodes, cursor
position, decisions, sessions, durations, and the human-facing activity log.
Its application layer defines `RunRepository`, `ArtifactRepository`, and
`TextRepository`; its infrastructure layer implements them with atomic JSON and
plain files.

`execution` is the anti-corruption layer around external CLIs. Native output
is parsed once into the normalized `Event` domain type. The application layer
exposes `Request`, `CommandSpec`, and `AgentAdapter`; Tokio process spawning,
line reading, raw event teeing, cancellation, and provider parsers live in
infrastructure.

`workflow` owns declarative workflow nodes and the human-gated orchestration
use case. Prompt assembly reads role/spec/artifact content through repository
ports. Severity in critique/review output is presentation data only; it must
never control a transition.

Working-tree inspection is exposed through the workflow application's
`WorkingTreeRepository` port. Git status parsing and diff process execution live
in workflow infrastructure, while the terminal consumes only normalized change
kinds and paths.

Agent output uses one Markdown artifact with a human `# Review` section and an
additional `# Handoff` section. Workflow prompt assembly includes the output
contract and an index of current completed upstream artifact paths, plus any
following revision feedback. It does not inline those reports. Agents must read
relevant files from disk even when resuming a session; missing required inputs
must be reported. The terminal extracts Review for display only, preserving the
full artifact for editing and legacy-output fallback. Neither section controls
approval, and length targets are advisory so findings are never discarded.

Workflow nodes may opt into a named session group. At process start, the
orchestrator selects only the newest captured session from an earlier node with
the same group and configured agent. The session ID remains part of persisted
run state; the execution layer only receives the resolved opaque ID and renders
the provider-specific resume arguments.

`terminal` is a driving adapter rather than a business domain. Its application
layer contains the pure `Model`, `Message`, and `Model::update` state machine.
Its infrastructure layer owns EventStream input, ratatui rendering, terminal
handoff, and effectful calls into the other contexts.

## Dependency rules

Allowed:

```text
runs/infrastructure   → runs/application → runs/domain
execution/infrastructure → execution/application → execution/domain
workflow/infrastructure → workflow/application → workflow/domain
terminal/infrastructure → terminal/application
```

Cross-context dependencies must target a public domain type or application
port. Examples: workflow may consume run repository ports and execution events;
terminal may coordinate workflow and execution. A domain module must not import
an application or infrastructure module.

The integration test [tests/architecture.rs](../tests/architecture.rs) protects
the most important inward-dependency rules. Extend it when a new context or
layer is added.

## Adding an agent

Implement the adapter/parser in `src/execution/infrastructure/`, add fixture
coverage under `tests/fixtures/`, and register only the infrastructure mapping
needed to construct it. Do not change UI or workflow transition code. The
adapter must emit existing normalized event variants; adding a provider must
not leak native JSON into the rest of the application.

Verify the installed CLI's `--help` output before committing command flags.
Never substitute an unverified flag. Keep unverified providers out of the
default configuration.

## Persistence invariants

The filesystem repository is the source of truth. Writes to `state.json` use a
temporary file and rename. Artifact edits are read again from disk after an
editor handoff. Agent artifact writes use the next unused `_vN` filename and
never overwrite an existing result. Snapshots use the next unused numbered
suffix and are never overwritten. A stale `Running` node loaded after a reboot
becomes a visible failure requiring a human decision.
