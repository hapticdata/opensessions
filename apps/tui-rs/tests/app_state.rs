use opensessions_sidebar::app::{AgentPanelScope, App, Modal, PanelFocus};
use opensessions_sidebar::generated::protocol::{
    ClientCommand, FocusUpdate, ServerMessage, ServerState, SessionFilterMode,
};
use opensessions_sidebar::input::{UiKey, apply_ui_key};

#[test]
fn re_identify_message_resends_identify_pane_command() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.identify_pane(
        "%99".to_string(),
        "opensessions".to_string(),
        Some("@7".to_string()),
    );
    // Drain the initial IdentifyPane queued by identify_pane().
    let initial = app.drain_commands();
    assert_eq!(
        initial,
        vec![ClientCommand::IdentifyPane {
            pane_id: "%99".to_string(),
            session_name: "opensessions".to_string(),
            window_id: Some("@7".to_string()),
        }]
    );

    app.apply_server_message(ServerMessage::ReIdentify);

    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::IdentifyPane {
            pane_id: "%99".to_string(),
            session_name: "opensessions".to_string(),
            window_id: Some("@7".to_string()),
        }]
    );
}

#[test]
fn re_identify_without_stored_identity_emits_no_command() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.apply_server_message(ServerMessage::ReIdentify);
    assert!(app.drain_commands().is_empty());
}

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
        app.focused_session_name(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(app.panel_focus, PanelFocus::Sessions);
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::SwitchSession {
            name: "plane-pdf-word-formatting".into(),
            client_tty: None,
            debounce: Some(true),
        }]
    );
}

#[test]
fn number_key_queues_one_based_switch_index_command() {
    let mut app = App::reference_fixture("pane-attached-session-list");

    app.handle_key_char('2');

    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::SwitchSession {
            name: "plane-feat-background-exports".into(),
            client_tty: None,
            debounce: None,
        }]
    );
}

#[test]
fn navigation_keys_move_focus_locally_without_server_echo() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_focused_session("opensessions");

    app.move_focus(1);

    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(app.drain_commands(), Vec::<ClientCommand>::new());
}

#[test]
fn worktree_group_is_focusable_and_enter_toggles_collapse() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    let first = app
        .sessions
        .iter_mut()
        .find(|session| session.name == "plane-feat-edit-pages-from-pi")
        .unwrap();
    first.dir = "/Users/me/work/plane-ee-wt/feat-databases".into();
    first.is_worktree = true;
    let second = app
        .sessions
        .iter_mut()
        .find(|session| session.name == "plane-feat-background-exports")
        .unwrap();
    second.dir = "/Users/me/work/plane-ee-wt/preview".into();
    second.is_worktree = true;
    let group_key = "/Users/me/work/plane-ee-wt";

    app.set_focused_session("plane-feat-edit-pages-from-pi");
    app.move_focus(-1);

    assert_eq!(app.focused_group_key(), Some(group_key));
    assert_eq!(app.focused_session_name(), None);
    app.activate_focused_item();
    assert!(!app.is_group_collapsed(group_key));
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::ToggleWorktreeGroup {
            key: group_key.into(),
        }]
    );

    app.apply_server_message(ServerMessage::State(ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("plane-feat-edit-pages-from-pi".into()),
        current_session: app.current_session.clone(),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: vec![group_key.into()],
        ts: 1,
    }));
    assert!(app.is_group_collapsed(group_key));
    assert_eq!(app.focused_group_key(), Some(group_key));

    app.activate_focused_item();
    assert!(app.is_group_collapsed(group_key));
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::ToggleWorktreeGroup {
            key: group_key.into(),
        }]
    );
}

#[test]
fn collapsed_worktree_state_focuses_visible_group_when_server_focus_is_hidden_child() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    for (name, dir) in [
        (
            "plane-feat-edit-pages-from-pi",
            "/Users/me/work/plane-ee-wt/feat-databases",
        ),
        (
            "plane-feat-background-exports",
            "/Users/me/work/plane-ee-wt/preview",
        ),
    ] {
        let session = app
            .sessions
            .iter_mut()
            .find(|session| session.name == name)
            .unwrap();
        session.dir = dir.into();
        session.is_worktree = true;
    }
    let group_key = "/Users/me/work/plane-ee-wt";

    let state = ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("plane-feat-edit-pages-from-pi".into()),
        current_session: Some("plane-feat-edit-pages-from-pi".into()),
        theme: None,
        session_filter: Some(SessionFilterMode::All),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: vec![group_key.into()],
        ts: 1,
    };

    let app = App::from_state(state);

    assert_eq!(app.focused_group_key(), Some(group_key));
    assert_eq!(app.focused_session_name(), None);
}

#[test]
fn state_refresh_preserves_local_session_list_cursor() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_focused_session("opensessions");
    app.move_focus(1);
    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting")
    );

    let state = ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("opensessions".into()),
        current_session: Some("opensessions".into()),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: Vec::new(),
        ts: 1,
    };
    app.apply_server_message(ServerMessage::State(state));

    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(app.current_session.as_deref(), Some("opensessions"));
}

#[test]
fn agent_panel_navigation_and_actions_match_typescript_key_model() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.set_focused_session("opensessions");

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
                debounce: None,
            },
            ClientCommand::FocusAgentPane {
                session: "opensessions".into(),
                agent: "amp".into(),
                thread_id: None,
                thread_name: Some("Query tmux for open sessions".into()),
                pane_id: None,
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
                pane_id: None,
            },
        ]
    );
}

#[test]
fn agent_panel_actions_prefer_exact_pane_id_when_available() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.set_focused_session("opensessions");
    let agent = app
        .sessions
        .iter_mut()
        .find(|session| session.name == "opensessions")
        .and_then(|session| session.agents.first_mut())
        .expect("fixture should include an opensessions agent");
    agent.pane_id = Some("%agent".into());

    app.focus_agents_panel();
    app.activate_focused_item();
    app.kill_focused_agent_pane();

    let commands = app.drain_commands();
    assert!(commands.iter().any(|command| matches!(
        command,
        ClientCommand::FocusAgentPane { pane_id, .. } if pane_id.as_deref() == Some("%agent")
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ClientCommand::KillAgentPane { pane_id, .. } if pane_id.as_deref() == Some("%agent")
    )));
}

#[test]
fn agent_panel_scope_toggle_exposes_all_session_agents() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.set_focused_session("learning");

    app.handle_key_char('a');

    assert_eq!(app.agent_panel_scope, AgentPanelScope::All);
    app.focus_agents_panel();
    assert_eq!(app.panel_focus, PanelFocus::Agents);
}

#[test]
fn pane_focus_controls_switch_between_sessions_and_agents() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.set_focused_session("opensessions");

    app.focus_agents_panel();
    assert_eq!(app.panel_focus, PanelFocus::Agents);

    app.focus_sessions_panel();
    assert_eq!(app.panel_focus, PanelFocus::Sessions);
}

#[test]
fn extra_typescript_key_commands_are_available() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_focused_session("plane-pdf-word-formatting");

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
    app.set_focused_session("plane-pdf-word-formatting");

    app.activate_focused_session();

    assert_eq!(
        app.current_session.as_deref(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::SwitchSession {
            name: "plane-pdf-word-formatting".into(),
            client_tty: None,
            debounce: None,
        }]
    );
}

#[test]
fn action_keys_queue_basic_session_commands() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_focused_session("plane-pdf-word-formatting");

    app.handle_key_char('r');
    app.handle_key_char('n');
    app.handle_key_char('d');
    app.handle_key_char('x');

    // 'x' now opens a kill confirmation modal instead of killing immediately
    assert!(matches!(
        app.modal,
        Modal::KillConfirm { ref session_name } if session_name == "plane-pdf-word-formatting"
    ));

    assert_eq!(
        app.drain_commands(),
        vec![
            ClientCommand::Refresh,
            ClientCommand::NewSession,
            ClientCommand::HideSession {
                name: "plane-pdf-word-formatting".into()
            },
        ]
    );

    // Confirming with 'y' sends the KillSession command
    apply_ui_key(&mut app, UiKey::Char('y'));
    assert!(matches!(app.modal, Modal::None));
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::KillSession {
            name: "plane-pdf-word-formatting".into()
        }]
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
    assert_eq!(app.focused_session_name(), Some("learning"));
    assert_eq!(app.current_session.as_deref(), Some("learning"));
    assert_eq!(app.sessions.len(), 7);
}
