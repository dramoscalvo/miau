//! Workflow use cases and prompt assembly.

use crate::{
    execution::domain::{Event, EventKind},
    runs::{
        application::{ArtifactRepository, RepositoryError, RunRepository, TextRepository},
        domain::{ChannelEntry, Decision, NodeStatus, Run},
    },
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("run has no current node")]
    NoCurrentNode,
    #[error("decision is invalid while node is {0:?}")]
    InvalidDecision(NodeStatus),
    #[error("run has no previous node")]
    NoPreviousNode,
    #[error("workflow node {0} does not exist")]
    InvalidNodeIndex(usize),
    #[error("node cannot be revisited while it is {0:?}")]
    InvalidRevisit(NodeStatus),
    #[error("a running node must be stopped before another node can be revisited")]
    RunIsActive,
}

pub struct Orchestrator<R> {
    repository: R,
    roles_dir: PathBuf,
}

impl<R: RunRepository + ArtifactRepository + TextRepository> Orchestrator<R> {
    pub fn new(repository: R, roles_dir: impl Into<PathBuf>) -> Self {
        Self {
            repository,
            roles_dir: roles_dir.into(),
        }
    }

    pub fn assemble_prompt(
        &self,
        run: &Run,
        note: Option<&str>,
    ) -> Result<String, OrchestratorError> {
        let node = run.current().ok_or(OrchestratorError::NoCurrentNode)?;
        let role_path = self.roles_dir.join(format!("{}.md", node.role));
        let role = self.repository.read_text(&role_path)?;
        let spec = self.spec_section(run)?;
        let feedback = run
            .current()
            .filter(|node| node.attempts > 0)
            .and_then(|_| {
                run.nodes
                    .get(run.cursor.saturating_add(1))
                    .filter(|node| node.status == NodeStatus::Done)
            });
        let previous = run
            .cursor
            .checked_sub(1)
            .and_then(|index| run.nodes.get(index));
        let input = feedback
            .or(previous)
            .map(|previous| {
                let path = self.repository.path(&run.id, &previous.writes)?;
                let text = self.repository.read(&run.id, &previous.writes)?;
                Ok::<_, RepositoryError>(format!("{}\n{}", path.display(), text))
            })
            .transpose()?
            .unwrap_or_else(|| "(none)".to_owned());
        Ok(format!(
            "# Task\n{role}\n\n# Spec\n{spec}\n\n# Input artifact\n{input}\n\n# Human note\n{}\n\n# Working directory\n{}\n",
            note.unwrap_or("(none)"),
            run.project.display()
        ))
    }

    fn spec_section(&self, run: &Run) -> Result<String, OrchestratorError> {
        let Some(path) = &run.spec else {
            return Ok("(none)".to_owned());
        };
        let absolute = if path.is_absolute() {
            path.clone()
        } else {
            run.project.join(path)
        };
        let text = self.repository.read_text(&absolute)?;
        let line_count = text.lines().count();
        if line_count < 200 {
            Ok(format!("{}\n{text}", absolute.display()))
        } else {
            Ok(absolute.display().to_string())
        }
    }

    pub fn begin(&self, run: &mut Run) -> Result<(), OrchestratorError> {
        let (agent, name) = {
            let node = run.current_mut().ok_or(OrchestratorError::NoCurrentNode)?;
            node.status = NodeStatus::Running;
            node.attempts += 1;
            (node.agent.clone(), node.name.clone())
        };
        run.channel.push(ChannelEntry::new(
            "you",
            Some(agent),
            format!("started {name}"),
        ));
        self.repository.save(run)?;
        Ok(())
    }

    pub fn session_for_current<'a>(&self, run: &'a Run) -> Option<&'a str> {
        let current = run.current()?;
        current.session_id.as_deref().or_else(|| {
            let group = current
                .session_group
                .as_deref()
                .filter(|group| !group.is_empty())?;
            run.nodes[..run.cursor].iter().rev().find_map(|node| {
                (node.agent == current.agent && node.session_group.as_deref() == Some(group))
                    .then_some(node.session_id.as_deref())
                    .flatten()
            })
        })
    }

    pub fn apply_event(
        &self,
        run: &mut Run,
        event: &Event,
        elapsed: Duration,
    ) -> Result<(), OrchestratorError> {
        let run_id = run.id.clone();
        let mut channel_entry = None;
        let node = run.current_mut().ok_or(OrchestratorError::NoCurrentNode)?;
        match &event.kind {
            EventKind::SessionStarted { id } => node.session_id = Some(id.clone()),
            EventKind::Done { text, session_id } => {
                node.writes = self
                    .repository
                    .write_versioned(&run_id, &node.writes, text)?;
                node.status = NodeStatus::Done;
                node.duration = Some(elapsed);
                if session_id.is_some() {
                    node.session_id.clone_from(session_id);
                }
                channel_entry = Some(ChannelEntry::new(
                    &event.agent,
                    Some("you".into()),
                    format!("{} ready for decision", node.name),
                ));
            }
            EventKind::Failed { message } => {
                node.status = NodeStatus::Failed;
                node.duration = Some(elapsed);
                channel_entry = Some(ChannelEntry::new(
                    &event.agent,
                    Some("you".into()),
                    format!("failed: {message}"),
                ));
            }
            _ => {}
        }
        if let Some(entry) = channel_entry {
            run.channel.push(entry);
        }
        self.repository.save(run)?;
        Ok(())
    }

    pub fn decide(
        &self,
        run: &mut Run,
        decision: Decision,
    ) -> Result<Option<String>, OrchestratorError> {
        let status = run
            .current()
            .ok_or(OrchestratorError::NoCurrentNode)?
            .status;
        if !matches!(status, NodeStatus::Done | NodeStatus::Failed) {
            return Err(OrchestratorError::InvalidDecision(status));
        }
        let result = match decision {
            Decision::Approve => {
                run.channel.push(ChannelEntry::new("you", None, "approved"));
                run.cursor = (run.cursor + 1).min(run.nodes.len());
                while run
                    .current()
                    .is_some_and(|node| node.status == NodeStatus::Skipped)
                {
                    run.cursor += 1;
                }
                if let Some(node) = run.current_mut()
                    && node.status == NodeStatus::Done
                {
                    node.status = NodeStatus::Pending;
                }
                None
            }
            Decision::Skip => {
                if let Some(node) = run.current_mut() {
                    node.status = NodeStatus::Skipped;
                }
                run.cursor = (run.cursor + 1).min(run.nodes.len());
                None
            }
            Decision::Revise { note } => {
                if let Some(node) = run.current_mut() {
                    node.status = NodeStatus::Pending;
                }
                run.channel
                    .push(ChannelEntry::new("you", None, format!("revision: {note}")));
                Some(note)
            }
            Decision::ReturnToPrevious => {
                if status != NodeStatus::Done {
                    return Err(OrchestratorError::InvalidDecision(status));
                }
                let previous = run
                    .cursor
                    .checked_sub(1)
                    .ok_or(OrchestratorError::NoPreviousNode)?;
                let from = run
                    .current()
                    .ok_or(OrchestratorError::NoCurrentNode)?
                    .name
                    .clone();
                let to = run
                    .nodes
                    .get_mut(previous)
                    .ok_or(OrchestratorError::NoPreviousNode)?;
                to.status = NodeStatus::Pending;
                let to = to.name.clone();
                run.cursor = previous;
                run.channel.push(ChannelEntry::new(
                    "you",
                    None,
                    format!("returned {from} to {to}"),
                ));
                None
            }
            Decision::Edit => {
                let node = run.current().ok_or(OrchestratorError::NoCurrentNode)?;
                self.repository.snapshot(&run.id, &node.writes)?;
                run.channel.push(ChannelEntry::new(
                    "you",
                    None,
                    format!("editing {}", node.writes),
                ));
                None
            }
        };
        self.repository.save(run)?;
        Ok(result)
    }

    pub fn revisit(
        &self,
        run: &mut Run,
        target: usize,
        note: String,
    ) -> Result<String, OrchestratorError> {
        if run
            .nodes
            .iter()
            .any(|node| node.status == NodeStatus::Running)
        {
            return Err(OrchestratorError::RunIsActive);
        }
        let node = run
            .nodes
            .get_mut(target)
            .ok_or(OrchestratorError::InvalidNodeIndex(target))?;
        if !matches!(node.status, NodeStatus::Done | NodeStatus::Failed) {
            return Err(OrchestratorError::InvalidRevisit(node.status));
        }
        let name = node.name.clone();
        let agent = node.agent.clone();
        node.status = NodeStatus::Pending;
        run.cursor = target;
        run.channel.push(ChannelEntry::new(
            "you",
            Some(agent),
            format!("revision for {name}: {note}"),
        ));
        self.repository.save(run)?;
        Ok(note)
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }
}

pub fn is_critic_role(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.contains("critic"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::{domain::Node, infrastructure::FileRepository};
    use chrono::Utc;
    use std::fs;
    fn fixture(root: &Path) -> Run {
        Run {
            id: "001".into(),
            project: root.into(),
            spec: None,
            cursor: 1,
            channel: vec![],
            created_at: Utc::now(),
            nodes: vec![
                Node {
                    name: "plan".into(),
                    agent: "claude".into(),
                    role: "planner".into(),
                    writes: "plan.md".into(),
                    status: NodeStatus::Done,
                    session_id: None,
                    session_group: None,
                    duration: None,
                    attempts: 1,
                    command: None,
                    skip_if_no_python_changes: false,
                },
                Node {
                    name: "critique".into(),
                    agent: "codex".into(),
                    role: "critic".into(),
                    writes: "critique.md".into(),
                    status: NodeStatus::Pending,
                    session_id: None,
                    session_group: None,
                    duration: None,
                    attempts: 0,
                    command: None,
                    skip_if_no_python_changes: false,
                },
            ],
        }
    }
    #[test]
    fn prompt_uses_canonical_artifact_not_memory() {
        let root = std::env::temp_dir().join(format!("miau-prompt-{}", std::process::id()));
        fs::create_dir_all(root.join("roles")).unwrap();
        fs::write(root.join("roles/critic.md"), "Find risks; do not decide.").unwrap();
        let repo = FileRepository::new(root.join("runs"));
        repo.write("001", "plan.md", "edited on disk").unwrap();
        let prompt = Orchestrator::new(repo, root.join("roles"))
            .assemble_prompt(&fixture(&root), Some("focus on data"))
            .unwrap();
        assert!(prompt.contains("edited on disk"));
        assert!(prompt.contains("focus on data"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn current_node_reuses_the_latest_session_in_its_group() {
        let root = std::env::temp_dir().join(format!("miau-session-{}", std::process::id()));
        let repo = FileRepository::new(root.join("runs"));
        let mut run = fixture(&root);
        run.nodes[0].agent = "codex".into();
        run.nodes[0].session_group = Some("delivery".into());
        run.nodes[0].session_id = Some("thread-1".into());
        run.nodes[1].session_group = Some("delivery".into());

        let session = Orchestrator::new(repo, root.join("roles")).session_for_current(&run);

        assert_eq!(session, Some("thread-1"));
    }

    #[test]
    fn current_node_does_not_reuse_a_group_from_another_agent() {
        let root = std::env::temp_dir().join(format!("miau-session-agent-{}", std::process::id()));
        let repo = FileRepository::new(root.join("runs"));
        let mut run = fixture(&root);
        run.nodes[0].session_group = Some("delivery".into());
        run.nodes[0].session_id = Some("session-1".into());
        run.nodes[1].session_group = Some("delivery".into());

        let session = Orchestrator::new(repo, root.join("roles")).session_for_current(&run);

        assert_eq!(session, None);
    }

    #[test]
    fn completed_rerun_versions_the_artifact_and_keeps_the_previous_result() {
        let root = std::env::temp_dir().join(format!("miau-versioned-{}", std::process::id()));
        let repo = FileRepository::new(root.join("runs"));
        repo.write("001", "plan.md", "first plan").unwrap();
        let mut run = fixture(&root);
        run.cursor = 0;
        run.nodes[0].status = NodeStatus::Running;
        run.nodes[0].attempts = 2;
        let orchestrator = Orchestrator::new(repo, root.join("roles"));

        orchestrator
            .apply_event(
                &mut run,
                &Event::now(
                    "claude",
                    EventKind::Done {
                        text: "second plan".into(),
                        session_id: None,
                    },
                ),
                Duration::from_secs(1),
            )
            .unwrap();

        assert_eq!(
            (
                orchestrator.repository().read("001", "plan.md").unwrap(),
                orchestrator.repository().read("001", "plan_v2.md").unwrap(),
                run.nodes[0].writes.as_str(),
            ),
            ("first plan".into(), "second plan".into(), "plan_v2.md")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn returning_to_previous_node_marks_it_pending_without_discarding_current_result() {
        let root = std::env::temp_dir().join(format!("miau-return-{}", std::process::id()));
        let repo = FileRepository::new(root.join("runs"));
        let mut run = fixture(&root);
        run.nodes[1].status = NodeStatus::Done;

        Orchestrator::new(repo, root.join("roles"))
            .decide(&mut run, Decision::ReturnToPrevious)
            .unwrap();

        assert_eq!(
            (run.cursor, run.nodes[0].status, run.nodes[1].status),
            (0, NodeStatus::Pending, NodeStatus::Done)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn approving_plan_returned_from_critique_requeues_critique() {
        let root = std::env::temp_dir().join(format!("miau-forward-{}", std::process::id()));
        let repo = FileRepository::new(root.join("runs"));
        let mut run = fixture(&root);
        run.nodes[1].status = NodeStatus::Done;
        run.nodes.push(Node {
            name: "implement".into(),
            agent: "codex".into(),
            role: "implementer".into(),
            writes: "implementation.md".into(),
            status: NodeStatus::Pending,
            session_id: None,
            session_group: None,
            duration: None,
            attempts: 0,
            command: None,
            skip_if_no_python_changes: false,
        });
        let orchestrator = Orchestrator::new(repo, root.join("roles"));
        orchestrator
            .decide(&mut run, Decision::ReturnToPrevious)
            .unwrap();
        run.nodes[0].status = NodeStatus::Done;

        orchestrator.decide(&mut run, Decision::Approve).unwrap();

        assert_eq!((run.cursor, run.nodes[1].status), (1, NodeStatus::Pending));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn revisited_node_prompt_reads_following_completed_artifact_from_disk() {
        let root = std::env::temp_dir().join(format!("miau-feedback-{}", std::process::id()));
        fs::create_dir_all(root.join("roles")).unwrap();
        fs::write(root.join("roles/planner.md"), "Revise the plan.").unwrap();
        let repo = FileRepository::new(root.join("runs"));
        repo.write("001", "critique.md", "canonical critique")
            .unwrap();
        let mut run = fixture(&root);
        run.cursor = 0;
        run.nodes[0].status = NodeStatus::Pending;
        run.nodes[0].attempts = 1;
        run.nodes[1].status = NodeStatus::Done;

        let prompt = Orchestrator::new(repo, root.join("roles"))
            .assemble_prompt(&run, None)
            .unwrap();

        assert!(prompt.contains("canonical critique"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn revisit_requeues_the_selected_completed_node_without_discarding_later_results() {
        let root = std::env::temp_dir().join(format!("miau-revisit-{}", std::process::id()));
        let repo = FileRepository::new(root.join("runs"));
        let mut run = fixture(&root);
        run.nodes[1].status = NodeStatus::Done;

        let note = Orchestrator::new(repo, root.join("roles"))
            .revisit(&mut run, 0, "consider retries".into())
            .unwrap();

        assert_eq!(
            (run.cursor, run.nodes[0].status, run.nodes[1].status, note),
            (
                0,
                NodeStatus::Pending,
                NodeStatus::Done,
                "consider retries".into()
            )
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn revisit_rejects_switching_away_from_a_running_node() {
        let root = std::env::temp_dir().join(format!("miau-active-{}", std::process::id()));
        let repo = FileRepository::new(root.join("runs"));
        let mut run = fixture(&root);
        run.nodes[1].status = NodeStatus::Running;

        let error = Orchestrator::new(repo, root.join("roles"))
            .revisit(&mut run, 0, "wait".into())
            .unwrap_err();

        assert!(matches!(error, OrchestratorError::RunIsActive));
        let _ = fs::remove_dir_all(root);
    }
}
