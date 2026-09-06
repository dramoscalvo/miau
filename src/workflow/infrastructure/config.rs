use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::Deserialize;
use thiserror::Error;

use crate::{execution::application::AgentConfig, workflow::domain::WorkflowConfig};

const DEFAULT_AGENTS: &str = include_str!("../../../agents.toml");
const DEFAULT_WORKFLOW: &str = include_str!("../../../workflow.toml");
const DEFAULT_PLANNER_ROLE: &str = include_str!("../../../roles/planner.md");
const DEFAULT_CRITIC_ROLE: &str = include_str!("../../../roles/critic.md");
const DEFAULT_IMPLEMENTER_ROLE: &str = include_str!("../../../roles/implementer.md");
const DEFAULT_REVIEWER_ROLE: &str = include_str!("../../../roles/reviewer.md");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    pub agents: PathBuf,
    pub workflow: PathBuf,
    pub roles: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid TOML in {path}: {source}")]
    Toml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("configuration directory is unavailable")]
    NoConfigDirectory,
    #[error("cannot create configuration directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot install default configuration at {path}: {source}")]
    Install {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub fn config_dir() -> Result<PathBuf, ConfigError> {
    ProjectDirs::from("dev", "miau", "miau")
        .map(|project| project.config_dir().to_path_buf())
        .ok_or(ConfigError::NoConfigDirectory)
}

pub fn ensure_config() -> Result<ConfigPaths, ConfigError> {
    ensure_config_at(&config_dir()?)
}

pub fn load_agents(path: &Path) -> Result<HashMap<String, AgentConfig>, ConfigError> {
    load(path)
}
pub fn load_workflow(path: &Path) -> Result<WorkflowConfig, ConfigError> {
    load(path)
}

fn load<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| ConfigError::Toml {
        path: path.to_path_buf(),
        source,
    })
}

fn ensure_config_at(directory: &Path) -> Result<ConfigPaths, ConfigError> {
    let roles = directory.join("roles");
    create_directory(directory)?;
    create_directory(&roles)?;

    let agents = directory.join("agents.toml");
    let workflow = directory.join("workflow.toml");
    write_default(&agents, DEFAULT_AGENTS)?;
    write_default(&workflow, DEFAULT_WORKFLOW)?;
    write_default(&roles.join("planner.md"), DEFAULT_PLANNER_ROLE)?;
    write_default(&roles.join("critic.md"), DEFAULT_CRITIC_ROLE)?;
    write_default(&roles.join("implementer.md"), DEFAULT_IMPLEMENTER_ROLE)?;
    write_default(&roles.join("reviewer.md"), DEFAULT_REVIEWER_ROLE)?;

    Ok(ConfigPaths {
        agents,
        workflow,
        roles,
    })
}

fn create_directory(path: &Path) -> Result<(), ConfigError> {
    fs::create_dir_all(path).map_err(|source| ConfigError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })
}

fn write_default(path: &Path, contents: &str) -> Result<(), ConfigError> {
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(source) => {
            return Err(ConfigError::Install {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    file.write_all(contents.as_bytes())
        .map_err(|source| ConfigError::Install {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "miau-config-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn ensure_config_at_writes_embedded_defaults_to_an_empty_directory() {
        let directory = temporary_directory("defaults");

        let paths = ensure_config_at(&directory).unwrap();

        assert_eq!(fs::read_to_string(paths.agents).unwrap(), DEFAULT_AGENTS);
        assert_eq!(
            fs::read_to_string(paths.workflow).unwrap(),
            DEFAULT_WORKFLOW
        );
        assert_eq!(
            fs::read_to_string(paths.roles.join("planner.md")).unwrap(),
            DEFAULT_PLANNER_ROLE
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn ensure_config_at_preserves_existing_configuration() {
        let directory = temporary_directory("preserves");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("agents.toml"), "user configuration").unwrap();

        let paths = ensure_config_at(&directory).unwrap();

        assert_eq!(
            fs::read_to_string(paths.agents).unwrap(),
            "user configuration"
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn embedded_workflow_reuses_the_codex_delivery_session() {
        let workflow: WorkflowConfig = toml::from_str(DEFAULT_WORKFLOW).unwrap();
        let groups: Vec<_> = workflow
            .nodes
            .iter()
            .filter_map(|node| {
                node.session_group
                    .as_deref()
                    .map(|group| (node.name.as_str(), group))
            })
            .collect();

        assert_eq!(
            groups,
            [("critique", "delivery"), ("implement", "delivery")]
        );
    }
}
