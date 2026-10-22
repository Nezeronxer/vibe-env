//! CLI versions, how they were installed, update with rollback, new models (v4 bounds). Skeleton of task 18.
use crate::agent::Adapter;
use crate::net::Fetch;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum UpdateError {
    #[error("not_built")]
    NotBuilt,
}

pub type Result<T> = std::result::Result<T, UpdateError>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Native,
    Npm,
    Brew,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CliStatus {
    pub installed: Option<String>,
    pub latest: Option<String>,
    pub method: Method,
    pub cmd: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UpdateLog {
    pub env: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub ok: bool,
    pub output: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NewModel {
    pub env: String,
    pub id: String,
    pub name: String,
}

pub struct Updates {
    _fetch: Box<dyn Fetch>,
    _adapters: Vec<Arc<dyn Adapter>>,
}

impl Updates {
    pub fn new(fetch: Box<dyn Fetch>, adapters: Vec<Arc<dyn Adapter>>) -> Self {
        Self { _fetch: fetch, _adapters: adapters }
    }
    pub fn check(&self, _env: &str) -> Result<CliStatus> {
        Err(UpdateError::NotBuilt)
    }
    /// Rolls back by itself when `--version` fails after the update.
    pub fn update(&self, _env: &str) -> Result<UpdateLog> {
        Err(UpdateError::NotBuilt)
    }
    pub fn models_radar(&self, _etag: Option<&str>) -> Result<Vec<NewModel>> {
        Err(UpdateError::NotBuilt)
    }
    pub fn history(&self, _env: &str) -> Result<Vec<UpdateLog>> {
        Err(UpdateError::NotBuilt)
    }
}
