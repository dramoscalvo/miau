use std::process::Command;

#[test]
fn help_identifies_the_executable_as_miau() {
    let output = Command::new(env!("CARGO_BIN_EXE_miau"))
        .arg("--help")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.starts_with("Human-gated coding-agent orchestrator\n\nUsage: miau"));
}

#[test]
fn help_shows_miaus_as_the_default_run_directory() {
    let output = Command::new(env!("CARGO_BIN_EXE_miau"))
        .arg("--help")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.lines().any(|line| {
        line.split_whitespace()
            .eq(["--runs", "<RUNS>", "[default:", "miaus]"])
    }));
}
