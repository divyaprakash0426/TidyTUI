use crate::core::{policy, scanner};
use crate::core::{CleanMode, CleanupItem, ItemStatus};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::SystemTime;

/// Progress messages from a background cleaning run. Jobs run in order, so
/// each `Started` is the next job; `idx` on `Finished` is the caller's index
/// into its own item list. Exactly one `Done` is sent last.
#[derive(Debug)]
pub enum CleanEvent {
    Started {
        name: String,
    },
    /// `freed` is what the run actually reclaimed (re-measured for commands).
    Finished {
        idx: usize,
        status: ItemStatus,
        freed: u64,
    },
    Done,
}

/// Cleans sequentially on a background thread (deletion is IO-bound, so
/// parallelism buys nothing and would muddle progress reporting).
pub fn spawn_clean(jobs: Vec<(usize, CleanupItem)>, dry_run: bool) -> Receiver<CleanEvent> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for (idx, mut item) in jobs {
            let _ = tx.send(CleanEvent::Started {
                name: item.name.clone(),
            });
            // Outcome is captured in item.status; the Err duplicates it.
            let _ = clean_item(&mut item, dry_run);
            let _ = tx.send(CleanEvent::Finished {
                idx,
                status: item.status,
                freed: item.size_bytes,
            });
        }
        let _ = tx.send(CleanEvent::Done);
    });
    rx
}

/// Removes every entry inside `dir` but keeps `dir` itself. Symlinks are
/// unlinked, never followed. Continues past individual failures and reports
/// them collectively so a single locked file does not abort the whole sweep.
pub fn remove_contents(dir: &Path, keep_days: Option<u64>) -> Result<()> {
    let entries = policy::eligible_entries(dir, keep_days, SystemTime::now())
        .with_context(|| format!("reading {}", dir.display()))?;
    let mut failures: Vec<String> = Vec::new();

    for entry in entries {
        let path = entry.path();
        // file_type() does not follow symlinks, so a symlinked dir is treated as a file.
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let result = if is_dir {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if let Err(e) = result {
            failures.push(format!("{}: {e}", path.display()));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        let plural = if failures.len() == 1 {
            "entry"
        } else {
            "entries"
        };
        anyhow::bail!(
            "{} {plural} could not be removed (first: {})",
            failures.len(),
            failures[0]
        )
    }
}

/// Runs `cmd` through `sh -c`, then re-measures the path so `size_bytes`
/// becomes what the command really reclaimed rather than the pre-run estimate.
fn run_command(item: &mut CleanupItem, cmd: &str) -> Result<()> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("running `{cmd}`"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let last = stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        let code = output
            .status
            .code()
            .map(|c| format!("exit code {c}"))
            .unwrap_or_else(|| "killed by signal".to_string());
        anyhow::bail!("`{cmd}` failed ({code}) {last}");
    }
    // Re-measure exactly as the scanner did (same mode and age filter), so
    // fresh content the rule never targeted cannot mask what was freed.
    let after = if item.path.exists() {
        scanner::measure(&item.path, item.mode, item.keep_days).map_or(0, |r| r.size_bytes)
    } else {
        0
    };
    item.size_bytes = item.size_bytes.saturating_sub(after);
    Ok(())
}

pub fn clean_item(item: &mut CleanupItem, dry_run: bool) -> Result<()> {
    if dry_run {
        item.status = ItemStatus::DryRun;
        return Ok(());
    }

    if !item.path.exists() {
        item.status = ItemStatus::Deleted;
        return Ok(());
    }

    let result = if let Some(cmd) = item.command.clone() {
        run_command(item, &cmd)
    } else if item.path.is_dir() {
        match item.mode {
            CleanMode::Contents => remove_contents(&item.path, item.keep_days),
            CleanMode::Dir => fs::remove_dir_all(&item.path).context("removing directory"),
        }
    } else {
        fs::remove_file(&item.path).context("removing file")
    };

    match result {
        Ok(()) => {
            item.status = ItemStatus::Deleted;
            Ok(())
        }
        Err(e) => {
            item.status = ItemStatus::Failed(e.to_string());
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_item_runs_shell_and_reports_freed_bytes_by_remeasuring() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("big"), [0u8; 100]).unwrap();
        fs::write(dir.path().join("small"), [0u8; 20]).unwrap();
        let mut it = item(dir.path(), CleanMode::Contents);
        it.size_bytes = 120;
        it.command = Some(format!("rm {}", dir.path().join("big").display()));

        clean_item(&mut it, false).unwrap();

        assert_eq!(it.status, ItemStatus::Deleted);
        assert_eq!(it.size_bytes, 100, "freed = before (120) - after (20)");
        assert!(dir.path().join("small").exists());
    }

    #[test]
    fn command_with_keep_days_measures_freed_bytes_against_eligible_entries_only() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        fs::write(&old, [0u8; 100]).unwrap();
        fs::write(dir.path().join("fresh"), [0u8; 1000]).unwrap();
        let t = std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 86_400);
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(t)
            .unwrap();
        let mut it = item(dir.path(), CleanMode::Contents);
        it.keep_days = Some(30);
        it.size_bytes = 100; // what the scanner measured: old entries only
        it.command = Some(format!("rm {}", old.display()));

        clean_item(&mut it, false).unwrap();

        assert_eq!(it.status, ItemStatus::Deleted);
        assert_eq!(
            it.size_bytes, 100,
            "fresh content must not mask what was freed"
        );
    }

    #[test]
    fn dry_run_never_executes_the_command() {
        let dir = tempfile::tempdir().unwrap();
        let sentinel = dir.path().join("ran");
        let mut it = item(dir.path(), CleanMode::Contents);
        it.command = Some(format!("touch {}", sentinel.display()));

        clean_item(&mut it, true).unwrap();

        assert_eq!(it.status, ItemStatus::DryRun);
        assert!(!sentinel.exists());
    }

    #[test]
    fn failing_command_reports_exit_code_and_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let mut it = item(dir.path(), CleanMode::Contents);
        it.command = Some("echo boom >&2; exit 3".into());

        assert!(clean_item(&mut it, false).is_err());

        match &it.status {
            ItemStatus::Failed(reason) => {
                assert!(reason.contains("exit code 3"), "{reason}");
                assert!(reason.contains("boom"), "{reason}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn remove_contents_with_keep_days_spares_fresh_entries() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        let fresh = dir.path().join("fresh");
        fs::write(&old, b"o").unwrap();
        fs::write(&fresh, b"f").unwrap();
        let t = std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 86_400);
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(t)
            .unwrap();

        remove_contents(dir.path(), Some(30)).unwrap();
        assert!(!old.exists());
        assert!(fresh.exists());
    }
    use crate::core::CleanMode;
    use std::fs;
    use std::path::Path;

    fn item(path: &Path, mode: CleanMode) -> CleanupItem {
        CleanupItem {
            group_id: String::new(),
            name: "t".into(),
            category: "c".into(),
            description: None,
            path: path.to_path_buf(),
            size_bytes: 0,
            file_count: 0,
            selected: true,
            status: ItemStatus::Scanned,
            mode,
            keep_days: None,
            locked: false,
            command: None,
        }
    }

    #[test]
    fn spawn_clean_reports_each_item_then_done() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), b"x").unwrap();
        let it = item(dir.path(), CleanMode::Contents);
        let rx = spawn_clean(vec![(7, it)], false);
        let events: Vec<CleanEvent> = rx.iter().collect();
        assert!(matches!(events[0], CleanEvent::Started { .. }));
        assert!(matches!(
            events[1],
            CleanEvent::Finished {
                idx: 7,
                status: ItemStatus::Deleted,
                ..
            }
        ));
        assert!(matches!(events[2], CleanEvent::Done));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn spawn_clean_dry_run_reports_dryrun_status() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), b"x").unwrap();
        let rx = spawn_clean(vec![(0, item(dir.path(), CleanMode::Contents))], true);
        let events: Vec<CleanEvent> = rx.iter().collect();
        assert!(matches!(
            events[1],
            CleanEvent::Finished {
                idx: 0,
                status: ItemStatus::DryRun,
                ..
            }
        ));
        assert!(dir.path().join("a").exists());
    }

    #[test]
    fn dry_run_deletes_nothing_and_marks_dryrun() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), b"x").unwrap();
        let mut it = item(dir.path(), CleanMode::Contents);
        clean_item(&mut it, true).unwrap();
        assert!(dir.path().join("a").exists());
        assert_eq!(it.status, ItemStatus::DryRun);
    }

    #[test]
    fn contents_mode_keeps_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), b"x").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/b"), b"y").unwrap();
        let mut it = item(dir.path(), CleanMode::Contents);
        clean_item(&mut it, false).unwrap();
        assert!(dir.path().exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        assert_eq!(it.status, ItemStatus::Deleted);
    }

    #[test]
    fn dir_mode_removes_directory() {
        let parent = tempfile::tempdir().unwrap();
        let target = parent.path().join("victim");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("a"), b"x").unwrap();
        let mut it = item(&target, CleanMode::Dir);
        clean_item(&mut it, false).unwrap();
        assert!(!target.exists());
        assert_eq!(it.status, ItemStatus::Deleted);
    }

    #[test]
    fn file_target_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f");
        fs::write(&f, b"x").unwrap();
        let mut it = item(&f, CleanMode::Contents);
        clean_item(&mut it, false).unwrap();
        assert!(!f.exists());
    }

    #[test]
    fn missing_path_is_noop_deleted() {
        let mut it = item(Path::new("/nope/nope"), CleanMode::Contents);
        clean_item(&mut it, false).unwrap();
        assert_eq!(it.status, ItemStatus::Deleted);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_inside_dir_is_unlinked_not_followed() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep"), b"k").unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        let mut it = item(dir.path(), CleanMode::Contents);
        clean_item(&mut it, false).unwrap();
        assert!(!dir.path().join("link").exists());
        assert!(
            outside.path().join("keep").exists(),
            "must not follow symlinks"
        );
    }

    #[test]
    fn failure_is_recorded_in_status() {
        let mut it = item(Path::new("/proc/1/mem"), CleanMode::Contents);
        // /proc/1/mem exists but is not removable by an unprivileged user
        if !Path::new("/proc/1/mem").exists() {
            return;
        }
        let res = clean_item(&mut it, false);
        assert!(res.is_err());
        assert!(matches!(it.status, ItemStatus::Failed(_)));
    }
}
