use miau::runs::application::diagram::parse;
use miau::terminal::application::{DetailView, Message, Mode, Model};

const ARTIFACT: &str = include_str!("fixtures/diagram.md");

#[test]
fn diagram_drills_into_one_level_and_returns_to_its_container() {
    let mut model = Model::default();
    model.diagram.load(ARTIFACT);
    assert_eq!(model.diagram.rows.len(), 1);
    model.update(Message::DiagramChild);
    assert_eq!(model.diagram.rows.len(), 2);
    model.update(Message::DiagramNext);
    model.diagram.load(ARTIFACT);
    assert_eq!(model.diagram.selected_node().unwrap().id, "files");
    model.update(Message::DiagramParent);
    assert_eq!(model.diagram.rows.len(), 1);
    assert_eq!(model.diagram.selected_node().unwrap().id, "runs");
}

#[test]
fn delivery_roles_request_before_and_after_diagrams() {
    let planner = include_str!("../roles/planner.md");
    let implementer = include_str!("../roles/implementer.md");
    assert!(planner.contains("proposed"));
    assert!(planner.contains("miau-graph"));
    assert!(implementer.contains("observed"));
    assert!(implementer.contains("miau-graph"));
}

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
        ARTIFACT.replace("\"version\": 1", "\"version\": 3"),
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
fn version_two_accepts_semantic_node_kinds_and_rejects_unknown_kinds() {
    let version_two = ARTIFACT
        .replace("\"version\": 1", "\"version\": 2")
        .replace(
            "\"label\": \"ArtifactRepository\"",
            "\"label\": \"ArtifactRepository\", \"kind\": \"interface\"",
        );
    let graph = parse(&version_two).unwrap().unwrap();
    assert_eq!(graph.nodes[1].kind.unwrap().label(), "Interface");
    assert!(parse(&version_two.replace("\"interface\"", "\"service\"")).is_err());
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
        .load(&ARTIFACT.replace("\"version\": 1", "\"version\": 3"));
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

fn uml_artifact() -> String {
    r##"# Handoff
```miau-graph
{"version":2,"title":"Classes","status":"proposed","nodes":[
{"id":"module","label":"model.ts","kind":"module"},
{"id":"a","label":"Service","kind":"class","parent":"module",
 "attributes":["- name: string"],"operations":["+ run(): void"]},
{"id":"b","label":"Port","kind":"interface","parent":"module","attributes":[],"operations":[]},
{"id":"c","label":"Other","kind":"class"}],
"edges":[{"from":"a","to":"b","kind":"implements"},
{"from":"a","to":"c","kind":"dependency"}]}
```
"##
    .into()
}

#[test]
fn uml_navigation_flattens_types_and_follows_relationships_with_history() {
    let mut model = Model::default();
    model.mode = Mode::Gate;
    model.diagram.load(&uml_artifact());
    assert!(model.diagram.error.is_none(), "{:?}", model.diagram.error);
    assert_eq!(model.diagram.rows.len(), 3);
    assert_eq!(model.diagram.selected_node().unwrap().id, "a");
    model.update(Message::DiagramChild);
    assert_eq!(model.diagram.selected_node().unwrap().id, "b");
    model.update(Message::DiagramParent);
    assert_eq!(model.diagram.selected_node().unwrap().id, "a");
    model.update(Message::DiagramNext);
    assert_eq!(model.diagram.selected_node().unwrap().id, "b");
    model.diagram.load(&uml_artifact());
    assert_eq!(model.diagram.selected_node().unwrap().id, "b");
    assert_eq!(model.mode, Mode::Gate);
    assert!(model.pending_prompt.is_none());
}

#[test]
fn uml_member_compartments_round_trip_and_reject_invalid_signatures() {
    let graph = parse(&uml_artifact()).unwrap().unwrap();
    let encoded = serde_json::to_value(&graph).unwrap();
    assert_eq!(encoded["nodes"][1]["attributes"][0], "- name: string");
    assert_eq!(encoded["nodes"][1]["operations"][0], "+ run(): void");
    assert!(parse(&uml_artifact().replace("- name: string", "")).is_err());
}

#[test]
fn shared_workflow_contract_example_is_a_navigable_uml_class_model() {
    let contract = include_str!("../src/workflow/artifact-contract.md");
    let json = contract
        .split("```miau-graph\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let mut model = Model::default();
    model
        .diagram
        .load(&format!("# Handoff\n```miau-graph\n{json}```\n"));
    assert!(model.diagram.is_uml());
    for node in &model.diagram.graph.as_ref().unwrap().nodes {
        if node.kind.is_some_and(|kind| kind.is_classifier()) {
            assert!(node.attributes.is_some());
            assert!(node.operations.is_some());
        }
    }
    for role in [
        include_str!("../roles/planner.md"),
        include_str!("../roles/implementer.md"),
    ] {
        assert!(role.contains("class diagram"));
    }
}

#[test]
fn uml_relationship_selection_pan_and_step_comparison_preserve_human_gate() {
    let mut model = Model::default();
    model.mode = Mode::Gate;
    model.diagram.load(&uml_artifact());
    model.update(Message::DiagramNextRelation);
    model.update(Message::DiagramChild);
    assert_eq!(model.diagram.selected_node().unwrap().id, "c");
    model.update(Message::DiagramPan { x: 8, y: 4 });
    assert_eq!((model.diagram.pan_x, model.diagram.pan_y), (8, 4));
    model.update(Message::DiagramHome);
    assert_eq!((model.diagram.pan_x, model.diagram.pan_y), (0, 0));
    let observed = uml_artifact()
        .replace("proposed", "observed")
        .replace(
            "\"kind\":\"implements\"",
            "\"kind\":\"implements\",\"source\":{\"file\":\"model.ts\",\"line\":1}",
        )
        .replace(
            "\"kind\":\"dependency\"",
            "\"kind\":\"dependency\",\"source\":{\"file\":\"model.ts\",\"line\":2}",
        );
    model.diagram.load(&observed);
    assert_eq!(model.diagram.selected_node().unwrap().id, "c");
    assert_eq!(
        model.diagram.graph.as_ref().unwrap().status,
        miau::runs::domain::diagram::Status::Observed
    );
    assert_eq!(model.mode, Mode::Gate);
    assert!(model.pending_prompt.is_none());
}

#[test]
fn uml_changed_members_invalidate_notes_and_legacy_members_remain_unknown() {
    use miau::runs::application::diagram_notes::Notes;
    let graph = parse(&uml_artifact()).unwrap().unwrap();
    let mut notes = Notes::default();
    notes.set(&graph, "a", "Make name public".into());
    let changed = parse(&uml_artifact().replace("- name: string", "+ name: string"))
        .unwrap()
        .unwrap();
    assert!(notes.feedback(&changed).is_none());
    assert!(notes.feedback(&graph).is_some());
    let legacy = parse(ARTIFACT).unwrap().unwrap();
    assert!(
        legacy
            .nodes
            .iter()
            .all(|node| node.attributes.is_none() && node.operations.is_none())
    );
    for invalid in [
        uml_artifact().replace("- name: string", "name\\nstring"),
        uml_artifact().replace(
            "\"kind\":\"class\",\"parent\"",
            "\"kind\":\"module\",\"parent\"",
        ),
        uml_artifact().replace("\"version\":2", "\"version\":1"),
    ] {
        assert!(parse(&invalid).is_err());
    }
}
