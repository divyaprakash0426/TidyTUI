//! Age-based eligibility rules shared by the scanner and the cleaner so that
//! what is *counted* is exactly what would be *removed*.

use std::fs::{self, DirEntry, Metadata};
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};

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

/// Top-level entries of `dir` that may be removed. With `keep_days = Some(n)`
/// only entries older than `n` days qualify; `None` means everything.
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
            // symlink_metadata: judge the link itself, never its target
            Some(days) => match fs::symlink_metadata(entry.path()) {
                Ok(meta) => is_older_than(&meta, days, now),
                Err(_) => false,
            },
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
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
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
