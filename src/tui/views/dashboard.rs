use crate::tui::app::App;
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Padding, Paragraph},
    Frame,
};
use std::collections::HashMap;

const PALETTE: [Color; 6] = [
    Color::Cyan,
    Color::Magenta,
    Color::Yellow,
    Color::Green,
    Color::Blue,
    Color::Red,
];

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .margin(1)
        .split(area);

    render_overview(f, app, chunks[0]);

    let sub_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    render_gauge(f, app, sub_chunks[0]);
    render_distribution(f, app, sub_chunks[1]);
}

fn render_overview(f: &mut Frame, app: &App, area: Rect) {
    let total_files: u64 = app.items.iter().map(|i| i.file_count).sum();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" System Overview ")
        .padding(Padding::uniform(1));

    let stats_text = vec![
        Line::from(vec![
            Span::raw("Discovered: "),
            Span::styled(
                format!("{} locations", app.items.len()),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" ({total_files} files)")),
        ]),
        Line::from(vec![
            Span::raw("Total Size: "),
            Span::styled(
                ByteSize(app.total_size).to_string(),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Selected for cleaning: "),
            Span::styled(
                format!("{} items", app.selected_count()),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" / {}", ByteSize(app.selected_size()))),
        ]),
    ];

    let thresholds_text = vec![
        Line::from(Span::styled(
            "Status Thresholds:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Clean    ", Style::default().fg(Color::Green)),
            Span::raw("< 100 MB"),
        ]),
        Line::from(vec![
            Span::styled("  Moderate ", Style::default().fg(Color::Yellow)),
            Span::raw("100 - 500 MB"),
        ]),
        Line::from(vec![
            Span::styled("  Critical ", Style::default().fg(Color::Red)),
            Span::raw("> 500 MB"),
        ]),
    ];

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

    f.render_widget(block, area);
    f.render_widget(Paragraph::new(stats_text), header_chunks[0]);
    f.render_widget(Paragraph::new(thresholds_text), header_chunks[1]);
}

fn render_distribution(f: &mut Frame, app: &App, area: Rect) {
    let mut distribution: HashMap<&str, u64> = HashMap::new();
    for item in &app.items {
        *distribution.entry(item.category.as_str()).or_insert(0) += item.size_bytes;
    }

    let mut data: Vec<(&str, u64)> = distribution.into_iter().collect();
    data.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let legend_items: Vec<ListItem> = data
        .iter()
        .enumerate()
        .map(|(idx, (cat, size))| {
            let percentage = if app.total_size > 0 {
                (*size as f64 / app.total_size as f64) * 100.0
            } else {
                0.0
            };
            let perc_str = if percentage > 0.0 && percentage < 0.1 {
                "< 0.1%".to_string()
            } else {
                format!("{percentage:>5.1}%")
            };
            let color = PALETTE[idx % PALETTE.len()];
            ListItem::new(Line::from(vec![
                Span::styled(" ● ", Style::default().fg(color)),
                Span::styled(
                    format!("{cat:<18}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {perc_str} ")),
                Span::styled(
                    format!("({})", ByteSize(*size)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let legend = List::new(legend_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Junk Distribution "),
    );
    f.render_widget(
        legend,
        area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }),
    );
}

fn render_gauge(f: &mut Frame, app: &App, area: Rect) {
    // 100 MB is 'Clean', 500 MB is 'Moderate', 1 GB+ pins the gauge at 100%.
    let junk_score = (app.total_size as f64 / 1_000_000_000.0).min(1.0);
    let percentage = (junk_score * 100.0) as u16;

    let (color, status) = if junk_score < 0.1 {
        (Color::Green, "Clean")
    } else if junk_score < 0.5 {
        (Color::Yellow, "Moderate")
    } else {
        (Color::Red, "Critical")
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" System Status: {status} ")),
        )
        .gauge_style(Style::default().fg(color).bg(Color::Black))
        .percent(percentage)
        .label(format!("{percentage}% Cluttered"));

    f.render_widget(
        gauge,
        area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }),
    );
}
