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
args = ["--model", "opus", "--effort", "high", "-p", "{prompt}", "--output-format", "stream-json", "--verbose"]
resume_args = ["--resume", "{session}"]
discuss_args = ["--resume", "{session}"]
parser = "claude"

[codex]
bin = "codex"
args = ["exec", "--json", "--model", "gpt-5.6-sol", "-c", "model_reasoning_effort=\"medium\"", "{prompt}"]
resume_args = ["resume", "{session}"]
resume_insert_at = 1
schema_args = ["--output-schema", "{schema}"]
discuss_args = ["resume", "{session}"]
parser = "codex"
```

`bin` may be a command on `PATH` or an absolute executable path. Arguments can
use the `{prompt}`, `{session}`, and `{schema}` placeholders. The `parser` must
currently be either `claude` or `codex`.

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
name = "implement"
agent = "codex"
role = "implementer"
writes = "implementation.md"
```

Role files are plain Markdown instructions named after the workflow role, such
as `roles/planner.md` for `role = "planner"`.

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
character. The `diw`/`ciw` commands delete/change the word under the cursor;
`daw`/`caw` include its adjacent whitespace, and uppercase `W` works for each
text object. Press `Esc` again from Normal mode to cancel the prompt. Use `a`,
`e`, and `d` at a decision gate to approve, edit, or open an interactive
discussion.

Use Left and Right to move between the workflow's agents. The highlighted row is
the agent being viewed; `>` still marks the current workflow gate. Each agent's
artifact and status remain available without changing workflow state. On a
completed or failed agent, press `p` to send a follow-up from the prompt box and
resume its captured session when available. This re-queues that workflow step
while preserving the results of later steps, which will be offered again in
order. Press `d` to open that agent's captured session interactively without
changing workflow state.

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
