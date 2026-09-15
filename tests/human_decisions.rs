use miau::{
    execution::{
        application::AgentAdapter,
        infrastructure::{ClaudeAdapter, CodexAdapter},
    },
    runs::{
        application::ArtifactRepository,
        domain::{Decision, Node, NodeStatus, Run},
        infrastructure::FileRepository,
    },
    terminal::application::{DetailView, Model},
    workflow::application::Orchestrator,
};
use std::time::Duration;

#[test]
fn critique_questions_from_both_providers_reach_answer_fields_and_revision_feedback() {
    let root = std::env::temp_dir().join(format!("miau-provider-decisions-{}", std::process::id()));
    let providers: [(Box<dyn AgentAdapter>, &str); 2] = [
        (
            Box::new(ClaudeAdapter::new("claude")),
            include_str!("fixtures/critique-claude.jsonl"),
        ),
        (
            Box::new(CodexAdapter::new("codex")),
            include_str!("fixtures/critique-codex.jsonl"),
        ),
    ];
    for (adapter, fixture) in providers {
        let repo = FileRepository::new(root.join(adapter.agent_name()));
        let orchestrator = Orchestrator::new(repo, "roles");
        let mut run = Run {
            id: "001".into(),
            project: root.clone(),
            spec: None,
            cursor: 0,
            channel: vec![],
            created_at: chrono::Utc::now(),
            nodes: vec![Node {
                name: "critique".into(),
                agent: adapter.agent_name().into(),
                role: "critic".into(),
                writes: "critique.md".into(),
                status: NodeStatus::Running,
                session_id: None,
                session_group: None,
                duration: None,
                attempts: 1,
                command: None,
                skip_if_no_python_changes: false,
            }],
        };
        for event in fixture.lines().filter_map(|line| adapter.parse_line(line)) {
            orchestrator
                .apply_event(&mut run, &event, Duration::ZERO)
                .unwrap();
        }
        assert_eq!(run.nodes[0].status, NodeStatus::Done);
        assert_eq!(
            orchestrator.session_for_current(&run),
            Some("critique-session")
        );
        let artifact = orchestrator
            .repository()
            .read(&run.id, &run.nodes[0].writes)
            .unwrap();
        let mut model = Model::default();
        model.load_decisions(&artifact);
        assert_eq!(model.detail_view, DetailView::Decisions);
        assert!(model.decision_error.is_none());
        assert_eq!(model.questions.len(), 1);
        assert_eq!(model.questions[0].title, "Preserve existing settings?");
        model
            .decision_drafts
            .set(&model.questions[0], "Preserve settings.".into());
        let feedback = model.decision_drafts.feedback(&model.questions).unwrap();
        orchestrator
            .decide(
                &mut run,
                Decision::Revise {
                    note: feedback.clone(),
                },
            )
            .unwrap();
        assert_eq!(
            orchestrator
                .repository()
                .read(&run.id, "feedback-0.md")
                .unwrap(),
            feedback
        );
        assert!(feedback.contains("## D1: Preserve existing settings?"));
        assert!(feedback.contains("Human answer:\nPreserve settings."));
        assert_eq!(run.cursor, 0);
        assert_eq!(run.nodes[0].status, NodeStatus::Pending);
    }
    std::fs::remove_dir_all(root).unwrap();
}
