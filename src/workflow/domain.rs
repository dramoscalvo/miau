//! Workflow configuration and project change value types.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkingTreeChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingTreeChange {
    pub path: PathBuf,
    pub previous_path: Option<PathBuf>,
    pub kind: WorkingTreeChangeKind,
}

impl WorkingTreeChange {
    pub fn new(path: impl Into<PathBuf>, kind: WorkingTreeChangeKind) -> Self {
        Self {
            path: path.into(),
            previous_path: None,
            kind,
        }
    }

    pub fn renamed(previous_path: impl Into<PathBuf>, path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            previous_path: Some(previous_path.into()),
            kind: WorkingTreeChangeKind::Renamed,
        }
    }

    pub fn display_path(&self) -> &Path {
        &self.path
    }
}

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
