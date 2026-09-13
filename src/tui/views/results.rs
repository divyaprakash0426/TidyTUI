use crate::core::paths::shorten_home;
use crate::core::{CleanMode, CleanupItem, ItemStatus};
use crate::tui::app::{App, ResultRow};
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::path::Path;

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let home = dirs::home_dir();
    render_with_home(f, app, area, home.as_deref());
}

pub fn render_with_home(f: &mut Frame, app: &mut App, area: Rect, home: Option<&Path>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let rows: Vec<ListItem> = app
        .rendered_rows
        .iter()
        .map(|row| match row {
            ResultRow::CategoryHeader(cat) => ListItem::new(Line::from(Span::styled(
                format!("── {cat} ──"),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ))),
            ResultRow::Item(idx) => item_row(&app.items[*idx], home),
            ResultRow::EmptyLine => ListItem::new(""),
        })
        .collect();

    let title = format!(" Cleanable Items ({} found) ", app.items.len());
    let list = List::new(rows)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, chunks[0], &mut app.state);
    f.render_widget(info_bar(app), chunks[1]);
}

/// One-line summary of the highlighted item: description, file count, mode.
fn info_bar(app: &App) -> Paragraph<'static> {
    let Some(item) = app.selected_item() else {
        return Paragraph::new("");
    };
    let mode = match item.mode {
        CleanMode::Contents => "empties folder, keeps it",
        CleanMode::Dir => "removes folder itself",
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
    spans.push(Span::styled(mode, Style::default().fg(Color::DarkGray)));
    Paragraph::new(Line::from(spans))
}

fn item_row<'a>(item: &'a CleanupItem, home: Option<&Path>) -> ListItem<'a> {
    let checkbox = if item.selected { "[x] " } else { "[ ] " };
    let path = shorten_home(&item.path, home);
    let size = ByteSize(item.size_bytes).to_string();

    let mut spans = vec![
        Span::raw("  "),
        Span::raw(checkbox),
        Span::styled(
            format!("{:<22}", item.name),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{path:<40}"), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{size:>10}"), Style::default().fg(Color::Magenta)),
    ];

    match &item.status {
        ItemStatus::Scanned => {}
        ItemStatus::DryRun => spans.push(Span::styled(
            "  would delete",
            Style::default().fg(Color::Cyan),
        )),
        ItemStatus::Deleted => {
            spans.push(Span::styled("  deleted", Style::default().fg(Color::Green)))
        }
        ItemStatus::Failed(reason) => spans.push(Span::styled(
            format!("  failed: {reason}"),
            Style::default().fg(Color::Red),
        )),
    }

    ListItem::new(Line::from(spans))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;

    fn item(name: &str, path: &str, size: u64, status: ItemStatus) -> CleanupItem {
        CleanupItem {
            name: name.into(),
            category: "Dev".into(),
            description: None,
            path: PathBuf::from(path),
            size_bytes: size,
            file_count: 1,
            selected: false,
            status,
            mode: CleanMode::Contents,
        }
    }

    fn render_to_string(app: &mut App) -> String {
        let backend = TestBackend::new(120, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_with_home(f, app, f.area(), Some(Path::new("/home/u"))))
            .unwrap();
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
        assert!(s.contains("── Dev ──"), "{s}");
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
