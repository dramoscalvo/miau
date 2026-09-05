use crate::execution::application::{AgentAdapter, CommandSpec, Request};
use crate::execution::domain::{ChangeKind, Event, EventKind};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
pub struct CodexAdapter {
    agent: String,
    last_message: Arc<Mutex<Option<String>>>,
}
impl CodexAdapter {
    pub fn new(agent: impl Into<String>) -> Self {
        Self {
            agent: agent.into(),
            last_message: Arc::new(Mutex::new(None)),
        }
    }
}
impl AgentAdapter for CodexAdapter {
    fn agent_name(&self) -> &str {
        &self.agent
    }
    fn command(&self, req: &Request) -> CommandSpec {
        CommandSpec {
            bin: "codex".into(),
            args: Vec::new(),
            cwd: req.cwd.clone(),
        }
    }
    fn parse_line(&self, line: &str) -> Option<Event> {
        let v: Value = serde_json::from_str(line).ok()?;
        let ty = v.get("type")?.as_str()?;
        let item = v.get("item");
        let kind = match ty {
            "thread.started" => EventKind::SessionStarted {
                id: v.get("thread_id")?.as_str()?.to_owned(),
            },
            "turn.started" => EventKind::Progress("working".into()),
            "item.completed" | "item.started" | "item.updated" => {
                match item?.get("type")?.as_str()? {
                    "agent_message" => {
                        let text = item?
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        if let Ok(mut last) = self.last_message.lock() {
                            *last = Some(text.clone());
                        }
                        EventKind::Text(text)
                    }
                    "reasoning" => EventKind::Reasoning(
                        item?
                            .get("text")
                            .or_else(|| item?.get("summary"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    ),
                    "command_execution" => EventKind::ToolCall {
                        name: "command".into(),
                        summary: item?
                            .get("command")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    },
                    "file_change" => EventKind::FileChange {
                        path: PathBuf::from(item?.get("path")?.as_str()?),
                        kind: parse_change(item?.get("kind").and_then(Value::as_str)),
                    },
                    other => EventKind::ToolCall {
                        name: other.to_owned(),
                        summary: item?
                            .get("text")
                            .or_else(|| item?.get("name"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    },
                }
            }
            "turn.completed" => EventKind::Done {
                text: v
                    .get("result")
                    .or_else(|| v.get("final_output"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| self.last_message.lock().ok().and_then(|last| last.clone()))
                    .unwrap_or_default(),
                session_id: v
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            },
            "turn.failed" | "error" => EventKind::Failed {
                message: v
                    .pointer("/error/message")
                    .or_else(|| v.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("Codex failed")
                    .to_owned(),
            },
            _ => return None,
        };
        Some(Event::now(&self.agent, kind))
    }
}
fn parse_change(value: Option<&str>) -> ChangeKind {
    match value {
        Some("added") => ChangeKind::Added,
        Some("deleted") => ChangeKind::Deleted,
        _ => ChangeKind::Modified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_normalizes_codex_events() {
        let parser = CodexAdapter::new("codex");
        let events: Vec<_> = include_str!("../../../tests/fixtures/codex.jsonl")
            .lines()
            .filter_map(|l| parser.parse_line(l))
            .collect();
        assert!(matches!(&events[0].kind, EventKind::SessionStarted { id } if id == "thread-1"));
        assert!(matches!(&events[1].kind, EventKind::Progress(message) if message == "working"));
        assert!(
            events
                .iter()
                .any(|e| matches!(&e.kind, EventKind::Text(t) if t == "Critique"))
        );
        assert!(matches!(
            events.last().map(|e| &e.kind),
            Some(EventKind::Done { .. })
        ));
    }
}
