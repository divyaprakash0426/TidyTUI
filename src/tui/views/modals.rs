use crate::core::paths::shorten_home;
use crate::tui::app::{App, CleanSummary};
use crate::tui::theme::Theme;
use crate::tui::views::text::truncate_right;
use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Padding, Paragraph},
    Frame,
};

const MAX_PREVIEW_PATHS: usize = 5;

/// How many preview paths fit in `room` text rows, and whether an
/// "… and N more" line follows. Paths give way before the mode/prompt lines.
fn fit_preview(selected: usize, room: usize) -> (usize, bool) {
    let shown = selected.min(MAX_PREVIEW_PATHS);
    if shown <= room && shown == selected {
        return (shown, false);
    }
    if room == 0 {
        return (0, false);
    }
    let shown = shown.min(room - 1);
    (shown, shown < selected)
}

/// Borders plus uniform padding, above and below.
const MODAL_CHROME_ROWS: usize = 4;
/// Blank separator rows in the confirm modal, dropped first when short on space.
const CONFIRM_SPACERS: usize = 3;

pub fn render_confirm(f: &mut Frame, app: &App, area: Rect) {
    let th = app.theme;
    let selected_items = app.selected_count();
    let selected_size = ByteSize(app.selected_size());
    let home = dirs::home_dir();
    let locked = app.items.iter().filter(|i| i.selected && i.locked).count();
    let warn_locked = locked > 0 && !app.dry_run;

    // Fit order: heading/mode/prompt (and the root warning) always; then
    // the path preview; blank separators take whatever is left.
    let avail = usize::from(area.height).saturating_sub(MODAL_CHROME_ROWS);
    let essential = 3 + usize::from(warn_locked);
    let (shown, more) = fit_preview(selected_items, avail.saturating_sub(essential));
    let used = essential + shown + usize::from(more);
    let spacers = CONFIRM_SPACERS.min(avail.saturating_sub(used));
    // Separator priority: after the paths, after the heading, before the prompt.
    let blank = |slot: usize| (spacers > slot).then(|| Line::from(""));

    let block = Block::default()
        .title(" CONFIRM CLEANUP ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(th.warn))
        .padding(Padding::uniform(1));

    let mut text = vec![Line::from(vec![
        Span::raw("Clean "),
        Span::styled(
            selected_items.to_string(),
            Style::default().add_modifier(Modifier::BOLD).fg(th.accent),
        ),
        Span::raw(" items, freeing "),
        Span::styled(
            selected_size.to_string(),
            Style::default().add_modifier(Modifier::BOLD).fg(th.size),
        ),
        Span::raw("?"),
    ])];
    text.extend(blank(1));

    for item in app.items.iter().filter(|i| i.selected).take(shown) {
        text.push(Line::from(Span::styled(
            format!("  {}", shorten_home(&item.path, home.as_deref())),
            Style::default().fg(th.dim),
        )));
    }
    if more {
        text.push(Line::from(Span::styled(
            format!("  … and {} more", selected_items - shown),
            Style::default().fg(th.dim),
        )));
    }
    text.extend(blank(0));

    if warn_locked {
        let noun = if locked == 1 {
            "item needs"
        } else {
            "items need"
        };
        text.push(Line::from(Span::styled(
            format!("⚠ {locked} selected {noun} root and will fail"),
            Style::default().fg(th.warn),
        )));
    }
    text.push(if app.dry_run {
        Line::from(Span::styled(
            "MODE: DRY-RUN (No files will be deleted)",
            Style::default().fg(th.ok),
        ))
    } else {
        Line::from(Span::styled(
            "WARNING: DANGER MODE (FILES WILL BE DELETED)",
            Style::default().fg(th.danger).add_modifier(Modifier::BOLD),
        ))
    });
    text.extend(blank(2));
    text.push(Line::from(vec![
        Span::raw("Press "),
        Span::styled(
            "[y]",
            Style::default().fg(th.ok).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to proceed, "),
        Span::styled(
            "[n]",
            Style::default().fg(th.danger).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to cancel."),
    ]));

    let height = (text.len() + MODAL_CHROME_ROWS) as u16;
    let area = centered_fixed_height(70, height, area);
    f.render_widget(Clear, area);
    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

const MAX_FAILURES_SHOWN: usize = 6;

pub fn render_summary(f: &mut Frame, th: Theme, summary: &CleanSummary, area: Rect) {
    // Width is fixed by the area, so it is known before the text is built;
    // failure reasons are clipped to one row each instead of wrapping.
    let inner_width = usize::from(centered_fixed_height(60, 1, area).width).saturating_sub(4);
    let (title, verb, color) = if summary.dry_run {
        (" Dry-Run Complete ", "Would delete", th.accent)
    } else {
        (" Cleanup Complete ", "Deleted", th.ok)
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
                Style::default().fg(th.size).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
    ];

    if summary.failed.is_empty() {
        text.push(Line::from(Span::styled(
            "No failures",
            Style::default().fg(th.dim),
        )));
    } else {
        text.push(Line::from(Span::styled(
            format!("{} failed:", summary.failed.len()),
            Style::default().fg(th.danger).add_modifier(Modifier::BOLD),
        )));
        for (name, reason) in summary.failed.iter().take(MAX_FAILURES_SHOWN) {
            let label = format!("  {name}: ");
            let budget = inner_width.saturating_sub(label.chars().count());
            text.push(Line::from(vec![
                Span::styled(label, Style::default().fg(th.danger)),
                Span::styled(truncate_right(reason, budget), Style::default().fg(th.dim)),
            ]));
        }
        if summary.failed.len() > MAX_FAILURES_SHOWN {
            text.push(Line::from(Span::styled(
                format!("  … and {} more", summary.failed.len() - MAX_FAILURES_SHOWN),
                Style::default().fg(th.dim),
            )));
        }
    }

    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::raw("Press "),
        Span::styled(
            "Enter",
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to continue"),
    ]));

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .padding(Padding::uniform(1));
    let height = (text.len() + MODAL_CHROME_ROWS) as u16;
    let area = centered_fixed_height(60, height, area);
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center),
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

pub fn render_progress(
    f: &mut Frame,
    th: Theme,
    current: usize,
    total: usize,
    item_name: &str,
    area: Rect,
) {
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
        .gauge_style(Style::default().fg(th.accent).bg(th.surface))
        .percent(percentage)
        .label(format!("{current}/{total}"));

    let info = Paragraph::new(format!("Cleaning: {item_name}"))
        .alignment(Alignment::Center)
        .style(Style::default().fg(th.warn));

    f.render_widget(info, chunks[0]);
    f.render_widget(gauge, chunks[1]);
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
        let s = render_to_string(100, 24, |f| {
            render_summary(f, Theme::default(), &summary, f.area())
        });
        assert!(s.contains("Cleanup Complete"), "{s}");
        assert!(s.contains("Deleted 3 items"), "{s}");
        assert!(s.contains("2.0 MiB"), "{s}");
        assert!(s.contains("1 failed"), "{s}");
        assert!(s.contains("Trash"), "{s}");
        assert!(s.contains("permission denied"), "{s}");
        assert!(s.contains("Enter"), "{s}");
    }

    #[test]
    fn confirm_modal_is_sized_to_its_content() {
        use crate::core::CleanMode;
        use crate::tui::app::App;
        use std::path::PathBuf;
        let mut app = App::new();
        app.set_items(
            (0..3)
                .map(|i| crate::core::CleanupItem {
                    group_id: format!("g{i}"),
                    name: format!("Item {i}"),
                    category: "Dev".into(),
                    description: None,
                    path: PathBuf::from(format!("/tmp/x{i}")),
                    size_bytes: 10,
                    file_count: 1,
                    selected: true,
                    status: crate::core::ItemStatus::Scanned,
                    mode: CleanMode::Contents,
                    keep_days: None,
                    locked: false,
                    command: None,
                })
                .collect(),
        );
        let s = render_to_string(120, 40, |f| render_confirm(f, &app, f.area()));
        let lines: Vec<&str> = s.lines().collect();
        let top = lines
            .iter()
            .position(|l| l.contains("CONFIRM CLEANUP"))
            .expect("title");
        let bottom = lines
            .iter()
            .rposition(|l| l.contains('└') || l.contains('╰'))
            .expect("bottom border");
        // 1 heading + blank + 3 paths + blank + mode + blank + prompt = 9 text
        // rows, plus 1 row of padding above and below and the bottom border.
        assert_eq!(bottom - top, 9 + 2 + 1, "{s}");
    }

    #[test]
    fn summary_keeps_every_failure_and_the_prompt_on_screen_despite_long_reasons() {
        let reason = format!(
            "12 entries could not be removed (first: /var/cache/pacman/pkg/{}: Permission denied (os error 13))",
            "x".repeat(60)
        );
        let summary = CleanSummary {
            dry_run: false,
            deleted: 0,
            freed_bytes: 0,
            failed: (0..7)
                .map(|i| (format!("Item{i}"), reason.clone()))
                .collect(),
        };
        let s = render_to_string(100, 40, |f| {
            render_summary(f, Theme::default(), &summary, f.area())
        });
        for i in 0..6 {
            assert!(s.contains(&format!("Item{i}:")), "Item{i} missing\n{s}");
        }
        assert!(s.contains("… and 1 more"), "{s}");
        assert!(s.contains("Press Enter to continue"), "{s}");
    }

    #[test]
    fn fit_preview_trades_paths_for_the_more_line_when_short_on_rows() {
        assert_eq!(fit_preview(3, 10), (3, false));
        assert_eq!(fit_preview(7, 10), (5, true));
        assert_eq!(fit_preview(3, 2), (1, true));
        assert_eq!(fit_preview(7, 3), (2, true));
        assert_eq!(fit_preview(3, 0), (0, false));
        assert_eq!(fit_preview(0, 5), (0, false));
    }

    #[test]
    fn summary_in_dry_run_says_would_delete() {
        let summary = CleanSummary {
            dry_run: true,
            deleted: 2,
            freed_bytes: 10,
            failed: vec![],
        };
        let s = render_to_string(100, 24, |f| {
            render_summary(f, Theme::default(), &summary, f.area())
        });
        assert!(s.contains("Dry-Run"), "{s}");
        assert!(s.contains("Would delete 2 items"), "{s}");
    }
}
