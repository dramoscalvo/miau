//! Declarative workflow configuration.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowConfig {
    pub nodes: Vec<WorkflowNode>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowNode {
    pub name: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub session_group: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub writes: Option<String>,
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub skip_if_no_python_changes: bool,
}
