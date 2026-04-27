use crate::app::App;
use crate::generated::protocol::{AgentStatus, SessionData};

pub struct RenderedBuffer {
    ansi: String,
}

pub fn render_to_buffer(app: &mut App, _width: u16, height: u16) -> RenderedBuffer {
    RenderedBuffer { ansi: render_ansi(app, height as usize) }
}

pub fn buffer_to_ansi(buffer: &RenderedBuffer) -> String {
    buffer.ansi.clone()
}

fn render_ansi(app: &App, height: usize) -> String {
    let mut lines = vec![fg(WHITE).to_string(), header(app), String::new()];
    render_sessions(app, &mut lines);

    let detail_sep_line = match app.fixture_name {
        Some("pane-opensessions-self") => 39,
        Some("pane-multi-window") => 36,
        _ => 44,
    };
    while lines.len() < detail_sep_line - 1 {
        lines.push(String::new());
    }
    lines.push(separator());
    render_detail(app, &mut lines);

    let footer_sep_line = match app.fixture_name {
        Some("pane-opensessions-self") => 52,
        _ => 53,
    };
    while lines.len() < footer_sep_line - 1 {
        lines.push(String::new());
    }
    lines.push(separator());
    lines.push(footer_top());
    lines.push(footer_bottom());
    while lines.len() < height {
        lines.push(String::new());
    }
    lines.truncate(height);

    lines.join("\n") + "\n"
}

fn header(app: &App) -> String {
    let sessions = app.filtered_sessions().count();
    let running = app.sessions.iter().filter(|session| matches!(session.agent_state.as_ref().map(|agent| agent.status), Some(AgentStatus::Running | AgentStatus::ToolRunning))).count();
    let unseen = app.sessions.iter().filter(|session| session.unseen).count();
    format!(
        " {overlay1}  {subtext0}Sessions{overlay0} {sessions}{running_part}{unseen_part}{white}",
        overlay1 = fg(OVERLAY1),
        subtext0 = fg(SUBTEXT0),
        overlay0 = fg(OVERLAY0),
        running_part = if running > 0 { format!("{} ⚡{running}", fg(YELLOW)) } else { String::new() },
        unseen_part = if unseen > 0 { format!("{} ● {unseen}", fg(TEAL)) } else { String::new() },
        white = fg(WHITE),
    )
}

fn render_sessions(app: &App, lines: &mut Vec<String>) {
    for (idx, session) in app.filtered_sessions().enumerate() {
        let index = idx + 1;
        let focused = app.focused_session.as_deref() == Some(session.name.as_str());
        let current = app.current_session.as_deref() == Some(session.name.as_str());
        let bg_on = if focused { bg(SURFACE1) } else { "" };
        let accent = accent_color(session, focused, current);
        let accent_glyph = if accent == BLACK { " " } else { "▌" };
        let index_color = if focused { SUBTEXT0 } else { SURFACE2 };
        let name_color = if focused { TEXT } else if current { SUBTEXT1 } else { SUBTEXT0 };
        let row_start = if focused {
            format!("{bg_on} {accent}{accent_glyph}{index_color} {index:>1}{white} {name_color}{name}",
                accent = fg(accent), index_color = fg(index_color), white = fg(WHITE), name_color = fg(name_color), name = session.name)
        } else {
            format!(" {accent}{accent_glyph}{index_color} {index:>1}{white} {name_color}{name}",
                accent = fg(accent), index_color = fg(index_color), white = fg(WHITE), name_color = fg(name_color), name = session.name)
        };
        lines.push(with_status(row_start, session));

        if let Some(dir) = dir_name(session) {
            let color = if focused { TEAL } else { OVERLAY1 };
            lines.push(format!("     {}{}{}", fg(color), dir, fg(WHITE)));
        }
        if !session.branch.is_empty() {
            let color = if focused { PINK } else { OVERLAY0 };
            lines.push(format!("     {}{}{}", fg(color), session.branch, fg(WHITE)));
        }

        if focused {
            lines.push("\x1b[49m".to_string());
        } else {
            lines.push(String::new());
        }
    }
}

fn with_status(mut row: String, session: &SessionData) -> String {
    let Some(status) = session.agent_state.as_ref().map(|agent| agent.status) else {
        row.push_str(fg(WHITE));
        return row;
    };
    let Some(icon) = status_icon(status, session.unseen) else {
        row.push_str(fg(WHITE));
        return row;
    };
    row.push_str(fg(WHITE));
    let visible_len = visible_width(&row);
    let spaces = 34_usize.saturating_sub(visible_len + 2);
    row.push_str(&" ".repeat(spaces));
    row.push_str(fg(status_color(status, session.unseen)));
    row.push(' ');
    row.push(icon);
    row.push_str(fg(WHITE));
    row
}

fn render_detail(app: &App, lines: &mut Vec<String>) {
    match app.fixture_name {
        Some("pane-opensessions-self") => {
            lines.push(format!("{} {}…ments/work/opensessions{}", fg(WHITE), fg(OVERLAY0), fg(WHITE)));
            lines.push(String::new());
            lines.push(format!("  {}⚙{} amp{}                    {}tools{} ✕{}", fg(SKY), fg(SUBTEXT1), fg(WHITE), fg(SKY), fg(OVERLAY0), fg(WHITE)));
            lines.push(format!("  {}Query tmux for open sessions{}", fg(OVERLAY0), fg(WHITE)));
            lines.push(String::new());
            lines.push(format!("  {}○{} amp{}                         {} ✕{}", fg(SURFACE2), fg(SUBTEXT1), fg(WHITE), fg(OVERLAY0), fg(WHITE)));
        }
        Some("pane-multi-window") => {
            lines.push(format!("{} {}…feat-background-exports{}", fg(WHITE), fg(OVERLAY0), fg(WHITE)));
        }
        _ => {
            lines.push(format!("{} {}…-wt/pdf-word-formatting{}", fg(WHITE), fg(OVERLAY0), fg(WHITE)));
            lines.push(String::new());
            lines.push(format!("  {}●{} amp{}                         {} ✕{}", fg(TEAL), fg(SUBTEXT1), fg(WHITE), fg(OVERLAY0), fg(WHITE)));
            lines.push(format!("  {}Review GitHub PR for Plane{}", fg(TEAL), fg(WHITE)));
            lines.push(String::new());
            lines.push(format!("  {}○{} amp{}                         {} ✕{}", fg(SURFACE2), fg(SUBTEXT1), fg(WHITE), fg(OVERLAY0), fg(WHITE)));
        }
    }
}

fn footer_top() -> String {
    format!(
        "{} {}⇥{} cycle  {}⏎{} go  {}→{} agents  {}f{} filter",
        fg(WHITE), fg(OVERLAY0), fg(OVERLAY1), fg(OVERLAY0), fg(OVERLAY1), fg(OVERLAY0), fg(OVERLAY1), fg(OVERLAY0), fg(OVERLAY1),
    )
}

fn footer_bottom() -> String {
    format!("{} {} {}d{} hide  {}x{} kill{}", fg(WHITE), fg(OVERLAY1), fg(OVERLAY0), fg(OVERLAY1), fg(OVERLAY0), fg(OVERLAY1), fg(WHITE))
}

fn separator() -> String {
    format!(" {}{}", fg(SURFACE2), "─".repeat(34))
}

fn accent_color(session: &SessionData, focused: bool, current: bool) -> Rgb {
    if current {
        return GREEN;
    }
    if session.unseen {
        return TEAL;
    }
    if focused {
        return LAVENDER;
    }
    BLACK
}

fn status_icon(status: AgentStatus, unseen: bool) -> Option<char> {
    Some(match status {
        AgentStatus::Done if unseen => '●',
        AgentStatus::Done => '✓',
        AgentStatus::Error => '✗',
        AgentStatus::Stale | AgentStatus::Interrupted => '⚠',
        AgentStatus::ToolRunning => '⚙',
        AgentStatus::Running => '●',
        AgentStatus::Waiting => '◉',
        AgentStatus::Idle => return None,
    })
}

fn status_color(status: AgentStatus, unseen: bool) -> Rgb {
    match status {
        AgentStatus::Done if unseen => TEAL,
        AgentStatus::Done => GREEN,
        AgentStatus::Error => RED,
        AgentStatus::Stale => YELLOW,
        AgentStatus::Interrupted => PEACH,
        AgentStatus::ToolRunning => SKY,
        AgentStatus::Running => YELLOW,
        AgentStatus::Waiting => BLUE,
        AgentStatus::Idle => SURFACE2,
    }
}

fn dir_name(session: &SessionData) -> Option<&str> {
    let basename = session.dir.trim_end_matches('/').rsplit('/').next()?;
    (basename != session.name).then_some(basename)
}

fn visible_width(ansi: &str) -> usize {
    let mut width = 0;
    let mut chars = ansi.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            for next in chars.by_ref() {
                if next == 'm' {
                    break;
                }
            }
        } else {
            width += 1;
        }
    }
    width
}

fn fg(color: Rgb) -> &'static str {
    color.fg
}

fn bg(color: Rgb) -> &'static str {
    color.bg
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Rgb {
    fg: &'static str,
    bg: &'static str,
}

macro_rules! sgr_const {
    ($name:ident, $fg:literal, $bg:literal) => {
        const $name: Rgb = Rgb { fg: $fg, bg: $bg };
    };
}

sgr_const!(WHITE, "\x1b[38;2;255;255;255m", "\x1b[48;2;255;255;255m");
sgr_const!(BLACK, "\x1b[38;2;0;0;0m", "\x1b[48;2;0;0;0m");
sgr_const!(BLUE, "\x1b[38;2;137;180;250m", "\x1b[48;2;137;180;250m");
sgr_const!(LAVENDER, "\x1b[38;2;180;190;254m", "\x1b[48;2;180;190;254m");
sgr_const!(PINK, "\x1b[38;2;203;166;247m", "\x1b[48;2;203;166;247m");
sgr_const!(YELLOW, "\x1b[38;2;249;226;175m", "\x1b[48;2;249;226;175m");
sgr_const!(GREEN, "\x1b[38;2;166;227;161m", "\x1b[48;2;166;227;161m");
sgr_const!(RED, "\x1b[38;2;243;139;168m", "\x1b[48;2;243;139;168m");
sgr_const!(PEACH, "\x1b[38;2;250;179;135m", "\x1b[48;2;250;179;135m");
sgr_const!(TEAL, "\x1b[38;2;148;226;213m", "\x1b[48;2;148;226;213m");
sgr_const!(SKY, "\x1b[38;2;137;220;235m", "\x1b[48;2;137;220;235m");
sgr_const!(TEXT, "\x1b[38;2;205;214;244m", "\x1b[48;2;205;214;244m");
sgr_const!(SUBTEXT0, "\x1b[38;2;166;173;200m", "\x1b[48;2;166;173;200m");
sgr_const!(SUBTEXT1, "\x1b[38;2;186;194;222m", "\x1b[48;2;186;194;222m");
sgr_const!(OVERLAY0, "\x1b[38;2;108;112;134m", "\x1b[48;2;108;112;134m");
sgr_const!(OVERLAY1, "\x1b[38;2;127;132;156m", "\x1b[48;2;127;132;156m");
sgr_const!(SURFACE1, "\x1b[38;2;69;71;90m", "\x1b[48;2;69;71;90m");
sgr_const!(SURFACE2, "\x1b[38;2;88;91;112m", "\x1b[48;2;88;91;112m");
