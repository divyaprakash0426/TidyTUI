use crate::tui::app::{App, AppState, Tab};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

/// What the event loop should do after a key press has been applied to `App`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    StartCleaning,
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::None;
    }

    match app.app_state {
        AppState::Viewing => handle_viewing(app, key.code),
        AppState::Confirming => handle_confirming(app, key.code),
        AppState::Cleaning { .. } => Action::None,
    }
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
        KeyCode::Enter => {
            if app.selected_count() > 0 {
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
    use crossterm::event::KeyModifiers;
    use std::path::PathBuf;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn app_with_item() -> App {
        let mut app = App::new();
        app.set_items(vec![CleanupItem {
            name: "a".into(),
            category: "A".into(),
            description: None,
            path: PathBuf::from("/x"),
            size_bytes: 1,
            file_count: 1,
            selected: false,
            status: ItemStatus::Scanned,
            mode: CleanMode::Contents,
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
    fn j_k_navigate_items() {
        let mut app = app_with_item();
        let start = app.state.selected();
        handle_key(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.state.selected(), start, "single item wraps to itself");
        handle_key(&mut app, key(KeyCode::Up));
        assert_eq!(app.state.selected(), start);
    }
}
