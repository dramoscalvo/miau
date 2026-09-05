use crate::execution::application::{AgentAdapter, CommandSpec, Request};
use crate::execution::domain::{Event, EventKind};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ClaudeAdapter {
    agent: String,
}
impl ClaudeAdapter {
    pub fn new(agent: impl Into<String>) -> Self {
        Self {
            agent: agent.into(),
        }
    }
}

impl AgentAdapter for ClaudeAdapter {
    fn agent_name(&self) -> &str {
        &self.agent
    }
    fn command(&self, req: &Request) -> CommandSpec {
        CommandSpec {
            bin: "claude".into(),
            args: Vec::new(),
            cwd: req.cwd.clone(),
        }
    }
    fn parse_line(&self, line: &str) -> Option<Event> {
        let value: Value = serde_json::from_str(line).ok()?;
        let kind = match value.get("type")?.as_str()? {
            "system" if value.get("subtype").and_then(Value::as_str) == Some("init") => {
                EventKind::SessionStarted {
                    id: value.get("session_id")?.as_str()?.to_owned(),
                }
            }
            "system" if value.get("subtype").and_then(Value::as_str) == Some("thinking_tokens") => {
                let estimate = value.get("estimated_tokens").and_then(Value::as_u64)?;
                EventKind::Progress(format!("thinking (~{estimate} tokens)"))
            }
            "assistant" => {
                let blocks = value.pointer("/message/content")?.as_array()?;
                let text = blocks
                    .iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("");
                if text.is_empty() {
                    return None;
                }
                EventKind::Text(text)
            }
            "result"
                if value
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false) =>
            {
                EventKind::Failed {
                    message: value
                        .get("result")
                        .and_then(Value::as_str)
                        .unwrap_or("Claude failed")
                        .to_owned(),
                }
            }
            "result" => EventKind::Done {
                text: value
                    .get("result")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                session_id: value
                    .get("session_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            },
            _ => return None,
        };
        Some(Event::now(&self.agent, kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_normalizes_session_text_and_done() {
        let parser = ClaudeAdapter::new("claude");
        let events: Vec<_> = include_str!("../../../tests/fixtures/claude.jsonl")
            .lines()
            .filter_map(|l| parser.parse_line(l))
            .collect();
        assert!(matches!(&events[0].kind, EventKind::SessionStarted { id } if id == "session-1"));
        assert!(
            matches!(&events[1].kind, EventKind::Progress(message) if message == "thinking (~50 tokens)")
        );
        assert!(matches!(&events[2].kind, EventKind::Text(text) if text == "A plan"));
        assert!(
            matches!(&events[3].kind, EventKind::Done { session_id: Some(id), .. } if id == "session-1")
        );
    }
}
