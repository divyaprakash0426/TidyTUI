use crate::core::registry::Target;
use crate::core::{CleanupItem, ItemStatus};
use ratatui::widgets::ListState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Results,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultRow {
    CategoryHeader(String),
    Item(usize), // index into app.items
    EmptyLine,
}

/// Outcome of a cleaning run, shown in the summary modal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanSummary {
    pub dry_run: bool,
    /// Items deleted (or, in dry-run, items that would have been deleted).
    pub deleted: usize,
    pub freed_bytes: u64,
    /// (item name, reason)
    pub failed: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    Viewing,
    Confirming,
    Cleaning {
        current: usize,
        total: usize,
        item_name: String,
    },
    Summary(CleanSummary),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanProgress {
    pub checked: usize,
    pub total: usize,
    pub finished: bool,
}

pub struct App {
    pub items: Vec<CleanupItem>,
    pub rendered_rows: Vec<ResultRow>,
    pub state: ListState,
    pub total_size: u64,
    pub dry_run: bool,
    pub active_tab: Tab,
    pub app_state: AppState,
    /// Resolved targets for the current OS; kept so a rescan can be started.
    pub targets: Vec<Target>,
    pub scan: ScanProgress,
}

impl App {
    pub fn new() -> App {
        App {
            items: Vec::new(),
            rendered_rows: Vec::new(),
            state: ListState::default(),
            total_size: 0,
            dry_run: true, // Safety default
            active_tab: Tab::Dashboard,
            app_state: AppState::Viewing,
            targets: Vec::new(),
            scan: ScanProgress {
                finished: true,
                ..ScanProgress::default()
            },
        }
    }

    pub fn set_items(&mut self, items: Vec<CleanupItem>) {
        self.items = items;
        self.total_size = self.items.iter().map(|i| i.size_bytes).sum();
        self.calculate_rendered_rows();
        self.state.select(Some(0));
        // Ensure first item is selected if possible
        if !self.rendered_rows.is_empty() {
            self.next(); // Find first selectable item
        }
    }

    // --- Scanning -------------------------------------------------------

    pub fn begin_scan(&mut self, total: usize) {
        self.set_items(Vec::new());
        self.state.select(None);
        self.scan = ScanProgress {
            checked: 0,
            total,
            finished: false,
        };
    }

    /// Adds an item from an in-progress scan, keeping the highlight on the
    /// same item even though category sorting may shift the rows. Appending
    /// never moves existing indices, so the item index is a stable identity.
    pub fn push_item(&mut self, item: CleanupItem) {
        let keep = self.selected_index();
        self.total_size += item.size_bytes;
        self.items.push(item);
        self.scan.checked += 1;
        self.calculate_rendered_rows();

        let row = keep.and_then(|idx| {
            self.rendered_rows
                .iter()
                .position(|r| matches!(r, ResultRow::Item(i) if *i == idx))
        });
        match row {
            Some(r) => self.state.select(Some(r)),
            None => {
                self.state.select(Some(0));
                self.next();
            }
        }
    }

    pub fn note_missing(&mut self) {
        self.scan.checked += 1;
    }

    pub fn finish_scan(&mut self) {
        self.scan.finished = true;
    }

    pub fn is_scanning(&self) -> bool {
        !self.scan.finished
    }

    // --- Cleaning -------------------------------------------------------

    pub fn apply_clean_result(&mut self, idx: usize, status: ItemStatus) {
        if let Some(item) = self.items.get_mut(idx) {
            item.status = status;
        }
    }

    /// Must be called before `cleanup_finished`, which drops deleted items.
    pub fn build_summary(&self) -> CleanSummary {
        let mut summary = CleanSummary {
            dry_run: self.dry_run,
            ..CleanSummary::default()
        };
        for item in self.items.iter().filter(|i| i.selected) {
            match &item.status {
                ItemStatus::Deleted | ItemStatus::DryRun => {
                    summary.deleted += 1;
                    summary.freed_bytes += item.size_bytes;
                }
                ItemStatus::Failed(reason) => {
                    summary.failed.push((item.name.clone(), reason.clone()));
                }
                ItemStatus::Scanned => {}
            }
        }
        summary
    }

    fn calculate_rendered_rows(&mut self) {
        let mut rows = Vec::new();
        let mut categories: Vec<String> = self.items.iter().map(|i| i.category.clone()).collect();
        categories.sort();
        categories.dedup();

        for cat in categories {
            rows.push(ResultRow::CategoryHeader(cat.clone()));
            for (idx, item) in self.items.iter().enumerate() {
                if item.category == cat {
                    rows.push(ResultRow::Item(idx));
                }
            }
            rows.push(ResultRow::EmptyLine);
        }
        self.rendered_rows = rows;
    }

    pub fn next(&mut self) {
        let len = self.rendered_rows.len();
        if len == 0 {
            return;
        }

        let current = self.state.selected().unwrap_or(len - 1);
        let mut next = (current + 1) % len;

        // Skip non-item rows
        let mut count = 0;
        while !matches!(self.rendered_rows[next], ResultRow::Item(_)) && count < len {
            next = (next + 1) % len;
            count += 1;
        }

        if matches!(self.rendered_rows[next], ResultRow::Item(_)) {
            self.state.select(Some(next));
        }
    }

    pub fn previous(&mut self) {
        let len = self.rendered_rows.len();
        if len == 0 {
            return;
        }

        let current = self.state.selected().unwrap_or(0);
        let mut prev = if current == 0 { len - 1 } else { current - 1 };

        // Skip non-item rows
        let mut count = 0;
        while !matches!(self.rendered_rows[prev], ResultRow::Item(_)) && count < len {
            prev = if prev == 0 { len - 1 } else { prev - 1 };
            count += 1;
        }

        if matches!(self.rendered_rows[prev], ResultRow::Item(_)) {
            self.state.select(Some(prev));
        }
    }

    pub fn toggle_selection(&mut self) {
        if let Some(idx) = self.selected_index() {
            self.items[idx].selected = !self.items[idx].selected;
        }
    }

    /// Index into `items` for the highlighted row, if it is an item row.
    pub fn selected_index(&self) -> Option<usize> {
        match self.rendered_rows.get(self.state.selected()?) {
            Some(ResultRow::Item(idx)) => Some(*idx),
            _ => None,
        }
    }

    pub fn selected_item(&self) -> Option<&CleanupItem> {
        self.selected_index().map(|idx| &self.items[idx])
    }

    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|i| i.selected).count()
    }

    pub fn selected_size(&self) -> u64 {
        self.items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.size_bytes)
            .sum()
    }

    pub fn toggle_dry_run(&mut self) {
        self.dry_run = !self.dry_run;
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Dashboard => Tab::Results,
            Tab::Results => Tab::Help,
            Tab::Help => Tab::Dashboard,
        };
    }

    pub fn previous_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Dashboard => Tab::Help,
            Tab::Results => Tab::Dashboard,
            Tab::Help => Tab::Results,
        };
    }

    pub fn cleanup_finished(&mut self) {
        // Deleted items vanish; DryRun/Failed stay so the user sees the outcome.
        self.items
            .retain(|i| !matches!(i.status, ItemStatus::Deleted));
        for item in &mut self.items {
            item.selected = false;
        }
        self.total_size = self.items.iter().map(|i| i.size_bytes).sum();
        self.calculate_rendered_rows();
        self.state.select(Some(0));
        if !self.rendered_rows.is_empty() {
            self.next(); // Find first selectable item
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CleanMode, ItemStatus};
    use std::path::PathBuf;

    fn item(name: &str, cat: &str, size: u64) -> CleanupItem {
        CleanupItem {
            name: name.into(),
            category: cat.into(),
            description: None,
            path: PathBuf::from("/x"),
            size_bytes: size,
            file_count: 1,
            selected: false,
            status: ItemStatus::Scanned,
            mode: CleanMode::Contents,
        }
    }

    #[test]
    fn set_items_groups_by_category_and_selects_first_item() {
        let mut app = App::new();
        app.set_items(vec![item("b", "B", 1), item("a", "A", 2)]);
        assert!(matches!(app.rendered_rows[0], ResultRow::CategoryHeader(ref c) if c == "A"));
        assert!(matches!(app.rendered_rows[1], ResultRow::Item(1)));
        assert_eq!(app.state.selected(), Some(1));
        assert_eq!(app.total_size, 3);
    }

    #[test]
    fn next_and_previous_skip_headers_and_wrap() {
        let mut app = App::new();
        app.set_items(vec![item("a", "A", 1), item("b", "B", 1)]);
        // rows: [H(A), I0, Empty, H(B), I1, Empty]
        assert_eq!(app.state.selected(), Some(1));
        app.next();
        assert_eq!(app.state.selected(), Some(4));
        app.next();
        assert_eq!(app.state.selected(), Some(1));
        app.previous();
        assert_eq!(app.state.selected(), Some(4));
    }

    #[test]
    fn toggle_selection_and_totals() {
        let mut app = App::new();
        app.set_items(vec![item("a", "A", 10), item("b", "A", 5)]);
        app.toggle_selection();
        assert_eq!(app.selected_count(), 1);
        assert_eq!(app.selected_size(), 10);
        assert_eq!(app.selected_item().unwrap().name, "a");
    }

    #[test]
    fn empty_app_has_no_selected_item() {
        let app = App::new();
        assert!(app.selected_item().is_none());
        assert_eq!(app.selected_count(), 0);
    }

    #[test]
    fn push_item_keeps_highlight_on_same_item() {
        let mut app = App::new();
        app.begin_scan(3);
        app.push_item(item("m", "M", 1));
        assert_eq!(app.selected_item().unwrap().name, "m");
        app.push_item(item("a", "A", 1)); // sorts before M → rows shift
        assert_eq!(app.selected_item().unwrap().name, "m");
        assert_eq!(app.scan.checked, 2);
        assert!(app.is_scanning());
        app.note_missing();
        app.finish_scan();
        assert!(!app.is_scanning());
        assert_eq!(app.scan.checked, 3);
        assert_eq!(app.total_size, 2);
    }

    #[test]
    fn begin_scan_clears_previous_items() {
        let mut app = App::new();
        app.set_items(vec![item("old", "A", 5)]);
        app.begin_scan(1);
        assert!(app.items.is_empty());
        assert_eq!(app.total_size, 0);
        assert!(app.selected_item().is_none());
    }

    #[test]
    fn build_summary_counts_deleted_failed_and_bytes() {
        let mut app = App::new();
        let mut a = item("a", "A", 10);
        a.selected = true;
        let mut b = item("b", "A", 5);
        b.selected = true;
        let c = item("c", "A", 99);
        app.set_items(vec![a, b, c]);
        app.apply_clean_result(0, ItemStatus::Deleted);
        app.apply_clean_result(1, ItemStatus::Failed("denied".into()));
        let s = app.build_summary();
        assert_eq!(s.deleted, 1);
        assert_eq!(s.freed_bytes, 10);
        assert_eq!(s.failed, vec![("b".to_string(), "denied".to_string())]);
        assert!(s.dry_run);
    }

    #[test]
    fn build_summary_dry_run_counts_would_delete() {
        let mut app = App::new();
        let mut a = item("a", "A", 10);
        a.selected = true;
        app.set_items(vec![a]);
        app.apply_clean_result(0, ItemStatus::DryRun);
        let s = app.build_summary();
        assert_eq!(s.deleted, 1);
        assert_eq!(s.freed_bytes, 10);
    }

    #[test]
    fn cleanup_finished_drops_deleted_keeps_dryrun_and_failed() {
        let mut app = App::new();
        let mut a = item("a", "A", 1);
        a.status = ItemStatus::Deleted;
        a.selected = true;
        let mut b = item("b", "A", 2);
        b.status = ItemStatus::DryRun;
        b.selected = true;
        let mut c = item("c", "A", 3);
        c.status = ItemStatus::Failed("denied".into());
        c.selected = true;
        app.set_items(vec![a, b, c]);
        app.cleanup_finished();
        let names: Vec<&str> = app.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["b", "c"]);
        assert!(app.items.iter().all(|i| !i.selected));
        assert_eq!(app.total_size, 5);
    }
}
