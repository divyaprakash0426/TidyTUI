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

#[cfg(test)]
pub(crate) mod test_util {
    use ratatui::{backend::TestBackend, Frame, Terminal};

    /// Renders one frame at the given size and returns it as text.
    pub fn render_to_string(width: u16, height: u16, draw: impl FnOnce(&mut Frame)) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(draw).unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(4),
        ])
        .split(f.area());

    render_tabs(f, app, chunks[0]);

    match &app.app_state {
        AppState::Viewing | AppState::Filtering => render_active_tab(f, app, chunks[1]),
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
        AppState::Summary(summary) => {
            let summary = summary.clone();
            render_active_tab(f, app, chunks[1]);
            modals::render_summary(f, &summary);
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
    let dim = Style::default().fg(Color::DarkGray);
    let key = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let mut status = vec![
        Span::raw("Found: "),
        Span::styled(
            ByteSize(app.total_size).to_string(),
            Style::default().fg(Color::Magenta),
        ),
        Span::styled("  ·  ", dim),
        Span::raw(format!("Selected: {} (", app.selected_count())),
        Span::styled(
            ByteSize(app.selected_size()).to_string(),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw(")"),
    ];
    if app.is_scanning() {
        status.push(Span::styled("  ·  ", dim));
        status.push(Span::styled(
            format!("⟳ Scanning {}/{}", app.scan.checked, app.scan.total),
            Style::default().fg(Color::Yellow),
        ));
    }
    status.push(Span::styled("  ·  ", dim));
    status.push(if app.dry_run {
        Span::styled(
            "DRY-RUN (Safe)",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            "DANGER (DELETING)",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    });

    let hints = if app.app_state == AppState::Filtering {
        Line::from(vec![
            Span::styled("Filter: ", key),
            Span::raw(app.filter.clone()),
            Span::styled("▏", Style::default().fg(Color::Yellow)),
            Span::styled("   Enter keep · Esc clear", dim),
        ])
    } else {
        let hint = |k: &'static str, what: &'static str| {
            vec![Span::styled(k, key), Span::raw(format!(" {what}  "))]
        };
        let mut spans = Vec::new();
        spans.extend(hint("↑↓", "nav"));
        spans.extend(hint("Space", "toggle"));
        spans.extend(hint("a/A", "all/none"));
        spans.extend(hint("z", "fold"));
        spans.extend(hint("/", "filter"));
        spans.extend(hint("s", "sort"));
        spans.extend(hint("r", "rescan"));
        spans.extend(hint("d", "mode"));
        spans.extend(hint("Enter", "clean"));
        spans.extend(hint("q", "quit"));
        Line::from(spans)
    };

    let footer = Paragraph::new(vec![Line::from(status), hints])
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}

#[cfg(test)]
mod tests {
    use super::test_util::render_to_string;
    use super::*;
    use crate::core::{CleanMode, CleanupItem, ItemStatus};
    use crate::tui::app::CleanSummary;
    use std::path::PathBuf;

    fn app_with_item() -> App {
        let mut app = App::new();
        app.set_items(vec![CleanupItem {
            group_id: String::new(),
            name: "Pip".into(),
            category: "Dev".into(),
            description: None,
            path: PathBuf::from("/x"),
            size_bytes: 1,
            file_count: 1,
            selected: false,
            status: ItemStatus::Scanned,
            mode: CleanMode::Contents,
            keep_days: None,
        }]);
        app.active_tab = Tab::Results;
        app
    }

    #[test]
    fn footer_shows_scan_progress_while_scanning() {
        let mut app = app_with_item();
        app.begin_scan(10);
        app.note_missing();
        app.note_missing();
        let s = render_to_string(120, 20, |f| render(f, &mut app));
        assert!(s.contains("Scanning 2/10"), "{s}");
    }

    #[test]
    fn footer_shows_filter_prompt_when_filtering() {
        let mut app = app_with_item();
        app.app_state = AppState::Filtering;
        app.set_filter("pi".into());
        let s = render_to_string(120, 20, |f| render(f, &mut app));
        assert!(s.contains("Filter: pi"), "{s}");
        assert!(s.contains("Enter"), "{s}");
    }

    #[test]
    fn footer_shows_selection_totals_and_mode() {
        let mut app = app_with_item();
        app.items[0].selected = true;
        let s = render_to_string(120, 20, |f| render(f, &mut app));
        assert!(s.contains("Selected: 1"), "{s}");
        assert!(s.contains("DRY-RUN"), "{s}");
    }

    #[test]
    fn summary_state_draws_modal_over_results() {
        let mut app = app_with_item();
        app.app_state = AppState::Summary(CleanSummary {
            dry_run: false,
            deleted: 2,
            freed_bytes: 1024,
            failed: vec![],
        });
        let s = render_to_string(120, 24, |f| render(f, &mut app));
        assert!(s.contains("Cleanup Complete"), "{s}");
    }
}
