//! Persistence ports used by run and workflow use cases.

use std::{
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::domain::Run;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid run state at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("artifact path escapes its run directory: {0}")]
    UnsafeArtifact(String),
    #[error("artifact version namespace exhausted for {0}")]
    ArtifactVersionsExhausted(String),
}

pub trait RunRepository {
    fn load(&self, id: &str) -> Result<Run, RepositoryError>;
    fn save(&self, run: &Run) -> Result<(), RepositoryError>;
    fn list(&self) -> Result<Vec<Run>, RepositoryError>;
}

pub trait ArtifactRepository {
    fn read(&self, run_id: &str, name: &str) -> Result<String, RepositoryError>;
    fn write(&self, run_id: &str, name: &str, text: &str) -> Result<(), RepositoryError>;
    fn write_versioned(
        &self,
        run_id: &str,
        name: &str,
        text: &str,
    ) -> Result<String, RepositoryError>;
    fn snapshot(&self, run_id: &str, name: &str) -> Result<Option<PathBuf>, RepositoryError>;
    fn path(&self, run_id: &str, name: &str) -> Result<PathBuf, RepositoryError>;
}

pub trait TextRepository {
    fn read_text(&self, path: &Path) -> Result<String, RepositoryError>;
}
