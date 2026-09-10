# miau contributor guide for AI agents

Read this file before changing the project. It is the operational contract for
future automated contributors.

## Product boundary

`miau` is a human-gated terminal orchestrator for coding-agent CLIs. It assembles
prompts, starts exactly one external process, persists artifacts and state, and
waits for a human at every decision gate.

Do not add automatic approval/rejection routing, concurrent agents, worktrees,
branch management, locking, transcript forwarding, an embedded terminal, an
LLM client, a web UI, a daemon, or a network server.

OpenCode is intentionally not part of the current product. Do not add an
OpenCode adapter or configuration until explicitly requested.

## Architecture

The code is organized by bounded context:

```text
src/runs/       run lifecycle and persistence
src/execution/  normalized events and external CLI processes
src/workflow/   workflow configuration and orchestration policy
src/terminal/   TUI model, updates, rendering, and handoff
```

Each context has `domain`, `application`, and `infrastructure` layers. Keep
dependencies pointing inward:

```text
infrastructure → application → domain
```

Use repository/port traits in application layers and concrete filesystem,
process, TOML, or terminal implementations in infrastructure. Never import
ratatui/crossterm into domain or workflow policy. Never import Tokio process
types into application ports. See [docs/architecture.md](docs/architecture.md).

## Source-of-truth rules

- Artifact files on disk are canonical. Never use cached artifact text as the
  authority after an editor handoff.
- Persist `state.json` atomically through `state.json.tmp` followed by rename.
- Preserve run directories; never delete them automatically.
- Handoffs pass artifact paths, not raw agent transcripts.
- Every adapter emits `execution::domain::Event`; UI and workflow code must not
  branch on provider-specific JSON.

## Safe Rust and terminal rules

- No `unwrap()` or `expect()` in event loops, adapters, handoff code, or any
  path reachable while raw mode is enabled. Return errors and surface them.
- No `println!`, `dbg!`, or ad-hoc stdout logging. TUI stdout belongs to the
  backend; debug output must be intentional and structured.
- Keep one agent process alive at most. Cancellation must leave persisted state
  consistent and visible.
- Always restore raw mode and the alternate screen on child errors and panics.
- Truncate display text by Unicode width; never slice strings by byte index.
- Keep UI code free of process spawning and adapter code free of ratatui.

## Documentation formatting

Hard-wrap Markdown prose with actual newline characters at word boundaries,
aiming for 120 characters per line and keeping lines within 150 characters where
practical. Preserve paragraph breaks and list indentation. Exempt code blocks,
tables, URLs, paths, and other indivisible tokens when wrapping would damage
their meaning or Markdown syntax.

## Development workflow

Use a test-first loop:

1. Add or update a failing focused test.
2. Implement the smallest behavior that makes it pass.
3. Refactor while preserving the test.
4. Run the complete verification commands before handing off.

The full testing policy is in [docs/testing.md](docs/testing.md). For normal
changes run:

```sh
cargo fmt --check
cargo test --offline --locked
cargo clippy --offline --all-targets --all-features --locked -- -D warnings
cargo build --offline --release --locked
```

Do not call real agent CLIs from tests. Use committed JSONL fixtures or a fake
script configured through `agents.toml`.

## Change checklist

Before finishing a change, confirm:

- The change belongs to the correct bounded context and layer.
- New behavior has a deterministic test.
- Architecture tests still pass.
- State/artifact/error behavior is documented where non-obvious.
- No forbidden scope or dependency direction was introduced.
- User-facing behavior and any remaining limitation are stated in the handoff.
