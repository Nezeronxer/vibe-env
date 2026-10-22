//! Catalogs of skills, MCP servers and mods (v4 bounds). Skeleton of task 18: types and signatures only.
use crate::kits::SnapshotInfo;
use crate::net::Fetch;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MarketError {
    #[error("not_built")]
    NotBuilt,
}

pub type Result<T> = std::result::Result<T, MarketError>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Skill,
    Mcp,
    Mod,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Source {
    pub id: String,
    pub kind: Kind,
    pub url: String,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ItemDetail {
    pub item: Item,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Global,
    Project(PathBuf),
}

/// What would be written and what of it is executable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InstallPlan {
    pub item: Item,
    pub env: String,
    pub scope: Scope,
    pub writes: Vec<String>,
    pub executable: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Installed {
    pub id: String,
    pub version: String,
    pub update: Option<String>,
}

pub struct Market {
    _fetch: Box<dyn Fetch>,
    _cache_dir: PathBuf,
}

impl Market {
    pub fn new(fetch: Box<dyn Fetch>, cache_dir: PathBuf) -> Self {
        Self { _fetch: fetch, _cache_dir: cache_dir }
    }
    pub fn sources(&self) -> Result<Vec<Source>> {
        Err(MarketError::NotBuilt)
    }
    pub fn set_sources(&self, _sources: Vec<Source>) -> Result<()> {
        Err(MarketError::NotBuilt)
    }
    pub fn search(&self, _kind: Kind, _query: &str, _cursor: Option<&str>) -> Result<Page<Item>> {
        Err(MarketError::NotBuilt)
    }
    pub fn item(&self, _id: &str) -> Result<ItemDetail> {
        Err(MarketError::NotBuilt)
    }
    pub fn install(&self, _item: &Item, _env: &str, _scope: Scope) -> Result<InstallPlan> {
        Err(MarketError::NotBuilt)
    }
    pub fn apply(&self, _plan: &InstallPlan, _enabled_exec: Vec<String>) -> Result<SnapshotInfo> {
        Err(MarketError::NotBuilt)
    }
    pub fn installed(&self, _env: &str) -> Result<Vec<Installed>> {
        Err(MarketError::NotBuilt)
    }
    pub fn remove(&self, _env: &str, _id: &str) -> Result<()> {
        Err(MarketError::NotBuilt)
    }
}
