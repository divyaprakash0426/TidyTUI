use crate::core::CleanupItem;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    Viewing,
    Confirming,
    Cleaning { current: usize, total: usize, item_name: String },
}

pub struct App {
    pub items: Vec<CleanupItem>,
    pub rendered_rows: Vec<ResultRow>,
    pub state: ListState,
    pub total_size: u64,
    pub dry_run: bool,
    pub active_tab: Tab,
    pub app_state: AppState,
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
        if len == 0 { return; }

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
        if len == 0 { return; }

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
        if let Some(i) = self.state.selected() {
            if i < self.rendered_rows.len() {
                if let ResultRow::Item(item_idx) = self.rendered_rows[i] {
                    self.items[item_idx].selected = !self.items[item_idx].selected;
                }
            }
        }
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
        use crate::core::ItemStatus;
        // Keep only items that were not successfully deleted
        self.items.retain(|i| !matches!(i.status, ItemStatus::Deleted));
        self.total_size = self.items.iter().map(|i| i.size_bytes).sum();
        self.calculate_rendered_rows();
        self.state.select(Some(0));
        if !self.rendered_rows.is_empty() {
            self.next(); // Find first selectable item
        }
    }
}
