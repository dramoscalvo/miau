//! Execution requests and outbound agent-adapter port.

use std::path::PathBuf;

use serde::Deserialize;

use super::domain::Event;

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub bin: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub resume_args: Vec<String>,
    #[serde(default)]
    pub resume_insert_at: Option<usize>,
    #[serde(default)]
    pub schema_args: Vec<String>,
    #[serde(default)]
    pub discuss_args: Vec<String>,
    pub parser: String,
}

impl AgentConfig {
    pub fn effort(&self) -> Option<&str> {
        self.effort.as_deref().or_else(|| {
            self.args
                .windows(2)
                .find(|pair| pair[0] == "--effort")
                .map(|pair| pair[1].as_str())
                .or_else(|| {
                    self.args.iter().find_map(|argument| {
                        argument
                            .strip_prefix("model_reasoning_effort=")
                            .map(|value| value.trim_matches(['\"', '\'']))
                    })
                })
        })
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub prompt: String,
    pub cwd: PathBuf,
    pub resume: Option<String>,
    pub schema: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub bin: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

pub trait AgentAdapter: Send + Sync + 'static {
    fn agent_name(&self) -> &str;
    fn command(&self, request: &Request) -> CommandSpec;
    fn parse_line(&self, line: &str) -> Option<Event>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(args: &[&str]) -> AgentConfig {
        AgentConfig {
            bin: "agent".into(),
            args: args.iter().map(|value| (*value).into()).collect(),
            effort: None,
            resume_args: vec![],
            resume_insert_at: None,
            schema_args: vec![],
            discuss_args: vec![],
            parser: "codex".into(),
        }
    }

    #[test]
    fn effort_reads_claude_argument_for_existing_configuration() {
        assert_eq!(config(&["--effort", "high"]).effort(), Some("high"));
    }

    #[test]
    fn effort_reads_codex_override_for_existing_configuration() {
        assert_eq!(
            config(&["-c", "model_reasoning_effort=\"medium\""]).effort(),
            Some("medium")
        );
    }
}
