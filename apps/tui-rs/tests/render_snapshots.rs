use opensessions_sidebar::app::App;
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
    assert_eq!(buffer_symbol_at(&buffer, 3, 1), "S");
    assert_eq!(buffer_symbol_at(&buffer, 1, 43), "─");
}

#[test]
fn focused_agent_row_uses_highlight_background() {
    let mut app = App::reference_fixture("pane-opensessions-self");
    app.focus_agents_panel();
    app.focused_agent_idx = 0;
    let buffer = render_to_buffer(&mut app, 35, 55);

    assert_eq!(buffer_bg_at(&buffer, 1, 41), Some((69, 71, 90)));
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
fn live_tui_draws_with_ratatui_backend() {
    let main_rs = include_str!("../src/main.rs");

    assert!(!main_rs.contains("snapshot"));
    assert!(!main_rs.contains("render_to_buffer"));
    assert!(!main_rs.contains("buffer_to_terminal_ansi"));
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
