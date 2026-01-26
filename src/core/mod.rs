use std::path::PathBuf;

pub mod discovery;
pub mod registry;
pub mod scanner;
pub mod cleaner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    Pending,
    Scanning,
    Scanned,
    Deleted,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct CleanupItem {
    pub group_id: String,
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub selected: bool,
    pub status: ItemStatus,
}
