#[cfg(unix)]
#[test]
fn debug_run_persists_fixture_result_and_session() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let root = std::env::temp_dir().join(format!("miau-integration-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude.jsonl");
    let script = root.join("fake-agent.sh");
    fs::write(&script, format!("#!/bin/sh\nwhile IFS= read -r line; do printf '%s\\n' \"$line\"; sleep 0.01; done < '{}'\n", fixture.display())).unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).unwrap();
    let agents = root.join("agents.toml");
    fs::write(
        &agents,
        format!(
            "[fake]\nbin = {:?}\nargs = [\"{{prompt}}\"]\nparser = \"claude\"\n",
            script.display().to_string()
        ),
    )
    .unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_miau"))
        .args([
            "--runs",
            root.join("runs").to_str().unwrap(),
            "--agents",
            agents.to_str().unwrap(),
            "debug",
            "run",
            "--agent",
            "fake",
            "--prompt",
            "hello",
            "--id",
            "014",
            "--cwd",
            root.to_str().unwrap(),
            "--artifact",
            "plan.md",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("runs/014/state.json")).unwrap()).unwrap();
    assert_eq!(
        state
            .pointer("/nodes/0/session_id")
            .and_then(|v| v.as_str()),
        Some("session-1")
    );
    assert_eq!(
        fs::read_to_string(root.join("runs/014/plan.md")).unwrap(),
        "A plan"
    );
    let _ = fs::remove_dir_all(root);
}
