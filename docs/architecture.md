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
The output contract asks agents to hard-wrap prose near 120 characters, staying
within 150 where practical, with exceptions for syntax-sensitive content such
as code, tables, and links. This is a writing instruction; persistence does not
reformat agent output or existing artifacts.

The shared contract gives human questions stable `D1`, `D2`, etc. headings within
Review, with explicit Open or Resolved status. Workflow application code parses
these questions independently of agent providers and terminal rendering. The
terminal presents open questions and edits answer drafts; explicit submission
uses the existing revision/revisit use case and never approves the workflow.
Drafts are stored through the artifact repository as `decision-drafts-N.json`,
scoped to a run and step. Each answer is associated with the full question text,
preventing reuse when an artifact edit changes the question. Old drafts remain
on disk. Submission re-reads the artifact and requires review if questions have
changed since display. The versioned feedback file preserves submitted questions
and answers before the agent starts. No transcript forwarding is involved.

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

Mutable artifact-repository writes, including answer drafts, use a sibling
`.tmp` file followed by rename so a failed write preserves the previous file.
Versioned agent artifacts and feedback retain their existing exclusive-create
behavior. Draft save errors propagate through terminal cleanup and are surfaced
to the operator; the last successfully saved draft remains available.

Human revision requests are preserved in full as versioned `feedback-N.md`
files in the run directory (N is the zero-based workflow step index), before
re-queuing the step. The activity channel remains a compact summary. A revision
prompt references the current artifact path so manual edits are read from disk;
a failed prior attempt may have no artifact. Feedback files are history, not
an automatic approval or routing mechanism.

A successful interactive discussion returns to an editable update request.
Only explicit submission enters the existing revision use case; cancelling
leaves the workflow gate unchanged. The resumed session supplies discussion
context, without transcript forwarding. Disk edits made during the interactive
session remain canonical even when the update request is cancelled.
