use crate::core::paths::shorten_home;
use crate::tui::app::App;
use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Padding, Paragraph},
    Frame,
};

const MAX_PREVIEW_PATHS: usize = 5;

pub fn render_confirm(f: &mut Frame, app: &App) {
    let selected_items = app.selected_count();
    let selected_size = ByteSize(app.selected_size());
    let home = dirs::home_dir();

    let area = centered_rect(70, 50, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" CONFIRM CLEANUP ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::uniform(1));

    let mut text = vec![
        Line::from(vec![
            Span::raw("Clean "),
            Span::styled(
                selected_items.to_string(),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            ),
            Span::raw(" items, freeing "),
            Span::styled(
                selected_size.to_string(),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Magenta),
            ),
            Span::raw("?"),
        ]),
        Line::from(""),
    ];

    for item in app
        .items
        .iter()
        .filter(|i| i.selected)
        .take(MAX_PREVIEW_PATHS)
    {
        text.push(Line::from(Span::styled(
            format!("  {}", shorten_home(&item.path, home.as_deref())),
            Style::default().fg(Color::DarkGray),
        )));
    }
    if selected_items > MAX_PREVIEW_PATHS {
        text.push(Line::from(Span::styled(
            format!("  … and {} more", selected_items - MAX_PREVIEW_PATHS),
            Style::default().fg(Color::DarkGray),
        )));
    }

    text.push(Line::from(""));
    text.push(if app.dry_run {
        Line::from(Span::styled(
            "MODE: DRY-RUN (No files will be deleted)",
            Style::default().fg(Color::Green),
        ))
    } else {
        Line::from(Span::styled(
            "WARNING: DANGER MODE (FILES WILL BE DELETED)",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ))
    });
    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::raw("Press "),
        Span::styled(
            "[y]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to proceed, "),
        Span::styled(
            "[n]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to cancel."),
    ]));

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

pub fn render_progress(f: &mut Frame, current: usize, total: usize, item_name: &str, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let percentage = if total > 0 {
        ((current as f64 / total as f64) * 100.0) as u16
    } else {
        100
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Cleaning Progress "),
        )
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .percent(percentage)
        .label(format!("{current}/{total}"));

    let info = Paragraph::new(format!("Cleaning: {item_name}"))
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Yellow));

    f.render_widget(info, chunks[0]);
    f.render_widget(gauge, chunks[1]);
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
