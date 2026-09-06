use std::process::Command;

#[test]
fn alternate_launcher_is_opt_in_in_the_manifest() {
    let manifest = toml::from_str::<toml::Value>(include_str!("../Cargo.toml")).unwrap();

    let default_features = manifest["features"]["default"].as_array().unwrap();
    let alternate_bin = manifest["bin"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bin| bin["name"].as_str() == Some("asdf"))
        .unwrap();

    assert!(
        default_features.is_empty()
            && alternate_bin["required-features"].as_array().unwrap()
                == &[toml::Value::String("asdf".into())]
    );
}

#[cfg(feature = "asdf")]
#[test]
fn alternate_launcher_executes_the_app_when_enabled() {
    let output = Command::new(env!("CARGO_BIN_EXE_asdf"))
        .arg("--help")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.starts_with("Human-gated coding-agent orchestrator\n\nUsage: asdf"));
}

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
