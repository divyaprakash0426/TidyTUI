use crate::core::paths::expand_tilde;
use crate::core::policy;
use crate::core::registry::Target;
use crate::core::{CleanMode, CleanupItem, ItemStatus};
use rayon::prelude::*;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::SystemTime;
use walkdir::WalkDir;

/// Progress messages from a background scan. Exactly one `Finished` is sent last.
#[derive(Debug)]
pub enum ScanEvent {
    Found(CleanupItem),
    /// A target path did not exist; counts toward progress but yields no item.
    Missing,
    Finished,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanResult {
    pub size_bytes: u64,
    pub file_count: u64,
}

pub fn scan_path(path: &Path) -> ScanResult {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .fold(ScanResult::default(), |acc, m| ScanResult {
            size_bytes: acc.size_bytes + m.len(),
            file_count: acc.file_count + 1,
        })
}

/// Measures exactly what cleaning would remove: with `keep_days` on a
/// contents-mode directory only the old top-level entries are walked; on a
/// dir-mode directory or a file, a fresh target is not eligible at all.
fn measure(path: &Path, mode: CleanMode, keep_days: Option<u64>) -> Option<ScanResult> {
    let now = SystemTime::now();
    match (keep_days, path.is_dir(), mode) {
        (Some(_), true, CleanMode::Contents) => {
            let entries = policy::eligible_entries(path, keep_days, now).ok()?;
            Some(entries.iter().fold(ScanResult::default(), |acc, e| {
                let r = scan_path(&e.path());
                ScanResult {
                    size_bytes: acc.size_bytes + r.size_bytes,
                    file_count: acc.file_count + r.file_count,
                }
            }))
        }
        (Some(days), _, _) => policy::is_entry_older_than(path, days, now).then(|| scan_path(path)),
        (None, _, _) => Some(scan_path(path)),
    }
}

pub fn scan_target(target: Target, home: Option<&Path>) -> Option<CleanupItem> {
    let path = expand_tilde(&target.path, home);
    if !path.exists() {
        return None;
    }
    let result = measure(&path, target.mode, target.keep_days)?;
    if let Some(min) = target.min_size {
        if result.size_bytes < min.as_u64() {
            return None;
        }
    }
    Some(CleanupItem {
        group_id: target.group_id,
        name: target.name,
        category: target.category,
        description: target.description,
        path,
        size_bytes: result.size_bytes,
        file_count: result.file_count,
        selected: false,
        status: ItemStatus::Scanned,
        mode: target.mode,
        keep_days: target.keep_days,
    })
}

/// Scans on a background thread and streams results so the UI can render
/// while large directories are still being walked.
pub fn spawn_scan(targets: Vec<Target>) -> Receiver<ScanEvent> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let home = dirs::home_dir();
        targets.into_par_iter().for_each_with(tx.clone(), |tx, t| {
            let event = match scan_target(t, home.as_deref()) {
                Some(item) => ScanEvent::Found(item),
                None => ScanEvent::Missing,
            };
            // Receiver gone means the UI quit; nothing left to do.
            let _ = tx.send(event);
        });
        let _ = tx.send(ScanEvent::Finished);
    });
    rx
}

/// Blocking scan: collects everything `spawn_scan` finds.
pub fn scan_targets(targets: Vec<Target>) -> Vec<CleanupItem> {
    spawn_scan(targets)
        .into_iter()
        .filter_map(|ev| match ev {
            ScanEvent::Found(item) => Some(item),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::registry::Target;
    use crate::core::CleanMode;
    use std::fs;

    fn set_age(path: &std::path::Path, days: u64) {
        let t = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 24 * 60 * 60);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn keep_days_counts_only_old_top_level_entries() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.bin");
        let fresh = dir.path().join("fresh.bin");
        fs::write(&old, vec![0u8; 100]).unwrap();
        fs::write(&fresh, vec![0u8; 1000]).unwrap();
        set_age(&old, 60);

        let mut t = target(dir.path().to_str().unwrap(), CleanMode::Contents);
        t.keep_days = Some(30);
        let item = scan_target(t, None).unwrap();
        assert_eq!(item.size_bytes, 100);
        assert_eq!(item.file_count, 1);
        assert_eq!(item.keep_days, Some(30));
    }

    #[test]
    fn keep_days_on_dir_mode_skips_recently_modified_target() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("f"), b"x").unwrap();
        let mut t = target(dir.path().to_str().unwrap(), CleanMode::Dir);
        t.keep_days = Some(30);
        assert!(scan_target(t, None).is_none(), "fresh dir is not eligible");
    }

    #[test]
    fn min_size_drops_small_items() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("f"), vec![0u8; 10]).unwrap();
        let mut t = target(dir.path().to_str().unwrap(), CleanMode::Contents);
        t.min_size = Some(bytesize::ByteSize::kib(1));
        assert!(scan_target(t.clone(), None).is_none());
        t.min_size = Some(bytesize::ByteSize::b(10));
        assert!(scan_target(t, None).is_some(), "threshold is inclusive");
    }

    fn target(path: &str, mode: CleanMode) -> Target {
        Target {
            group_id: "x".into(),
            name: "X".into(),
            category: "C".into(),
            description: Some("d".into()),
            path: path.into(),
            mode,
            keep_days: None,
            min_size: None,
        }
    }

    #[test]
    fn counts_bytes_and_files_recursively() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.bin"), vec![0u8; 100]).unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/b.bin"), vec![0u8; 50]).unwrap();
        let r = scan_path(dir.path());
        assert_eq!(r.size_bytes, 150);
        assert_eq!(r.file_count, 2);
    }

    #[test]
    fn spawn_scan_streams_found_missing_then_finished() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), b"abc").unwrap();
        let rx = spawn_scan(vec![
            target(&dir.path().to_string_lossy(), CleanMode::Contents),
            target("/definitely/not/here", CleanMode::Contents),
        ]);
        let events: Vec<ScanEvent> = rx.iter().collect();
        assert_eq!(events.len(), 3);
        assert!(matches!(events.last(), Some(ScanEvent::Finished)));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, ScanEvent::Found(_)))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, ScanEvent::Missing))
                .count(),
            1
        );
    }

    #[test]
    fn skips_missing_targets() {
        let t = target("/definitely/not/here", CleanMode::Contents);
        assert!(scan_targets(vec![t]).is_empty());
    }

    #[test]
    fn builds_item_from_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), b"abc").unwrap();
        let t = target(&dir.path().to_string_lossy(), CleanMode::Dir);
        let items = scan_targets(vec![t]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].size_bytes, 3);
        assert_eq!(items[0].file_count, 1);
        assert_eq!(items[0].mode, CleanMode::Dir);
        assert_eq!(items[0].description.as_deref(), Some("d"));
        assert_eq!(items[0].status, ItemStatus::Scanned);
    }
}
