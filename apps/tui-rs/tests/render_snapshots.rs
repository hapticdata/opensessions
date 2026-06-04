use opensessions_sidebar::app::App;
use opensessions_sidebar::generated::protocol::AgentStatus;
use opensessions_sidebar::snapshot::{
    buffer_bg_at, buffer_dimensions, buffer_symbol_at, buffer_to_ansi, render_to_buffer,
};

#[test]
fn matches_attached_session_list_ansi_snapshot() {
    assert_snapshot("pane-attached-session-list", 35, 56);
}

#[test]
fn matches_opensessions_self_ansi_snapshot() {
    assert_snapshot("pane-opensessions-self", 35, 55);
}

#[test]
fn matches_multi_window_ansi_snapshot() {
    assert_snapshot("pane-multi-window", 35, 56);
}

#[test]
fn render_to_buffer_exposes_ratatui_test_backend_cells() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    let buffer = render_to_buffer(&mut app, 35, 56);

    assert_eq!(buffer_dimensions(&buffer), (35, 56));
    assert_eq!(buffer_symbol_at(&buffer, 0, 0), "S");
    assert_eq!(buffer_symbol_at(&buffer, 1, 43), "─");
}

#[test]
fn temporary_session_focus_uses_weaker_marker_than_active_session() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.current_session = Some("opensessions".into());
    app.my_session = Some("opensessions".into());
    app.set_focused_session("plane-pdf-word-formatting");

    let buffer = render_to_buffer(&mut app, 35, 56);
    let active_row = row_containing(&buffer, 35, 56, "opensessions")
        .expect("active opensessions row should be visible");
    let temp_focus_row = row_containing(&buffer, 35, 56, "plane-pdf-word-formatting")
        .expect("temporary focused row should be visible");

    assert_eq!(buffer_symbol_at(&buffer, 0, active_row), "▌");
    assert_eq!(buffer_symbol_at(&buffer, 0, temp_focus_row), "›");
}

#[test]
fn collapsed_worktree_group_uses_active_marker_when_it_represents_current_session() {
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
            .expect("fixture worktree session should exist");
        session.dir = dir.into();
        session.is_worktree = true;
    }
    app.current_session = Some("plane-feat-edit-pages-from-pi".into());
    app.my_session = Some("plane-feat-edit-pages-from-pi".into());
    app.apply_server_message(
        opensessions_sidebar::generated::protocol::ServerMessage::State(
            opensessions_sidebar::generated::protocol::ServerState {
                sessions: app.sessions.clone(),
                focused_session: Some("plane-feat-edit-pages-from-pi".into()),
                current_session: Some("plane-feat-edit-pages-from-pi".into()),
                theme: app.theme.clone(),
                session_filter: Some(app.session_filter),
                sidebar_width: 26,
                initializing: false,
                init_label: None,
                collapsed_worktree_groups: vec!["/Users/me/work/plane-ee-wt".into()],
                ts: 1,
            },
        ),
    );

    let buffer = render_to_buffer(&mut app, 35, 56);
    let group_row = row_containing(&buffer, 35, 56, "plane-ee-wt")
        .expect("collapsed worktree group row should be visible");

    assert_eq!(buffer_symbol_at(&buffer, 0, group_row), "▌");
}

#[test]
fn focused_agent_row_uses_highlight_background() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.focus_agents_panel();
    app.focused_agent_idx = 0;
    let buffer = render_to_buffer(&mut app, 35, 55);

    let mut found_highlight = false;
    for y in 39..51 {
        if buffer_bg_at(&buffer, 1, y) == Some((69, 71, 90)) {
            found_highlight = true;
            break;
        }
    }
    assert!(
        found_highlight,
        "focused agent row must be highlighted in the detail panel"
    );
}

#[test]
fn focused_detail_panel_streams_agent_status_labels() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.set_focused_session("opensessions");
    let session = app
        .sessions
        .iter_mut()
        .find(|session| session.name == "opensessions")
        .expect("fixture should include opensessions");
    let mut running = session.agents[0].clone();
    running.status = AgentStatus::Running;
    running.thread_name = Some("Implement shim protocol".into());
    let mut waiting = running.clone();
    waiting.status = AgentStatus::Waiting;
    waiting.thread_name = Some("Waiting for approval".into());
    let mut error = running.clone();
    error.status = AgentStatus::Error;
    error.thread_name = Some("Fix failed build".into());
    session.agents = vec![running, waiting, error];
    app.focus_agents_panel();

    let buffer = render_to_buffer(&mut app, 35, 55);
    let ansi = buffer_to_ansi(&buffer);

    assert!(ansi.contains("working"));
    assert!(ansi.contains("blocked"));
    assert!(ansi.contains("error"));
    assert!(ansi.contains("Implement shim"));
}

#[test]
fn worktree_sessions_render_under_parent_group() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    let first = app
        .sessions
        .iter_mut()
        .find(|session| session.name == "plane-feat-edit-pages-from-pi")
        .expect("fixture should include plane worktree session");
    first.dir = "/Users/me/work/plane-ee-wt/feat-databases".into();
    first.is_worktree = true;
    let second = app
        .sessions
        .iter_mut()
        .find(|session| session.name == "plane-feat-background-exports")
        .expect("fixture should include second plane worktree session");
    second.dir = "/Users/me/work/plane-ee-wt/preview".into();
    second.is_worktree = true;

    let buffer = render_to_buffer(&mut app, 35, 56);
    let ansi = buffer_to_ansi(&buffer);

    assert!(ansi.contains("plane-ee-wt"));
    assert!(ansi.contains("2wt"));
    let mut found_second_worktree = false;
    for y in 0..20 {
        let mut row = String::new();
        for x in 0..35 {
            row.push_str(&buffer_symbol_at(&buffer, x, y));
        }
        if row.contains("02  plane-feat-background") {
            found_second_worktree = true;
            break;
        }
    }
    assert!(found_second_worktree);
}

#[test]
fn large_session_list_keeps_detail_and_footer_anchored() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    let template = app.sessions[1].clone();
    for idx in 0..100 {
        let mut session = template.clone();
        session.name = format!("extra-{idx}");
        session.dir = format!("/tmp/extra-{idx}");
        session.branch = "main".into();
        app.sessions.push(session);
    }

    let buffer = render_to_buffer(&mut app, 35, 56);

    assert_eq!(buffer_symbol_at(&buffer, 1, 43), "─");
    assert_eq!(buffer_symbol_at(&buffer, 1, 52), "─");
    assert_eq!(buffer_symbol_at(&buffer, 1, 53), "⇥");
}

#[test]
fn focused_session_far_down_list_is_scrolled_into_view() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    let template = app.sessions[1].clone();
    for idx in 0..50 {
        let mut session = template.clone();
        session.name = format!("extra-{idx}");
        session.dir = format!("/tmp/extra-{idx}");
        session.branch = "main".into();
        app.sessions.push(session);
    }
    app.set_focused_session("extra-40");

    let buffer = render_to_buffer(&mut app, 35, 56);

    // The sessions list lives between the header (rows 0..3) and the detail
    // separator at row 42. The focused row must be rendered with the highlight
    // background (SURFACE1 = 69,71,90) somewhere in that range.
    let mut found_focused_row = false;
    for y in 3..42 {
        if buffer_bg_at(&buffer, 5, y) == Some((69, 71, 90)) {
            // Reconstruct the row's text from the buffer to verify it is the
            // focused session, not a different highlighted row.
            let mut row = String::new();
            for x in 0..35 {
                row.push_str(&buffer_symbol_at(&buffer, x, y));
            }
            if row.contains("extra-40") {
                found_focused_row = true;
                break;
            }
        }
    }
    assert!(
        found_focused_row,
        "focused session row 'extra-40' must be rendered in the sessions list area",
    );
}

#[test]
fn sessions_list_renders_scroll_indicator_when_overflow() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    let template = app.sessions[1].clone();
    for idx in 0..50 {
        let mut session = template.clone();
        session.name = format!("extra-{idx}");
        session.dir = format!("/tmp/extra-{idx}");
        session.branch = "main".into();
        app.sessions.push(session);
    }
    app.set_focused_session("extra-40");

    let buffer = render_to_buffer(&mut app, 35, 56);

    let mut found_track = false;
    let mut found_thumb = false;
    for y in 0..43 {
        let glyph = buffer_symbol_at(&buffer, 34, y);
        if glyph == "│" {
            found_track = true;
        } else if glyph == "┃" {
            found_thumb = true;
        }
    }
    assert!(
        found_track,
        "overflowing sessions list must render a scrollbar track at the right edge",
    );
    assert!(
        found_thumb,
        "overflowing sessions list must render a scrollbar thumb at the right edge",
    );
}

#[test]
fn focused_agent_below_visible_window_scrolls_into_detail_view() {
    use opensessions_sidebar::generated::protocol::{AgentEvent, AgentLiveness, AgentStatus};

    let mut app = App::reference_fixture("pane-opensessions-self");
    let session = app
        .sessions
        .iter_mut()
        .find(|session| session.name == "opensessions")
        .expect("opensessions session in fixture");
    session.agents = (0..8)
        .map(|i| AgentEvent {
            agent: "amp".to_string(),
            session: "opensessions".to_string(),
            status: AgentStatus::Idle,
            ts: 0,
            thread_id: Some(format!("thread-{i}")),
            thread_name: Some(format!("Thread number {i:02}")),
            unseen: None,
            pane_id: None,
            liveness: Some(AgentLiveness::Alive),
        })
        .collect();
    app.focus_agents_panel();
    app.focused_agent_idx = 7;

    let buffer = render_to_buffer(&mut app, 35, 55);

    // Detail area for fixture pane-opensessions-self is lines [39..51].
    // The focused agent must render with the SURFACE1 highlight inside that
    // window even when it sits well below the natural visible region.
    let mut focused_row_text: Option<String> = None;
    for y in 39..51 {
        if buffer_bg_at(&buffer, 4, y) == Some((69, 71, 90)) {
            let mut row = String::new();
            for x in 0..35 {
                row.push_str(&buffer_symbol_at(&buffer, x, y));
            }
            focused_row_text = Some(row);
            break;
        }
    }
    assert!(
        focused_row_text.is_some(),
        "focused agent (idx 7) must be scrolled into the detail panel",
    );
    let mut visible_threads = String::new();
    for y in 39..51 {
        for x in 0..35 {
            visible_threads.push_str(&buffer_symbol_at(&buffer, x, y));
        }
        visible_threads.push('\n');
    }
    assert!(
        visible_threads.contains("Thread number 07"),
        "focused agent's thread label must appear in the visible detail area; got:\n{visible_threads}",
    );
}

#[test]
fn live_tui_draws_with_ratatui_backend() {
    let main_rs = include_str!("../src/main.rs");

    assert!(!main_rs.contains("snapshot"));
    assert!(!main_rs.contains("render_to_buffer"));
    assert!(!main_rs.contains("buffer_to_terminal_ansi"));
}

#[test]
fn live_tui_uses_quit_http_fallback_and_deadline() {
    let main_rs = include_str!("../src/main.rs");

    assert!(
        main_rs.contains("fire_quit_http"),
        "main.rs must fire the HTTP /quit fallback, mirroring fetch(`http://HOST:PORT/quit`)"
    );
    assert!(
        main_rs.contains("quit_deadline"),
        "main.rs must consult app.quit_deadline so the renderer is torn down within 500ms even if neither WS Quit nor HTTP /quit reaches the server"
    );
}

#[test]
fn live_tui_wires_runtime_context_helpers() {
    let main_rs = include_str!("../src/main.rs");

    assert!(
        main_rs.contains("resolve_endpoint_from_env"),
        "main.rs must resolve endpoint from env so tmux-derived ports are picked up"
    );
    assert!(
        main_rs.contains("pane_identity_resolve"),
        "main.rs must read TMUX_PANE + OPENSESSIONS_SESSION_NAME (with tmux display-message fallback) to identify the pane"
    );
    assert!(
        main_rs.contains("report_width_command"),
        "foreground sidebar pane resizes must report width so deliberate user drags update the server-owned sidebar width"
    );
    assert!(
        main_rs.contains("IdentifyPane"),
        "main.rs must send identify-pane to the server after connecting"
    );
}

fn assert_snapshot(name: &str, width: u16, height: u16) {
    let mut app = App::reference_fixture(name);
    let buffer = render_to_buffer(&mut app, width, height);
    let actual = buffer_to_ansi(&buffer);
    let expected = match name {
        "pane-attached-session-list" => include_str!(
            "../../../docs/ratatui-migration/reference-snapshots/pane-attached-session-list.ansi"
        ),
        "pane-opensessions-self" => include_str!(
            "../../../docs/ratatui-migration/reference-snapshots/pane-opensessions-self.ansi"
        ),
        "pane-multi-window" => include_str!(
            "../../../docs/ratatui-migration/reference-snapshots/pane-multi-window.ansi"
        ),
        _ => unreachable!(),
    };
    assert_eq!(actual, expected);
}

fn row_containing(
    buffer: &opensessions_sidebar::snapshot::RenderedBuffer,
    width: u16,
    height: u16,
    needle: &str,
) -> Option<u16> {
    for y in 0..height {
        let mut row = String::new();
        for x in 0..width {
            row.push_str(&buffer_symbol_at(buffer, x, y));
        }
        if row.contains(needle) {
            return Some(y);
        }
    }
    None
}
