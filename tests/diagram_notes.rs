use miau::runs::{
    application::{diagram::parse, diagram_notes::Notes},
    infrastructure::FileRepository,
};

const ARTIFACT: &str = include_str!("fixtures/diagram.md");

#[test]
fn notes_survive_reopening_without_leaking_into_other_steps_or_changed_diagrams() {
    let root = std::env::temp_dir().join(format!(
        "miau-notes-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let repo = FileRepository::new(&root);
    let graph = parse(ARTIFACT).unwrap().unwrap();
    let mut notes = Notes::default();
    notes.set(&graph, "port", "Keep this boundary 猫\nAdd tests".into());
    notes.save(&repo, "001", 0).unwrap();
    let reopened = Notes::load(&repo, "001", 0).unwrap();
    assert_eq!(
        reopened.note(&graph, "port"),
        "Keep this boundary 猫\nAdd tests"
    );
    assert!(
        reopened
            .feedback(&graph)
            .unwrap()
            .contains("ArtifactRepository [port]")
    );
    assert!(
        Notes::load(&repo, "001", 1)
            .unwrap()
            .feedback(&graph)
            .is_none()
    );
    let changed = parse(&ARTIFACT.replace("\"implements\"", "\"dependency\""))
        .unwrap()
        .unwrap();
    assert!(reopened.feedback(&changed).is_none());
    assert!(!reopened.note(&graph, "port").is_empty());
    notes.set(&graph, "port", String::new());
    assert!(notes.feedback(&graph).is_none());
    std::fs::remove_dir_all(root).unwrap();
}
