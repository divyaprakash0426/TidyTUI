use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
    Frame,
};

fn heading(text: &str) -> Line<'_> {
    Line::from(Span::styled(
        text,
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Cyan),
    ))
}

pub fn render(f: &mut Frame, area: Rect) {
    let help_text = vec![
        heading("Navigation"),
        Line::from("  Tab / l / Right    : Next tab"),
        Line::from("  BackTab / h / Left : Previous tab"),
        Line::from("  1, 2, 3            : Jump to Dashboard / Results / Help"),
        Line::from("  j / Down, k / Up   : Move through the list (headers included)"),
        Line::from(""),
        heading("Selection"),
        Line::from(
            "  Space              : Toggle item — on a category header, toggle the whole category",
        ),
        Line::from("  a / A              : Select all / none (respects the active filter)"),
        Line::from(""),
        heading("View"),
        Line::from("  z / Z              : Fold the highlighted category / fold or unfold all"),
        Line::from("  /                  : Filter by name or path (Enter keeps, Esc clears)"),
        Line::from("  s                  : Cycle sort: default → size → name"),
        Line::from("  r                  : Rescan"),
        Line::from(""),
        heading("Actions"),
        Line::from("  d                  : Toggle Dry-Run / Danger mode"),
        Line::from("  Enter              : Clean selected items (asks for confirmation)"),
        Line::from("  y / n, Esc         : Confirm / cancel in the confirmation dialog"),
        Line::from("  q                  : Quit TidyTUI"),
        Line::from(""),
        heading("Legend"),
        Line::from(vec![
            Span::raw("  [x]  selected      "),
            Span::styled("would delete", Style::default().fg(Color::Cyan)),
            Span::raw("  simulated in dry-run      "),
            Span::styled("deleted", Style::default().fg(Color::Green)),
            Span::raw("  removed      "),
            Span::styled("failed: …", Style::default().fg(Color::Red)),
            Span::raw("  could not remove"),
        ]),
        Line::from(""),
        heading("Cleaning modes (definitions.yaml)"),
        Line::from("  contents (default) : empties the folder but keeps it"),
        Line::from("  dir                : removes the folder itself"),
        Line::from(""),
        heading("About"),
        Line::from("  TidyTUI is a blazingly fast system cleaner."),
        Line::from("  Always check 'Results' before pressing Enter in Danger mode!"),
    ];

    let help = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help & Controls ")
            .padding(Padding::uniform(1)),
    );
    f.render_widget(help, area);
}
