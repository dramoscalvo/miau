use super::super::*;
use ratatui::backend::TestBackend;
use std::fs;

const DOCUMENT: &str = include_str!("../../../../tests/fixtures/diagram.md");

#[test]
fn diagram_shows_review_stage_boxes_and_saved_node_notes() {
    let mut model = Model::default();
    model.diagram.load(DOCUMENT);
    model.update(Message::DiagramChild);
    let graph = model.diagram.graph.as_ref().unwrap();
    model
        .diagram_notes
        .set(graph, "port", "Keep the port small".into());
    let mut terminal = Terminal::new(TestBackend::new(160, 35)).unwrap();
    terminal
        .draw(|frame| super::render(frame, frame.area(), &model, "plan"))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Before implementation"));
    assert!(text.contains("Keep the port small"));
    assert!(text.contains("[note]"));
    assert!(text.contains("╭──"));
    assert!(text.contains("Runs"));
}

#[test]
fn diagram_shows_extractor_provenance_and_coverage_warnings() {
    let metadata = r#""provenance":{"extractor":"miau-typescript","version":1,"typescript":"5.9.3","project":"tsconfig.json","scope":"module-dependencies","fingerprint":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","warnings":["nonliteral import omitted"]},"#;
    let document = DOCUMENT
        .replace("\"proposed\"", "\"observed\"")
        .replace("\"version\": 1,", &format!("\"version\": 1, {metadata}"));
    let mut model = Model::default();
    model.diagram.load(&document);
    let mut terminal = Terminal::new(TestBackend::new(160, 35)).unwrap();
    terminal
        .draw(|frame| super::render(frame, frame.area(), &model, "architecture"))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("extractor-reported"));
    assert!(text.contains("TypeScript 5.9.3"));
    assert!(text.contains("nonliteral import omitted"));
}

#[test]
fn diagram_shows_semantic_node_kind() {
    let document = DOCUMENT
        .replace("\"version\": 1", "\"version\": 2")
        .replace(
            "\"label\": \"ArtifactRepository\"",
            "\"label\": \"ArtifactRepository\", \"kind\": \"interface\"",
        );
    let mut model = Model::default();
    model.diagram.load(&document);
    model.update(Message::DiagramChild);
    let mut terminal = Terminal::new(TestBackend::new(120, 25)).unwrap();
    terminal
        .draw(|frame| super::render(frame, frame.area(), &model, "architecture"))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("ArtifactRepository [port]"));
    assert!(text.contains("Interface"));
}

#[test]
fn diagram_renders_selected_relationships_and_unicode_at_multiple_sizes() {
    let mut model = Model::default();
    model
        .diagram
        .load(&DOCUMENT.replace("FileRepository", "FileRepository 猫"));
    model.update(Message::DiagramChild);
    for (width, height) in [(120, 25), (60, 30), (10, 4), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| super::render(frame, frame.area(), &model, "plan"))
            .unwrap();
        if width > 10 {
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(text.contains("Incoming:"));
            assert!(text.contains("implements FileRepository 猫"));
            assert!(text.contains("src/runs/application.rs:40"));
        }
    }
}

#[test]
fn diagram_error_is_rendered_without_hiding_document_access() {
    let mut model = Model::default();
    model
        .diagram
        .load(&DOCUMENT.replace("\"version\": 1", "\"version\": 9"));
    let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
    terminal
        .draw(|frame| super::render(frame, frame.area(), &model, "plan"))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Unsupported diagram version: 9"));
    assert!(text.contains("v document views"));
}

#[test]
fn diagram_disk_reload_and_source_selection_preserve_persisted_run() {
    let root = std::env::temp_dir().join(format!(
        "miau-diagram-ui-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    fs::create_dir_all(root.join("src/runs")).unwrap();
    fs::write(root.join("src/runs/application.rs"), "// port").unwrap();
    let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut app = App::load(UiConfig {
        runs: root.join("runs"),
        agents: checkout.join("agents.toml"),
        workflow: checkout.join("workflow.toml"),
        roles: checkout.join("roles"),
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
            agent: "codex".into(),
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
    let before = fs::read(root.join("runs/001/state.json")).unwrap();
    app.ui.update(Message::DiagramChild);
    let changed = DOCUMENT.replace("\"line\": 40", "\"line\": 41");
    app.repository.write("001", "plan.md", &changed).unwrap();
    assert!(app.diagram_source_path().unwrap().is_none());
    assert!(app.ui.error.as_deref().unwrap().contains("changed on disk"));
    assert_eq!(app.ui.diagram.selected_source().unwrap().line, 41);
    assert_eq!(
        app.diagram_source_path().unwrap().unwrap(),
        root.join("src/runs/application.rs").canonicalize().unwrap()
    );
    assert_eq!(app.ui.mode, Mode::Gate);
    assert!(app.running.is_none());
    assert!(app.ui.pending_prompt.is_none());
    assert_eq!(fs::read(root.join("runs/001/state.json")).unwrap(), before);
    app.repository
        .write("001", "plan.md", "# Review\nNo diagram\n# Handoff\nDetails")
        .unwrap();
    assert!(app.diagram_source_path().unwrap().is_none());
    assert!(app.ui.diagram.graph.is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn uml_renders_compartments_and_connected_realization_arrow() {
    let artifact = r##"# Handoff
```miau-graph
{"version":2,"title":"Storage","status":"proposed","nodes":[
{"id":"adapter","label":"FileRepository","kind":"class","attributes":["- path: Path"],"operations":["+ save(): Result"]},
{"id":"port","label":"Repository","kind":"interface","attributes":[],"operations":["+ save(): Result"]}],
"edges":[{"from":"adapter","to":"port","kind":"implements"}]}
```
"##;
    let mut model = Model::default();
    model.diagram.load(artifact);
    for (width, height) in [(140, 40), (65, 25), (10, 4), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| super::render(frame, frame.area(), &model, "plan"))
            .unwrap();
        if width == 140 {
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(text.contains("UML classes"), "{text}");
            assert!(text.contains("- path: Path"));
            assert!(text.contains("+ save(): Result"));
            assert!(text.contains("«interface»"));
            assert!(text.contains("┼"));
            assert!(text.contains("╌╌▷"), "missing realization arrow: {text}");
        }
    }
}

#[test]
fn uml_relationship_navigation_brings_later_neighbors_into_view() {
    let graph = serde_json::json!({
        "version": 2, "title": "Related classes", "status": "proposed",
        "nodes": (0..6).map(|index| serde_json::json!({
            "id": format!("c{index}"), "label": format!("Class{index}"), "kind": "class",
            "attributes": [], "operations": []
        })).collect::<Vec<_>>(),
        "edges": (1..6).map(|index| serde_json::json!({
            "from": "c0", "to": format!("c{index}"), "kind": "association"
        })).collect::<Vec<_>>()
    });
    let mut model = Model::default();
    model
        .diagram
        .load(&format!("# Handoff\n```miau-graph\n{graph}\n```\n"));
    for _ in 0..4 {
        model.update(Message::DiagramNextRelation);
    }
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| super::render(frame, frame.area(), &model, "implementation"))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Class5"));
    assert!(text.contains("relationship 5/5"));
    model.update(Message::DiagramChild);
    assert_eq!(model.diagram.selected_node().unwrap().id, "c5");
    model.update(Message::DiagramParent);
    assert_eq!(model.diagram.selected_node().unwrap().id, "c0");
}
