use crate::tui::app::{App, AppState, Tab};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

/// What the event loop should do after a key press has been applied to `App`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    StartCleaning,
    Rescan,
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::None;
    }

    match app.app_state {
        AppState::Viewing => handle_viewing(app, key.code),
        AppState::Confirming => handle_confirming(app, key.code),
        AppState::Cleaning { .. } => Action::None,
        AppState::Filtering => handle_filtering(app, key.code),
        AppState::Summary(_) => handle_summary(app, key.code),
    }
}

fn handle_filtering(app: &mut App, code: KeyCode) -> Action {
    match code {
        KeyCode::Enter => app.app_state = AppState::Viewing,
        KeyCode::Esc => {
            app.set_filter(String::new());
            app.app_state = AppState::Viewing;
        }
        KeyCode::Backspace => {
            let mut f = app.filter.clone();
            f.pop();
            app.set_filter(f);
        }
        KeyCode::Char(c) => {
            let mut f = app.filter.clone();
            f.push(c);
            app.set_filter(f);
        }
        _ => {}
    }
    Action::None
}

fn handle_summary(app: &mut App, code: KeyCode) -> Action {
    if matches!(code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q')) {
        app.app_state = AppState::Viewing;
        app.active_tab = Tab::Results;
    }
    Action::None
}

fn handle_viewing(app: &mut App, code: KeyCode) -> Action {
    match code {
        KeyCode::Char('q') => return Action::Quit,
        KeyCode::Char('d') => app.toggle_dry_run(),
        KeyCode::Char('1') => app.active_tab = Tab::Dashboard,
        KeyCode::Char('2') => app.active_tab = Tab::Results,
        KeyCode::Char('3') => app.active_tab = Tab::Help,
        KeyCode::Char('j') | KeyCode::Down => app.next(),
        KeyCode::Char('k') | KeyCode::Up => app.previous(),
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => app.next_tab(),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => app.previous_tab(),
        KeyCode::Char(' ') => app.toggle_selection(),
        KeyCode::Char('a') => app.select_all(true),
        KeyCode::Char('A') => app.select_all(false),
        KeyCode::Char('z') => app.toggle_collapse(),
        KeyCode::Char('Z') => app.toggle_collapse_all(),
        KeyCode::Char('s') => app.cycle_sort(),
        KeyCode::Char('r') => {
            if !app.is_scanning() {
                return Action::Rescan;
            }
        }
        KeyCode::Char('/') => {
            app.active_tab = Tab::Results;
            app.app_state = AppState::Filtering;
        }
        KeyCode::Esc => app.set_filter(String::new()),
        KeyCode::Enter => {
            if app.selected_count() > 0 && !app.is_scanning() {
                app.app_state = AppState::Confirming;
            }
        }
        _ => {}
    }
    Action::None
}

fn handle_confirming(app: &mut App, code: KeyCode) -> Action {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Action::StartCleaning,
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.app_state = AppState::Viewing;
            Action::None
        }
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CleanMode, CleanupItem, ItemStatus};
    use crate::tui::app::{CleanSummary, SortMode};
    use crossterm::event::KeyModifiers;
    use std::path::PathBuf;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn app_with_item() -> App {
        let mut app = App::new();
        app.set_items(vec![CleanupItem {
            group_id: String::new(),
            name: "a".into(),
            category: "A".into(),
            description: None,
            path: PathBuf::from("/x"),
            size_bytes: 1,
            file_count: 1,
            selected: false,
            status: ItemStatus::Scanned,
            mode: CleanMode::Contents,
            keep_days: None,
            locked: false,
        }]);
        app
    }

    #[test]
    fn q_quits() {
        assert_eq!(
            handle_key(&mut App::new(), key(KeyCode::Char('q'))),
            Action::Quit
        );
    }

    #[test]
    fn release_events_are_ignored() {
        let mut app = App::new();
        let mut k = key(KeyCode::Char('q'));
        k.kind = KeyEventKind::Release;
        assert_eq!(handle_key(&mut app, k), Action::None);
    }

    #[test]
    fn enter_without_selection_does_nothing() {
        let mut app = app_with_item();
        assert_eq!(handle_key(&mut app, key(KeyCode::Enter)), Action::None);
        assert_eq!(app.app_state, AppState::Viewing);
    }

    #[test]
    fn enter_with_selection_confirms_then_y_starts() {
        let mut app = app_with_item();
        handle_key(&mut app, key(KeyCode::Char(' ')));
        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.app_state, AppState::Confirming);
        assert_eq!(
            handle_key(&mut app, key(KeyCode::Char('y'))),
            Action::StartCleaning
        );
    }

    #[test]
    fn esc_cancels_confirm() {
        let mut app = app_with_item();
        app.app_state = AppState::Confirming;
        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.app_state, AppState::Viewing);
    }

    #[test]
    fn q_does_not_quit_while_confirming() {
        let mut app = app_with_item();
        app.app_state = AppState::Confirming;
        assert_eq!(handle_key(&mut app, key(KeyCode::Char('q'))), Action::None);
    }

    #[test]
    fn d_toggles_mode_and_tab_keys_switch() {
        let mut app = App::new();
        handle_key(&mut app, key(KeyCode::Char('d')));
        assert!(!app.dry_run);
        handle_key(&mut app, key(KeyCode::Char('2')));
        assert_eq!(app.active_tab, Tab::Results);
        handle_key(&mut app, key(KeyCode::Tab));
        assert_eq!(app.active_tab, Tab::Help);
        handle_key(&mut app, key(KeyCode::BackTab));
        assert_eq!(app.active_tab, Tab::Results);
    }

    #[test]
    fn j_k_navigate_between_header_and_item() {
        let mut app = app_with_item();
        // rows: [H(A), I0, Empty]
        assert_eq!(app.state.selected(), Some(1));
        handle_key(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.state.selected(), Some(0), "wraps onto the header");
        handle_key(&mut app, key(KeyCode::Up));
        assert_eq!(app.state.selected(), Some(1));
    }

    #[test]
    fn a_selects_all_and_shift_a_clears() {
        let mut app = app_with_item();
        handle_key(&mut app, key(KeyCode::Char('a')));
        assert_eq!(app.selected_count(), 1);
        handle_key(&mut app, key(KeyCode::Char('A')));
        assert_eq!(app.selected_count(), 0);
    }

    #[test]
    fn z_collapses_category_and_shift_z_collapses_all() {
        let mut app = app_with_item();
        handle_key(&mut app, key(KeyCode::Char('z')));
        assert!(app.collapsed.contains("A"));
        handle_key(&mut app, key(KeyCode::Char('Z')));
        assert!(
            app.collapsed.is_empty(),
            "Z toggles all off when any collapsed"
        );
    }

    #[test]
    fn s_cycles_sort_mode() {
        let mut app = app_with_item();
        handle_key(&mut app, key(KeyCode::Char('s')));
        assert_eq!(app.sort, SortMode::SizeDesc);
    }

    #[test]
    fn r_requests_rescan_only_when_idle() {
        let mut app = app_with_item();
        assert_eq!(
            handle_key(&mut app, key(KeyCode::Char('r'))),
            Action::Rescan
        );
        app.begin_scan(3);
        assert_eq!(handle_key(&mut app, key(KeyCode::Char('r'))), Action::None);
    }

    #[test]
    fn enter_is_ignored_while_scanning() {
        let mut app = app_with_item();
        app.items[0].selected = true;
        app.begin_scan(1);
        app.items.push(CleanupItem {
            group_id: String::new(),
            name: "b".into(),
            category: "A".into(),
            description: None,
            path: PathBuf::from("/y"),
            size_bytes: 1,
            file_count: 1,
            selected: true,
            status: ItemStatus::Scanned,
            mode: CleanMode::Contents,
            keep_days: None,
            locked: false,
        });
        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.app_state, AppState::Viewing);
    }

    #[test]
    fn slash_enters_filter_mode_and_typing_filters_live() {
        let mut app = app_with_item();
        handle_key(&mut app, key(KeyCode::Char('/')));
        assert_eq!(app.app_state, AppState::Filtering);
        assert_eq!(app.active_tab, Tab::Results);
        handle_key(&mut app, key(KeyCode::Char('z')));
        assert_eq!(app.filter, "z");
        assert!(app.visible_item_indices().is_empty());
        handle_key(&mut app, key(KeyCode::Backspace));
        assert_eq!(app.filter, "");
        handle_key(&mut app, key(KeyCode::Char('a')));
        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.app_state, AppState::Viewing);
        assert_eq!(app.filter, "a", "Enter keeps the filter");
    }

    #[test]
    fn esc_in_filter_mode_clears_filter() {
        let mut app = app_with_item();
        handle_key(&mut app, key(KeyCode::Char('/')));
        handle_key(&mut app, key(KeyCode::Char('a')));
        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.app_state, AppState::Viewing);
        assert_eq!(app.filter, "");
    }

    #[test]
    fn esc_in_viewing_clears_active_filter() {
        let mut app = app_with_item();
        app.set_filter("a".into());
        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.filter, "");
    }

    #[test]
    fn summary_dismisses_on_enter_esc_or_q() {
        for code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Char('q')] {
            let mut app = app_with_item();
            app.app_state = AppState::Summary(CleanSummary::default());
            assert_eq!(handle_key(&mut app, key(code)), Action::None);
            assert_eq!(app.app_state, AppState::Viewing);
        }
    }
}
