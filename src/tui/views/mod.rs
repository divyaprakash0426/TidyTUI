use crate::tui::app::{App, AppState, Tab};
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

pub mod dashboard;
pub mod help;
pub mod modals;
pub mod results;

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    render_tabs(f, app, chunks[0]);

    match &app.app_state {
        AppState::Viewing => render_active_tab(f, app, chunks[1]),
        AppState::Confirming => {
            render_active_tab(f, app, chunks[1]);
            modals::render_confirm(f, app);
        }
        AppState::Cleaning {
            current,
            total,
            item_name,
        } => {
            let (current, total, name) = (*current, *total, item_name.clone());
            modals::render_progress(f, current, total, &name, chunks[1]);
        }
    }

    render_footer(f, app, chunks[2]);
}

fn render_active_tab(f: &mut Frame, app: &mut App, area: Rect) {
    match app.active_tab {
        Tab::Dashboard => dashboard::render(f, app, area),
        Tab::Results => results::render(f, app, area),
        Tab::Help => help::render(f, area),
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec![" [1] Dashboard ", " [2] Results ", " [3] Help "];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" TidyTUI "))
        .select(match app.active_tab {
            Tab::Dashboard => 0,
            Tab::Results => 1,
            Tab::Help => 2,
        })
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let mode_text = if app.dry_run {
        Span::styled(
            "DRY-RUN (Safe)",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            "DANGER (DELETING)",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK),
        )
    };

    let footer_text = Line::from(vec![
        Span::raw(format!("Total Found: {} | ", ByteSize(app.total_size))),
        Span::raw("Tab: <Tab>, Nav: <Up/Down>, Toggle: <Space>, Mode: <d>, Clean: <Enter> | "),
        mode_text,
    ]);

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}
