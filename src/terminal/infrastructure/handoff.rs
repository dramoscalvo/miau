//! Safe terminal suspension and restoration around interactive children.

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, Stdout},
    process::{Command, ExitStatus, Stdio},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HandoffError {
    #[error("terminal handoff failed: {0}")]
    Io(#[from] io::Error),
}

pub fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    command: &mut Command,
) -> Result<ExitStatus, HandoffError> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;
    let child_result = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();
    let restore_result = restore(terminal);
    match (child_result, restore_result) {
        (Ok(status), Ok(())) => Ok(status),
        (Err(error), _) | (_, Err(error)) => Err(HandoffError::Io(error)),
    }
}

pub fn restore(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    terminal.clear()
}

pub fn restore_stdio() {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste
    );
}
