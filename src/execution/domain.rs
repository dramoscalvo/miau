//! Agent-independent execution events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub at: DateTime<Utc>,
    pub agent: String,
    pub kind: EventKind,
}

impl Event {
    pub fn now(agent: impl Into<String>, kind: EventKind) -> Self {
        Self {
            at: Utc::now(),
            agent: agent.into(),
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Progress(String),
    Text(String),
    Reasoning(String),
    ToolCall {
        name: String,
        summary: String,
    },
    FileChange {
        path: PathBuf,
        kind: ChangeKind,
    },
    SessionStarted {
        id: String,
    },
    Done {
        text: String,
        session_id: Option<String>,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}
