use miau::runs::application::diagram;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn typescript_command_generates_viewable_artifact_and_preserves_existing_output() {
    let root = std::env::temp_dir().join(format!(
        "miau-ts-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    // A fake Node runner keeps the standard Cargo suite independent of Node/npm.
    let graph = r#"{"version":1,"title":"Fixture","status":"observed","nodes":[{"id":"a","label":"a.ts","source":{"file":"a.ts","line":1}}],"edges":[],"provenance":{"extractor":"miau-typescript","version":1,"typescript":"5.9.3","project":"tsconfig.json","scope":"module-dependencies","fingerprint":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","warnings":[]}}"#;
    fs::write(root.join("graph.json"), graph).unwrap();
    fs::write(root.join("tsconfig.json"), "{}").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let runner = root.join("node-fixture");
        fs::write(&runner, "#!/bin/sh\ncat graph.json\n").unwrap();
        fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
        let command = || {
            let mut command = Command::new(env!("CARGO_BIN_EXE_miau"));
            command
                .current_dir(&root)
                .args([
                    "diagram",
                    "typescript",
                    "--project",
                    "tsconfig.json",
                    "--root",
                    ".",
                    "--output",
                    "diagram.md",
                    "--node",
                ])
                .arg(&runner);
            command
        };
        let output = command().output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let artifact = fs::read_to_string(root.join("diagram.md")).unwrap();
        let parsed = diagram::parse(&artifact).unwrap().unwrap();
        assert_eq!(parsed.provenance.unwrap().extractor, "miau-typescript");
        assert!(!command().output().unwrap().status.success());
        assert_eq!(
            fs::read_to_string(root.join("diagram.md")).unwrap(),
            artifact
        );
        assert!(!root.join("miaus").exists());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn typescript_command_is_documented_in_cli_help() {
    let result = Command::new(env!("CARGO_BIN_EXE_miau"))
        .args(["diagram", "typescript", "--help"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let help = String::from_utf8_lossy(&result.stdout);
    assert!(help.contains("--project"));
    assert!(help.contains("--scope"));
    assert!(help.contains("modules"));
    assert!(help.contains("types"));
}

#[test]
fn typescript_extraction_errors_do_not_create_an_artifact() {
    let directory = std::env::temp_dir().join(format!("miau-ts-error-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let output: PathBuf = directory.join("diagram.md");
    let result = Command::new(env!("CARGO_BIN_EXE_miau"))
        .args([
            "diagram",
            "typescript",
            "--project",
            "missing-tsconfig.json",
            "--output",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!output.exists());
    fs::remove_dir_all(directory).unwrap();
}
