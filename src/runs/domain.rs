//! Run aggregate and lifecycle value types.

pub mod diagram;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    /// Short human label shown in the run list; older runs have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub project: PathBuf,
    pub spec: Option<PathBuf>,
    pub nodes: Vec<Node>,
    pub cursor: usize,
    pub channel: Vec<ChannelEntry>,
    pub created_at: DateTime<Utc>,
}

/// Maximum display width of a title taken from an initial prompt.
const PROMPT_TITLE_WIDTH: usize = 80;

impl Run {
    /// Name an untitled run after the first nonblank line of its initial prompt.
    pub fn name_from_prompt(&mut self, prompt: &str) {
        if self.title.is_some() {
            return;
        }
        self.title = prompt
            .lines()
            .map(|line| line.trim().trim_start_matches('#').trim())
            .find(|line| !line.is_empty())
            .map(|line| truncate(line, PROMPT_TITLE_WIDTH));
    }

    /// Replace the title; a blank title removes it.
    pub fn rename(&mut self, title: &str) {
        let title = title.trim();
        self.title = (!title.is_empty()).then(|| title.to_owned());
    }

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

    fn run() -> Run {
        Run {
            id: "001".into(),
            title: None,
            project: PathBuf::from("/tmp/project"),
            spec: None,
            nodes: Vec::new(),
            cursor: 0,
            channel: Vec::new(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn initial_prompt_names_an_untitled_run_from_its_first_nonblank_line() {
        let mut run = run();
        run.name_from_prompt("\n  # Add CSV export  \nwith headers");
        assert_eq!(run.title.as_deref(), Some("Add CSV export"));
        run.name_from_prompt("Later revision");
        assert_eq!(run.title.as_deref(), Some("Add CSV export"));
    }

    #[test]
    fn prompt_title_is_truncated_by_display_width() {
        let mut run = run();
        run.name_from_prompt(&"界".repeat(60));
        assert_eq!(run.title, Some("界".repeat(40)));
        let mut blank = super::tests::run();
        blank.name_from_prompt("  \n ");
        assert_eq!(blank.title, None);
    }

    #[test]
    fn renaming_trims_the_title_and_blank_clears_it() {
        let mut run = run();
        run.rename("  Fix login  ");
        assert_eq!(run.title.as_deref(), Some("Fix login"));
        run.rename("   ");
        assert_eq!(run.title, None);
    }

    #[test]
    fn run_without_title_remains_deserializable() {
        let mut value = serde_json::to_value(run()).unwrap();
        value.as_object_mut().unwrap().remove("title");
        let run: Run = serde_json::from_value(value).unwrap();
        assert_eq!(run.title, None);
    }
}
