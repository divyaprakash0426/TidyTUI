use crate::core::paths::shorten_home;
use crate::tui::app::{App, CleanSummary};
use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Padding, Paragraph, Wrap},
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
    let locked = app.items.iter().filter(|i| i.selected && i.locked).count();
    if locked > 0 && !app.dry_run {
        let noun = if locked == 1 {
            "item needs"
        } else {
            "items need"
        };
        text.push(Line::from(Span::styled(
            format!("⚠ {locked} selected {noun} root and will fail"),
            Style::default().fg(Color::Yellow),
        )));
    }
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

const MAX_FAILURES_SHOWN: usize = 6;

pub fn render_summary(f: &mut Frame, summary: &CleanSummary) {
    let (title, verb, color) = if summary.dry_run {
        (" Dry-Run Complete ", "Would delete", Color::Cyan)
    } else {
        (" Cleanup Complete ", "Deleted", Color::Green)
    };

    let mut text = vec![
        Line::from(vec![
            Span::raw(format!("{verb} ")),
            Span::styled(
                format!("{} items", summary.deleted),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(if summary.dry_run {
                ", freeing "
            } else {
                ", freed "
            }),
            Span::styled(
                ByteSize(summary.freed_bytes).to_string(),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
    ];

    if summary.failed.is_empty() {
        text.push(Line::from(Span::styled(
            "No failures",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        text.push(Line::from(Span::styled(
            format!("{} failed:", summary.failed.len()),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        for (name, reason) in summary.failed.iter().take(MAX_FAILURES_SHOWN) {
            text.push(Line::from(vec![
                Span::styled(format!("  {name}: "), Style::default().fg(Color::Red)),
                Span::styled(reason.clone(), Style::default().fg(Color::DarkGray)),
            ]));
        }
        if summary.failed.len() > MAX_FAILURES_SHOWN {
            text.push(Line::from(Span::styled(
                format!("  … and {} more", summary.failed.len() - MAX_FAILURES_SHOWN),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::raw("Press "),
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to continue"),
    ]));

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .padding(Padding::uniform(1));
    // borders + padding
    let height = text.len() as u16 + 4;
    let area = centered_fixed_height(60, height, f.area());
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// A horizontally centred rect of `percent_x` width and exactly `height` rows
/// (clamped to the available area), vertically centred.
fn centered_fixed_height(percent_x: u16, height: u16, r: Rect) -> Rect {
    let height = height.min(r.height);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(height),
            Constraint::Fill(1),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(rows[1])[1]
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
        (((current as f64 / total as f64) * 100.0) as u16).min(100)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::views::test_util::render_to_string;

    #[test]
    fn summary_lists_counts_freed_and_failures() {
        let summary = CleanSummary {
            dry_run: false,
            deleted: 3,
            freed_bytes: 2 * 1024 * 1024,
            failed: vec![("Trash".into(), "permission denied".into())],
        };
        let s = render_to_string(100, 24, |f| render_summary(f, &summary));
        assert!(s.contains("Cleanup Complete"), "{s}");
        assert!(s.contains("Deleted 3 items"), "{s}");
        assert!(s.contains("2.0 MiB"), "{s}");
        assert!(s.contains("1 failed"), "{s}");
        assert!(s.contains("Trash"), "{s}");
        assert!(s.contains("permission denied"), "{s}");
        assert!(s.contains("Enter"), "{s}");
    }

    #[test]
    fn summary_in_dry_run_says_would_delete() {
        let summary = CleanSummary {
            dry_run: true,
            deleted: 2,
            freed_bytes: 10,
            failed: vec![],
        };
        let s = render_to_string(100, 24, |f| render_summary(f, &summary));
        assert!(s.contains("Dry-Run"), "{s}");
        assert!(s.contains("Would delete 2 items"), "{s}");
    }
}
