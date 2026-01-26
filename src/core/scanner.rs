use std::path::Path;
use walkdir::WalkDir;
use rayon::prelude::*;
use crate::core::{CleanupItem, ItemStatus};

pub fn scan_path(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

pub fn scan_targets(targets: Vec<(String, String, String)>) -> Vec<CleanupItem> {
    targets
        .into_par_iter()
        .filter_map(|(name, category, path_str)| {
            // Expand ~ to user home if necessary
            let path = if path_str.starts_with("~") {
                if let Some(home) = dirs::home_dir() {
                    let without_tilde = path_str.strip_prefix("~").unwrap_or(&path_str);
                    let without_slash = without_tilde.strip_prefix("/").unwrap_or(without_tilde);
                    home.join(without_slash)
                } else {
                    std::path::PathBuf::from(path_str)
                }
            } else {
                std::path::PathBuf::from(path_str)
            };

            if path.exists() {
                let size_bytes = scan_path(&path);
                
                Some(CleanupItem {
                    name,
                    category,
                    path,
                    size_bytes,
                    selected: false,
                    status: ItemStatus::Scanned,
                })
            } else {
                None
            }
        })
        .collect()
}
