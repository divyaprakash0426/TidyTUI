mod cli;
mod core;
mod headless;
mod tui;

use std::{
    io,
    sync::mpsc::{Receiver, TryRecvError},
    time::Duration,
};

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use ratatui::{backend::Backend, Terminal};

use crate::cli::Cli;
use crate::core::{
    cleaner::{self, CleanEvent},
    discovery, registry,
    scanner::{self, ScanEvent},
    ItemStatus,
};
use crate::tui::theme::Theme;
use crate::tui::{
    app::{App, AppState, Tab},
    events::{handle_key, handle_mouse, Action},
    views,
};
use clap::Parser;

/// Background work the event loop pumps every frame.
#[derive(Default)]
struct Runtime {
    scan_rx: Option<Receiver<ScanEvent>>,
    clean_rx: Option<Receiver<CleanEvent>>,
}

impl Runtime {
    fn busy(&self) -> bool {
        self.scan_rx.is_some() || self.clean_rx.is_some()
    }

    fn start_scan(&mut self, app: &mut App) {
        app.begin_scan(app.targets.len());
        self.scan_rx = Some(scanner::spawn_scan(app.targets.clone()));
    }

    fn start_clean(&mut self, app: &mut App) {
        let jobs: Vec<(usize, core::CleanupItem)> = app
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.selected && !matches!(i.status, ItemStatus::Deleted))
            .map(|(idx, i)| (idx, i.clone()))
            .collect();
        app.app_state = AppState::Cleaning {
            current: 0,
            total: jobs.len(),
            item_name: String::new(),
        };
        self.clean_rx = Some(cleaner::spawn_clean(jobs, app.dry_run));
    }

    /// Drains pending scan events without blocking.
    fn pump_scan(&mut self, app: &mut App) {
        let Some(rx) = &self.scan_rx else { return };
        let mut done = false;
        loop {
            match rx.try_recv() {
                Ok(ScanEvent::Found(item)) => app.push_item(*item),
                Ok(ScanEvent::Missing) => app.note_missing(),
                Ok(ScanEvent::Finished) | Err(TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if done {
            app.finish_scan();
            self.scan_rx = None;
        }
    }

    /// Drains pending clean events without blocking.
    fn pump_clean(&mut self, app: &mut App) {
        let Some(rx) = &self.clean_rx else { return };
        let mut done = false;
        loop {
            match rx.try_recv() {
                // Jobs run sequentially, so each Started is the next ordinal;
                // `idx` is the item's position in the list, not a job number.
                Ok(CleanEvent::Started { name, .. }) => {
                    if let AppState::Cleaning {
                        current, item_name, ..
                    } = &mut app.app_state
                    {
                        *current += 1;
                        *item_name = name;
                    }
                }
                Ok(CleanEvent::Finished { idx, status, freed }) => {
                    app.apply_clean_result(idx, status, freed)
                }
                Ok(CleanEvent::Done) | Err(TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if done {
            let summary = app.build_summary();
            app.cleanup_finished();
            app.app_state = AppState::Summary(summary);
            app.active_tab = Tab::Results;
            self.clean_rx = None;
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load config before touching the terminal so that errors print normally
    // instead of inside raw mode.
    let os_type = discovery::detect_os();
    let definitions = registry::load_definitions_from(cli.config.as_deref())?;
    let targets = registry::filter_rules(&definitions, &os_type);

    if cli.list || cli.yes {
        return run_headless(&cli, targets);
    }

    let mut app = App::new();
    app.dry_run = cli.dry_run();
    app.preselect_groups = cli.select.clone();
    app.theme = Theme::by_name(&cli.theme).unwrap_or_default();
    app.targets = targets;

    let mut runtime = Runtime::default();
    runtime.start_scan(&mut app);

    // ratatui::init installs a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    let mouse = !cli.no_mouse;
    if mouse {
        let _ = execute!(io::stdout(), EnableMouseCapture);
        // ratatui's hook restores raw mode and the screen but not mouse capture.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = execute!(io::stdout(), DisableMouseCapture);
            previous(info);
        }));
    }
    let result = run_app(&mut terminal, &mut app, &mut runtime);
    if mouse {
        let _ = execute!(io::stdout(), DisableMouseCapture);
    }
    ratatui::restore();
    Ok(result?)
}

/// `--list [--json]` prints the scan; `--yes` cleans the selected groups.
fn run_headless(cli: &Cli, targets: Vec<registry::Target>) -> anyhow::Result<()> {
    let home = dirs::home_dir();
    let mut items = scanner::scan_targets(targets);
    items.sort_by_key(|i| std::cmp::Reverse(i.size_bytes));

    if !cli.select.is_empty() {
        let (kept, unknown) = headless::select_groups(items, &cli.select);
        for id in &unknown {
            eprintln!("warning: no scanned item belongs to group '{id}'");
        }
        items = kept;
    }

    if cli.yes {
        let dry_run = cli.dry_run();
        if items.is_empty() {
            anyhow::bail!("nothing to clean for the selected groups");
        }
        let report = headless::clean_all(&mut items, dry_run);
        print!("{}", report.render(home.as_deref(), &items));
        if !report.failed.is_empty() {
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.json {
        println!("{}", headless::list_json(&items));
    } else {
        print!("{}", headless::list_table(&items, home.as_deref()));
    }
    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    runtime: &mut Runtime,
) -> io::Result<()> {
    loop {
        runtime.pump_scan(app);
        runtime.pump_clean(app);
        terminal.draw(|f| {
            app.viewport = f.area();
            views::render(f, app)
        })?;

        let tick = if runtime.busy() { 50 } else { 250 };
        if event::poll(Duration::from_millis(tick))? {
            let action = match event::read()? {
                Event::Key(key) => handle_key(app, key),
                Event::Mouse(mouse) => handle_mouse(app, mouse),
                _ => Action::None,
            };
            match action {
                Action::Quit => return Ok(()),
                Action::StartCleaning => runtime.start_clean(app),
                Action::Rescan => runtime.start_scan(app),
                Action::None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn pump_clean_counts_jobs_not_item_indices() {
        let mut app = App::new();
        app.app_state = AppState::Cleaning {
            current: 0,
            total: 1,
            item_name: String::new(),
        };
        let (tx, rx) = mpsc::channel();
        let mut runtime = Runtime {
            scan_rx: None,
            clean_rx: Some(rx),
        };
        // Only the third item was selected: one job, whose item index is 2.
        tx.send(CleanEvent::Started { name: "c".into() }).unwrap();
        runtime.pump_clean(&mut app);
        match &app.app_state {
            AppState::Cleaning { current, total, .. } => {
                assert_eq!((*current, *total), (1, 1));
            }
            other => panic!("unexpected state {other:?}"),
        }
    }
}
