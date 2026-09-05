//! Git-backed deterministic workflow skip policy.

use std::{io, path::Path, process::Command};

pub struct GitChangeDetector;

impl GitChangeDetector {
    pub fn has_python_changes(project: &Path) -> io::Result<bool> {
        let output = Command::new("git")
            .args(["status", "--porcelain", "--untracked-files=normal"])
            .current_dir(project)
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(String::from_utf8_lossy(&output.stderr)));
        }
        Ok(Self::porcelain_has_python(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    fn porcelain_has_python(output: &str) -> bool {
        output.lines().any(|line| {
            let path = line.get(3..).unwrap_or_default();
            let destination = path.rsplit(" -> ").next().unwrap_or(path);
            destination.trim_end().ends_with(".py")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::GitChangeDetector;

    #[test]
    fn detects_modified_renamed_and_untracked_python() {
        assert!(GitChangeDetector::porcelain_has_python(" M addon/a.py\n"));
        assert!(GitChangeDetector::porcelain_has_python(
            "R  old.py -> addon/new.py\n"
        ));
        assert!(GitChangeDetector::porcelain_has_python("?? fresh.py\n"));
        assert!(!GitChangeDetector::porcelain_has_python(" M README.md\n"));
    }
}
