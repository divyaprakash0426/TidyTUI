use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use bytesize::ByteSize;
use crate::tui::app::App;
use crate::core::ItemStatus;

pub fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(" TidyTUI ");
    
    let title = Paragraph::new("System Cleaner - Scan & Clean")
        .style(Style::default().fg(Color::Cyan))
        .block(title_block);
    
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|i| {
            let checkbox = if i.selected { "[x] " } else { "[ ] " };
            let size = ByteSize(i.size_bytes).to_string();
            let status_style = match i.status {
                ItemStatus::Deleted => Style::default().fg(Color::Green),
                ItemStatus::Failed(_) => Style::default().fg(Color::Red),
                _ => Style::default(),
            };
            
            let content = Line::from(vec![
                Span::raw(checkbox),
                Span::styled(format!("{:<20}", i.name), Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("({}) - {}", i.path.display(), size)),
            ]);

            ListItem::new(content).style(status_style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Cleanable Items "))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, chunks[1], &mut app.state);

    let total_size = ByteSize(app.total_size);
    let mode_text = if app.dry_run { 
        Span::styled("DRY-RUN", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)) 
    } else { 
        Span::styled("DANGER (DELETING)", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK)) 
    };

    let footer_text = Line::from(vec![
        Span::raw(format!("Total Found: {} | ", total_size)),
        Span::raw("<Space> Toggle, <d> Toggle Mode, <Enter> Clean "),
        Span::raw("("),
        mode_text,
        Span::raw("), <q> Quit"),
    ]);

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL));
    
    f.render_widget(footer, chunks[2]);
}
