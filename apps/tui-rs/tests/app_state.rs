use opensessions_sidebar::app::{App, PanelFocus};
use opensessions_sidebar::generated::protocol::{ClientCommand, SessionFilterMode};

#[test]
fn resolve_synced_focus_keeps_background_sidebar_pinned_to_local_session() {
    assert_eq!(App::resolve_synced_focus(Some("alpha"), Some("alpha"), Some("beta")), Some("beta".into()));
    assert_eq!(App::resolve_synced_focus(Some("beta"), Some("beta"), Some("beta")), Some("beta".into()));
    assert_eq!(App::resolve_synced_focus(None, None, Some("beta")), Some("beta".into()));
}

#[test]
fn filters_sessions_and_omits_os_stash() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.session_filter = SessionFilterMode::Running;
    let names: Vec<_> = app.filtered_sessions().map(|session| session.name.as_str()).collect();
    assert_eq!(names, vec!["opensessions"]);
}

#[test]
fn q_key_starts_quit_sequence_and_queues_quit_command() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.handle_key_char('q');
    assert!(app.quit_deadline.is_some());
    assert_eq!(app.drain_commands(), vec![ClientCommand::Quit]);
}

#[test]
fn tab_switches_to_next_visible_session_optimistically() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.current_session = Some("opensessions".into());
    app.handle_tab(false);
    assert_eq!(app.current_session.as_deref(), Some("plane-pdf-word-formatting"));
    assert_eq!(app.focused_session.as_deref(), Some("plane-pdf-word-formatting"));
    assert_eq!(app.panel_focus, PanelFocus::Sessions);
    assert_eq!(app.drain_commands(), vec![ClientCommand::SwitchSession { name: "plane-pdf-word-formatting".into(), client_tty: None }]);
}
