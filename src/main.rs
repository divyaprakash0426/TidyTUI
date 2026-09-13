mod core;
mod tui;

use std::{io, time::Duration};

use crossterm::event::{self, Event};
use ratatui::{backend::Backend, Terminal};

use crate::core::{cleaner, discovery, registry, scanner, ItemStatus};
use crate::tui::{
    app::{App, AppState, Tab},
    events::{handle_key, Action},
    views,
};

fn main() -> anyhow::Result<()> {
    // Load config and scan before touching the terminal so that errors print
    // normally instead of inside raw mode.
    let os_type = discovery::detect_os();
    let definitions = registry::load_definitions()?;
    let targets = registry::filter_rules(&definitions, &os_type);
    let items = scanner::scan_targets(targets);

    let mut app = App::new();
    app.set_items(items);

    // ratatui::init installs a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal, &mut app);
    ratatui::restore();
    Ok(result?)
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| views::render(f, app))?;

        if matches!(app.app_state, AppState::Cleaning { .. }) {
            run_cleaning(terminal, app)?;
            continue;
        }

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match handle_key(app, key) {
                    Action::Quit => return Ok(()),
                    Action::StartCleaning => {
                        app.app_state = AppState::Cleaning {
                            current: 0,
                            total: app.selected_count(),
                            item_name: String::new(),
                        };
                    }
                    Action::None => {}
                }
            }
        }
    }
}

fn run_cleaning<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    let indices: Vec<usize> = app
        .items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.selected && !matches!(i.status, ItemStatus::Deleted))
        .map(|(idx, _)| idx)
        .collect();
    let total = indices.len();
    let dry_run = app.dry_run;

    for (n, &idx) in indices.iter().enumerate() {
        app.app_state = AppState::Cleaning {
            current: n + 1,
            total,
            item_name: app.items[idx].name.clone(),
        };
        terminal.draw(|f| views::render(f, app))?;

        // clean_item records a Failed status on the item itself; the returned
        // error carries the same message, so it is intentionally not re-raised.
        let _ = cleaner::clean_item(&mut app.items[idx], dry_run);
        std::thread::sleep(Duration::from_millis(120));
    }

    app.cleanup_finished();
    app.app_state = AppState::Viewing;
    app.active_tab = Tab::Results;
    Ok(())
}
