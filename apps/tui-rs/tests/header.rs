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

    // Spinner glyph at ts=0 is "◐"; the label text must follow.
    assert!(
        ansi.contains("◐"),
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
    app.ts = 250; // selects spinner index 1 = "◓"

    let buffer = render_to_buffer(&mut app, 60, 56);
    let ansi = buffer_to_ansi(&buffer);

    assert!(
        ansi.contains("◓"),
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
        header.push_str(&buffer_symbol_at(&buffer, x, 1));
    }

    assert!(
        header.contains("adjusting"),
        "header at width=26 must show the lifecycle label; got: {header:?}",
    );
    assert!(
        header.contains('◐'),
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

    // The header line is row 1 (after marker line).
    let mut header = String::new();
    for x in 0..60 {
        header.push_str(&buffer_symbol_at(&buffer, x, 1));
    }
    assert!(
        !header.contains('◐'),
        "spinner must not appear when initializing=false; got header: {header:?}"
    );
    assert!(
        !header.contains("ignored"),
        "init_label must be hidden when initializing=false; got header: {header:?}"
    );
}
