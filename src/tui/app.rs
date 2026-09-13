use crate::core::registry::Target;
use crate::core::{CleanupItem, ItemStatus};
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};

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
    /// Typing into the `/` filter prompt.
    Filtering,
    Summary(CleanSummary),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortMode {
    #[default]
    Default,
    SizeDesc,
    NameAsc,
}

impl SortMode {
    pub fn next(self) -> SortMode {
        match self {
            SortMode::Default => SortMode::SizeDesc,
            SortMode::SizeDesc => SortMode::NameAsc,
            SortMode::NameAsc => SortMode::Default,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortMode::Default => "default",
            SortMode::SizeDesc => "size",
            SortMode::NameAsc => "name",
        }
    }
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
    /// Case-insensitive substring filter over item name and path.
    pub filter: String,
    pub sort: SortMode,
    /// Categories whose items are hidden in the results list.
    pub collapsed: HashSet<String>,
    /// Group ids from `--select`; matching items are selected as they arrive.
    pub preselect_groups: Vec<String>,
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
            filter: String::new(),
            sort: SortMode::default(),
            collapsed: HashSet::new(),
            preselect_groups: Vec::new(),
        }
    }

    /// Selects every item whose group id was requested on the command line.
    pub fn apply_preselection(&mut self) {
        if self.preselect_groups.is_empty() {
            return;
        }
        for item in &mut self.items {
            if self.preselect_groups.contains(&item.group_id) {
                item.selected = true;
            }
        }
    }

    pub fn set_items(&mut self, items: Vec<CleanupItem>) {
        self.items = items;
        self.apply_preselection();
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
    pub fn push_item(&mut self, mut item: CleanupItem) {
        let keep = self.selected_index();
        if self.preselect_groups.contains(&item.group_id) {
            item.selected = true;
        }
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

    pub fn apply_clean_result(&mut self, idx: usize, status: ItemStatus, freed: u64) {
        if let Some(item) = self.items.get_mut(idx) {
            if status == ItemStatus::Deleted {
                item.size_bytes = freed;
            }
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

    fn item_matches_filter(&self, item: &CleanupItem) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let needle = self.filter.to_lowercase();
        item.name.to_lowercase().contains(&needle)
            || item.path.to_string_lossy().to_lowercase().contains(&needle)
    }

    /// Indices of items that pass the filter, in display order (sorted within
    /// each category, categories alphabetical). Ignores collapsing.
    pub fn visible_item_indices(&self) -> Vec<usize> {
        let mut categories: Vec<&str> = self
            .items
            .iter()
            .filter(|i| self.item_matches_filter(i))
            .map(|i| i.category.as_str())
            .collect();
        categories.sort_unstable();
        categories.dedup();

        let mut out = Vec::new();
        for cat in categories {
            out.extend(self.sorted_indices_in(cat));
        }
        out
    }

    fn sorted_indices_in(&self, category: &str) -> Vec<usize> {
        let mut idxs: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.category == category && self.item_matches_filter(i))
            .map(|(idx, _)| idx)
            .collect();
        match self.sort {
            SortMode::Default => {}
            SortMode::SizeDesc => {
                idxs.sort_by(|a, b| self.items[*b].size_bytes.cmp(&self.items[*a].size_bytes))
            }
            SortMode::NameAsc => idxs.sort_by(|a, b| {
                self.items[*a]
                    .name
                    .to_lowercase()
                    .cmp(&self.items[*b].name.to_lowercase())
            }),
        }
        idxs
    }

    fn calculate_rendered_rows(&mut self) {
        let mut categories: Vec<String> = self
            .items
            .iter()
            .filter(|i| self.item_matches_filter(i))
            .map(|i| i.category.clone())
            .collect();
        categories.sort();
        categories.dedup();

        let mut rows = Vec::new();
        for cat in categories {
            rows.push(ResultRow::CategoryHeader(cat.clone()));
            if !self.collapsed.contains(&cat) {
                rows.extend(
                    self.sorted_indices_in(&cat)
                        .into_iter()
                        .map(ResultRow::Item),
                );
            }
            rows.push(ResultRow::EmptyLine);
        }
        self.rendered_rows = rows;
        self.clamp_selection();
    }

    /// Keeps the highlight on a navigable row after rows are rebuilt.
    fn clamp_selection(&mut self) {
        if self.rendered_rows.is_empty() {
            self.state.select(None);
            return;
        }
        let sel = self.state.selected().unwrap_or(0);
        let sel = sel.min(self.rendered_rows.len() - 1);
        if matches!(self.rendered_rows[sel], ResultRow::EmptyLine) {
            self.state.select(Some(sel));
            self.previous();
        } else {
            self.state.select(Some(sel));
        }
    }

    fn is_navigable(&self, row: usize) -> bool {
        !matches!(self.rendered_rows[row], ResultRow::EmptyLine)
    }

    pub fn next(&mut self) {
        let len = self.rendered_rows.len();
        if len == 0 {
            return;
        }
        let current = self.state.selected().unwrap_or(len - 1);
        let mut next = (current + 1) % len;
        let mut count = 0;
        while !self.is_navigable(next) && count < len {
            next = (next + 1) % len;
            count += 1;
        }
        if self.is_navigable(next) {
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
        let mut count = 0;
        while !self.is_navigable(prev) && count < len {
            prev = if prev == 0 { len - 1 } else { prev - 1 };
            count += 1;
        }
        if self.is_navigable(prev) {
            self.state.select(Some(prev));
        }
    }

    /// Space: on an item toggles it; on a header toggles every visible item
    /// in that category (all on unless every one is already selected).
    pub fn toggle_selection(&mut self) {
        match self.highlighted_row().cloned() {
            Some(ResultRow::Item(idx)) => self.items[idx].selected = !self.items[idx].selected,
            Some(ResultRow::CategoryHeader(cat)) => {
                let idxs = self.sorted_indices_in(&cat);
                let all_on = idxs.iter().all(|i| self.items[*i].selected);
                for i in idxs {
                    self.items[i].selected = !all_on;
                }
            }
            _ => {}
        }
    }

    pub fn select_all(&mut self, on: bool) {
        for idx in self.visible_item_indices() {
            self.items[idx].selected = on;
        }
    }

    /// Number of filter-matching items per category.
    pub fn category_counts(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for item in self.items.iter().filter(|i| self.item_matches_filter(i)) {
            *counts.entry(item.category.clone()).or_insert(0) += 1;
        }
        counts
    }

    pub fn highlighted_row(&self) -> Option<&ResultRow> {
        self.rendered_rows.get(self.state.selected()?)
    }

    /// Category of the highlighted row (header or item).
    pub fn highlighted_category(&self) -> Option<String> {
        match self.highlighted_row()? {
            ResultRow::CategoryHeader(c) => Some(c.clone()),
            ResultRow::Item(idx) => Some(self.items[*idx].category.clone()),
            ResultRow::EmptyLine => None,
        }
    }

    pub fn toggle_collapse(&mut self) {
        let Some(cat) = self.highlighted_category() else {
            return;
        };
        if !self.collapsed.remove(&cat) {
            self.collapsed.insert(cat.clone());
        }
        self.calculate_rendered_rows();
        let header = self
            .rendered_rows
            .iter()
            .position(|r| matches!(r, ResultRow::CategoryHeader(c) if *c == cat));
        if let Some(h) = header {
            self.state.select(Some(h));
        }
    }

    pub fn toggle_collapse_all(&mut self) {
        let mut categories: Vec<String> = self.items.iter().map(|i| i.category.clone()).collect();
        categories.sort();
        categories.dedup();
        if self.collapsed.is_empty() {
            self.collapsed = categories.into_iter().collect();
        } else {
            self.collapsed.clear();
        }
        self.calculate_rendered_rows();
    }

    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.calculate_rendered_rows();
        if self.state.selected().is_none() && !self.rendered_rows.is_empty() {
            self.state.select(Some(0));
            self.next();
        }
    }

    pub fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        self.calculate_rendered_rows();
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

    #[test]
    fn apply_clean_result_records_freed_bytes_for_deleted_items() {
        let mut app = App::new();
        app.set_items(vec![item("a", "A", 100), item("b", "B", 50)]);
        app.apply_clean_result(0, ItemStatus::Deleted, 30);
        app.apply_clean_result(1, ItemStatus::Failed("x".into()), 0);
        assert_eq!(app.items[0].size_bytes, 30);
        assert_eq!(app.items[1].size_bytes, 50, "failed items keep their size");
    }

    fn item(name: &str, cat: &str, size: u64) -> CleanupItem {
        CleanupItem {
            group_id: String::new(),
            name: name.into(),
            category: cat.into(),
            description: None,
            path: PathBuf::from("/x"),
            size_bytes: size,
            file_count: 1,
            selected: false,
            status: ItemStatus::Scanned,
            mode: CleanMode::Contents,
            keep_days: None,
            locked: false,
            command: None,
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
    fn navigation_visits_headers_and_items_skipping_blank_lines() {
        let mut app = App::new();
        app.set_items(vec![item("a", "A", 1), item("b", "B", 1)]);
        // rows: [H(A), I0, Empty, H(B), I1, Empty]; initial highlight = first item
        assert_eq!(app.state.selected(), Some(1));
        app.next();
        assert_eq!(app.state.selected(), Some(3), "lands on header B");
        app.next();
        assert_eq!(app.state.selected(), Some(4));
        app.next();
        assert_eq!(app.state.selected(), Some(0), "wraps to header A");
        app.previous();
        assert_eq!(app.state.selected(), Some(4));
    }

    #[test]
    fn space_on_header_toggles_whole_category() {
        let mut app = App::new();
        app.set_items(vec![
            item("a", "A", 1),
            item("b", "A", 1),
            item("c", "B", 1),
        ]);
        app.state.select(Some(0)); // header A
        app.toggle_selection();
        assert_eq!(app.selected_count(), 2);
        app.toggle_selection();
        assert_eq!(app.selected_count(), 0);
    }

    #[test]
    fn preselect_marks_only_matching_groups() {
        let mut app = App::new();
        let mut a = item("a", "A", 1);
        a.group_id = "dev_pip".into();
        let mut b = item("b", "A", 1);
        b.group_id = "user_trash".into();
        app.preselect_groups = vec!["user_trash".into()];
        app.set_items(vec![a.clone(), b.clone()]);
        assert!(!app.items[0].selected);
        assert!(app.items[1].selected);

        let mut app = App::new();
        app.preselect_groups = vec!["user_trash".into()];
        app.begin_scan(2);
        app.push_item(a);
        app.push_item(b);
        assert!(!app.items[0].selected);
        assert!(app.items[1].selected, "streamed items are preselected too");
    }

    #[test]
    fn select_all_and_none() {
        let mut app = App::new();
        app.set_items(vec![item("a", "A", 1), item("b", "B", 1)]);
        app.select_all(true);
        assert_eq!(app.selected_count(), 2);
        app.select_all(false);
        assert_eq!(app.selected_count(), 0);
    }

    #[test]
    fn collapse_hides_items_but_keeps_header() {
        let mut app = App::new();
        app.set_items(vec![item("a", "A", 1), item("b", "A", 1)]);
        app.toggle_collapse();
        assert!(app.collapsed.contains("A"));
        assert!(app
            .rendered_rows
            .iter()
            .all(|r| !matches!(r, ResultRow::Item(_))));
        assert!(matches!(app.rendered_rows[0], ResultRow::CategoryHeader(_)));
        assert_eq!(app.state.selected(), Some(0), "highlight moves to header");
        app.toggle_collapse();
        assert!(app
            .rendered_rows
            .iter()
            .any(|r| matches!(r, ResultRow::Item(_))));
    }

    #[test]
    fn collapse_all_toggles_every_category() {
        let mut app = App::new();
        app.set_items(vec![item("a", "A", 1), item("b", "B", 1)]);
        app.toggle_collapse_all();
        assert_eq!(app.collapsed.len(), 2);
        app.toggle_collapse_all();
        assert!(app.collapsed.is_empty());
    }

    #[test]
    fn filter_matches_name_or_path_case_insensitively() {
        let mut app = App::new();
        let mut p = item("Pip Cache", "Dev", 1);
        p.path = PathBuf::from("/home/u/.cache/pip");
        let mut n = item("NPM Cache", "Dev", 1);
        n.path = PathBuf::from("/home/u/.npm");
        app.set_items(vec![p, n, item("Trash", "System", 1)]);
        app.set_filter("PIP".into());
        assert_eq!(app.visible_item_indices(), vec![0]);
        app.set_filter(".npm".into());
        assert_eq!(app.visible_item_indices(), vec![1]);
        assert!(!app
            .rendered_rows
            .iter()
            .any(|r| matches!(r, ResultRow::CategoryHeader(c) if c == "System")));
        app.set_filter(String::new());
        assert_eq!(app.visible_item_indices().len(), 3);
    }

    #[test]
    fn select_all_respects_filter() {
        let mut app = App::new();
        app.set_items(vec![item("pip", "A", 1), item("npm", "A", 1)]);
        app.set_filter("pip".into());
        app.select_all(true);
        assert_eq!(app.selected_count(), 1);
    }

    #[test]
    fn sort_cycles_and_orders_within_category() {
        let mut app = App::new();
        app.set_items(vec![
            item("b", "A", 5),
            item("a", "A", 50),
            item("c", "A", 1),
        ]);
        app.cycle_sort();
        assert_eq!(app.sort, SortMode::SizeDesc);
        assert_eq!(app.visible_item_indices(), vec![1, 0, 2]);
        app.cycle_sort();
        assert_eq!(app.sort, SortMode::NameAsc);
        assert_eq!(app.visible_item_indices(), vec![1, 0, 2]);
        app.cycle_sort();
        assert_eq!(app.sort, SortMode::Default);
        assert_eq!(app.visible_item_indices(), vec![0, 1, 2]);
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
        app.apply_clean_result(0, ItemStatus::Deleted, 10);
        app.apply_clean_result(1, ItemStatus::Failed("denied".into()), 0);
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
        app.apply_clean_result(0, ItemStatus::DryRun, 0);
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
