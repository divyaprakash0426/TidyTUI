use crate::tui::app::{App, AppState, Tab};
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

pub mod dashboard;
pub mod help;
pub mod modals;
pub mod results;
mod text;

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

/// The three fixed regions of the screen. Also used for mouse hit-testing,
/// so rendering and input agree on where things are.
pub struct Chunks {
    pub tabs: Rect,
    pub body: Rect,
    pub footer: Rect,
}

pub fn layout(area: Rect) -> Chunks {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(4),
        ])
        .split(area);
    Chunks {
        tabs: chunks[0],
        body: chunks[1],
        footer: chunks[2],
    }
}

const TAB_TITLES: [(&str, Tab); 3] = [
    (" [1] Dashboard ", Tab::Dashboard),
    (" [2] Results ", Tab::Results),
    (" [3] Help ", Tab::Help),
];

/// Column span of each tab title as ratatui's `Tabs` lays them out:
/// border, then per tab `" " title " "` separated by a one-cell divider.
fn tab_spans(tabs_area: Rect) -> impl Iterator<Item = (std::ops::Range<u16>, Tab)> {
    let mut x = tabs_area.x + 1;
    TAB_TITLES.into_iter().map(move |(title, tab)| {
        let start = x + 1;
        let width = title.chars().count() as u16;
        x += width + 3;
        (start..start + width, tab)
    })
}

pub fn tab_at(viewport: Rect, x: u16) -> Option<Tab> {
    tab_spans(layout(viewport).tabs)
        .find(|(range, _)| range.contains(&x))
        .map(|(_, tab)| tab)
}

/// Inner area of the Results list (inside its border) for the current viewport.
pub fn results_list_inner(app: &App) -> Rect {
    results::list_inner(layout(app.viewport).body)
}

/// First column of the tab's word (after the `[n] ` prefix) on a viewport at x = 0.
#[cfg(test)]
pub fn tab_hit_test_column(tab: Tab) -> u16 {
    tab_spans(layout(Rect::new(0, 0, 120, 40)).tabs)
        .find(|(_, t)| *t == tab)
        .map(|(range, _)| range.start + 5)
        .unwrap()
}

pub fn render(f: &mut Frame, app: &mut App) {
    let Chunks { tabs, body, footer } = layout(f.area());
    let chunks = [tabs, body, footer];

    render_tabs(f, app, chunks[0]);
    // The footer goes down first so a modal can never be painted over.
    render_footer(f, app, chunks[2]);

    match &app.app_state {
        AppState::Viewing | AppState::Filtering => render_active_tab(f, app, chunks[1]),
        AppState::Confirming => {
            render_active_tab(f, app, chunks[1]);
            modals::render_confirm(f, app, chunks[1]);
        }
        AppState::Cleaning {
            current,
            total,
            item_name,
        } => {
            let (current, total, name) = (*current, *total, item_name.clone());
            modals::render_progress(f, app.theme, current, total, &name, chunks[1]);
        }
        AppState::Summary(summary) => {
            let summary = summary.clone();
            render_active_tab(f, app, chunks[1]);
            modals::render_summary(f, app.theme, &summary, chunks[1]);
        }
    }
}

fn render_active_tab(f: &mut Frame, app: &mut App, area: Rect) {
    match app.active_tab {
        Tab::Dashboard => dashboard::render(f, app, area),
        Tab::Results => results::render(f, app, area),
        Tab::Help => help::render(f, app.theme, area),
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let th = app.theme;
    let titles: Vec<&str> = TAB_TITLES.iter().map(|(t, _)| *t).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" TidyTUI "))
        .select(match app.active_tab {
            Tab::Dashboard => 0,
            Tab::Results => 1,
            Tab::Help => 2,
        })
        .style(Style::default().fg(th.accent))
        .highlight_style(Style::default().fg(th.warn).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let th = app.theme;
    let dim = Style::default().fg(th.dim);
    let key = Style::default().fg(th.accent).add_modifier(Modifier::BOLD);

    let mut status = vec![
        Span::raw("Found: "),
        Span::styled(
            ByteSize(app.total_size).to_string(),
            Style::default().fg(th.size),
        ),
        Span::styled("  ·  ", dim),
        Span::raw(format!("Selected: {} (", app.selected_count())),
        Span::styled(
            ByteSize(app.selected_size()).to_string(),
            Style::default().fg(th.size),
        ),
        Span::raw(")"),
    ];
    if app.is_scanning() {
        status.push(Span::styled("  ·  ", dim));
        status.push(Span::styled(
            format!("⟳ Scanning {}/{}", app.scan.checked, app.scan.total),
            Style::default().fg(th.warn),
        ));
    }
    status.push(Span::styled("  ·  ", dim));
    status.push(if app.dry_run {
        Span::styled(
            "DRY-RUN (Safe)",
            Style::default().fg(th.ok).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            "DANGER (DELETING)",
            Style::default().fg(th.danger).add_modifier(Modifier::BOLD),
        )
    });

    let hints = if app.app_state == AppState::Filtering {
        Line::from(vec![
            Span::styled("Filter: ", key),
            Span::raw(app.filter.clone()),
            Span::styled("▏", Style::default().fg(th.warn)),
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
            locked: false,
            command: None,
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
    fn confirm_modal_warns_about_locked_selection_in_danger_mode() {
        let mut app = app_with_item();
        app.items[0].locked = true;
        app.items[0].selected = true;
        app.dry_run = false;
        app.app_state = AppState::Confirming;
        let s = render_to_string(120, 24, |f| render(f, &mut app));
        assert!(s.contains("1 selected item needs root"), "{s}");
        app.dry_run = true;
        let s = render_to_string(120, 24, |f| render(f, &mut app));
        assert!(!s.contains("selected item needs root"), "{s}");
    }

    #[test]
    fn tab_hit_columns_land_on_the_rendered_titles() {
        let mut app = app_with_item();
        let s = render_to_string(120, 24, |f| render(f, &mut app));
        let tabs_line = s.lines().nth(1).unwrap();
        for (tab, word) in [
            (Tab::Dashboard, "Dashboard"),
            (Tab::Results, "Results"),
            (Tab::Help, "Help"),
        ] {
            let x = tab_hit_test_column(tab) as usize;
            let cell: String = tabs_line.chars().skip(x).take(word.len()).collect();
            assert_eq!(cell, word, "{tabs_line}");
            assert_eq!(tab_at(Rect::new(0, 0, 120, 40), x as u16), Some(tab));
        }
        assert_eq!(tab_at(Rect::new(0, 0, 120, 40), 119), None);
    }

    #[test]
    fn theme_colours_reach_the_buffer() {
        use crate::tui::theme::{Theme, NORD};
        use ratatui::{backend::TestBackend, Terminal};
        let colour_of = |theme: Theme| {
            let mut app = app_with_item();
            app.theme = theme;
            let mut t = Terminal::new(TestBackend::new(120, 24)).unwrap();
            t.draw(|f| render(f, &mut app)).unwrap();
            // Cell inside the "Junk found" size value on the footer: styled th.size.
            let buf = t.backend().buffer();
            (0..buf.area.width)
                .filter_map(|x| {
                    let c = &buf[(x, buf.area.height - 3)];
                    (c.fg == theme.size).then_some(c.fg)
                })
                .next()
        };
        assert_eq!(colour_of(NORD), Some(NORD.size));
        assert_ne!(NORD.size, Theme::default().size);
    }

    #[test]
    fn confirm_modal_stays_clear_of_the_footer_on_a_short_terminal() {
        let mut app = App::new();
        app.set_items(
            (0..7)
                .map(|i| CleanupItem {
                    group_id: format!("g{i}"),
                    name: format!("Item{i}"),
                    category: "Dev".into(),
                    description: None,
                    path: PathBuf::from(format!("/x{i}")),
                    size_bytes: 1,
                    file_count: 1,
                    selected: true,
                    status: ItemStatus::Scanned,
                    mode: CleanMode::Contents,
                    keep_days: None,
                    locked: i == 0,
                    command: None,
                })
                .collect(),
        );
        app.dry_run = false;
        app.app_state = AppState::Confirming;
        let s = render_to_string(100, 17, |f| render(f, &mut app));
        let lines: Vec<&str> = s.lines().collect();
        let prompt = lines
            .iter()
            .position(|l| l.contains("to cancel."))
            .unwrap_or_else(|| panic!("prompt clipped:\n{s}"));
        let footer_top = lines
            .iter()
            .rposition(|l| l.contains("DANGER"))
            .expect("footer");
        assert!(
            prompt < footer_top,
            "prompt drawn on/after the footer:\n{s}"
        );
        assert!(
            lines[prompt + 1..].iter().any(|l| l.contains('└')),
            "modal bottom border missing:\n{s}"
        );
        assert!(s.contains("WARNING: DANGER MODE"), "{s}");
        assert!(
            !lines[footer_top].contains("cancel"),
            "modal bled into footer:\n{s}"
        );
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
