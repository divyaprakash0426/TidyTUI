use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod cleaner;
pub mod discovery;
pub mod disk;
pub mod paths;
pub mod perms;
pub mod policy;
pub mod registry;
pub mod scanner;

/// How a directory target is cleaned. `Contents` empties the directory but
/// keeps it (safe for caches that tools expect to exist); `Dir` removes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CleanMode {
    #[default]
    Contents,
    Dir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    Scanned,
    /// Cleaning was simulated (dry-run); nothing was removed.
    DryRun,
    Deleted,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct CleanupItem {
    /// Id of the definitions group this item came from (for `--select`).
    pub group_id: String,
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub file_count: u64,
    pub selected: bool,
    pub status: ItemStatus,
    pub mode: CleanMode,
    /// Only entries older than this many days are counted and removed.
    pub keep_days: Option<u64>,
    /// The current user lacks permission to clean this (needs root).
    pub locked: bool,
    /// Shell command that performs the cleaning instead of deleting `path`.
    pub command: Option<String>,
}
