mod claude;
mod codex;

use crate::execution::{
    application::{AgentAdapter, AgentConfig, CommandSpec, Request},
    domain::{Event, EventKind},
};
use std::{
    collections::VecDeque,
    path::Path,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use thiserror::Error;
use tokio::{
    fs::OpenOptions,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{mpsc, oneshot},
};

pub use claude::ClaudeAdapter;
pub use codex::CodexAdapter;

#[derive(Debug, Clone)]
pub struct ShellAdapter {
    name: String,
    command: Vec<String>,
}
impl ShellAdapter {
    pub fn new(name: impl Into<String>, command: Vec<String>) -> Self {
        Self {
            name: name.into(),
            command,
        }
    }
}
impl AgentAdapter for ShellAdapter {
    fn agent_name(&self) -> &str {
        &self.name
    }
    fn command(&self, req: &Request) -> CommandSpec {
        let executable = self
            .command
            .first()
            .cloned()
            .unwrap_or_else(|| "false".into());
        CommandSpec {
            bin: executable,
            args: self.command.iter().skip(1).cloned().collect(),
            cwd: req.cwd.clone(),
        }
    }
    fn parse_line(&self, line: &str) -> Option<Event> {
        Some(Event::now(&self.name, EventKind::Text(line.to_owned())))
    }
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("unknown parser: {0}")]
    UnknownParser(String),
    #[error("failed to create event log directory: {0}")]
    LogDirectory(#[source] std::io::Error),
    #[error("failed to spawn agent: {0}")]
    Spawn(#[source] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct ConfiguredAdapter {
    config: AgentConfig,
    agent: String,
    parser: Parser,
}

#[derive(Debug, Clone)]
enum Parser {
    Claude(ClaudeAdapter),
    Codex(CodexAdapter),
}

impl ConfiguredAdapter {
    pub fn new(agent: impl Into<String>, config: AgentConfig) -> Result<Self, AdapterError> {
        let agent = agent.into();
        let parser = match config.parser.as_str() {
            "claude" => Parser::Claude(ClaudeAdapter::new(agent.clone())),
            "codex" => Parser::Codex(CodexAdapter::new(agent.clone())),
            other => return Err(AdapterError::UnknownParser(other.to_owned())),
        };
        Ok(Self {
            config,
            agent,
            parser,
        })
    }

    fn render(value: &str, req: &Request) -> String {
        value
            .replace("{prompt}", &req.prompt)
            .replace("{session}", req.resume.as_deref().unwrap_or_default())
            .replace(
                "{schema}",
                &req.schema
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            )
    }

    fn expanded_args(&self, req: &Request) -> Vec<String> {
        let mut output: Vec<_> = self
            .config
            .args
            .iter()
            .map(|a| Self::render(a, req))
            .collect();
        let prompt_at = output
            .iter()
            .position(|a| a == &req.prompt)
            .unwrap_or(output.len());
        if req.schema.is_some() {
            let schema = self
                .config
                .schema_args
                .iter()
                .map(|a| Self::render(a, req))
                .collect::<Vec<_>>();
            output.splice(prompt_at..prompt_at, schema);
        }
        if req.resume.is_some() {
            let resume = self.config.resume_args.iter().map(|a| Self::render(a, req));
            let at = self
                .config
                .resume_insert_at
                .unwrap_or(prompt_at)
                .min(output.len());
            output.splice(at..at, resume);
        }
        output
    }
}

impl AgentAdapter for ConfiguredAdapter {
    fn agent_name(&self) -> &str {
        &self.agent
    }
    fn command(&self, req: &Request) -> CommandSpec {
        CommandSpec {
            bin: self.config.bin.clone(),
            args: self.expanded_args(req),
            cwd: req.cwd.clone(),
        }
    }
    fn parse_line(&self, line: &str) -> Option<Event> {
        match &self.parser {
            Parser::Claude(p) => p.parse_line(line),
            Parser::Codex(p) => p.parse_line(line),
        }
    }
}

pub struct RunningAgent {
    pub events: mpsc::Receiver<Event>,
    kill: Option<oneshot::Sender<()>>,
}

impl RunningAgent {
    pub fn kill(&mut self) -> bool {
        self.kill
            .take()
            .is_some_and(|sender| sender.send(()).is_ok())
    }
}

pub async fn spawn<A: AgentAdapter>(
    adapter: A,
    req: Request,
    raw_log: &Path,
) -> Result<RunningAgent, AdapterError> {
    if let Some(parent) = raw_log.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(AdapterError::LogDirectory)?;
    }
    let spec = adapter.command(&req);
    let mut command = Command::new(spec.bin);
    command.args(spec.args).current_dir(spec.cwd);
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(AdapterError::Spawn)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AdapterError::Spawn(std::io::Error::other("stdout pipe unavailable")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AdapterError::Spawn(std::io::Error::other("stderr pipe unavailable")))?;
    let (tx, rx) = mpsc::channel(256);
    let (kill_tx, mut kill_rx) = oneshot::channel();
    let done = Arc::new(AtomicBool::new(false));
    let done_reader = done.clone();
    let agent = adapter.agent_name().to_owned();
    let failure_agent = agent.clone();
    let log_path = raw_log.to_path_buf();
    let stdout_tx = tx.clone();
    tokio::spawn(async move {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .await;
        let mut file = match file {
            Ok(file) => file,
            Err(error) => {
                let _ = stdout_tx
                    .send(Event::now(
                        &agent,
                        EventKind::Failed {
                            message: error.to_string(),
                        },
                    ))
                    .await;
                return;
            }
        };
        let mut lines = BufReader::new(stdout).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if file
                        .write_all(format!("{line}\n").as_bytes())
                        .await
                        .is_err()
                    {
                        break;
                    }
                    if let Some(event) = adapter.parse_line(&line) {
                        if matches!(event.kind, EventKind::Done { .. }) {
                            done_reader.store(true, Ordering::Relaxed);
                        }
                        if stdout_tx.send(event).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = stdout_tx
                        .send(Event::now(
                            &agent,
                            EventKind::Failed {
                                message: error.to_string(),
                            },
                        ))
                        .await;
                    break;
                }
            }
        }
    });
    let stderr_task = tokio::spawn(async move {
        let mut tail = VecDeque::with_capacity(8);
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tail.len() == 8 {
                tail.pop_front();
            }
            tail.push_back(line);
        }
        tail.into_iter().collect::<Vec<_>>().join("\n")
    });
    tokio::spawn(async move {
        let killed = tokio::select! {
            status = child.wait() => Some(status),
            _ = &mut kill_rx => { let _ = child.kill().await; Some(child.wait().await) },
        };
        let stderr = stderr_task.await.unwrap_or_else(|e| e.to_string());
        match killed {
            Some(Ok(status)) if !status.success() && !done.load(Ordering::Relaxed) => {
                let message = if stderr.is_empty() {
                    status.to_string()
                } else {
                    stderr
                };
                let _ = tx
                    .send(Event::now(&failure_agent, EventKind::Failed { message }))
                    .await;
            }
            Some(Err(error)) => {
                let _ = tx
                    .send(Event::now(
                        &failure_agent,
                        EventKind::Failed {
                            message: error.to_string(),
                        },
                    ))
                    .await;
            }
            _ => {}
        }
    });
    Ok(RunningAgent {
        events: rx,
        kill: Some(kill_tx),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codex_config() -> AgentConfig {
        AgentConfig {
            bin: "codex".into(),
            args: vec!["exec".into(), "--json".into(), "{prompt}".into()],
            resume_args: vec!["resume".into(), "{session}".into()],
            resume_insert_at: Some(1),
            schema_args: vec!["--output-schema".into(), "{schema}".into()],
            discuss_args: vec![],
            parser: "codex".into(),
        }
    }

    #[test]
    fn codex_resume_command_matches_verified_cli_order() {
        let adapter = ConfiguredAdapter::new("codex", codex_config()).unwrap();
        let command = adapter.command(&Request {
            prompt: "continue".into(),
            cwd: ".".into(),
            resume: Some("thread-1".into()),
            schema: Some("schema.json".into()),
        });
        assert_eq!(
            command.args,
            [
                "exec",
                "resume",
                "thread-1",
                "--json",
                "--output-schema",
                "schema.json",
                "continue"
            ]
        );
    }

    #[tokio::test]
    async fn killed_process_emits_failure() {
        let log = std::env::temp_dir().join(format!("miau-kill-{}.jsonl", std::process::id()));
        let request = Request {
            prompt: String::new(),
            cwd: ".".into(),
            resume: None,
            schema: None,
        };
        let mut running = spawn(
            ShellAdapter::new("test", vec!["sh".into(), "-c".into(), "sleep 30".into()]),
            request,
            &log,
        )
        .await
        .unwrap();
        assert!(running.kill());
        let failed = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while let Some(event) = running.events.recv().await {
                if matches!(event.kind, EventKind::Failed { .. }) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap();
        assert!(failed);
        let _ = std::fs::remove_file(log);
    }
}
