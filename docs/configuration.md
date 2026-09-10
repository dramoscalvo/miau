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

[[nodes]] name = "critique" agent = "codex" session_group = "delivery" role = "critic" writes = "critique.md"

[[nodes]]
name = "implement"
agent = "codex"
session_group = "delivery"
role = "implementer"
writes = "implementation.md"
```

Role files are plain Markdown instructions named after the workflow role, such as `roles/planner.md` for `role =
"planner"`.

Each agent returns one Markdown artifact with `# Review` and `# Handoff` sections. Review targets one screen: goal,
decisions/risks, a proposed or observed A/M/D/R file tree, short Given/When/Then cases with stable IDs, and verification
results. Handoff adds only technical constraints, evidence paths, unresolved issues, and the next task. Unchanged cases
and upstream plans are referenced instead of copied. This output contract is built into prompt assembly, so it also
applies to existing user role files without overwriting them.

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
