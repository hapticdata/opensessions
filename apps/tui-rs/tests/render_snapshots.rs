use opensessions_sidebar::app::App;
use opensessions_sidebar::snapshot::{buffer_to_ansi, render_to_buffer};

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

fn assert_snapshot(name: &str, width: u16, height: u16) {
    let mut app = App::reference_fixture(name);
    let buffer = render_to_buffer(&mut app, width, height);
    let actual = buffer_to_ansi(&buffer);
    let expected = match name {
        "pane-attached-session-list" => include_str!("../../../docs/ratatui-migration/reference-snapshots/pane-attached-session-list.ansi"),
        "pane-opensessions-self" => include_str!("../../../docs/ratatui-migration/reference-snapshots/pane-opensessions-self.ansi"),
        "pane-multi-window" => include_str!("../../../docs/ratatui-migration/reference-snapshots/pane-multi-window.ansi"),
        _ => unreachable!(),
    };
    assert_eq!(actual, expected);
}
