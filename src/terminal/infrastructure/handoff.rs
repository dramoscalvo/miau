//! Safe terminal suspension and restoration around interactive children.
//!
//! The TUI never enables mouse capture, so the terminal keeps its native text
//! selection and copy behaviour.

use crate::execution::{
    domain::{
        Event,
        operator::{OperatorRequest, summary},
    },
    infrastructure::{RunningAgent, operator::DIR_ENV},
};
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    style::Print,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, Stdout},
    path::Path,
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
    if let Err(error) = suspend(terminal) {
        let _ = restore(terminal);
        return Err(error.into());
    }
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

pub fn suspend(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()
}

/// Starts the operator's `$SHELL` (or `sh`) on the suspended terminal.
///
/// The caller must suspend the terminal first and keep draining agent events
/// with [`next_shell_wait`] so a running agent never blocks on a full pipe.
pub fn spawn_shell(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    cwd: &Path,
    agent_running: bool,
    operator: Option<(&Path, &[OperatorRequest])>,
) -> io::Result<tokio::process::Child> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
    let agent = if agent_running {
        " The agent keeps running."
    } else {
        ""
    };
    execute!(
        terminal.backend_mut(),
        Print(format!(
            "miau: shell in {}.{agent} Type `exit` to return.\r\n",
            cwd.display()
        ))
    )?;
    let mut command = tokio::process::Command::new(shell);
    if let Some((directory, requests)) = operator {
        execute!(
            terminal.backend_mut(),
            Print(operator_notice(requests).replace('\n', "\r\n"))
        )?;
        command.env(DIR_ENV, directory);
    }
    command
        .current_dir(cwd)
        .env("MIAU_SHELL", "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
}

/// What the shell banner says about commands the agent asked for.
pub fn operator_notice(requests: &[OperatorRequest]) -> String {
    if requests.is_empty() {
        return "miau: no agent request is waiting. `miau op` lists requests that arrive later.\n"
            .into();
    }
    let mut notice = String::from(
        "miau: the agent waits for you to run these commands. Read each one before you run it.\n",
    );
    for request in requests {
        notice.push_str(&summary(request));
        notice.push('\n');
    }
    notice
}

#[derive(Debug)]
pub enum ShellWait {
    Exited(io::Result<ExitStatus>),
    Agent(Option<Event>),
}

/// Waits for the shell to exit or for the next agent event, whichever comes first.
pub async fn next_shell_wait(
    shell: &mut tokio::process::Child,
    running: &mut Option<RunningAgent>,
) -> ShellWait {
    tokio::select! {
        status = shell.wait() => ShellWait::Exited(status),
        event = async {
            match running {
                Some(agent) => agent.events.recv().await,
                None => std::future::pending().await,
            }
        } => ShellWait::Agent(event),
    }
}

pub fn restore(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableBracketedPaste
    )?;
    terminal.clear()
}

pub fn restore_stdio() {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(stdout, LeaveAlternateScreen, DisableBracketedPaste);
}

#[cfg(test)]
mod tests {
    use super::{ShellWait, next_shell_wait, operator_notice};
    use crate::execution::{
        application::Request,
        domain::{EventKind, operator::OperatorRequest},
        infrastructure::{ShellAdapter, spawn},
    };
    use std::time::Duration;

    #[test]
    fn the_shell_banner_lists_each_waiting_request_with_its_command_line() {
        let request = |id| OperatorRequest {
            id,
            token: format!("t{id}"),
            argv: vec!["sudo".into(), "true".into()],
            cwd: "/work".into(),
            cancelled: false,
        };
        let notice = operator_notice(&[request(1), request(2)]);
        assert!(notice.contains("[1] sudo true") && notice.contains("miau op 1"));
        assert!(notice.contains("[2] sudo true") && notice.contains("miau op 2"));
        assert!(operator_notice(&[]).contains("no agent request is waiting"));
    }

    #[tokio::test]
    async fn shell_wait_delivers_agent_events_before_the_shell_exits() {
        let log = std::env::temp_dir().join(format!("miau-shell-{}.jsonl", std::process::id()));
        let request = Request {
            prompt: String::new(),
            cwd: ".".into(),
            resume: None,
            schema: None,
            env: vec![],
        };
        let mut running = Some(
            spawn(
                ShellAdapter::new("agent", vec!["sh".into(), "-c".into(), "echo hi".into()]),
                request,
                &log,
            )
            .await
            .unwrap(),
        );
        let mut shell = tokio::process::Command::new("sh")
            .args(["-c", "sleep 0.5"])
            .spawn()
            .unwrap();

        let mut order = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match next_shell_wait(&mut shell, &mut running).await {
                    ShellWait::Agent(Some(event)) => {
                        if let EventKind::Text(text) = event.kind {
                            order.push(text);
                        }
                    }
                    ShellWait::Agent(None) => running = None,
                    ShellWait::Exited(status) => {
                        assert!(status.unwrap().success());
                        order.push("exited".into());
                        break;
                    }
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(order, ["hi", "exited"]);
        let _ = std::fs::remove_file(log);
    }
}
