//! Project source references resolved before an editor handoff.

use crate::{runs::domain::diagram::Source, terminal::application::diagram::SourceRepository};
use std::{
    io,
    path::{Path, PathBuf},
};

pub struct ProjectSources;

impl SourceRepository for ProjectSources {
    fn resolve(&self, project: &Path, source: &Source) -> io::Result<PathBuf> {
        if !source.is_valid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Source must be a project-relative file with a positive line",
            ));
        }
        let root = project.canonicalize()?;
        let path = root.join(&source.file).canonicalize()?;
        if !path.starts_with(&root) || !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Source must resolve to a file inside this project",
            ));
        }
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectSources;
    use crate::{runs::domain::diagram::Source, terminal::application::diagram::SourceRepository};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn source_lookup_checks_real_project_files_and_rejects_escapes() {
        let root = std::env::temp_dir().join(format!(
            "miau-source-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("猫.rs"), "// source").unwrap();
        fs::write(root.join("outside.rs"), "// outside").unwrap();
        let source = |file: &str| Source {
            file: file.into(),
            line: 1,
        };
        assert_eq!(
            ProjectSources.resolve(&project, &source("猫.rs")).unwrap(),
            project.join("猫.rs").canonicalize().unwrap()
        );
        for file in [
            "../outside.rs",
            "missing.rs",
            "/etc/passwd",
            "C:/outside.rs",
            ".",
        ] {
            assert!(ProjectSources.resolve(&project, &source(file)).is_err());
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("outside.rs"), project.join("linked.rs")).unwrap();
            assert!(
                ProjectSources
                    .resolve(&project, &source("linked.rs"))
                    .is_err()
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
