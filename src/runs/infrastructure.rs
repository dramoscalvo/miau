//! Filesystem persistence adapters for runs and artifacts.

use super::{
    application::{ArtifactRepository, RepositoryError, RunRepository, TextRepository},
    domain::Run,
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone)]
pub struct FileRepository {
    root: PathBuf,
}

impl FileRepository {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn run_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }
    fn map_io(path: &Path, source: io::Error) -> RepositoryError {
        RepositoryError::Io {
            path: path.to_path_buf(),
            source,
        }
    }
    fn safe_name(name: &str) -> Result<(), RepositoryError> {
        let path = Path::new(name);
        if path.components().count() != 1 || name.is_empty() {
            return Err(RepositoryError::UnsafeArtifact(name.to_owned()));
        }
        Ok(())
    }

    fn version_parts(name: &str) -> (&str, Option<&str>, u32) {
        let path = Path::new(name);
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(name);
        let extension = path.extension().and_then(|value| value.to_str());
        let Some((base, version)) = stem.rsplit_once("_v") else {
            return (stem, extension, 1);
        };
        match version.parse::<u32>() {
            Ok(version) if version >= 2 => (base, extension, version),
            _ => (stem, extension, 1),
        }
    }

    fn versioned_name(name: &str, version: u32) -> String {
        let (stem, extension, _) = Self::version_parts(name);
        match extension {
            Some(extension) => format!("{stem}_v{version}.{extension}"),
            None => format!("{stem}_v{version}"),
        }
    }
}

impl RunRepository for FileRepository {
    fn load(&self, id: &str) -> Result<Run, RepositoryError> {
        let path = self.run_dir(id).join("state.json");
        let bytes = fs::read(&path).map_err(|e| Self::map_io(&path, e))?;
        serde_json::from_slice(&bytes).map_err(|source| RepositoryError::Json { path, source })
    }

    fn save(&self, run: &Run) -> Result<(), RepositoryError> {
        let dir = self.run_dir(&run.id);
        fs::create_dir_all(&dir).map_err(|e| Self::map_io(&dir, e))?;
        let destination = dir.join("state.json");
        let temporary = dir.join("state.json.tmp");
        let bytes = serde_json::to_vec_pretty(run).map_err(|source| RepositoryError::Json {
            path: destination.clone(),
            source,
        })?;
        fs::write(&temporary, bytes).map_err(|e| Self::map_io(&temporary, e))?;
        fs::rename(&temporary, &destination).map_err(|e| Self::map_io(&destination, e))
    }

    fn list(&self) -> Result<Vec<Run>, RepositoryError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&self.root).map_err(|e| Self::map_io(&self.root, e))?;
        let mut runs = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| Self::map_io(&self.root, e))?;
            if entry.path().is_dir() {
                let id = entry.file_name().to_string_lossy().into_owned();
                if let Ok(run) = self.load(&id) {
                    runs.push(run);
                }
            }
        }
        runs.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(runs)
    }
}

impl ArtifactRepository for FileRepository {
    fn read(&self, run_id: &str, name: &str) -> Result<String, RepositoryError> {
        let path = self.path(run_id, name)?;
        match fs::read_to_string(&path) {
            Ok(text) => Ok(text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(Self::map_io(&path, e)),
        }
    }
    fn write(&self, run_id: &str, name: &str, text: &str) -> Result<(), RepositoryError> {
        let path = self.path(run_id, name)?;
        let parent = path.parent().unwrap_or(&self.root);
        fs::create_dir_all(parent).map_err(|e| Self::map_io(parent, e))?;
        let temporary = parent.join(format!("{name}.tmp"));
        fs::write(&temporary, text).map_err(|e| Self::map_io(&temporary, e))?;
        fs::rename(&temporary, &path).map_err(|e| Self::map_io(&path, e))
    }
    fn write_versioned(
        &self,
        run_id: &str,
        name: &str,
        text: &str,
    ) -> Result<String, RepositoryError> {
        let path = self.path(run_id, name)?;
        let parent = path.parent().unwrap_or(&self.root);
        fs::create_dir_all(parent).map_err(|e| Self::map_io(parent, e))?;
        let (_, _, current_version) = Self::version_parts(name);
        let candidates = std::iter::once((name.to_owned(), path)).chain(
            current_version
                .checked_add(1)
                .into_iter()
                .flat_map(|first| first..=u32::MAX)
                .map(|version| {
                    let candidate = Self::versioned_name(name, version);
                    let path = self.run_dir(run_id).join(&candidate);
                    (candidate, path)
                }),
        );
        for (candidate, path) in candidates {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(text.as_bytes())
                        .map_err(|e| Self::map_io(&path, e))?;
                    return Ok(candidate);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(Self::map_io(&path, error)),
            }
        }
        Err(RepositoryError::ArtifactVersionsExhausted(name.to_owned()))
    }
    fn snapshot(&self, run_id: &str, name: &str) -> Result<Option<PathBuf>, RepositoryError> {
        let source = self.path(run_id, name)?;
        if !source.exists() {
            return Ok(None);
        }
        for number in 1_u32.. {
            let target = self.run_dir(run_id).join(format!("{name}.{number}"));
            if !target.exists() {
                fs::copy(&source, &target).map_err(|e| Self::map_io(&target, e))?;
                return Ok(Some(target));
            }
        }
        unreachable!("u32 namespace exhausted")
    }
    fn path(&self, run_id: &str, name: &str) -> Result<PathBuf, RepositoryError> {
        Self::safe_name(name)?;
        Ok(self.run_dir(run_id).join(name))
    }
}

impl TextRepository for FileRepository {
    fn read_text(&self, path: &Path) -> Result<String, RepositoryError> {
        fs::read_to_string(path).map_err(|source| Self::map_io(path, source))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn failed_atomic_write_preserves_existing_draft() {
        use crate::runs::application::ArtifactRepository;
        let root = std::env::temp_dir().join(format!("miau-atomic-draft-{}", std::process::id()));
        let repo = super::FileRepository::new(&root);
        repo.write("001", "decision-drafts-0.json", "old draft")
            .unwrap();
        std::fs::create_dir(root.join("001/decision-drafts-0.json.tmp")).unwrap();
        assert!(
            repo.write("001", "decision-drafts-0.json", "new draft")
                .is_err()
        );
        assert_eq!(
            repo.read("001", "decision-drafts-0.json").unwrap(),
            "old draft"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    use super::*;
    use chrono::Utc;
    fn run(id: &str) -> Run {
        Run {
            id: id.into(),
            project: ".".into(),
            spec: None,
            nodes: vec![],
            cursor: 0,
            channel: vec![],
            created_at: Utc::now(),
        }
    }
    #[test]
    fn state_round_trips_and_tmp_is_removed() {
        let dir = std::env::temp_dir().join(format!("miau-repo-{}", std::process::id()));
        let repo = FileRepository::new(&dir);
        let expected = run("014");
        repo.save(&expected).unwrap();
        assert_eq!(repo.load("014").unwrap(), expected);
        assert!(!dir.join("014/state.json.tmp").exists());
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn snapshots_are_numbered_and_never_overwritten() {
        let dir = std::env::temp_dir().join(format!("miau-snapshot-{}", std::process::id()));
        let repo = FileRepository::new(&dir);
        repo.write("001", "plan.md", "one").unwrap();
        repo.snapshot("001", "plan.md").unwrap();
        repo.write("001", "plan.md", "two").unwrap();
        repo.snapshot("001", "plan.md").unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("001/plan.md.1")).unwrap(),
            "one"
        );
        assert_eq!(
            fs::read_to_string(dir.join("001/plan.md.2")).unwrap(),
            "two"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn versioned_writes_continue_from_the_current_artifact_name() {
        let dir = std::env::temp_dir().join(format!("miau-versions-{}", std::process::id()));
        let repo = FileRepository::new(&dir);
        repo.write("001", "plan.md", "one").unwrap();

        let second = repo.write_versioned("001", "plan.md", "two").unwrap();
        let third = repo.write_versioned("001", &second, "three").unwrap();

        assert_eq!(
            (second.as_str(), third.as_str()),
            ("plan_v2.md", "plan_v3.md")
        );
        let _ = fs::remove_dir_all(dir);
    }
}
