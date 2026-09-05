# Testing guide

Tests must be deterministic, cheap, and independent of paid agent services.
The test suite is part of the architecture: tests should exercise ports and
domain behavior without requiring a terminal or network.

## Test layers

### Domain and application unit tests

Prefer pure inputs and in-memory fakes. Cover:

- decision transitions and invalid decisions;
- prompt envelope assembly and human-note handling;
- Unicode-safe truncation and pane scrolling;
- repository round-tripping and numbered snapshots;
- cancellation and stale-running recovery invariants.

### Adapter parser tests

Commit representative JSONL lines under `tests/fixtures/`. Each parser test
should assert the normalized `Event` kind and important payload fields:
session ID, text, tool call, file change, done, and failure. Include malformed
or irrelevant lines where the correct result is `None`.

### Integration tests

Point an agent configuration at a local shell script that emits a fixture with
a small delay. Assert persisted `state.json`, artifact content, session ID,
raw event tee, and failure behavior. The orchestrator must not be able to tell
the script from a real adapter process.

Do not invoke Claude, Codex, or any other paid/networked agent from `cargo test`.

## TDD workflow

For each behavior:

1. Write the smallest test that describes the contract.
2. Run it and confirm it fails for the intended reason.
3. Implement the behavior through the appropriate port/use case.
4. Refactor names and ownership after it passes.
5. Run all tests, clippy, and the release build.

Avoid rendered-frame snapshots until layout and wording have stabilized. Test
layout calculations and model transitions directly first.

## Required verification

```sh
cargo fmt --check
cargo test --offline --locked
cargo clippy --offline --all-targets --all-features --locked -- -D warnings
cargo build --offline --release --locked
```

When a test needs a temporary directory, use a unique path and clean it up at
the end of that test. Never clean a real `miaus/` directory as part of tests.
