use std::{error::Error, io, time::Duration};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

mod core;
mod tui;

use crate::core::{discovery, registry, scanner};
use crate::tui::{app::{App, AppState}, ui};

fn main() -> Result<(), Box<dyn Error>> {
    // 1. Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 2. Initialize App State
    let mut app = App::new();

    // 3. Scan Phase (Synchronous for MVP)
    // In a real app, this would be async or thread-pooled with UI updates
    let os_type = discovery::detect_os();
    let definitions = registry::load_definitions()?;
    let targets = registry::filter_rules(&definitions, &os_type);
    
    // Simple "Loading" indication could go here if we had a render loop running, 
    // but for MVP we just block on scan.
    let items = scanner::scan_targets(targets);
    app.set_items(items);

    // 4. Run App Loop
    let res = run_app(&mut terminal, app);

    // 5. Cleanup Terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::ui(f, &mut app))?;

        if let AppState::Cleaning { .. } = app.app_state {
            let selected_indices: Vec<usize> = app.items.iter().enumerate()
                .filter(|(_, i)| i.selected && !matches!(i.status, crate::core::ItemStatus::Deleted))
                .map(|(idx, _)| idx)
                .collect();
            
            let total = selected_indices.len();
            let dry_run = app.dry_run;

            for (i, &idx) in selected_indices.iter().enumerate() {
                let item_name = app.items[idx].name.clone();
                app.app_state = AppState::Cleaning { 
                    current: i + 1, 
                    total, 
                    item_name: item_name.clone() 
                };
                
                terminal.draw(|f| ui::ui(f, &mut app))?;

                match crate::core::cleaner::clean_item(&mut app.items[idx], dry_run) {
                    Ok(_) => {},
                    Err(e) => {
                        app.items[idx].status = crate::core::ItemStatus::Failed(e.to_string());
                    }
                }
                
                std::thread::sleep(Duration::from_millis(200));
            }
            
            app.cleanup_finished();
            app.app_state = AppState::Viewing;
            app.active_tab = crate::tui::app::Tab::Dashboard; // Go to dashboard to see updated stats
            continue;
        }

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match app.app_state {
                    AppState::Viewing => {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('d') => app.toggle_dry_run(),
                            KeyCode::Char('1') => app.active_tab = crate::tui::app::Tab::Dashboard,
                            KeyCode::Char('2') => app.active_tab = crate::tui::app::Tab::Results,
                            KeyCode::Char('3') => app.active_tab = crate::tui::app::Tab::Help,
                            KeyCode::Char('j') | KeyCode::Down => app.next(),
                            KeyCode::Char('k') | KeyCode::Up => app.previous(),
                            KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => app.next_tab(),
                            KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => app.previous_tab(),
                            KeyCode::Char(' ') => app.toggle_selection(),
                            KeyCode::Enter => {
                                if app.items.iter().any(|i| i.selected) {
                                    app.app_state = AppState::Confirming;
                                }
                            }
                            _ => {}
                        }
                    }
                    AppState::Confirming => {
                        match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                                let total = app.items.iter().filter(|i| i.selected).count();
                                app.app_state = AppState::Cleaning { current: 0, total, item_name: String::new() };
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.app_state = AppState::Viewing;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
