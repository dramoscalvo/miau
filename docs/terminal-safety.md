# Terminal safety guide

Terminal handoff is a reliability boundary. The operator's shell must remain
usable even when an editor or interactive agent fails to start.

The required sequence is implemented in
`src/terminal/infrastructure/handoff.rs`:

1. Disable raw mode.
2. Leave the alternate screen and disable bracketed paste.
3. Show the cursor.
4. Run the child with inherited stdio and wait synchronously.
5. Re-enable raw mode.
6. Re-enter the alternate screen and bracketed paste.
7. Clear the terminal.
8. Reload artifact and state files from disk.

The TUI never enables mouse capture. Mouse capture would stop the terminal's
native text selection, so the operator could not select and copy text from the
screen. Do not add `EnableMouseCapture` to the setup, handoff, or restore paths.

The `!` shell handoff uses the same suspend and restore steps, but waits for the shell asynchronously. While the shell
runs, the loop keeps receiving agent events and applies them to state without drawing, so a running agent never blocks
on a full output pipe. It never polls terminal input during the handoff, so the shell receives every keystroke.
Interactive shells move themselves into their own process group, so Ctrl+C at the shell prompt does not reach miau
or the agent.

`miau op` runs in that shell, outside the TUI process, so a requested command and its `sudo` prompt never share the
terminal with raw mode. The TUI only reads request files on its tick and rings the bell for a new request.

Restoration must run when child creation or waiting returns an error. The panic
hook in `main.rs` must restore stdio before printing the panic report. Never add
a blocking child wait to the asynchronous TUI loop; editor and discussion
handoff is the explicit exception because drawing is suspended and no agent
runs.

For a streaming child, `q` first enters confirmation. Only confirmation with
`y` sends cancellation, persists a failure event, and exits. A killed process
must not leave a node silently marked `Running`.

When changing this code, smoke-test alternate-screen entry and exit in a PTY,
then exercise editor failure and repeated editor handoffs with a fake `$EDITOR`.
