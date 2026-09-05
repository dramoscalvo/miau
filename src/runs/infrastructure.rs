//! Filesystem persistence adapters for runs and artifacts.

use super::{
    application::{ArtifactRepository, RepositoryError, RunRepository, TextRepository},
    domain::Run,
};
use std::{
    fs, io,
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
        fs::write(&path, text).map_err(|e| Self::map_io(&path, e))
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
}
