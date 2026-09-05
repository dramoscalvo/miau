# Terminal safety guide

Terminal handoff is a reliability boundary. The operator's shell must remain
usable even when an editor or interactive agent fails to start.

The required sequence is implemented in
`src/terminal/infrastructure/handoff.rs`:

1. Disable raw mode.
2. Leave the alternate screen and disable mouse capture.
3. Show the cursor.
4. Run the child with inherited stdio and wait synchronously.
5. Re-enable raw mode.
6. Re-enter the alternate screen and mouse capture.
7. Clear the terminal.
8. Reload artifact and state files from disk.

Restoration must run when child creation or waiting returns an error. The panic
hook in `main.rs` must restore stdio before printing the panic report. Never add
a blocking child wait to the asynchronous TUI loop; handoff is the explicit
exception because drawing is suspended.

For a streaming child, `q` first enters confirmation. Only confirmation with
`y` sends cancellation, persists a failure event, and exits. A killed process
must not leave a node silently marked `Running`.

When changing this code, smoke-test alternate-screen entry and exit in a PTY,
then exercise editor failure and repeated editor handoffs with a fake `$EDITOR`.
