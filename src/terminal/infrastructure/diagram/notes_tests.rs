use super::super::*;
use std::fs;

const DOCUMENT: &str = include_str!("../../../../tests/fixtures/diagram.md");

struct Fixture {
    root: PathBuf,
    app: App,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "miau-note-ui-{}-{}",
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

    fn note(&mut self) {
        self.app.ui.detail_view = DetailView::Diagram;
        self.app.ui.update(Message::DiagramChild);
        self.app.open_diagram_note().unwrap();
        self.app
            .ui
            .update(Message::Paste("Keep this port 猫.\nAdd tests.".into()));
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
fn node_notes_survive_reopening_and_do_not_change_the_gate() {
    let mut fixture = Fixture::new();
    let before = fs::read(fixture.root.join("runs/001/state.json")).unwrap();
    fixture.note();
    fixture.app.return_to_run_list();
    fixture.app.open_selected().unwrap();
    fixture.app.ui.update(Message::DiagramChild);
    fixture.app.open_diagram_note().unwrap();
    assert_eq!(fixture.app.ui.prompt, "Keep this port 猫.\nAdd tests.");
    assert!(fixture.app.ui.submit_prompt().is_none());
    assert_eq!(
        fs::read(fixture.root.join("runs/001/state.json")).unwrap(),
        before
    );
    assert!(fixture.app.running.is_none());
    assert!(!fixture.root.join("runs/001/feedback-0.md").exists());
}

#[tokio::test]
async fn changed_diagram_cannot_submit_old_notes_or_start_an_agent() {
    let mut fixture = Fixture::new();
    fixture.note();
    fixture
        .app
        .repository
        .write(
            "001",
            "plan.md",
            &DOCUMENT.replace("\"implements\"", "\"dependency\""),
        )
        .unwrap();
    fixture.app.submit_diagram_notes().await.unwrap();
    assert!(fixture.app.running.is_none());
    assert!(
        fixture
            .app
            .ui
            .error
            .as_deref()
            .unwrap()
            .contains("changed on disk")
    );
    fixture.app.submit_diagram_notes().await.unwrap();
    assert!(fixture.app.running.is_none());
    assert!(
        fixture
            .app
            .ui
            .error
            .as_deref()
            .unwrap()
            .contains("Add a note")
    );
    fixture
        .app
        .repository
        .write("001", "plan.md", DOCUMENT)
        .unwrap();
    fixture.app.reload_artifact().unwrap();
    fixture.app.open_diagram_note().unwrap();
    assert_eq!(fixture.app.ui.prompt, "Keep this port 猫.\nAdd tests.");
}

#[tokio::test]
async fn explicit_diagram_submission_preserves_feedback_and_returns_to_review() {
    let mut fixture = Fixture::new();
    fixture.note();
    fixture.app.submit_diagram_notes().await.unwrap();
    let feedback = fixture.app.repository.read("001", "feedback-0.md").unwrap();
    assert!(feedback.contains("ArtifactRepository [port]"));
    assert!(feedback.contains("Keep this port 猫.\nAdd tests."));
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
    let run = fixture.app.repository.load("001").unwrap();
    assert_eq!(run.cursor, 0);
    assert_eq!(run.nodes[0].status, NodeStatus::Done);
    assert_eq!(fixture.app.ui.mode, Mode::Gate);
    assert_eq!(
        fixture.app.repository.read("001", "plan.md").unwrap(),
        DOCUMENT
    );
}

#[test]
fn browsing_before_and_after_keeps_diagram_and_entity_selected() {
    let mut fixture = Fixture::new();
    fixture.note();
    let run = fixture.app.run.as_mut().unwrap();
    let mut implementation = run.nodes[0].clone();
    implementation.name = "implement".into();
    implementation.writes = "implementation.md".into();
    run.nodes.push(implementation);
    fixture
        .app
        .repository
        .write(
            "001",
            "implementation.md",
            &DOCUMENT.replace("\"proposed\"", "\"observed\""),
        )
        .unwrap();
    fixture.app.browse_node(true).unwrap();
    assert_eq!(fixture.app.ui.detail_view, DetailView::Diagram);
    assert_eq!(fixture.app.ui.diagram.selected_node().unwrap().id, "port");
    let graph = fixture.app.ui.diagram.graph.as_ref().unwrap();
    assert_eq!(graph.status, crate::runs::domain::diagram::Status::Observed);
    assert!(fixture.app.ui.diagram_notes.note(graph, "port").is_empty());
    fixture.app.browse_node(false).unwrap();
    let graph = fixture.app.ui.diagram.graph.as_ref().unwrap();
    assert_eq!(
        fixture.app.ui.diagram_notes.note(graph, "port"),
        "Keep this port 猫.\nAdd tests."
    );
    assert_eq!(fixture.app.run.as_ref().unwrap().cursor, 0);
    assert!(fixture.app.running.is_none());
}
