//! Non-interactive modes: `--list [--json]` and `--yes`.

use crate::core::paths::shorten_home;
use crate::core::{cleaner, CleanupItem, ItemStatus};
use bytesize::ByteSize;
use serde::Serialize;
use std::fmt::Write as _;
use std::path::Path;

pub fn list_table(items: &[CleanupItem], home: Option<&Path>) -> String {
    let name_w = items.iter().map(|i| i.name.len()).max().unwrap_or(4).max(4);
    let group_w = items
        .iter()
        .map(|i| i.group_id.len())
        .max()
        .unwrap_or(5)
        .max(5);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<name_w$}  {:<group_w$}  {:>10}  {:>8}  PATH",
        "NAME", "GROUP", "SIZE", "FILES"
    );
    for i in items {
        let _ = writeln!(
            out,
            "{:<name_w$}  {:<group_w$}  {:>10}  {:>8}  {}{}",
            i.name,
            i.group_id,
            ByteSize(i.size_bytes).to_string(),
            i.file_count,
            shorten_home(&i.path, home),
            if i.locked { "  (needs root)" } else { "" }
        );
    }
    let total: u64 = items.iter().map(|i| i.size_bytes).sum();
    let _ = writeln!(
        out,
        "\n{} locations, {} reclaimable",
        items.len(),
        ByteSize(total)
    );
    out
}

#[derive(Serialize)]
struct JsonItem<'a> {
    group: &'a str,
    name: &'a str,
    category: &'a str,
    path: String,
    size_bytes: u64,
    file_count: u64,
    mode: crate::core::CleanMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_days: Option<u64>,
    locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a str>,
}

pub fn list_json(items: &[CleanupItem]) -> String {
    let rows: Vec<JsonItem> = items
        .iter()
        .map(|i| JsonItem {
            group: &i.group_id,
            name: &i.name,
            category: &i.category,
            path: i.path.to_string_lossy().into_owned(),
            size_bytes: i.size_bytes,
            file_count: i.file_count,
            mode: i.mode,
            keep_days: i.keep_days,
            locked: i.locked,
            command: i.command.as_deref(),
        })
        .collect();
    serde_json::to_string_pretty(&rows).expect("serialising plain data cannot fail")
}

/// Outcome of a headless clean.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub dry_run: bool,
    pub cleaned: usize,
    pub freed_bytes: u64,
    pub failed: Vec<(String, String)>,
}

impl Report {
    pub fn render(&self, home: Option<&Path>, items: &[CleanupItem]) -> String {
        let mut out = String::new();
        for i in items {
            let status = match &i.status {
                ItemStatus::DryRun => "would delete".to_string(),
                ItemStatus::Deleted => "deleted".to_string(),
                ItemStatus::Failed(r) => format!("FAILED: {r}"),
                ItemStatus::Scanned => "skipped".to_string(),
            };
            let _ = writeln!(
                out,
                "{:>10}  {:<14} {}",
                ByteSize(i.size_bytes).to_string(),
                status,
                shorten_home(&i.path, home)
            );
        }
        let verb = if self.dry_run { "Would free" } else { "Freed" };
        let _ = write!(
            out,
            "\n{verb} {} across {} items",
            ByteSize(self.freed_bytes),
            self.cleaned
        );
        if !self.failed.is_empty() {
            let _ = write!(out, "; {} failed", self.failed.len());
        }
        out.push('\n');
        out
    }
}

/// Cleans `items` in place, sequentially, and summarises the outcome.
pub fn clean_all(items: &mut [CleanupItem], dry_run: bool) -> Report {
    let mut report = Report {
        dry_run,
        ..Report::default()
    };
    for item in items.iter_mut() {
        let _ = cleaner::clean_item(item, dry_run);
        match &item.status {
            ItemStatus::Deleted | ItemStatus::DryRun => {
                report.cleaned += 1;
                report.freed_bytes += item.size_bytes;
            }
            ItemStatus::Failed(reason) => report.failed.push((item.name.clone(), reason.clone())),
            ItemStatus::Scanned => {}
        }
    }
    report
}

/// Keeps only items whose group id is in `ids`; unknown ids are returned so the
/// caller can warn about typos.
pub fn select_groups(items: Vec<CleanupItem>, ids: &[String]) -> (Vec<CleanupItem>, Vec<String>) {
    let unknown: Vec<String> = ids
        .iter()
        .filter(|id| !items.iter().any(|i| &i.group_id == *id))
        .cloned()
        .collect();
    let kept = items
        .into_iter()
        .filter(|i| ids.contains(&i.group_id))
        .collect();
    (kept, unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::CleanMode;
    use std::path::PathBuf;

    fn item(group: &str, name: &str, path: &str, size: u64) -> CleanupItem {
        CleanupItem {
            group_id: group.into(),
            name: name.into(),
            category: "Dev".into(),
            description: None,
            path: PathBuf::from(path),
            size_bytes: size,
            file_count: 2,
            selected: false,
            status: ItemStatus::Scanned,
            mode: CleanMode::Contents,
            keep_days: None,
            locked: false,
            command: None,
        }
    }

    #[test]
    fn locked_items_are_flagged_in_table_and_json() {
        let mut it = item("pkg", "Pacman", "/var/cache/pacman/pkg", 5);
        it.locked = true;
        let table = list_table(std::slice::from_ref(&it), None);
        assert!(table.contains("(needs root)"), "{table}");
        let json = list_json(std::slice::from_ref(&it));
        assert!(json.contains("\"locked\": true"), "{json}");
    }

    #[test]
    fn table_lists_name_group_size_and_path() {
        let s = list_table(
            &[item("dev_pip", "Pip Cache", "/home/u/.cache/pip", 2 << 20)],
            Some(Path::new("/home/u")),
        );
        assert!(s.contains("NAME"), "{s}");
        assert!(s.contains("Pip Cache"), "{s}");
        assert!(s.contains("dev_pip"), "{s}");
        assert!(s.contains("2.0 MiB"), "{s}");
        assert!(s.contains("~/.cache/pip"), "{s}");
        assert!(s.contains("1 locations, 2.0 MiB reclaimable"), "{s}");
    }

    #[test]
    fn json_is_valid_and_has_expected_fields() {
        let s = list_json(&[item("dev_pip", "Pip", "/p", 5)]);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v[0]["group"], "dev_pip");
        assert_eq!(v[0]["path"], "/p");
        assert_eq!(v[0]["size_bytes"], 5);
        assert_eq!(v[0]["mode"], "contents");
        assert!(v[0].get("keep_days").is_none());
    }

    #[test]
    fn select_groups_keeps_matches_and_reports_unknown_ids() {
        let items = vec![item("a", "A", "/a", 1), item("b", "B", "/b", 1)];
        let (kept, unknown) = select_groups(items, &["b".into(), "zzz".into()]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].group_id, "b");
        assert_eq!(unknown, vec!["zzz"]);
    }

    #[test]
    fn clean_all_dry_run_reports_would_free() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), b"xx").unwrap();
        let mut items = vec![item("a", "A", dir.path().to_str().unwrap(), 2)];
        let report = clean_all(&mut items, true);
        assert_eq!(report.cleaned, 1);
        assert_eq!(report.freed_bytes, 2);
        assert!(dir.path().join("f").exists());
        let s = report.render(None, &items);
        assert!(s.contains("would delete"), "{s}");
        assert!(s.contains("Would free 2 B across 1 items"), "{s}");
    }

    #[test]
    #[cfg(unix)]
    fn clean_all_real_run_deletes_and_counts_failures() {
        use std::os::unix::fs::PermissionsExt;
        let ok = tempfile::tempdir().unwrap();
        std::fs::write(ok.path().join("f"), b"xx").unwrap();
        // A read-only directory: its entries cannot be unlinked by a normal user.
        let locked = tempfile::tempdir().unwrap();
        std::fs::write(locked.path().join("f"), b"x").unwrap();
        std::fs::set_permissions(locked.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

        let mut items = vec![
            item("a", "A", ok.path().to_str().unwrap(), 2),
            item("b", "B", locked.path().to_str().unwrap(), 1),
        ];
        let report = clean_all(&mut items, false);
        std::fs::set_permissions(locked.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(!ok.path().join("f").exists());
        assert_eq!(report.cleaned, 1);
        assert_eq!(report.failed.len(), 1, "{:?}", items[1].status);
        let s = report.render(None, &items);
        assert!(s.contains("FAILED"), "{s}");
        assert!(s.contains("1 failed"), "{s}");
    }
}
