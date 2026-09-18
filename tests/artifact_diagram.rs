use miau::runs::application::diagram::parse;
use miau::terminal::application::{DetailView, Message, Mode, Model};

const ARTIFACT: &str = include_str!("fixtures/diagram.md");

#[test]
fn diagram_reads_hierarchy_and_source_references_from_handoff() {
    let graph = parse(ARTIFACT).unwrap().unwrap();
    assert_eq!(graph.nodes[1].parent.as_deref(), Some("runs"));
    assert_eq!(graph.edges[0].to, "port");
    assert_eq!(graph.nodes[1].source.as_ref().unwrap().line, 40);
}

#[test]
fn observed_diagram_requires_evidence_but_dependency_cycles_are_valid() {
    let observed = ARTIFACT.replace("\"proposed\"", "\"observed\"");
    assert!(parse(&observed).unwrap().is_some());
    let without_evidence = observed.replace(
        ",\n     \"source\": {\"file\": \"src/runs/infrastructure.rs\", \"line\": 1}",
        "",
    );
    assert!(
        parse(&without_evidence)
            .unwrap_err()
            .contains("Observed relationships")
    );
    let self_dependency = ARTIFACT.replace("\"to\": \"port\"", "\"to\": \"files\"");
    assert!(parse(&self_dependency).unwrap().is_some());
}

#[test]
fn diagram_reloads_preserve_selection_by_identity_and_clear_invalid_data() {
    let mut model = Model::default();
    model.diagram.load(ARTIFACT);
    model.update(Message::DiagramChild);
    model.update(Message::DiagramNextSource);
    assert_eq!(
        model.diagram.selected_source().unwrap().file,
        "src/runs/infrastructure.rs"
    );
    model
        .diagram
        .load(&ARTIFACT.replace("FileRepository", "Files"));
    assert_eq!(model.diagram.selected_node().unwrap().id, "port");
    assert_eq!(
        model.diagram.selected_source().unwrap().file,
        "src/runs/infrastructure.rs"
    );
    model
        .diagram
        .load(&ARTIFACT.replace("\"version\": 1", "\"version\": 99"));
    assert!(model.diagram.graph.is_none());
    assert!(model.diagram.rows.is_empty());
    assert!(model.diagram.selected_source().is_none());
}

#[test]
fn diagram_rejects_oversized_payloads_and_unknown_fields() {
    let large = ARTIFACT.replace("Storage boundary", &"x".repeat(1_048_576));
    assert!(parse(&large).unwrap_err().contains("1 MiB"));
    let typo = ARTIFACT.replace("\"title\":", "\"titel\":");
    assert!(parse(&typo).unwrap_err().contains("unknown field"));
}

#[test]
fn diagram_ignores_review_and_nested_examples() {
    let in_review = ARTIFACT.replace("# Handoff\n", "");
    assert!(parse(&in_review).unwrap().is_none());
    let example = format!("# Handoff\n````markdown\n{ARTIFACT}\n````\n");
    assert!(parse(&example).unwrap().is_none());
    assert!(parse("# Review\nLegacy report").unwrap().is_none());
}

#[test]
fn diagram_rejects_ambiguous_or_broken_graphs() {
    for artifact in [
        ARTIFACT.replace("\"version\": 1", "\"version\": 2"),
        ARTIFACT.replace("\"id\": \"files\"", "\"id\": \"port\""),
        ARTIFACT.replace("\"to\": \"port\"", "\"to\": \"missing\""),
        ARTIFACT.replace("\"parent\": \"runs\"", "\"parent\": \"missing\""),
        ARTIFACT.replace(
            "\"id\": \"runs\",",
            "\"id\": \"runs\", \"parent\": \"files\",",
        ),
        ARTIFACT.replace("\"line\": 40", "\"line\": 0"),
        ARTIFACT.replace("src/runs/application.rs", "../outside.rs"),
        ARTIFACT.replace("src/runs/application.rs", "/outside.rs"),
        ARTIFACT.replace("\"kind\": \"implements\"", "\"kind\": \"invented\""),
        ARTIFACT.replace("\"version\": 1", "broken json"),
        ARTIFACT.trim_end().trim_end_matches("```").to_owned(),
        format!("{ARTIFACT}\n{ARTIFACT}"),
    ] {
        assert!(parse(&artifact).is_err(), "accepted {artifact}");
    }
}

#[test]
fn diagram_navigation_and_reload_leave_the_gate_unchanged() {
    let mut model = Model::default();
    model.mode = Mode::Gate;
    model.diagram.load(ARTIFACT);
    for _ in 0..3 {
        model.update(Message::ToggleDetailView { has_review: true });
    }
    assert_eq!(model.detail_view, DetailView::Diagram);
    model.update(Message::DiagramChild);
    assert_eq!(model.diagram.selected_node().unwrap().id, "port");
    model.update(Message::DiagramNext);
    assert_eq!(model.diagram.selected_node().unwrap().id, "files");
    model.update(Message::DiagramParent);
    assert_eq!(model.diagram.selected_node().unwrap().id, "runs");
    model
        .diagram
        .load(&ARTIFACT.replace("Storage boundary", "Edited on disk"));
    assert_eq!(
        model.diagram.graph.as_ref().unwrap().title,
        "Edited on disk"
    );
    assert_eq!(model.mode, Mode::Gate);
    assert!(model.pending_prompt.is_none());
}

#[test]
fn diagram_error_remains_inspectable_and_valid_reload_recovers() {
    let mut model = Model::default();
    model
        .diagram
        .load(&ARTIFACT.replace("\"version\": 1", "\"version\": 2"));
    assert!(model.diagram.error.is_some());
    for _ in 0..3 {
        model.update(Message::ToggleDetailView { has_review: true });
    }
    assert_eq!(model.detail_view, DetailView::Diagram);
    model.diagram.load(ARTIFACT);
    assert!(model.diagram.error.is_none());
    model.diagram.load("# Review\nNo diagram\n# Handoff\nNotes");
    assert!(model.diagram.graph.is_none());
}
