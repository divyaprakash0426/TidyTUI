use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Gauge, Padding, Clear},
    Frame,
};
use bytesize::ByteSize;
use crate::tui::app::{App, Tab, ResultRow, AppState};
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

    // --- Header ---
    let titles = vec![" [1] Dashboard ", " [2] Results ", " [3] Help "];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" TidyTUI "))
        .select(match app.active_tab {
            Tab::Dashboard => 0,
            Tab::Results => 1,
            Tab::Help => 2,
        })
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    
    f.render_widget(tabs, chunks[0]);

    // --- Content ---
    match app.app_state {
        AppState::Viewing => {
            match app.active_tab {
                Tab::Dashboard => render_dashboard(f, app, chunks[1]),
                Tab::Results => render_results(f, app, chunks[1]),
                Tab::Help => render_help(f, app, chunks[1]),
            }
        }
        AppState::Confirming => {
            // Render underlying content first (optional but looks nice)
            match app.active_tab {
                Tab::Dashboard => render_dashboard(f, app, chunks[1]),
                Tab::Results => render_results(f, app, chunks[1]),
                Tab::Help => render_help(f, app, chunks[1]),
            }
            render_confirm_modal(f, app);
        }
        AppState::Cleaning { current, total, ref item_name } => {
            render_progress_screen(f, current, total, item_name, chunks[1]);
        }
    }

    // --- Footer ---
    let total_size = ByteSize(app.total_size);
    let mode_text = if app.dry_run { 
        Span::styled("DRY-RUN (Safe)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)) 
    } else { 
        Span::styled("DANGER (DELETING)", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK)) 
    };

    let footer_text = Line::from(vec![
        Span::raw(format!("Total Found: {} | ", total_size)),
        Span::raw("Tab: <Tab>, Nav: <Up/Down>, Toggle: <Space>, Mode: <d>, Clean: <Enter> | "),
        mode_text,
    ]);

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL));
    
    f.render_widget(footer, chunks[2]);
}

fn render_confirm_modal(f: &mut Frame, app: &App) {
    let selected_items = app.items.iter().filter(|i| i.selected).count();
    let selected_size = ByteSize(app.items.iter().filter(|i| i.selected).map(|i| i.size_bytes).sum());
    
    let area = centered_rect(60, 25, f.area());
    f.render_widget(Clear, area); // This clears the area under the modal

    let block = Block::default()
        .title(" CONFIRM CLEANUP ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::uniform(1));

    let text = vec![
        Line::from(vec![
            Span::raw("Are you sure you want to clean "),
            Span::styled(format!("{}", selected_items), Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
            Span::raw(" items?"),
        ]),
        Line::from(vec![
            Span::raw("Total Space: "),
            Span::styled(format!("{}", selected_size), Style::default().add_modifier(Modifier::BOLD).fg(Color::Magenta)),
        ]),
        Line::from(""),
        if app.dry_run {
            Line::from(vec![Span::styled("MODE: DRY-RUN (No files will be deleted)", Style::default().fg(Color::Green))])
        } else {
            Line::from(vec![Span::styled("WARNING: DANGER MODE (FILES WILL BE DELETED)", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))])
        },
        Line::from(""),
        Line::from(vec![
            Span::raw("Press "),
            Span::styled("[y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" to proceed, "),
            Span::styled("[n]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" to cancel."),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(block).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(paragraph, area);
}

fn render_progress_screen(f: &mut Frame, current: usize, total: usize, item_name: &str, area: Rect) {
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
        .block(Block::default().borders(Borders::ALL).title(" Cleaning Progress "))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .percent(percentage)
        .label(format!("{}/{}", current, total));

    let info = Paragraph::new(format!("Deleting: {}", item_name))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(Color::Yellow));

    f.render_widget(info, chunks[0]);
    f.render_widget(gauge, chunks[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

fn render_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Min(0),
        ])
        .margin(1)
        .split(area);

    let total_items = app.items.len();
    let selected_items = app.items.iter().filter(|i| i.selected).count();
    let selected_size = ByteSize(app.items.iter().filter(|i| i.selected).map(|i| i.size_bytes).sum());

    let welcome_block = Block::default()
        .borders(Borders::ALL)
        .title(" System Overview ")
        .padding(Padding::uniform(1));

    let stats_text = vec![
        Line::from(vec![
            Span::raw("Discovered: "),
            Span::styled(format!("{} items", total_items), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("Total Size: "),
            Span::styled(app.total_size.to_string(), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" ({})", ByteSize(app.total_size))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Selected for cleaning: "),
            Span::styled(format!("{} items", selected_items), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" / {}", selected_size)),
        ]),
    ];

    let stats = Paragraph::new(stats_text);

    // Thresholds Help
    let thresholds_text = vec![
        Line::from(vec![Span::styled("Status Thresholds:", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  Clean    ", Style::default().fg(Color::Green)), Span::raw("< 100 MB")]),
        Line::from(vec![Span::styled("  Moderate ", Style::default().fg(Color::Yellow)), Span::raw("100 - 500 MB")]),
        Line::from(vec![Span::styled("  Critical ", Style::default().fg(Color::Red)), Span::raw("> 500 MB")]),
    ];
    let thresholds = Paragraph::new(thresholds_text);
    
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[0].inner(Margin { horizontal: 1, vertical: 1 }));
    
    // Also need titles block for the whole header
    f.render_widget(welcome_block, chunks[0]);
    
    f.render_widget(stats, header_chunks[0]);
    f.render_widget(thresholds, header_chunks[1]);

    // Distribution and Gauge Area
    let sub_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ])
        .split(chunks[1]);

    // 1. Distribution Legend
    let mut distribution: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for item in &app.items {
        *distribution.entry(item.category.clone()).or_insert(0) += item.size_bytes;
    }

    let mut data: Vec<(String, u64)> = distribution.into_iter().collect();
    data.sort_by(|a, b| b.1.cmp(&a.1));

    let colors = [Color::Cyan, Color::Magenta, Color::Yellow, Color::Green, Color::Blue, Color::Red];
    let legend_items: Vec<ListItem> = data.iter().enumerate().map(|(idx, (cat, size))| {
        let percentage = if app.total_size > 0 {
            (*size as f64 / app.total_size as f64) * 100.0
        } else {
            0.0
        };
        let perc_str = if percentage > 0.0 && percentage < 0.1 {
            "< 0.1%".to_string()
        } else {
            format!("{:>5.1}%", percentage)
        };
        let color = colors[idx % colors.len()];
        let content = Line::from(vec![
            Span::styled(" ● ", Style::default().fg(color)),
            Span::styled(format!("{:<15}", cat), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {} ", perc_str)),
            Span::styled(format!("({})", ByteSize(*size)), Style::default().fg(Color::DarkGray)),
        ]);
        ListItem::new(content)
    }).collect();

    let legend = List::new(legend_items)
        .block(Block::default().borders(Borders::ALL).title(" Junk Distribution "));
    
    f.render_widget(legend, sub_chunks[1].inner(ratatui::layout::Margin { horizontal: 1, vertical: 1 }));

    // 2. Cleanup Gauge
    // Logic: 100MB is 'Minor', 500MB is 'Noticeable', 1GB+ is 'Critical'
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
        .block(Block::default().borders(Borders::ALL).title(format!(" System Status: {} ", status)))
        .gauge_style(Style::default().fg(color).bg(Color::Black))
        .percent(percentage)
        .label(format!("{}% Cluttered", percentage));
    
    f.render_widget(gauge, sub_chunks[0].inner(ratatui::layout::Margin { horizontal: 1, vertical: 1 }));
}

fn render_results(f: &mut Frame, app: &mut App, area: Rect) {
    let list_items: Vec<ListItem> = app.rendered_rows.iter().map(|row| {
        match row {
            ResultRow::CategoryHeader(cat) => ListItem::new(Line::from(vec![
                Span::styled(format!("── {} ──", cat), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))
            ])),
            ResultRow::Item(idx) => {
                let i = &app.items[*idx];
                let checkbox = if i.selected { "[x] " } else { "[ ] " };
                let size = ByteSize(i.size_bytes).to_string();
                
                let status_style = match i.status {
                    ItemStatus::Deleted => Style::default().fg(Color::Green),
                    ItemStatus::Failed(_) => Style::default().fg(Color::Red),
                    _ => Style::default(),
                };
                
                let item_text = format!("{:<20} | {}", i.name, size);
                ListItem::new(Line::from(vec![
                    Span::raw("  "), 
                    Span::raw(checkbox),
                    Span::styled(item_text, status_style),
                ]))
            },
            ResultRow::EmptyLine => ListItem::new(""),
        }
    }).collect();

    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title(" Cleanable Items "))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, &mut app.state);
}

fn render_help(f: &mut Frame, _app: &App, area: Rect) {
    let help_text = vec![
        Line::from(vec![Span::styled("Navigation", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))]),
        Line::from("  Tab / l / Right : Next Tab"),
        Line::from("  BackTab / h / Left : Previous Tab"),
        Line::from("  1, 2, 3       : Quick Tab Access (Dashboard/Results/Help)"),
        Line::from("  j / Down      : Navigate List"),
        Line::from("  k / Up        : Navigate List"),
        Line::from(""),
        Line::from(vec![Span::styled("Actions", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))]),
        Line::from("  Space         : Toggle Selection"),
        Line::from("  d             : Toggle Dry-Run / Danger Mode"),
        Line::from("  Enter         : Clean Selected Items"),
        Line::from("  q             : Quit TidyTUI"),
        Line::from(""),
        Line::from(vec![Span::styled("About", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))]),
        Line::from("  TidyTUI is a blazingly fast system cleaner."),
        Line::from("  Always check 'Results' before pressing Enter in Danger mode!"),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title(" Help & Controls ").padding(Padding::uniform(1)));
    
    f.render_widget(help, area);
}
