# Configuration

[Back to README](../README.md)

On first launch, miau creates missing `agents.toml`, `workflow.toml`, and `roles/` files in the platform user
configuration directory (`${XDG_CONFIG_HOME:-$HOME/.config}/miau/` on Linux). Existing files are never overwritten,
including after reinstalling.

## Configure agents

Edit `agents.toml` in the user configuration directory to define each external agent command. The generated file
includes Claude Code and Codex examples:

```toml
[claude]
bin = "claude"
effort = "high"
args = ["--model", "opus", "--effort", "{effort}", "-p", "{prompt}", "--output-format", "stream-json", "--verbose"]
resume_args = ["--resume", "{session}"]
discuss_args = ["--resume", "{session}"]
parser = "claude"

[codex]
bin = "codex"
effort = "medium"
args = ["exec", "--json", "--model", "gpt-5.6-sol", "-c", "model_reasoning_effort=\"{effort}\"", "{prompt}"]
resume_args = ["resume", "{session}"]
resume_insert_at = 1
schema_args = ["--output-schema", "{schema}"]
discuss_args = ["resume", "{session}"]
parser = "codex"
```

`bin` may be a command on `PATH` or an absolute executable path. Arguments can use the `{prompt}`, `{session}`,
`{schema}`, and `{effort}` placeholders. The optional `effort` value is shown beside the current agent on the main run
list and selected-run summary. Existing configurations without that field remain compatible; miau recognizes Claude's
`--effort` argument and Codex's `model_reasoning_effort` override. The `parser` must currently be either `claude` or
`codex`.

## Configure the workflow

Edit `workflow.toml` to choose the ordered, human-gated steps. An agent node selects an entry from `agents.toml`, reads
its role prompt from `roles/`, and writes an artifact into the run directory:

```toml
[[nodes]]
name = "plan"
agent = "claude"
role = "planner"
writes = "plan.md"

[[nodes]]
name = "critique"
agent = "codex"
session_group = "delivery"
role = "critic"
writes = "critique.md"

[[nodes]]
name = "implement"
agent = "codex"
session_group = "delivery"
role = "implementer"
writes = "implementation.md"
```

Role files are plain Markdown instructions named after the workflow role, such as `roles/planner.md` for `role =
"planner"`.

Each agent returns one Markdown artifact with `# Review` and `# Handoff` sections. Plans use a feature document in
Review: Objective, Scope (in/out), Behavior, Tests (Unit/Integration/E2E Given/When/Then scenarios with stable IDs),
Assumptions, and References. Implementation reports describe the delivered feature using the same sections and add
Implemented plan with shipped changes, design decisions, deviations, changed files, and verification results. The
approved upstream plan stays intact. Critiques and reviews use Findings, Verification, and References instead of
repeating the feature. Human decisions appear before these sections using the dedicated decision format.

Documents have no one-screen limit or mandatory file tree. Handoff adds only information needed by the next agent
and the observed implementation diagram when applicable. Run metadata stays in miau rather than YAML frontmatter.
This output contract is built into prompt assembly, so it also applies to existing user role files without
overwriting them.

Later agents receive an index of completed upstream artifact paths, using their current versions, plus a feedback
reference when revisiting a step. They are instructed to read relevant artifacts from disk, including the plan before
implementation or review. Reports and transcripts are not embedded in the handoff prompt. Referenced file reads still
consume tokens; savings depend on which material the agent needs. No additional summary model call is made.

`session_group` is optional. A node with a group resumes the latest captured session from an earlier node that has both
the same group and the same configured agent. Nodes without it start a fresh session, and re-running a node still
resumes that node's own session. The default workflow shares Codex's `delivery` session between critique and
implementation, where retained analysis is useful, but keeps planning and review independent to avoid a self-review
bias.

Session reuse avoids some repeated repository discovery and can improve prompt cache hits, but it does not make the
earlier transcript free: that history still occupies context and may be reprocessed after cache expiry or CLI changes.
Use a group for closely related steps and leave unrelated or independence-sensitive steps fresh.

Agent artifacts are append-only. Re-running a completed node keeps its prior output and writes the next version beside
it, such as `plan_v2.md` and `plan_v3.md`. The run state and later workflow nodes use the newest version.

## Add a TypeScript diagram step

Insert this command node after implementation and before the final agent review to generate a deterministic graph of
the changed TypeScript files and their directly related types:

```toml
[[nodes]]
name = "impacted-diagram"
command = ["miau", "diagram", "typescript", "--scope", "types", "--changed", "--project", "tsconfig.json", "--root", "."]
writes = "impacted-diagram.md"
```

`--changed` reads Git's working-tree status, including staged, unstaged, and untracked files. The graph contains those
TypeScript files, their supported top-level types, and types with a direct relationship to a changed type. It does not
include second-degree relationships. Deleted files are omitted. Git must be available, and the command analyzes the
full selected TypeScript project to resolve relationships before narrowing the graph. The fingerprint covers the
compiler inputs and selected changed files. If no TypeScript files changed, the step writes a short “no diagram
applies” artifact and completes normally. Existing unrelated edits are included because miau does not attribute Git
changes to a specific workflow step.

Changed paths are restricted to `--root` and normalized relative to it, including when the project is inside a larger
Git repository. Renames use their destination paths. Node and relationship limits apply to the filtered graph, so
unrelated types do not exhaust its display limits. If the project is not a Git worktree, the step emits an explanatory
artifact without a graph and returns to human review. This means the changed files could not be determined; it does
not assert that there were no changes. Missing Git and other Git errors remain failures.

Remove `--changed` to generate the full project graph. The default scope is `modules`; `--scope types` selects
compiler-derived classes, interfaces, enums, inheritance, implementations, associations, and operation dependencies.

The command runs in the project's directory. Install `miau` and Node.js 18+ on `PATH`, and install the project's locked
dependencies, including TypeScript 5.6–6.x. Omit `--output`: miau captures stdout as the node's versioned artifact.
Command nodes need no `agent` or `role`; this one uses no model or API key. Workflow changes apply to newly created
runs. The checked-in default workflow places this step after implementation; add the same node to existing user
workflow files because miau preserves those files instead of overwriting them.

Start the step with `s`, inspect its report and Diagram view with `v`, then approve with `a` when ready to continue.
Completion of extraction does not approve the result. Keep `--root` as the run's project directory so source links
resolve correctly. For a monorepo, choose a leaf config such as `packages/app/tsconfig.json` rather than a config with
project references. See the [diagram guide](diagrams.md) for scope, warnings, and deterministic output guarantees.

An existing agent step may also run the extractor and include its generated graph in its report. The agent chooses
the config and explains the results; the compiler determines the observed relationships. Preserve generated metadata
and regenerate from source instead of asking an agent to repair individual edges. Proposed designs belong in a
separate diagram marked `proposed`.

## Override configuration paths

The generated user configuration is the default. For a temporary or project-specific setup, override any location
explicitly:

```sh
miau \
  --agents ./miau-config/agents.toml \
  --workflow ./miau-config/workflow.toml \
  --roles ./miau-config/roles \
  --runs ./miaus
```
