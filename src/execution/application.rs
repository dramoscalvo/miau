//! Execution requests and outbound agent-adapter port.

use std::path::PathBuf;

use serde::Deserialize;

use super::domain::Event;

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub bin: String,
    pub args: Vec<String>,
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
