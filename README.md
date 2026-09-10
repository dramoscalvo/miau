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
| `v` | Cycle through Review summary, Complete document, and live Git changes. |
| Left / Right | Browse workflow agents and their artifacts. |
| `x` | Stop the running agent after confirmation. |
| `f` | Finish the run after confirmation, skipping unfinished steps. |
| `b` | Return to the run list once the agent has stopped. |

Run data stays in the project's `miaus/` directory. Revisions preserve prior artifact versions, and later agents read
current artifact files from disk. See the [usage guide](docs/usage.md) for prompt editing, discussions, navigation,
and revisiting steps.

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
