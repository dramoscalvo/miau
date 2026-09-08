# miau

A human-gated terminal orchestrator for Claude Code and Codex.

## Requirements

- A terminal on Linux, macOS, or Windows.
- A Rust toolchain with `cargo` available. Install one from
  [rustup.rs](https://rustup.rs/) if needed.
- At least one supported coding-agent CLI (`claude` or `codex`) installed,
  authenticated, and available on `PATH`.

## Install

From a checkout of this repository, run:

```sh
cargo install --path . --locked
```

Cargo installs the executable as `~/.cargo/bin/miau` by default. If the `miau`
command is not found, add Cargo's binary directory to your shell `PATH`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

Add that line to `~/.zshrc`, `~/.bashrc`, or the equivalent startup file for
your shell to keep it across sessions.

The installed executable contains the default configuration, so it does not
depend on the repository remaining at the same path. Reinstall after updating
the source checkout by running the same `cargo install` command.

<details>
<summary>A quest for the incurably curious (and adequately caffeinated)</summary>

> A second path exists, although it has filed the necessary paperwork to deny it.
>
> Seek the key among Cargo's optional powers, where improbable things wait for flags.
>
> Defaults stroll past without noticing, which is generally safest for everyone involved.
>
> Features awaken it; the first letters know its name and are feeling unbearably smug.

</details>

## First run

Change to the project that the agents should work on and start miau:

```sh
cd path/to/your/project
miau
```

On first launch, miau creates any missing default configuration files in the
platform user configuration directory. On Linux this is:

```text
${XDG_CONFIG_HOME:-$HOME/.config}/miau/
├── agents.toml
├── workflow.toml
└── roles/
    ├── critic.md
    ├── implementer.md
    ├── planner.md
    └── reviewer.md
```

Existing files are never overwritten, including after reinstalling miau. Run
data remains project-specific and is stored in `miaus/` under the directory from
which miau was started.

## Configure agents

Edit `agents.toml` in the user configuration directory to define each external
agent command. The generated file includes Claude Code and Codex examples:

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

`bin` may be a command on `PATH` or an absolute executable path. Arguments can
use the `{prompt}`, `{session}`, `{schema}`, and `{effort}` placeholders. The
optional `effort` value is shown beside the current agent on the main run list
and selected-run summary. Existing configurations without that field remain
compatible; miau recognizes Claude's `--effort` argument and Codex's
`model_reasoning_effort` override. The `parser` must currently be either
`claude` or `codex`.

## Configure the workflow

Edit `workflow.toml` to choose the ordered, human-gated steps. An agent node
selects an entry from `agents.toml`, reads its role prompt from `roles/`, and
writes an artifact into the run directory:

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

Role files are plain Markdown instructions named after the workflow role, such
as `roles/planner.md` for `role = "planner"`.

Each agent returns one Markdown artifact with `# Review` and `# Handoff`
sections. Review targets one screen: goal, decisions/risks, a proposed or observed
A/M/D/R file tree, short Given/When/Then cases with stable IDs, and verification
results. Handoff adds only technical constraints, evidence paths, unresolved
issues, and the next task. Unchanged cases and upstream plans are referenced
instead of copied. This output contract is built into prompt assembly, so it
also applies to existing user role files without overwriting them.

Later agents receive an index of completed upstream artifact paths, using their
current versions, plus a feedback reference when revisiting a step. They are
instructed to read relevant artifacts from disk, including the plan before
implementation or review. Reports and transcripts are not embedded in the
handoff prompt. Referenced file reads still consume tokens; savings depend on
which material the agent needs. No additional summary model call is made.

`session_group` is optional. A node with a group resumes the latest captured
session from an earlier node that has both the same group and the same configured
agent. Nodes without it start a fresh session, and re-running a node still
resumes that node's own session. The default workflow shares Codex's `delivery`
session between critique and implementation, where retained analysis is useful,
but keeps planning and review independent to avoid a self-review bias.

Session reuse avoids some repeated repository discovery and can improve prompt
cache hits, but it does not make the earlier transcript free: that history still
occupies context and may be reprocessed after cache expiry or CLI changes. Use a
group for closely related steps and leave unrelated or independence-sensitive
steps fresh.

Agent artifacts are append-only. Re-running a completed node keeps its prior
output and writes the next version beside it, such as `plan_v2.md` and
`plan_v3.md`. The run state and later workflow nodes use the newest version.

## Override configuration paths

The generated user configuration is the default. For a temporary or
project-specific setup, override any location explicitly:

```sh
miau \
  --agents ./miau-config/agents.toml \
  --workflow ./miau-config/workflow.toml \
  --roles ./miau-config/roles \
  --runs ./miaus
```

## Use the TUI

The initial screen summarizes each run's status, project, current workflow step,
progress, and latest activity time. Moving the selection updates the detail pane
with the next human action, current attempt and duration, and recent activity.
Press `n` to create a run, or Enter to open the selected run.

Before starting a pending node, press `p`, type an initial prompt, and press
Enter to save it; then press `s` to start the node. After an agent finishes, `p`
opens a revision prompt and Enter sends it immediately. The existing `r`
shortcut opens the same context-sensitive prompt box. The box opens in Vim-style
Normal mode and wraps and grows to one-third of the window. Use `i`, `a`, `I`,
or `A` to enter Insert mode, or `o`/`O` to open a new line below/above the
current line; `Esc` returns to Normal mode. In Normal mode, `h`,
`j`, `k`, `l` (or the arrow keys) move the cursor; `w`/`b`/`e` move by word;
and `W`/`B`/`E` move by whitespace-delimited WORD. Use `0`, `^`, and `$` for
line boundaries, `gg`/`G` for document boundaries, and `x` to delete a
character. Use `dd` to delete the current line, `D` to delete through the end
of the line, `yy` or `Y` to yank the current line, and `p` to paste the last
yanked or deleted text. The `diw`/`ciw`/`yiw` commands delete/change/yank the
word under the cursor; `daw`/`caw`/`yaw` include its adjacent whitespace, and
uppercase `W` works for each text object. Press `Esc` again from Normal mode to
cancel the prompt. In Insert mode, the terminal's clipboard shortcut (commonly
`Ctrl+Shift+V` or `Shift+Insert`) pastes system clipboard text, including
multiple lines. Normal-mode `p` only pastes text yanked or deleted within miau.
Use `a`, `e`, and `d` at a decision gate to approve, edit, or
open an interactive discussion. Press `f` and confirm to finish the run; miau
preserves completed steps and marks every unfinished step as skipped.

Use Left and Right to move between the workflow's agents. The highlighted row is
the agent being viewed; `>` still marks the current workflow gate. Each agent's
artifact and status remain available without changing workflow state. On a
completed or failed agent, press `p` to send a follow-up from the prompt box and
resume its captured session when available. This re-queues that workflow step
while preserving the results of later steps, which will be offered again in
order. Press `d` to open that agent's captured session interactively without
changing workflow state.

The project detail area starts with the selected agent's compact Review section.
Press `v` to cycle through Review, the full artifact (including Handoff), and
the project's live Git working tree. Use Up/Down or Page Up/Page Down to scroll
review/artifact text when it exceeds the pane. Old or incomplete artifacts
without the two headings fall back to their full text. Editing still opens the
single canonical artifact, and the preview reloads after the editor returns.
The one-screen target is guidance, not a truncation or approval rule; diagrams
and Gherkin are displayed as text, and scenarios are not automatically executed.

The live Git tree marks added
files in green, modified files in yellow, deleted files in red, and renamed
files in cyan. Use `j`/`k` or Up/Down to select a file and Page Up/Page Down to
scroll its unified diff. This view reports all current working-tree changes,
including changes that existed before the run; it does not yet attribute files
to an individual run or workflow step.

The Activity pane follows the newest activity by default. Press Tab to focus it,
then use Up, Down, Page Up, or Page Down to browse its history; scrolling down
returns toward the latest entry.

While an agent is running, the output pane title reports its latest normalized
activity, such as starting, thinking, writing a response, using a tool, or
changing a file. Workflow nodes use action-oriented states: `Working` means the
agent process is active, while `Waiting for you` means the artifact is ready and
miau needs your decision. If a run fails, its captured output remains visible
under `Needs attention` instead of being replaced by an empty artifact.

When a revisited step finishes, approving it queues the completed following
step again, so a revised plan is critiqued again before implementation.

From a project screen, press `b` to return to the initial run list. While an
agent is running, stop it first; the project screen remains visible so active
work cannot be hidden accidentally. Press `x` and confirm with `y` to stop the
agent without quitting miau. The interrupted node remains at its decision
gate.

## Development

Run directly from the repository with:

```sh
cargo run
```

For adapter diagnostics:

```sh
cargo run -- debug run --agent claude --prompt "Reply briefly"
```

Run the complete verification suite before submitting changes:

```sh
cargo fmt --check
cargo test --offline --locked
cargo clippy --offline --all-targets --all-features --locked -- -D warnings
cargo build --offline --release --locked
```

## Architecture

The code is organized by bounded context, then by architectural layer:

```text
src/
├── runs/       domain · application ports · filesystem infrastructure
├── execution/  normalized event domain · adapter port · CLI infrastructure
├── workflow/   workflow domain · orchestration use cases · TOML/git infrastructure
└── terminal/   application model/update · ratatui/crossterm infrastructure
```

Dependencies point inward: infrastructure may use application and domain;
application may use domain; domain never imports an outer layer. Contexts
communicate through application contracts and public domain types. Automated
architecture tests protect these rules.

Current product defaults are deliberately conservative: run data lives in the
project's `miaus/` directory, edit snapshots are numbered files, and the project
view keeps a 3/4–1/4 flow/activity ratio. The initial run list uses a 2/3–1/3
split so its selected-run summary remains readable. These presentation choices
can evolve without changing domain policy; `--runs` already overrides the run
location.

## License

Copyright (C) 2026 David Ramos Calvo.

miau is free software: you can redistribute it and/or modify it under the terms
of the GNU General Public License as published by the Free Software Foundation,
either version 3 of the License, or (at your option) any later version.

See [LICENSE](LICENSE) for the full license text.
