# miau

A human-gated terminal orchestrator for Claude Code and Codex. Run a workflow one agent at a time, review its
Markdown artifacts, and decide when to continue or request changes.

## Quick start

You need a terminal on Linux, macOS, or Windows, a [Rust toolchain](https://rustup.rs/), and at least one supported
agent CLI (`claude` or `codex`) installed, authenticated, and on `PATH`.

Install from a checkout of this repository, then launch in the project you want the agents to work on:

```sh
cargo install --path . --locked
cd path/to/your/project
miau
```

Ensure Cargo's binary directory (usually `~/.cargo/bin`) is on your `PATH`. Re-run the install command after updating
the source; the installed executable does not depend on the checkout remaining in place.

## Workflow

Press `n` to create a run or Enter to open one. Press `p` to write an initial prompt, `i` to enter Insert mode,
Enter to save, and `s` to start the step. After each agent finishes, choose what happens next:

| Key | Action |
| --- | --- |
| `a` | Approve the result and start the next step, or complete the workflow. |
| `r` | Request changes; Enter submits feedback and starts a new artifact version. |
| `d` | Discuss in the agent's interactive session, then optionally request an updated artifact. |
| `e` | Edit the artifact in `$EDITOR`; saving does not approve the step. |
| `v` | Cycle through Decisions, Review, Complete document, Git changes, and optional Diagram views. |
| Left / Right | Browse workflow agents and their artifacts. |
| `x` | Stop the running agent after confirmation. |
| `f` | Finish the run after confirmation, skipping unfinished steps. |
| `b` | Return to the run list once the agent has stopped. |

Run data stays in the project's `miaus/` directory. Revisions preserve prior artifact versions, and later agents read
current artifact files from disk. See the [usage guide](docs/usage.md) for prompt editing, discussions, navigation,
and revisiting steps.

## Architecture diagrams

UML is requested only after implementation. The TUI shows the whole change on one pannable canvas, with each class
appearing once, member compartments, and connected UML arrows. Press `g` to open it, `j`/`k` to select a class,
`}` to select a relationship, Enter to follow it, and Backspace to return. Use `H`/`J`/`K`/`L` to pan and Home to reset.
Left/Right compares workflow steps while retaining class selection. Existing reports are preserved; use Request changes
to ask for an updated class diagram if an older report contains only a dependency graph.

Extract a TypeScript project's module dependencies (the default) or semantic type relationships into a review
artifact:

```sh
miau diagram typescript --project tsconfig.json --root . --output architecture.md
miau diagram typescript --project tsconfig.json --root . --scope types --output types.md
```

Generation needs Node.js 18+ and TypeScript 5.6–6.x installed in the target project. It uses the compiler's parser and
module resolution, with no agent, model, or API key. Unchanged inputs and compiler/runtime environment produce the
same sorted output; source references, extraction warnings, and an input fingerprint accompany the graph. Type scope
extracts classes, abstract classes, interfaces, enums, inheritance, implements, declared property associations, and
declared operation dependencies. It does not infer runtime calls, ownership, aggregation, composition, framework
injection, or architecture layers. The command creates a new file and refuses to overwrite one.

miau's default workflow runs a `--scope types --changed` command after implementation, including new and edited
TypeScript files and their directly related types. See the
[workflow configuration](docs/configuration.md#add-a-typescript-diagram-step) to change the project scope or command.
Open cited sources in your editor with `o`; use `n` to save notes on classes and `S` to request changes with those notes.
Module-only graphs retain the hierarchy browser, with Enter to drill into children.
When no TypeScript files changed, the command records that no graph applies. Every workflow gate still waits for your
decision. The [diagram guide](docs/diagrams.md) covers setup, reproducibility, controls, limitations, and the optional
artifact format for proposed designs.

Outside a Git worktree, the changed-file step reports that the changed files cannot be determined and produces no
diagram. It still returns to the human review gate. Missing Git and other Git errors remain failures.

## Configuration

First launch creates `agents.toml`, `workflow.toml`, and Markdown role prompts in your platform's user configuration
directory (`${XDG_CONFIG_HOME:-$HOME/.config}/miau/` on Linux). Existing files are never overwritten.

Customize agent commands, workflow steps, roles, and session reuse in the [configuration guide](docs/configuration.md).
For project-specific configuration, override the paths:

```sh
miau --agents ./miau-config/agents.toml --workflow ./miau-config/workflow.toml --roles ./miau-config/roles --runs ./miaus
```

## Development

Use `cargo run` to run from the checkout. See the [contributor guide](AGENTS.md), [architecture](docs/architecture.md),
[testing guide](docs/testing.md), and [terminal safety notes](docs/terminal-safety.md). The [landing page](site/README.md)
has its own development and publishing instructions.

Before submitting changes:

```sh
cargo fmt --check
cargo test --offline --locked
cargo clippy --offline --all-targets --all-features --locked -- -D warnings
cargo build --offline --release --locked
```

For adapter diagnostics, run `cargo run -- debug run --agent claude --prompt "Reply briefly"`.

## License

Copyright (C) 2026 David Ramos Calvo. Licensed under [GNU GPL v3 or later](LICENSE).
