//! Run aggregate and lifecycle value types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub project: PathBuf,
    pub spec: Option<PathBuf>,
    pub nodes: Vec<Node>,
    pub cursor: usize,
    pub channel: Vec<ChannelEntry>,
    pub created_at: DateTime<Utc>,
}

impl Run {
    pub fn current(&self) -> Option<&Node> {
        self.nodes.get(self.cursor)
    }
    pub fn current_mut(&mut self) -> Option<&mut Node> {
        self.nodes.get_mut(self.cursor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    pub agent: String,
    pub role: String,
    pub writes: String,
    pub status: NodeStatus,
    pub session_id: Option<String>,
    #[serde(default)]
    pub session_group: Option<String>,
    pub duration: Option<Duration>,
    pub attempts: u32,
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub skip_if_no_python_changes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Approve,
    Revise { note: String },
    ReturnToPrevious,
    Edit,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelEntry {
    pub at: DateTime<Utc>,
    pub from: String,
    pub to: Option<String>,
    pub summary: String,
}

impl ChannelEntry {
    pub fn new(from: impl Into<String>, to: Option<String>, summary: impl AsRef<str>) -> Self {
        Self {
            at: Utc::now(),
            from: from.into(),
            to,
            summary: truncate(summary.as_ref(), 60),
        }
    }
}

fn truncate(value: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut used = 0;
    value
        .chars()
        .take_while(|c| {
            let next = used + c.width().unwrap_or(0);
            if next <= width {
                used = next;
                true
            } else {
                false
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn channel_summary_truncates_unicode_safely() {
        let entry = ChannelEntry::new("you", None, "á".repeat(61));
        assert_eq!(entry.summary.chars().count(), 60);
    }

    #[test]
    fn node_without_session_group_remains_deserializable() {
        let node: Node = serde_json::from_str(
            r#"{
                "name":"plan",
                "agent":"claude",
                "role":"planner",
                "writes":"plan.md",
                "status":"Pending",
                "session_id":null,
                "duration":null,
                "attempts":0
            }"#,
        )
        .unwrap();

        assert_eq!(node.session_group, None);
    }
}
