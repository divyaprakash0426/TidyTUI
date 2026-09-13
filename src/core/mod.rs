use std::path::PathBuf;

pub mod cleaner;
pub mod discovery;
pub mod paths;
pub mod registry;
pub mod scanner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    Scanned,
    Deleted,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct CleanupItem {
    pub name: String,
    pub category: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub selected: bool,
    pub status: ItemStatus,
}
