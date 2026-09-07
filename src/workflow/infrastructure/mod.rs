//! TOML configuration and deterministic process adapters.

mod change_detector;
mod config;
mod working_tree;

pub use change_detector::GitChangeDetector;
pub use config::{ConfigError, ConfigPaths, config_dir, ensure_config, load_agents, load_workflow};
pub use working_tree::GitWorkingTree;
