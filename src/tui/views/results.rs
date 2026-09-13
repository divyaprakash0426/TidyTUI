use crate::core::paths::shorten_home;
use crate::core::{CleanMode, CleanupItem, ItemStatus};
use crate::tui::app::{App, ResultRow, SortMode};
use crate::tui::theme::Theme;
use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::path::Path;

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let home = dirs::home_dir();
    render_with_home(f, app, area, home.as_deref());
}

/// Minimum terminal width at which the detail pane replaces the info bar.
const DETAIL_PANE_MIN_WIDTH: u16 = 100;
const DETAIL_PANE_WIDTH: u16 = 44;

/// Splits the tab into the list and its companion: a details pane beside it
/// on wide terminals, a one-line info bar below it otherwise.
fn split(area: Rect) -> (Rect, Rect, bool) {
    let wide = area.width >= DETAIL_PANE_MIN_WIDTH;
    let chunks = if wide {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(DETAIL_PANE_WIDTH)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area)
    };
    (chunks[0], chunks[1], wide)
}

/// Area inside the list's border where rows are drawn (for mouse hit-testing).
pub fn list_inner(area: Rect) -> Rect {
    Block::default().borders(Borders::ALL).inner(split(area).0)
}

pub fn render_with_home(f: &mut Frame, app: &mut App, area: Rect, home: Option<&Path>) {
    let (list, side, wide) = split(area);
    render_list(f, app, list, home);
    if wide {
        f.render_widget(detail_pane(app, home), side);
    } else {
        f.render_widget(info_bar(app), side);
    }
}

fn render_list(f: &mut Frame, app: &mut App, area: Rect, home: Option<&Path>) {
    let th = app.theme;
    let block = Block::default().borders(Borders::ALL).title(title(app));

    if app.rendered_rows.is_empty() {
        let msg = if app.is_scanning() {
            "Scanning… nothing found yet"
        } else if !app.filter.is_empty() {
            "No items match the filter (Esc to clear)"
        } else {
            "Nothing to clean 🎉"
        };
        let para = Paragraph::new(Line::from(Span::styled(msg, Style::default().fg(th.dim))))
            .block(block)
            .alignment(Alignment::Center);
        f.render_widget(para, area);
        return;
    }

    let counts = app.category_counts();
    let inner_width = area.width.saturating_sub(2) as usize;
    let (items, collapsed) = (&app.items, &app.collapsed);
    let highlighted = app.state.selected();
    let rows: Vec<ListItem> = app
        .rendered_rows
        .iter()
        .enumerate()
        .map(|(i, row)| match row {
            ResultRow::CategoryHeader(cat) => {
                let count = counts.get(cat).copied().unwrap_or(0);
                header_row(th, items, collapsed.contains(cat), cat, count)
            }
            ResultRow::Item(idx) => {
                item_row(th, &items[*idx], home, inner_width, highlighted == Some(i))
            }
            ResultRow::EmptyLine => ListItem::new(""),
        })
        .collect();

    let list = List::new(rows)
        .block(block)
        .highlight_style(Style::default().bg(th.surface).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, &mut app.state);
}

fn title(app: &App) -> String {
    let mut t = format!(" Cleanable Items ({} found", app.items.len());
    if !app.filter.is_empty() {
        t.push_str(&format!(" · filter: {}", app.filter));
    }
    if app.sort != SortMode::Default {
        t.push_str(&format!(" · sort: {}", app.sort.label()));
    }
    let locked = app.items.iter().filter(|i| i.locked).count();
    if locked > 0 {
        t.push_str(&format!(" · {locked} need root"));
    }
    t.push_str(") ");
    t
}

fn header_row<'a>(
    th: Theme,
    items: &[CleanupItem],
    collapsed: bool,
    cat: &str,
    count: usize,
) -> ListItem<'a> {
    let marker = if collapsed { "▸" } else { "▾" };
    let selected = items
        .iter()
        .filter(|i| i.category == cat && i.selected)
        .count();
    let mut spans = vec![Span::styled(
        format!("{marker} {cat} ({count})"),
        Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
    )];
    if selected > 0 {
        spans.push(Span::styled(
            format!("  {selected} selected"),
            Style::default().fg(th.dim),
        ));
    }
    ListItem::new(Line::from(spans))
}

fn mode_label(item: &CleanupItem) -> &'static str {
    if item.command.is_some() {
        return "runs command";
    }
    match item.mode {
        CleanMode::Contents => "empties folder, keeps it",
        CleanMode::Dir => "removes folder itself",
    }
}

fn status_span(th: Theme, item: &CleanupItem) -> Span<'static> {
    match &item.status {
        ItemStatus::Scanned if item.locked => {
            Span::styled("needs root", Style::default().fg(th.danger))
        }
        ItemStatus::Scanned => Span::styled("scanned", Style::default().fg(th.dim)),
        ItemStatus::DryRun if item.command.is_some() => {
            Span::styled("would run", Style::default().fg(th.accent))
        }
        ItemStatus::DryRun => Span::styled("would delete", Style::default().fg(th.accent)),
        ItemStatus::Deleted if item.command.is_some() => {
            Span::styled("ran", Style::default().fg(th.ok))
        }
        ItemStatus::Deleted => Span::styled("deleted", Style::default().fg(th.ok)),
        ItemStatus::Failed(reason) => {
            Span::styled(format!("failed: {reason}"), Style::default().fg(th.danger))
        }
    }
}

/// One-line summary of the highlighted item: description, file count, mode.
fn info_bar(app: &App) -> Paragraph<'static> {
    let th = app.theme;
    let Some(item) = app.selected_item() else {
        return Paragraph::new("");
    };
    let mut spans = vec![Span::raw(" ")];
    if let Some(desc) = &item.description {
        spans.push(Span::styled(
            desc.clone(),
            Style::default().add_modifier(Modifier::ITALIC),
        ));
        spans.push(Span::raw("  ·  "));
    }
    spans.push(Span::raw(format!("{} files", item.file_count)));
    spans.push(Span::raw("  ·  "));
    spans.push(Span::styled(mode_label(item), Style::default().fg(th.dim)));
    Paragraph::new(Line::from(spans))
}

/// Right-hand pane with the full details of the highlighted row.
fn detail_pane(app: &App, home: Option<&Path>) -> Paragraph<'static> {
    let th = app.theme;
    let block = Block::default().borders(Borders::ALL).title(" Details ");
    let label = |s: &'static str| Span::styled(format!("{s:<9}"), Style::default().fg(th.accent));

    let lines: Vec<Line> = match app.highlighted_row() {
        Some(ResultRow::Item(idx)) => {
            let item = &app.items[*idx];
            let mut lines = vec![
                Line::from(vec![
                    label("Name"),
                    Span::styled(
                        item.name.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    label("Path"),
                    Span::raw(shorten_home(&item.path, home)),
                ]),
                Line::from(vec![
                    label("Size"),
                    Span::styled(
                        ByteSize(item.size_bytes).to_string(),
                        Style::default().fg(th.size),
                    ),
                    Span::raw(format!(" · {} files", item.file_count)),
                ]),
                Line::from(vec![label("Mode"), Span::raw(mode_label(item))]),
                Line::from(vec![label("Status"), status_span(th, item)]),
                Line::from(vec![
                    label("Group"),
                    Span::styled(item.group_id.clone(), Style::default().fg(th.dim)),
                ]),
            ];
            if let Some(cmd) = &item.command {
                lines.push(Line::from(vec![
                    label("Command"),
                    Span::styled(cmd.clone(), Style::default().fg(th.warn)),
                ]));
            }
            if item.locked {
                lines.push(Line::from(vec![
                    label("Access"),
                    Span::styled(
                        "read-only — re-run with sudo to clean",
                        Style::default().fg(th.danger),
                    ),
                ]));
            }
            if let Some(days) = item.keep_days {
                lines.push(Line::from(vec![
                    label("Keeps"),
                    Span::raw(format!("entries newer than {days} days")),
                ]));
            }
            if let Some(desc) = &item.description {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    desc.clone(),
                    Style::default().add_modifier(Modifier::ITALIC),
                )));
            }
            lines
        }
        Some(ResultRow::CategoryHeader(cat)) => {
            let idxs: Vec<&CleanupItem> = app.items.iter().filter(|i| i.category == *cat).collect();
            let size: u64 = idxs.iter().map(|i| i.size_bytes).sum();
            let files: u64 = idxs.iter().map(|i| i.file_count).sum();
            vec![
                Line::from(vec![
                    label("Category"),
                    Span::styled(cat.clone(), Style::default().add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![label("Items"), Span::raw(idxs.len().to_string())]),
                Line::from(vec![
                    label("Size"),
                    Span::styled(ByteSize(size).to_string(), Style::default().fg(th.size)),
                    Span::raw(format!(" · {files} files")),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Space selects all · z folds",
                    Style::default().fg(th.dim),
                )),
            ]
        }
        _ => vec![Line::from(Span::styled(
            "Select a row to see details",
            Style::default().fg(th.dim),
        ))],
    };

    Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
}

const NAME_COL: usize = 22;
const SIZE_COL: usize = 10;
/// ">> " highlight symbol + "[x] " checkbox.
const ROW_PREFIX: usize = 3 + 4;
const MIN_PATH_COL: usize = 8;

fn item_row<'a>(
    th: Theme,
    item: &'a CleanupItem,
    home: Option<&Path>,
    width: usize,
    highlighted: bool,
) -> ListItem<'a> {
    // The highlight background can equal the dim colour (it does in the
    // default theme), so the path switches to the text colour on that row.
    let path_fg = if highlighted { th.text } else { th.dim };
    let checkbox = if item.selected { "[x] " } else { "[ ] " };
    let size = ByteSize(item.size_bytes).to_string();
    let status = (item.status != ItemStatus::Scanned || item.locked).then(|| status_span(th, item));
    let status_width = status.as_ref().map_or(0, |s| s.width() + 2);

    // Path and status share what is left; the path keeps a minimum and the
    // status text is clipped if the row is still too narrow.
    let avail = width.saturating_sub(ROW_PREFIX + NAME_COL + 1 + SIZE_COL);
    let path_col = if status_width == 0 {
        avail
    } else {
        avail
            .saturating_sub(status_width)
            .max(MIN_PATH_COL)
            .min(avail)
    };
    let status_budget = avail.saturating_sub(path_col).saturating_sub(2);
    let status = status.map(|s| {
        if s.width() > status_budget {
            Span::styled(truncate_right(&s.content, status_budget), s.style)
        } else {
            s
        }
    });
    let path = truncate_left(&shorten_home(&item.path, home), path_col);

    let mut spans = vec![
        Span::raw(checkbox),
        Span::styled(
            format!("{:<NAME_COL$}", truncate_right(&item.name, NAME_COL)),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(format!("{path:<path_col$}"), Style::default().fg(path_fg)),
        Span::styled(format!("{size:>SIZE_COL$}"), Style::default().fg(th.size)),
    ];
    if let Some(status) = status {
        spans.push(Span::raw("  "));
        spans.push(status);
    }

    ListItem::new(Line::from(spans))
}

/// Keeps the tail of `s` (the most specific part of a path) within `max`.
fn truncate_left(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let tail: String = s.chars().skip(n - (max - 1)).collect();
    format!("…{tail}")
}

fn truncate_right(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let head: String = s.chars().take(max - 1).collect();
    format!("{head}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::views::test_util;
    use std::path::PathBuf;

    fn item(name: &str, path: &str, size: u64, status: ItemStatus) -> CleanupItem {
        CleanupItem {
            group_id: String::new(),
            name: name.into(),
            category: "Dev".into(),
            description: None,
            path: PathBuf::from(path),
            size_bytes: size,
            file_count: 1,
            selected: false,
            status,
            mode: CleanMode::Contents,
            keep_days: None,
            locked: false,
            command: None,
        }
    }

    fn render_to_string(app: &mut App) -> String {
        render_at(app, 120)
    }

    fn render_at(app: &mut App, width: u16) -> String {
        test_util::render_to_string(width, 14, |f| {
            render_with_home(f, app, f.area(), Some(Path::new("/home/u")))
        })
    }

    #[test]
    fn highlighted_row_path_is_readable_on_the_highlight_background() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut app = App::new();
        app.set_items(vec![item(
            "Yay Cache",
            "/home/u/.cache/yay",
            10,
            ItemStatus::Scanned,
        )]);
        let th = Theme::default();
        assert_eq!(th.dim, th.surface, "test relies on the default theme clash");
        let mut t = Terminal::new(TestBackend::new(120, 14)).unwrap();
        t.draw(|f| render_with_home(f, &mut app, f.area(), Some(Path::new("/home/u"))))
            .unwrap();
        let buf = t.backend().buffer();
        let row = (0..buf.area.height)
            .find(|&y| (0..buf.area.width).any(|x| buf[(x, y)].bg == th.surface))
            .expect("a highlighted row");
        let path_x = (0..buf.area.width)
            .find(|&x| buf[(x, row)].symbol() == "~")
            .expect("path drawn on the highlighted row");
        let cell = &buf[(path_x, row)];
        assert_eq!(cell.bg, th.surface);
        assert_ne!(cell.fg, th.dim, "dim-on-dim path is invisible");
    }

    #[test]
    fn shows_shortened_path_and_size() {
        let mut app = App::new();
        app.set_items(vec![item(
            "Pip Cache",
            "/home/u/.cache/pip",
            2 * 1024 * 1024,
            ItemStatus::Scanned,
        )]);
        let s = render_to_string(&mut app);
        assert!(s.contains("Pip Cache"), "{s}");
        assert!(s.contains("~/.cache/pip"), "{s}");
        assert!(s.contains("2.0 MiB"), "{s}");
        assert!(s.contains("▾ Dev (1)"), "{s}");
    }

    #[test]
    fn locked_item_shows_needs_root_in_row_title_and_details() {
        let mut app = App::new();
        let mut locked = item("Pacman", "/var/cache/pacman/pkg", 5, ItemStatus::Scanned);
        locked.locked = true;
        app.set_items(vec![
            locked,
            item("Pip", "/home/u/.cache/pip", 1, ItemStatus::Scanned),
        ]);
        let s = render_to_string(&mut app);
        assert!(s.contains("needs root"), "{s}");
        assert!(s.contains("1 need root"), "{s}");
        assert!(s.contains("re-run with sudo"), "{s}");
    }

    #[test]
    fn command_item_shows_would_run_and_the_command_in_details() {
        let mut app = App::new();
        let mut it = item("Pacman", "/var/cache/pacman/pkg", 5, ItemStatus::DryRun);
        it.command = Some("paccache -rk2".into());
        app.set_items(vec![it]);
        let s = render_to_string(&mut app);
        assert!(s.contains("would run"), "{s}");
        assert!(s.contains("Command  paccache -rk2"), "{s}");
        assert!(s.contains("runs command"), "{s}");
    }

    #[test]
    fn long_paths_are_truncated_from_the_left() {
        assert_eq!(truncate_left("/a/b/c/d", 5), "…/c/d");
        assert_eq!(truncate_left("/a/b", 10), "/a/b");
        assert_eq!(truncate_right("abcdef", 4), "abc…");
    }

    #[test]
    fn narrow_row_keeps_size_and_status_visible() {
        let mut app = App::new();
        app.set_items(vec![item(
            "Very Long Name Here",
            "/home/u/.cache/some/deeply/nested/path/that/is/long",
            1024,
            ItemStatus::DryRun,
        )]);
        let s = render_at(&mut app, 80);
        assert!(s.contains("1.0 KiB"), "{s}");
        assert!(s.contains("would delete"), "{s}");
    }

    #[test]
    fn collapsed_header_shows_closed_marker_and_hides_items() {
        let mut app = App::new();
        app.set_items(vec![item("Pip Cache", "/a", 1, ItemStatus::Scanned)]);
        app.toggle_collapse();
        let s = render_to_string(&mut app);
        assert!(s.contains("▸ Dev (1)"), "{s}");
        assert!(!s.contains("Pip Cache"), "{s}");
    }

    #[test]
    fn title_shows_filter_and_sort_when_active() {
        let mut app = App::new();
        app.set_items(vec![item("Pip Cache", "/a", 1, ItemStatus::Scanned)]);
        app.set_filter("pip".into());
        app.cycle_sort();
        let s = render_to_string(&mut app);
        assert!(s.contains("filter: pip"), "{s}");
        assert!(s.contains("sort: size"), "{s}");
    }

    #[test]
    fn wide_layout_shows_detail_pane_with_labels() {
        let mut app = App::new();
        let mut it = item("Pip Cache", "/home/u/.cache/pip", 1, ItemStatus::DryRun);
        it.description = Some("Python package cache".into());
        it.group_id = "dev_pip".into();
        it.keep_days = Some(30);
        app.set_items(vec![it]);
        let s = render_at(&mut app, 120);
        assert!(s.contains("Path"), "{s}");
        assert!(s.contains("Status"), "{s}");
        assert!(s.contains("Python package cache"), "{s}");
        assert!(s.contains("dev_pip"), "{s}");
        assert!(s.contains("newer than 30 days"), "{s}");
    }

    #[test]
    fn narrow_layout_uses_info_bar_without_detail_pane() {
        let mut app = App::new();
        let mut it = item("Pip Cache", "/home/u/.cache/pip", 1, ItemStatus::Scanned);
        it.file_count = 42;
        app.set_items(vec![it]);
        let s = render_at(&mut app, 80);
        assert!(!s.contains("Status"), "{s}");
        assert!(s.contains("42 files"), "{s}");
    }

    #[test]
    fn empty_states_explain_why_list_is_empty() {
        let mut app = App::new();
        app.begin_scan(5);
        let s = render_to_string(&mut app);
        assert!(s.contains("Scanning"), "{s}");

        app.finish_scan();
        let s = render_to_string(&mut app);
        assert!(s.contains("Nothing to clean"), "{s}");

        app.set_items(vec![item("A", "/a", 1, ItemStatus::Scanned)]);
        app.set_filter("zzz".into());
        let s = render_to_string(&mut app);
        assert!(s.contains("No items match"), "{s}");
    }

    #[test]
    fn shows_status_markers() {
        let mut app = App::new();
        app.set_items(vec![
            item("A", "/a", 1, ItemStatus::DryRun),
            item("B", "/b", 1, ItemStatus::Failed("permission denied".into())),
        ]);
        let s = render_to_string(&mut app);
        assert!(s.contains("would delete"), "{s}");
        assert!(s.contains("failed: permission"), "{s}");
        app.next(); // highlight B: detail pane shows the full reason
        let s = render_to_string(&mut app);
        assert!(s.contains("failed: permission denied"), "{s}");
    }

    #[test]
    fn info_bar_shows_description_files_and_mode_of_highlighted_item() {
        let mut app = App::new();
        let mut it = item("Pip Cache", "/a", 1, ItemStatus::Scanned);
        it.description = Some("Python package cache".into());
        it.file_count = 42;
        app.set_items(vec![it]);
        let s = render_to_string(&mut app);
        assert!(s.contains("Python package cache"), "{s}");
        assert!(s.contains("42 files"), "{s}");
        assert!(s.contains("empties folder"), "{s}");
    }

    #[test]
    fn info_bar_falls_back_when_no_description() {
        let mut app = App::new();
        let mut it = item("X", "/a", 1, ItemStatus::Scanned);
        it.mode = CleanMode::Dir;
        app.set_items(vec![it]);
        let s = render_to_string(&mut app);
        assert!(s.contains("removes folder"), "{s}");
    }

    #[test]
    fn title_shows_item_count() {
        let mut app = App::new();
        app.set_items(vec![item("A", "/a", 1, ItemStatus::Scanned)]);
        let s = render_to_string(&mut app);
        assert!(s.contains("Cleanable Items (1 found)"), "{s}");
    }
}
