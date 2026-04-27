use opensessions_sidebar::app::{App, PanelFocus};
use opensessions_sidebar::generated::protocol::{
    ClientCommand, FocusUpdate, ServerMessage, SessionFilterMode,
};

#[test]
fn resolve_synced_focus_keeps_background_sidebar_pinned_to_local_session() {
    assert_eq!(
        App::resolve_synced_focus(Some("alpha"), Some("alpha"), Some("beta")),
        Some("beta".into())
    );
    assert_eq!(
        App::resolve_synced_focus(Some("beta"), Some("beta"), Some("beta")),
        Some("beta".into())
    );
    assert_eq!(
        App::resolve_synced_focus(None, None, Some("beta")),
        Some("beta".into())
    );
}

#[test]
fn filters_sessions_and_omits_os_stash() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.session_filter = SessionFilterMode::Running;
    let names: Vec<_> = app
        .filtered_sessions()
        .map(|session| session.name.as_str())
        .collect();
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
    assert_eq!(
        app.current_session.as_deref(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(
        app.focused_session.as_deref(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(app.panel_focus, PanelFocus::Sessions);
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::SwitchSession {
            name: "plane-pdf-word-formatting".into(),
            client_tty: None
        }]
    );
}

#[test]
fn number_key_queues_one_based_switch_index_command() {
    let mut app = App::reference_fixture("pane-attached-session-list");

    app.handle_key_char('2');

    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::SwitchIndex { index: 2 }]
    );
}

#[test]
fn navigation_keys_move_focus_optimistically_and_queue_focus_command() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.focused_session = Some("opensessions".into());

    app.move_focus(1);

    assert_eq!(
        app.focused_session.as_deref(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::FocusSession {
            name: "plane-pdf-word-formatting".into()
        }]
    );
}

#[test]
fn agent_panel_navigation_and_actions_match_typescript_key_model() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.focused_session = Some("opensessions".into());

    app.focus_agents_panel();
    assert_eq!(app.panel_focus, PanelFocus::Agents);

    app.move_agent_focus(1);
    assert_eq!(app.focused_agent_idx, 1);

    app.move_agent_focus(-1);
    assert_eq!(app.focused_agent_idx, 0);

    app.activate_focused_item();
    app.dismiss_focused_agent();
    app.kill_focused_agent_pane();

    assert_eq!(
        app.drain_commands(),
        vec![
            ClientCommand::SwitchSession {
                name: "opensessions".into(),
                client_tty: None,
            },
            ClientCommand::FocusAgentPane {
                session: "opensessions".into(),
                agent: "amp".into(),
                thread_id: None,
                thread_name: Some("Query tmux for open sessions".into()),
            },
            ClientCommand::DismissAgent {
                session: "opensessions".into(),
                agent: "amp".into(),
                thread_id: None,
            },
            ClientCommand::KillAgentPane {
                session: "opensessions".into(),
                agent: "amp".into(),
                thread_id: None,
                thread_name: Some("Query tmux for open sessions".into()),
            },
        ]
    );
}

#[test]
fn pane_focus_controls_switch_between_sessions_and_agents() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.focused_session = Some("opensessions".into());

    app.focus_agents_panel();
    assert_eq!(app.panel_focus, PanelFocus::Agents);

    app.focus_sessions_panel();
    assert_eq!(app.panel_focus, PanelFocus::Sessions);
}

#[test]
fn extra_typescript_key_commands_are_available() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.focused_session = Some("plane-pdf-word-formatting".into());

    app.handle_key_char('u');
    app.handle_key_char('c');
    app.reorder_focused_session(-1);
    app.reorder_focused_session(1);

    assert_eq!(
        app.drain_commands(),
        vec![
            ClientCommand::ShowAllSessions,
            ClientCommand::NewSession,
            ClientCommand::ReorderSession {
                name: "plane-pdf-word-formatting".into(),
                delta: -1,
            },
            ClientCommand::ReorderSession {
                name: "plane-pdf-word-formatting".into(),
                delta: 1,
            },
        ]
    );
}

#[test]
fn enter_switches_to_focused_session() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.focused_session = Some("plane-pdf-word-formatting".into());

    app.activate_focused_session();

    assert_eq!(
        app.current_session.as_deref(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::SwitchSession {
            name: "plane-pdf-word-formatting".into(),
            client_tty: None
        }]
    );
}

#[test]
fn action_keys_queue_basic_session_commands() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.focused_session = Some("plane-pdf-word-formatting".into());

    app.handle_key_char('r');
    app.handle_key_char('n');
    app.handle_key_char('d');
    app.handle_key_char('x');

    assert_eq!(
        app.drain_commands(),
        vec![
            ClientCommand::Refresh,
            ClientCommand::NewSession,
            ClientCommand::HideSession {
                name: "plane-pdf-word-formatting".into()
            },
            ClientCommand::KillSession {
                name: "plane-pdf-word-formatting".into()
            },
        ]
    );
}

#[test]
fn filter_key_cycles_filter_modes_and_queues_set_filter() {
    let mut app = App::reference_fixture("pane-attached-session-list");

    app.handle_key_char('f');
    app.handle_key_char('f');
    app.handle_key_char('f');

    assert_eq!(
        app.drain_commands(),
        vec![
            ClientCommand::SetFilter {
                filter: SessionFilterMode::Active
            },
            ClientCommand::SetFilter {
                filter: SessionFilterMode::Running
            },
            ClientCommand::SetFilter {
                filter: SessionFilterMode::All
            },
        ]
    );
}

#[test]
fn applies_focus_and_your_session_messages_without_replacing_sessions() {
    let mut app = App::reference_fixture("pane-attached-session-list");

    app.apply_server_message(ServerMessage::YourSession {
        name: "opensessions".into(),
        client_tty: Some("/dev/ttys001".into()),
    });
    app.apply_server_message(ServerMessage::Focus(FocusUpdate {
        focused_session: Some("learning".into()),
        current_session: Some("learning".into()),
    }));

    assert_eq!(app.my_session.as_deref(), Some("opensessions"));
    assert_eq!(app.focused_session.as_deref(), Some("learning"));
    assert_eq!(app.current_session.as_deref(), Some("learning"));
    assert_eq!(app.sessions.len(), 7);
}
