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
fn tab_switch_queues_next_visible_session_without_changing_confirmed_current_session() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.current_session = Some("opensessions".into());
    app.handle_tab(false);
    assert_eq!(app.current_session.as_deref(), Some("opensessions"));
    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting"),
        "tab sends immediate switch intent and shows the concrete pending target while the active row stays on the confirmed session",
    );
    assert_eq!(
        app.pending_switch_session.as_deref(),
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
fn session_switch_request_does_not_make_target_the_confirmed_active_session() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_pane_identity("%1".into(), "opensessions".into(), Some("@1".into()));
    app.set_focused_session("opensessions");

    app.click_session("learning".into());

    assert_eq!(
        app.current_session.as_deref(),
        Some("opensessions"),
        "the green/current row must stay on the confirmed tmux client session until tmux identifies the switched sidebar",
    );
    assert_eq!(
        app.focused_session_name(),
        Some("learning"),
        "a concrete session switch request should make the target row the pending focus while the active row stays on the confirmed session",
    );
    assert_eq!(app.pending_switch_session.as_deref(), Some("learning"));
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::SwitchSession {
            name: "learning".into(),
            client_tty: None,
            debounce: None,
        }]
    );
}

#[test]
fn local_pane_identity_overrides_stale_server_current_session_on_startup() {
    let mut app = App::from_state(ServerState {
        sessions: App::reference_fixture("pane-attached-session-list").sessions,
        focused_session: Some("opensessions".into()),
        current_session: Some("opensessions".into()),
        theme: None,
        session_filter: Some(SessionFilterMode::All),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: Vec::new(),
        ts: 1,
    });

    app.set_pane_identity("%2".into(), "learning".into(), Some("@2".into()));

    assert_eq!(app.current_session.as_deref(), Some("learning"));
    assert_eq!(app.my_session.as_deref(), Some("learning"));
    assert_eq!(
        app.focused_session_name(),
        Some("learning"),
        "a newly attached sidebar should render its own session before stale shared focus/current state",
    );
}

#[test]
fn shared_state_current_session_is_not_a_confirmed_client_active_session() {
    let app = App::from_state(ServerState {
        sessions: App::reference_fixture("pane-attached-session-list").sessions,
        focused_session: Some("opensessions".into()),
        current_session: Some("opensessions".into()),
        theme: None,
        session_filter: Some(SessionFilterMode::All),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: Vec::new(),
        ts: 1,
    });

    assert_eq!(
        app.current_session, None,
        "a shared server snapshot may include legacy currentSession, but the sidebar must not color an active row until local identity confirms it",
    );
    assert_eq!(
        app.focused_session_name(),
        Some("plane-feat-edit-pages-from-pi"),
        "legacy focusedSession must not seed this client's cursor before local identity arrives",
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
fn navigation_keys_move_temporary_session_selection_without_switching() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_focused_session("opensessions");

    app.move_focus(1);

    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting"),
        "keyboard navigation moves a temporary selection but does not switch tmux sessions",
    );
    assert_eq!(app.drain_commands(), Vec::<ClientCommand>::new());
}

#[test]
fn keyboard_navigation_can_focus_worktree_group_headers_and_enter_toggles_them() {
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

    app.current_session = Some("plane-feat-edit-pages-from-pi".into());
    app.my_session = Some("plane-feat-edit-pages-from-pi".into());
    app.set_focused_session("plane-feat-edit-pages-from-pi");
    app.move_focus(-1);

    assert_eq!(
        app.focused_group_key(),
        Some(group_key),
        "keyboard navigation should be able to land on a worktree group header so Enter can collapse it",
    );
    assert_eq!(app.focused_session_name(), None);
    assert_eq!(app.drain_commands(), Vec::<ClientCommand>::new());

    app.activate_focused_item();

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
    assert_eq!(
        app.focused_group_key(),
        Some(group_key),
        "if the active session is hidden by a collapsed group, focus the visible group representing it",
    );
    assert_eq!(app.focused_session_name(), None);
    assert!(app.is_group_collapsed(group_key));
    assert_eq!(app.drain_commands(), Vec::<ClientCommand>::new());
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
fn state_refresh_preserves_temporary_session_selection() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_focused_session("opensessions");
    app.move_focus(1);
    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting"),
        "keyboard navigation can keep a temporary selection until Enter, Tab, or local session confirmation resolves it",
    );
    assert_eq!(app.drain_commands(), Vec::<ClientCommand>::new());

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
        Some("plane-pdf-word-formatting"),
        "server state refresh must not stomp an in-progress temporary keyboard selection",
    );
    assert_eq!(app.current_session.as_deref(), Some("opensessions"));
}

#[test]
fn focus_broadcast_from_another_client_does_not_move_local_session_list_cursor() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_pane_identity("%2".into(), "learning".into(), Some("@2".into()));
    assert_eq!(app.focused_session_name(), Some("learning"));

    app.apply_server_message(ServerMessage::Focus(FocusUpdate {
        focused_session: Some("opensessions".into()),
        current_session: Some("opensessions".into()),
    }));

    assert_eq!(
        app.focused_session_name(),
        Some("learning"),
        "server focus is a cross-client hint; it must not overwrite this client's keyboard cursor",
    );
    assert_eq!(app.current_session.as_deref(), Some("learning"));
    assert_eq!(app.my_session.as_deref(), Some("learning"));
}

#[test]
fn your_session_confirmation_rehomes_focus_even_with_pane_identity() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_pane_identity("%2".into(), "opensessions".into(), Some("@2".into()));
    app.set_focused_session("plane-pdf-word-formatting");

    app.apply_server_message(ServerMessage::YourSession {
        name: "learning".into(),
        client_tty: Some("/dev/ttys002".into()),
    });

    assert_eq!(app.current_session.as_deref(), Some("learning"));
    assert_eq!(app.my_session.as_deref(), Some("learning"));
    assert_eq!(
        app.focused_session_name(),
        Some("learning"),
        "tmux confirmation resolves any temporary selection to the newly active local session",
    );
}

#[test]
fn state_refresh_preserves_agent_panel_when_session_focus_is_unchanged() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.set_pane_identity("%3".into(), "opensessions".into(), Some("@3".into()));
    app.focus_agents_panel();
    app.focused_agent_idx = 1;

    app.apply_server_message(ServerMessage::State(ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("learning".into()),
        current_session: Some("learning".into()),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: Vec::new(),
        ts: 2,
    }));

    assert_eq!(app.focused_session_name(), Some("opensessions"));
    assert_eq!(app.panel_focus, PanelFocus::Agents);
    assert_eq!(app.focused_agent_idx, 1);
}

#[test]
fn state_refresh_rehomes_missing_cursor_to_local_session_not_server_focus() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.set_pane_identity("%2".into(), "learning".into(), Some("@2".into()));
    app.set_focused_session("soon-to-be-hidden");

    app.apply_server_message(ServerMessage::State(ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("opensessions".into()),
        current_session: Some("opensessions".into()),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: Vec::new(),
        ts: 2,
    }));

    assert_eq!(
        app.focused_session_name(),
        Some("learning"),
        "when the local cursor disappears, recover to the local pane's session instead of shared server focus",
    );
    assert_eq!(app.current_session.as_deref(), Some("learning"));
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
fn enter_switches_temporary_selection_then_rehomes_focus_to_active_session() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.current_session = Some("opensessions".into());
    app.my_session = Some("opensessions".into());
    app.set_focused_session("plane-pdf-word-formatting");

    app.activate_focused_session();

    assert_eq!(app.current_session.as_deref(), Some("opensessions"));
    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting"),
        "Enter keeps the temporary selection visible as the pending switch target, avoiding an active-session snap-back frame",
    );
    assert_eq!(
        app.pending_switch_session.as_deref(),
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

    app.apply_server_message(ServerMessage::YourSession {
        name: "plane-pdf-word-formatting".into(),
        client_tty: Some("/dev/ttys002".into()),
    });

    assert_eq!(
        app.current_session.as_deref(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(app.pending_switch_session, None);
}

#[test]
fn background_source_sidebar_keeps_pending_on_focus_echo_then_clears_on_state_settle() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.current_session = Some("opensessions".into());
    app.my_session = Some("opensessions".into());
    app.set_focused_session("plane-pdf-word-formatting");

    app.activate_focused_session();
    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(
        app.pending_switch_session.as_deref(),
        Some("plane-pdf-word-formatting")
    );
    let _ = app.drain_commands();

    app.apply_server_message(ServerMessage::Focus(FocusUpdate {
        focused_session: Some("plane-pdf-word-formatting".into()),
        current_session: Some("plane-pdf-word-formatting".into()),
    }));

    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting"),
        "Focus is a fast intent echo and may arrive before tmux visibly switches; it must not clear pending or cause a snap-back frame",
    );
    assert_eq!(
        app.pending_switch_session.as_deref(),
        Some("plane-pdf-word-formatting")
    );

    app.apply_server_message(ServerMessage::State(ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("plane-pdf-word-formatting".into()),
        current_session: Some("plane-pdf-word-formatting".into()),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: Vec::new(),
        ts: 6,
    }));

    assert_eq!(
        app.focused_session_name(),
        Some("opensessions"),
        "the settled state snapshot can clean up the old source sidebar after the switch has moved the client away",
    );
    assert_eq!(app.current_session.as_deref(), Some("opensessions"));
    assert_eq!(app.pending_switch_session, None);
}

#[test]
fn state_focused_echo_without_current_settle_does_not_clear_pending_switch() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.current_session = Some("opensessions".into());
    app.my_session = Some("opensessions".into());
    app.set_focused_session("plane-pdf-word-formatting");

    app.activate_focused_session();
    let _ = app.drain_commands();

    app.apply_server_message(ServerMessage::State(ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("plane-pdf-word-formatting".into()),
        current_session: Some("opensessions".into()),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: Vec::new(),
        ts: 7,
    }));

    assert_eq!(
        app.focused_session_name(),
        Some("plane-pdf-word-formatting"),
        "focused_session inside State is still a shared focus echo; only current_session settling may clear local pending switch state",
    );
    assert_eq!(
        app.pending_switch_session.as_deref(),
        Some("plane-pdf-word-formatting")
    );
    assert_eq!(app.current_session.as_deref(), Some("opensessions"));
}

#[test]
fn missing_pending_session_is_cleared_before_focus_rehome() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.current_session = Some("opensessions".into());
    app.my_session = Some("opensessions".into());
    app.set_focused_session("plane-pdf-word-formatting");
    app.activate_focused_session();
    let _ = app.drain_commands();

    let sessions = app
        .sessions
        .iter()
        .filter(|session| session.name != "plane-pdf-word-formatting")
        .cloned()
        .collect();
    app.apply_server_message(ServerMessage::State(ServerState {
        sessions,
        focused_session: Some("plane-pdf-word-formatting".into()),
        current_session: Some("opensessions".into()),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: Vec::new(),
        ts: 8,
    }));

    assert_eq!(app.pending_switch_session, None);
    assert_eq!(app.focused_session_name(), Some("opensessions"));
}

#[test]
fn pending_worktree_child_uses_visible_group_surrogate_when_state_collapses_group() {
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
    app.current_session = Some("opensessions".into());
    app.my_session = Some("opensessions".into());
    app.set_focused_session("plane-feat-edit-pages-from-pi");

    app.activate_focused_session();
    assert_eq!(
        app.pending_switch_session.as_deref(),
        Some("plane-feat-edit-pages-from-pi")
    );
    let _ = app.drain_commands();

    app.apply_server_message(ServerMessage::State(ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("opensessions".into()),
        current_session: Some("opensessions".into()),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: vec![group_key.into()],
        ts: 3,
    }));

    assert_eq!(
        app.focused_group_key(),
        Some(group_key),
        "when a pending worktree child is hidden by collapse, focus should move to its visible group surrogate instead of snapping to the old active session",
    );
    assert_eq!(
        app.pending_switch_session.as_deref(),
        Some("plane-feat-edit-pages-from-pi")
    );
    assert_eq!(app.current_session.as_deref(), Some("opensessions"));
}

#[test]
fn clicking_expanded_worktree_child_replaces_group_focus_with_concrete_session_focus() {
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
    app.current_session = Some("plane-feat-edit-pages-from-pi".into());
    app.my_session = Some("plane-feat-edit-pages-from-pi".into());
    app.set_sidebar_focus(opensessions_sidebar::app::SidebarFocus::WorktreeGroup(
        group_key.into(),
    ));

    app.click_session("plane-feat-background-exports".into());

    assert_eq!(app.focused_group_key(), None);
    assert_eq!(
        app.focused_session_name(),
        Some("plane-feat-background-exports"),
        "once the user chooses a concrete session inside an expanded worktree group, the group header is no longer the pending focus",
    );
    assert_eq!(
        app.pending_switch_session.as_deref(),
        Some("plane-feat-background-exports")
    );
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::SwitchSession {
            name: "plane-feat-background-exports".into(),
            client_tty: None,
            debounce: None,
        }]
    );

    app.apply_server_message(ServerMessage::YourSession {
        name: "plane-feat-background-exports".into(),
        client_tty: Some("/dev/ttys003".into()),
    });

    assert_eq!(
        app.focused_session_name(),
        Some("plane-feat-background-exports")
    );
    assert_eq!(app.focused_group_key(), None);
    assert_eq!(app.pending_switch_session, None);
}

#[test]
fn expanded_worktree_group_surrogate_rehomes_to_concrete_active_child() {
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
    app.current_session = Some("plane-feat-background-exports".into());
    app.my_session = Some("plane-feat-background-exports".into());
    app.set_focused_session("plane-feat-background-exports");

    app.apply_server_message(ServerMessage::State(ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("plane-feat-background-exports".into()),
        current_session: Some("plane-feat-background-exports".into()),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: vec![group_key.into()],
        ts: 4,
    }));

    assert_eq!(
        app.focused_group_key(),
        Some(group_key),
        "collapsed group is a surrogate for the active hidden child",
    );

    app.apply_server_message(ServerMessage::State(ServerState {
        sessions: app.sessions.clone(),
        focused_session: Some("plane-feat-background-exports".into()),
        current_session: Some("plane-feat-background-exports".into()),
        theme: app.theme.clone(),
        session_filter: Some(app.session_filter),
        sidebar_width: 26,
        initializing: false,
        init_label: None,
        collapsed_worktree_groups: Vec::new(),
        ts: 5,
    }));

    assert_eq!(app.focused_group_key(), None);
    assert_eq!(
        app.focused_session_name(),
        Some("plane-feat-background-exports"),
        "when the group expands, surrogate group focus must resolve to the concrete active child row",
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
fn applies_your_session_and_ignores_focus_messages_without_replacing_sessions() {
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
    assert_eq!(app.focused_session_name(), Some("opensessions"));
    assert_eq!(app.current_session.as_deref(), Some("opensessions"));
    assert_eq!(app.sessions.len(), 7);
}
