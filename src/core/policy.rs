//! Age-based eligibility rules shared by the scanner and the cleaner so that
//! what is *counted* is exactly what would be *removed*.

use std::fs::{self, DirEntry, Metadata};
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

const SECS_PER_DAY: u64 = 24 * 60 * 60;

/// True when the entry was last modified at least `days` days before `now`.
pub fn is_older_than(meta: &Metadata, days: u64, now: SystemTime) -> bool {
    let Ok(modified) = meta.modified() else {
        // Unknown mtime: err on the side of keeping the entry.
        return false;
    };
    let cutoff = now - Duration::from_secs(days.saturating_mul(SECS_PER_DAY));
    modified <= cutoff
}

/// Whether the entry (and, for a directory, everything beneath it) is older
/// than `days`. A directory's own mtime only tracks its direct children, so
/// bucketed caches look ancient while holding files written today; walking
/// the subtree is the only way to know nothing fresh would be removed.
/// Anything with an unreadable mtime is treated as fresh (kept).
pub fn is_entry_older_than(path: &Path, days: u64, now: SystemTime) -> bool {
    // symlink_metadata: judge the link itself, never its target
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !is_older_than(&meta, days, now) {
        return false;
    }
    if !meta.is_dir() {
        return true;
    }
    WalkDir::new(path)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .all(|e| match e.and_then(|e| e.metadata()) {
            Ok(m) => is_older_than(&m, days, now),
            Err(_) => false,
        })
}

/// Top-level entries of `dir` that may be removed. With `keep_days = Some(n)`
/// only entries whose whole subtree is older than `n` days qualify; `None`
/// means everything.
pub fn eligible_entries(
    dir: &Path,
    keep_days: Option<u64>,
    now: SystemTime,
) -> io::Result<Vec<DirEntry>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let eligible = match keep_days {
            None => true,
            Some(days) => is_entry_older_than(&entry.path(), days, now),
        };
        if eligible {
            out.push(entry);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn set_age(path: &Path, days: u64) {
        let t = SystemTime::now() - Duration::from_secs(days * SECS_PER_DAY);
        // A read-only handle is enough for futimens, and it works on directories.
        File::open(path).unwrap().set_modified(t).unwrap();
    }

    #[test]
    fn is_older_than_respects_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f");
        File::create(&f).unwrap();
        set_age(&f, 10);
        let meta = fs::metadata(&f).unwrap();
        let now = SystemTime::now();
        assert!(is_older_than(&meta, 9, now));
        assert!(is_older_than(&meta, 10, now));
        assert!(!is_older_than(&meta, 11, now));
    }

    #[test]
    fn directory_entry_is_judged_by_its_newest_descendant() {
        let dir = tempfile::tempdir().unwrap();
        let bucket = dir.path().join("http");
        fs::create_dir_all(bucket.join("a")).unwrap();
        let fresh = bucket.join("a").join("fresh");
        File::create(&fresh).unwrap();
        // Bucket dirs keep an old mtime because their direct children never change.
        set_age(&bucket.join("a"), 60);
        set_age(&bucket, 60);

        let now = SystemTime::now();
        assert!(
            eligible_entries(dir.path(), Some(30), now)
                .unwrap()
                .is_empty(),
            "a directory containing a fresh file must not be eligible"
        );
        set_age(&fresh, 45);
        assert_eq!(
            eligible_entries(dir.path(), Some(30), now).unwrap().len(),
            1
        );
    }

    #[test]
    fn eligible_entries_filters_by_age_or_returns_all() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        let fresh = dir.path().join("fresh");
        File::create(&old).unwrap();
        File::create(&fresh).unwrap();
        set_age(&old, 40);

        let names = |v: Vec<DirEntry>| {
            let mut n: Vec<String> = v
                .into_iter()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            n.sort();
            n
        };
        let now = SystemTime::now();
        assert_eq!(
            names(eligible_entries(dir.path(), None, now).unwrap()),
            vec!["fresh", "old"]
        );
        assert_eq!(
            names(eligible_entries(dir.path(), Some(30), now).unwrap()),
            vec!["old"]
        );
    }
}
