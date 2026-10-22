//! The config file `~/.config/nightshift/config` (`key = value`, comments and order kept) (v4 bounds). Skeleton of task 18.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConfigFileError {
    #[error("not_built")]
    NotBuilt,
}

pub type Result<T> = std::result::Result<T, ConfigFileError>;

/// One line the file could not apply.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConfigError {
    pub line: usize,
    pub key: String,
    pub msg: String,
}

pub struct Config {
    pub path: PathBuf,
}

impl Config {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    /// A patch of `Settings` (JSON, merged by the caller) and the lines that were wrong.
    pub fn load(&self) -> Result<(serde_json::Value, Vec<ConfigError>)> {
        Err(ConfigFileError::NotBuilt)
    }
    pub fn write_key(&self, _key: &str, _value: &str) -> Result<()> {
        Err(ConfigFileError::NotBuilt)
    }
    pub fn watch(&self, _cb: Box<dyn Fn(Vec<ConfigError>) + Send>) -> Result<()> {
        Err(ConfigFileError::NotBuilt)
    }
}
