use super::super::*;
use std::fs;

const DOCUMENT: &str = "# Review\n## D1: Storage?\nStatus: Open\nRecommendation: TOML.\n## D2: Migrate?\nStatus: Open\nRecommendation: No.\n# Handoff\nDetails.";

struct Fixture {
    root: PathBuf,
    app: App,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "miau-answer-ui-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        fs::write(root.join("agents.toml"), format!(
            "[fake]\nbin = '/bin/sh'\nparser = 'claude'\nargs = ['-c', 'printf \"%s\" \"$1\" > submitted-prompt.md; cat \"$2\"', 'fake', '{{prompt}}', '{}']\n",
            source.join("tests/fixtures/claude.jsonl").display()
        )).unwrap();
        let mut app = App::load(UiConfig {
            runs: root.join("runs"),
            agents: root.join("agents.toml"),
            workflow: source.join("workflow.toml"),
            roles: source.join("roles"),
        })
        .unwrap();
        let run = Run {
            id: "001".into(),
            project: root.clone(),
            spec: None,
            cursor: 0,
            channel: vec![],
            created_at: Utc::now(),
            nodes: vec![Node {
                name: "plan".into(),
                agent: "fake".into(),
                role: "planner".into(),
                writes: "plan.md".into(),
                status: NodeStatus::Done,
                session_id: None,
                session_group: None,
                duration: None,
                attempts: 1,
                command: None,
                skip_if_no_python_changes: false,
            }],
        };
        app.repository.save(&run).unwrap();
        app.repository.write("001", "plan.md", DOCUMENT).unwrap();
        app.runs.push(run);
        app.open_selected().unwrap();
        Self { root, app }
    }

    fn answer(&mut self) {
        self.app.ui.open_answer();
        self.app
            .ui
            .update(Message::Paste("Sí, TOML.\nPreserve settings.".into()));
        self.app.save_answer().unwrap();
        self.app.ui.cancel_prompt();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn answer_draft_survives_navigation_without_submitting() {
    let mut fixture = Fixture::new();
    fixture.answer();
    fixture.app.return_to_run_list();
    fixture.app.open_selected().unwrap();
    fixture.app.ui.open_answer();
    assert_eq!(fixture.app.ui.prompt, "Sí, TOML.\nPreserve settings.");
    assert!(!fixture.root.join("runs/001/feedback-0.md").exists());
    assert!(fixture.app.running.is_none());
}

#[tokio::test]
async fn submitting_empty_answers_does_not_start_an_agent() {
    let mut fixture = Fixture::new();
    fixture.app.submit_answers().await.unwrap();
    assert!(fixture.app.running.is_none());
    assert!(
        fixture
            .app
            .ui
            .error
            .as_ref()
            .unwrap()
            .contains("Enter an answer")
    );
}

#[tokio::test]
async fn submission_reloads_edited_artifact_before_sending() {
    let mut fixture = Fixture::new();
    fixture.answer();
    fixture
        .app
        .repository
        .write(
            "001",
            "plan.md",
            &DOCUMENT.replace("Storage?", "Delete data?"),
        )
        .unwrap();
    fixture.app.submit_answers().await.unwrap();
    assert!(fixture.app.running.is_none());
    assert!(
        fixture
            .app
            .ui
            .error
            .as_ref()
            .unwrap()
            .contains("changed on disk")
    );
    assert!(
        fixture
            .app
            .ui
            .decision_drafts
            .feedback(&fixture.app.ui.questions)
            .is_none()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn submitted_answers_reach_fake_agent_and_return_to_human_review() {
    let mut fixture = Fixture::new();
    fixture.answer();
    fixture.app.submit_answers().await.unwrap();
    assert_eq!(fixture.app.ui.mode, Mode::Streaming);
    assert_ne!(fixture.app.ui.detail_view, DetailView::Decisions);
    let feedback = fixture.app.repository.read("001", "feedback-0.md").unwrap();
    assert!(feedback.contains("## D1: Storage?"));
    assert!(feedback.contains("Sí, TOML.\nPreserve settings."));
    assert!(!feedback.contains("## D2"));
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = next_agent_event(&mut fixture.app.running).await {
            fixture.app.accept_event(event).unwrap();
        }
        fixture.app.stream_closed().unwrap();
    })
    .await
    .unwrap();
    let prompt = fs::read_to_string(fixture.root.join("submitted-prompt.md")).unwrap();
    assert!(prompt.contains(&feedback));
    assert_eq!(fixture.app.ui.mode, Mode::Gate);
    let run = fixture.app.repository.load("001").unwrap();
    assert_eq!(run.cursor, 0);
    assert_eq!(run.nodes[0].status, NodeStatus::Done);
    assert_eq!(run.nodes[0].writes, "plan_v2.md");
    assert_eq!(
        fixture.app.repository.read("001", "plan.md").unwrap(),
        DOCUMENT
    );
}

#[test]
fn decision_screen_renders_questions_and_answer_on_small_and_regular_terminals() {
    let mut fixture = Fixture::new();
    fixture.answer();
    for (width, height) in [(1, 1), (25, 8), (100, 30)] {
        let mut terminal =
            Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, &fixture.app)).unwrap();
        if width == 100 {
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(text.contains("D1: Storage?"));
            assert!(text.contains("D2: Migrate?"));
            assert!(text.contains("Sí, TOML."));
        }
    }
}
