use opensessions_sidebar::app::App;
use opensessions_sidebar::snapshot::{buffer_symbol_at, buffer_to_ansi, render_to_buffer};

#[test]
fn header_renders_init_label_with_spinner_when_initializing() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.initializing = true;
    app.init_label = Some("loading sessions".into());
    app.ts = 0;

    let buffer = render_to_buffer(&mut app, 60, 56);
    let ansi = buffer_to_ansi(&buffer);

    // Loader uses the same braille spinner as running agents.
    assert!(
        ansi.contains("⠋"),
        "header must include a spinner glyph while initializing; got:\n{ansi}"
    );
    assert!(
        ansi.contains("loading sessions"),
        "header must include the init_label string; got:\n{ansi}"
    );
}

#[test]
fn header_falls_back_to_warming_up_when_init_label_missing() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.initializing = true;
    app.init_label = None;
    app.ts = 250; // selects agent spinner index 2 = "⠹"

    let buffer = render_to_buffer(&mut app, 60, 56);
    let ansi = buffer_to_ansi(&buffer);

    assert!(
        ansi.contains("⠹"),
        "spinner must advance frame with ts; got:\n{ansi}"
    );
    assert!(
        ansi.contains("warming up"),
        "missing init_label must fall back to 'warming up…'; got:\n{ansi}"
    );
}

#[test]
fn header_at_narrow_width_preserves_short_init_label() {
    // The sidebar behavior contract says warming/adjusting states must be
    // visible while spawn/width convergence is in flight. At the common 26-col
    // fallback width, prefer the init label over secondary counters.
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.initializing = true;
    app.init_label = Some("adjusting…".into());
    app.ts = 0;

    let buffer = render_to_buffer(&mut app, 26, 10);

    let mut header = String::new();
    for x in 0..26 {
        header.push_str(&buffer_symbol_at(&buffer, x, 0));
    }

    assert!(
        header.contains("adjusting"),
        "header at width=26 must show the lifecycle label; got: {header:?}",
    );
    assert!(
        header.contains('⠋'),
        "header at width=26 must show the spinner with the label; got: {header:?}",
    );
    assert!(
        header.contains("Sessions"),
        "header must still show the Sessions label even at narrow widths; \
         got: {header:?}",
    );
}

#[test]
fn header_omits_spinner_when_not_initializing() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.initializing = false;
    app.init_label = Some("ignored".into());
    app.ts = 0;

    let buffer = render_to_buffer(&mut app, 60, 56);

    let mut header = String::new();
    for x in 0..60 {
        header.push_str(&buffer_symbol_at(&buffer, x, 0));
    }
    assert!(
        !header.contains('⠋'),
        "spinner must not appear when initializing=false; got header: {header:?}"
    );
    assert!(
        !header.contains("ignored"),
        "init_label must be hidden when initializing=false; got header: {header:?}"
    );
}

#[test]
fn empty_initializing_session_list_renders_inline_loader_rows() {
    let mut app = App::reference_fixture("pane-attached-session-list");
    app.sessions.clear();
    app.initializing = true;
    app.init_label = Some("warming up…".into());
    app.ts = 0;

    let buffer = render_to_buffer(&mut app, 35, 12);

    let mut first_loader = String::new();
    let mut second_loader = String::new();
    for x in 0..35 {
        first_loader.push_str(&buffer_symbol_at(&buffer, x, 2));
        second_loader.push_str(&buffer_symbol_at(&buffer, x, 3));
    }

    assert!(
        first_loader.contains("   ⠋  warming up"),
        "session loader must align with the list rail and use the agent spinner; got: {first_loader:?}"
    );
    assert!(
        second_loader.contains("      reading tmux + git"),
        "session loader must show the secondary loading step; got: {second_loader:?}"
    );
}
