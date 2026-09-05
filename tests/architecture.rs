use std::{
    fs,
    path::{Path, PathBuf},
};

#[test]
fn inner_layers_do_not_depend_on_infrastructure() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let inner_layers = [
        source.join("runs/application.rs"),
        source.join("runs/domain.rs"),
        source.join("execution/application.rs"),
        source.join("execution/domain.rs"),
        source.join("workflow/application.rs"),
        source.join("workflow/domain.rs"),
        source.join("terminal/application.rs"),
    ];

    let violations: Vec<PathBuf> = inner_layers
        .into_iter()
        .filter(|path| production_source(path).contains("infrastructure::"))
        .collect();

    assert!(
        violations.is_empty(),
        "inner-layer infrastructure dependencies: {violations:?}"
    );
}

#[test]
fn domain_layers_do_not_depend_on_application_layers() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let domains = [
        source.join("runs/domain.rs"),
        source.join("execution/domain.rs"),
        source.join("workflow/domain.rs"),
    ];

    let violations: Vec<PathBuf> = domains
        .into_iter()
        .filter(|path| production_source(path).contains("application::"))
        .collect();

    assert!(
        violations.is_empty(),
        "domain-to-application dependencies: {violations:?}"
    );
}

fn production_source(path: &Path) -> String {
    let source = fs::read_to_string(path).unwrap();
    source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or(&source)
        .to_owned()
}
