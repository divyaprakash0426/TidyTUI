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
use crate::tui::{app::App, ui};

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
    let definitions = registry::load_definitions("definitions.yaml")?;
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

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('d') => app.toggle_dry_run(),
                    KeyCode::Char('j') | KeyCode::Down => app.next(),
                    KeyCode::Char('k') | KeyCode::Up => app.previous(),
                    KeyCode::Char(' ') => app.toggle_selection(),
                    KeyCode::Enter => {
                        // Perform cleaning
                        // We need to capture the current mode before mutable borrow of items
                        let current_dry_run = app.dry_run; 
                        
                        for item in &mut app.items {
                            if item.selected {
                                match crate::core::cleaner::clean_item(item, current_dry_run) {
                                    Ok(_) => {},
                                    Err(e) => {
                                        item.status = crate::core::ItemStatus::Failed(e.to_string());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
