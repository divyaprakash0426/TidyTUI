use crate::core::disk::{self, DiskUsage};
use crate::tui::app::App;
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Padding, Paragraph},
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

const TOP_ITEMS: usize = 5;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    // Home holds most junk; fall back to / for machines without a home dir.
    let probe = dirs::home_dir().unwrap_or_else(|| "/".into());
    render_with_disk(f, app, area, disk::usage(&probe));
}

pub fn render_with_disk(f: &mut Frame, app: &App, area: Rect, disk: Option<DiskUsage>) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .margin(1)
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);
    render_overview(f, app, top[0]);
    render_disk(f, app, disk, top[1]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(rows[1]);
    render_distribution(f, app, bottom[0]);
    render_largest(f, app, bottom[1]);
}

fn bold(color: Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn render_overview(f: &mut Frame, app: &App, area: Rect) {
    let total_files: u64 = app.items.iter().map(|i| i.file_count).sum();

    let mut lines = vec![
        Line::from(vec![
            Span::raw("Locations  "),
            Span::styled(app.items.len().to_string(), bold(Color::Cyan)),
            Span::styled(
                format!("  ({total_files} files)"),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw("Junk found "),
            Span::styled(ByteSize(app.total_size).to_string(), bold(Color::Magenta)),
        ]),
        Line::from(vec![
            Span::raw("Selected   "),
            Span::styled(
                format!("{} items", app.selected_count()),
                bold(Color::Yellow),
            ),
            Span::raw(format!("  {}", ByteSize(app.selected_size()))),
        ]),
    ];
    if app.is_scanning() {
        lines.push(Line::from(Span::styled(
            format!("⟳ Scanning {}/{}", app.scan.checked, app.scan.total),
            Style::default().fg(Color::Yellow),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "Scan complete · press r to rescan",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Overview ")
        .padding(Padding::horizontal(1));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

/// Junk relative to the disk (when known) decides the verdict; the absolute
/// thresholds are only a fallback for exotic filesystems.
fn verdict(total_junk: u64, disk: Option<DiskUsage>) -> (Color, &'static str, String) {
    match disk {
        Some(d) => {
            let frac = d.fraction_of_total(total_junk);
            let (color, label) = if frac < 0.01 {
                (Color::Green, "Clean")
            } else if frac < 0.05 {
                (Color::Yellow, "Moderate")
            } else {
                (Color::Red, "Critical")
            };
            (color, label, format!("{:.1}% of disk", frac * 100.0))
        }
        None => {
            let (color, label) = if total_junk < 100_000_000 {
                (Color::Green, "Clean")
            } else if total_junk < 500_000_000 {
                (Color::Yellow, "Moderate")
            } else {
                (Color::Red, "Critical")
            };
            (color, label, String::new())
        }
    }
}

fn render_disk(f: &mut Frame, app: &App, disk: Option<DiskUsage>, area: Rect) {
    let (color, label, relative) = verdict(app.total_size, disk);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Disk · {label} "))
        .border_style(Style::default().fg(color))
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    match disk {
        Some(d) => {
            let used_pct = (d.used_fraction() * 100.0).round().clamp(0.0, 100.0) as u16;
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(Color::Blue).bg(Color::Black))
                .percent(used_pct)
                .label(format!(
                    "{used_pct}% used · {} / {}",
                    ByteSize(d.used),
                    ByteSize(d.total)
                ));
            f.render_widget(gauge, rows[0]);
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw("Reclaimable "),
                    Span::styled(ByteSize(app.total_size).to_string(), bold(Color::Magenta)),
                    Span::styled(format!("  ({relative})"), Style::default().fg(color)),
                ])),
                rows[2],
            );
        }
        None => {
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw("Reclaimable "),
                    Span::styled(ByteSize(app.total_size).to_string(), bold(Color::Magenta)),
                ])),
                rows[0],
            );
            f.render_widget(
                Paragraph::new(Span::styled(
                    "Disk capacity unavailable · thresholds: <100 MB clean, <500 MB moderate",
                    Style::default().fg(Color::DarkGray),
                )),
                rows[2],
            );
        }
    }
}

fn render_distribution(f: &mut Frame, app: &App, area: Rect) {
    let mut by_cat: HashMap<&str, u64> = HashMap::new();
    for item in &app.items {
        *by_cat.entry(item.category.as_str()).or_insert(0) += item.size_bytes;
    }
    let mut data: Vec<(&str, u64)> = by_cat.into_iter().collect();
    data.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Junk by Category ")
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let label_w = 18usize;
    let pct_w = 7usize;
    let size_w = 11usize;
    let bar_w = (inner.width as usize)
        .saturating_sub(label_w + pct_w + size_w + 3)
        .max(4);
    let max = data.first().map(|d| d.1).unwrap_or(0).max(1);

    let lines: Vec<Line> = data
        .iter()
        .enumerate()
        .map(|(idx, (cat, size))| {
            let color = PALETTE[idx % PALETTE.len()];
            let pct = if app.total_size > 0 {
                *size as f64 / app.total_size as f64 * 100.0
            } else {
                0.0
            };
            let filled = ((*size as f64 / max as f64) * bar_w as f64).round() as usize;
            let filled = filled.clamp(usize::from(*size > 0), bar_w);
            let pct_str = if pct > 0.0 && pct < 0.1 {
                "<0.1%".to_string()
            } else {
                format!("{pct:.1}%")
            };
            Line::from(vec![
                Span::styled(format!("{:<label_w$}", truncate(cat, label_w)), bold(color)),
                Span::styled("█".repeat(filled), Style::default().fg(color)),
                Span::styled(
                    "░".repeat(bar_w - filled),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(format!(" {pct_str:>6}")),
                Span::styled(
                    format!(" {:>10}", ByteSize(*size).to_string()),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_largest(f: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<_> = app.items.iter().collect();
    items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    let lines: Vec<Line> = items
        .iter()
        .take(TOP_ITEMS)
        .enumerate()
        .map(|(n, item)| {
            Line::from(vec![
                Span::styled(format!("{}. ", n + 1), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:<24}", truncate(&item.name, 24)),
                    bold(Color::White),
                ),
                Span::styled(
                    format!("{:>10}", ByteSize(item.size_bytes).to_string()),
                    Style::default().fg(Color::Magenta),
                ),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Largest Items ")
        .padding(Padding::horizontal(1));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::disk::DiskUsage;
    use crate::core::{CleanMode, CleanupItem, ItemStatus};
    use crate::tui::views::test_util::render_to_string;
    use std::path::PathBuf;

    fn item(name: &str, cat: &str, size: u64) -> CleanupItem {
        CleanupItem {
            group_id: String::new(),
            name: name.into(),
            category: cat.into(),
            description: None,
            path: PathBuf::from("/x"),
            size_bytes: size,
            file_count: 1,
            selected: false,
            status: ItemStatus::Scanned,
            mode: CleanMode::Contents,
            keep_days: None,
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.set_items(vec![
            item("Big", "System", 800),
            item("Mid", "Dev", 150),
            item("Small", "Dev", 50),
        ]);
        app
    }

    fn render(app: &App, disk: Option<DiskUsage>) -> String {
        render_to_string(140, 40, |f| render_with_disk(f, app, f.area(), disk))
    }

    #[test]
    fn distribution_shows_bars_sorted_by_size_with_percentages() {
        let s = render(&app(), None);
        let sys = s.find("System").unwrap();
        let dev = s.find("Dev").unwrap();
        assert!(sys < dev, "largest category first:\n{s}");
        assert!(s.contains("80.0%"), "{s}");
        assert!(s.contains("20.0%"), "{s}");
        assert!(s.contains("█"), "{s}");
    }

    #[test]
    fn largest_items_panel_lists_top_items_by_size() {
        let s = render(&app(), None);
        assert!(s.contains("Largest"), "{s}");
        let big = s.find("Big").unwrap();
        let mid = s.find("Mid").unwrap();
        let small = s.find("Small").unwrap();
        assert!(big < mid && mid < small, "{s}");
    }

    #[test]
    fn gauge_is_disk_relative_when_disk_info_is_available() {
        let disk = DiskUsage {
            total: 100_000,
            used: 40_000,
        };
        let s = render(&app(), Some(disk));
        assert!(s.contains("Disk"), "{s}");
        assert!(s.contains("40%"), "{s}");
        assert!(s.contains("Reclaimable"), "{s}");
        assert!(s.contains("1.0% of disk"), "{s}");
        assert!(s.contains("Moderate"), "{s}");
    }

    #[test]
    fn gauge_falls_back_to_absolute_thresholds_without_disk_info() {
        let s = render(&app(), None);
        assert!(s.contains("Clean"), "{s}");
        assert!(!s.contains("of disk"), "{s}");
    }

    #[test]
    fn overview_shows_scan_progress_while_scanning() {
        let mut app = app();
        app.begin_scan(9);
        app.note_missing();
        let s = render(&app, None);
        assert!(s.contains("Scanning 1/9"), "{s}");
    }
}
