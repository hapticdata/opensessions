use opensessions_sidebar::app::{AgentPanelScope, App, PanelFocus};
use opensessions_sidebar::generated::protocol::ClientCommand;
use opensessions_sidebar::input::{UiMouse, apply_ui_mouse};
use opensessions_sidebar::renderer::{
    HitTarget, compute_hit_map, compute_hit_target, detail_separator_row,
};
use opensessions_sidebar::snapshot::{buffer_bg_at, buffer_symbol_at, render_to_buffer};

const W: u16 = 35;
const H: u16 = 56;
const HOVER_BG: (u8, u8, u8) = (88, 91, 112);

#[test]
fn scroll_down_in_sessions_panel_moves_viewport_without_queueing_focus_command() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    let template = app.sessions[1].clone();
    for idx in 0..30 {
        let mut session = template.clone();
        session.name = format!("extra-{idx}");
        session.dir = format!("/tmp/extra-{idx}");
        session.branch = "main".into();
        app.sessions.push(session);
    }
    app.set_focused_session("plane-feat-edit-pages-from-pi");

    apply_ui_mouse(
        &mut app,
        UiMouse::ScrollDown {
            x: 5,
            y: 5,
            width: W,
            height: H,
        },
    );

    assert_eq!(app.session_scroll_offset(), 1);
    assert_eq!(
        app.focused_session_name(),
        Some("plane-feat-edit-pages-from-pi")
    );
    assert!(app.drain_commands().is_empty());

    let buffer = render_to_buffer(&mut app, W, H);
    let mut visible_session_rows = String::new();
    for y in 0..detail_separator_row(&app, W, H) {
        for x in 0..W {
            visible_session_rows.push_str(&buffer_symbol_at(&buffer, x, y));
        }
        visible_session_rows.push('\n');
    }
    assert!(
        visible_session_rows.contains("plane-feat-background-expo"),
        "wheel scroll should move the viewport immediately instead of forcing the focused row back into view; got {visible_session_rows:?}",
    );
}

#[test]
fn scroll_up_in_agents_panel_moves_agent_focus() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.focus_agents_panel();
    app.focused_agent_idx = 1;

    apply_ui_mouse(
        &mut app,
        UiMouse::ScrollUp {
            x: 5,
            y: 45,
            width: W,
            height: H,
        },
    );

    assert_eq!(app.focused_agent_idx, 0);
    assert_eq!(app.panel_focus, PanelFocus::Agents);
}

#[test]
fn compute_hit_map_marks_session_rows_with_session_targets() {
    let app = App::reference_fixture("pane-attached-session-list");
    let hits = compute_hit_map(&app, W, H);

    assert_eq!(hits.len(), H as usize);

    // Find a row that maps to "opensessions".
    let opensessions_row = hits
        .iter()
        .position(|hit| matches!(hit, Some(HitTarget::Session(name)) if name == "opensessions"));
    assert!(
        opensessions_row.is_some(),
        "hit map must mark at least one row as the opensessions session card",
    );

    // Find a row mapped to plane-pdf-word-formatting (the focused session in this fixture).
    let pdf_row = hits.iter().position(
        |hit| matches!(hit, Some(HitTarget::Session(name)) if name == "plane-pdf-word-formatting"),
    );
    assert!(
        pdf_row.is_some(),
        "hit map must include the focused session card rows",
    );
}

#[test]
fn click_on_session_row_queues_switch_without_changing_confirmed_active_session() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.current_session = Some("opensessions".into());
    app.set_focused_session("opensessions");

    let hits = compute_hit_map(&app, W, H);
    let target_row = hits
        .iter()
        .position(|hit| matches!(hit, Some(HitTarget::Session(name)) if name == "learning"))
        .expect("learning session must have a clickable row");

    apply_ui_mouse(
        &mut app,
        UiMouse::Click {
            x: 5,
            y: target_row as u16,
            width: W,
            height: H,
        },
    );

    assert_eq!(
        app.focused_session_name(),
        Some("learning"),
        "mouse clicks request a switch and make the clicked session the pending concrete focus target",
    );
    assert_eq!(app.pending_switch_session.as_deref(), Some("learning"));
    assert_eq!(
        app.current_session.as_deref(),
        Some("opensessions"),
        "mouse clicks request a tmux switch, but only pane identity / your-session confirms the active row",
    );
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
fn click_on_diff_count_launches_lazydiffs_for_that_session() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.current_session = Some("opensessions".into());
    app.my_session = Some("opensessions".into());
    app.set_focused_session("opensessions");
    let session = app
        .sessions
        .iter_mut()
        .find(|session| session.name == "learning")
        .expect("fixture must include learning session");
    session.changed_files = 3;
    session.insertions = 4;
    session.deletions = 10;

    let mut target = None;
    for y in 0..H {
        for x in 0..W {
            if compute_hit_target(&app, x, y, W, H) == Some(HitTarget::DiffCount("learning".into()))
            {
                target = Some((x, y));
                break;
            }
        }
        if target.is_some() {
            break;
        }
    }
    let (x, y) = target.expect("changed-file count must be a clickable target");

    apply_ui_mouse(
        &mut app,
        UiMouse::Click {
            x,
            y,
            width: W,
            height: H,
        },
    );

    assert_eq!(
        app.focused_session_name(),
        Some("opensessions"),
        "diff-count clicks launch for the clicked session without moving durable session focus",
    );
    assert!(app.drain_commands().is_empty());
    let launches = app.drain_launches();
    assert_eq!(launches.len(), 1);
    assert_eq!(launches[0].session_name(), Some("learning"));
}

#[test]
fn hover_on_diff_count_highlights_diff_stats() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    let session = app
        .sessions
        .iter_mut()
        .find(|session| session.name == "learning")
        .expect("fixture must include learning session");
    session.insertions = 4;
    session.deletions = 10;

    let mut target = None;
    for y in 0..H {
        for x in 0..W {
            if compute_hit_target(&app, x, y, W, H) == Some(HitTarget::DiffCount("learning".into()))
            {
                target = Some((x, y));
                break;
            }
        }
        if target.is_some() {
            break;
        }
    }
    let (x, y) = target.expect("diff stats must expose a hover target");

    apply_ui_mouse(
        &mut app,
        UiMouse::Move {
            x,
            y,
            width: W,
            height: H,
        },
    );

    let buffer = render_to_buffer(&mut app, W, H);
    let plus_x = (0..W)
        .find(|candidate| buffer_symbol_at(&buffer, *candidate, y) == "+")
        .expect("hovered diff row must render a plus sign");
    assert_eq!(buffer_bg_at(&buffer, plus_x, y), Some(HOVER_BG));
}

#[test]
fn click_on_agent_scope_label_toggles_between_current_and_all() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.set_focused_session("opensessions");

    let mut target = None;
    for y in 0..H {
        for x in 0..W {
            if compute_hit_target(&app, x, y, W, H) == Some(HitTarget::AgentScopeToggle) {
                target = Some((x, y));
                break;
            }
        }
        if target.is_some() {
            break;
        }
    }
    let (x, y) = target.expect("agent scope label must be clickable");

    apply_ui_mouse(
        &mut app,
        UiMouse::Click {
            x,
            y,
            width: W,
            height: H,
        },
    );

    assert_eq!(app.agent_panel_scope, AgentPanelScope::All);
}

#[test]
fn click_on_worktree_group_toggles_like_enter() {
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

    let target = (0..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .find(|(x, y)| {
            compute_hit_target(&app, *x, *y, W, H) == Some(HitTarget::Group(group_key.into()))
        })
        .expect("worktree group header should be clickable");

    apply_ui_mouse(
        &mut app,
        UiMouse::Click {
            x: target.0,
            y: target.1,
            width: W,
            height: H,
        },
    );
    assert_eq!(app.focused_group_key(), Some(group_key));
    assert!(!app.is_group_collapsed(group_key));
    assert_eq!(
        app.drain_commands(),
        vec![ClientCommand::ToggleWorktreeGroup {
            key: group_key.into(),
        }]
    );
}

#[test]
fn click_on_agent_row_focuses_agents_panel_and_switches_pane() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.set_focused_session("opensessions");

    let w: u16 = 35;
    let h: u16 = 55;
    let hits = compute_hit_map(&app, w, h);
    let agent_row = hits
        .iter()
        .position(|hit| matches!(hit, Some(HitTarget::Agent(1))))
        .expect("second agent row must be clickable in the detail panel");

    apply_ui_mouse(
        &mut app,
        UiMouse::Click {
            x: 5,
            y: agent_row as u16,
            width: w,
            height: h,
        },
    );

    assert_eq!(app.panel_focus, PanelFocus::Agents);
    assert_eq!(app.focused_agent_idx, 1);

    let commands = app.drain_commands();
    assert!(
        commands.iter().any(|cmd| matches!(
            cmd,
            ClientCommand::SwitchSession { name, .. } if name == "opensessions"
        )),
        "click on agent must switch to its session; got: {commands:?}"
    );
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, ClientCommand::FocusAgentPane { session, .. } if session == "opensessions")),
        "click on agent must queue focus-agent-pane for that agent; got: {commands:?}"
    );
}

#[test]
fn click_outside_any_target_is_a_noop() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    let drained_before = app.drain_commands().len();

    apply_ui_mouse(
        &mut app,
        UiMouse::Click {
            x: 0,
            y: 0,
            width: W,
            height: H,
        },
    );

    let drained_after = app.drain_commands().len();
    assert_eq!(drained_before, 0);
    assert_eq!(drained_after, 0);
}

#[test]
fn detail_separator_row_starts_drag_resize() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.detail_panel_height = 10;
    let separator_row = detail_separator_row(&app, W, H);

    apply_ui_mouse(
        &mut app,
        UiMouse::Click {
            x: 5,
            y: separator_row,
            width: W,
            height: H,
        },
    );
    apply_ui_mouse(
        &mut app,
        UiMouse::Drag {
            y: separator_row.saturating_sub(2),
        },
    );
    apply_ui_mouse(&mut app, UiMouse::DragEnd);

    assert_eq!(app.detail_panel_height, 12);
    assert!(app.resize_drag_state.is_none());
}

#[test]
fn live_tui_enables_mouse_capture_and_handles_mouse_events() {
    let main_rs = include_str!("../src/main.rs");
    assert!(
        main_rs.contains("EnableMouseCapture"),
        "main.rs must enable mouse capture so the user can click sessions and scroll"
    );
    assert!(
        main_rs.contains("DisableMouseCapture"),
        "TerminalGuard must disable mouse capture on drop to restore the parent terminal"
    );
    assert!(
        main_rs.contains("Event::Mouse"),
        "main.rs must dispatch crossterm Event::Mouse to apply_ui_mouse"
    );
    assert!(
        main_rs.contains("apply_ui_mouse"),
        "main.rs must route mouse events through the shared apply_ui_mouse helper"
    );
}
