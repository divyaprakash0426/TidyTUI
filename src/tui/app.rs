use crate::core::{CleanupItem, ItemStatus};
use ratatui::widgets::ListState;

pub struct App {
    pub items: Vec<CleanupItem>,
    pub state: ListState,
    pub is_scanning: bool,
    pub total_size: u64,
    pub dry_run: bool,
}

impl App {
    pub fn new() -> App {
        App {
            items: Vec::new(),
            state: ListState::default(),
            is_scanning: false,
            total_size: 0,
            dry_run: true, // Safety default
        }
    }

    pub fn set_items(&mut self, items: Vec<CleanupItem>) {
        self.items = items;
        self.total_size = self.items.iter().map(|i| i.size_bytes).sum();
        self.state.select(Some(0));
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn toggle_selection(&mut self) {
        if let Some(i) = self.state.selected() {
            if i < self.items.len() {
                self.items[i].selected = !self.items[i].selected;
            }
        }
    }
    
    pub fn toggle_dry_run(&mut self) {
        self.dry_run = !self.dry_run;
    }

    pub fn delete_selected(&mut self) {
        // Mark for deletion - actual deletion happens in main loop or separate async task
        // For now, just change status
        for item in &mut self.items {
            if item.selected {
                // In a real app, this would trigger the Cleaner module
                // For MVP, we might handle this in main.rs event loop or separate controller
                // item.status = ItemStatus::Deleting; // logical change only
            }
        }
    }
}
