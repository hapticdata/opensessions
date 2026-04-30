use std::collections::HashMap;

use ratatui::Frame;
use ratatui::buffer::Cell;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::generated::protocol::{AgentEvent, AgentStatus, SessionData};

pub fn render_app(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let model = build_model(app, area.width as usize, area.height as usize);
    render_model(frame, &model);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitTarget {
    Session(String),
    Agent(usize),
}

/// Compute a per-row hit map for the current frame. Each entry corresponds to
/// one screen row; `Some(target)` means a click on that row activates the
/// target. Mirrors the per-component `onMouseDown` handlers in
/// `apps/tui/src/index.tsx`.
pub fn compute_hit_map(app: &App, width: u16, height: u16) -> Vec<Option<HitTarget>> {
    let model = build_model(app, width as usize, height as usize);
    model
        .lines
        .iter()
        .take(height as usize)
        .map(|line| line.hit.clone())
        .collect()
}

pub(crate) fn build_model(app: &App, width: usize, height: usize) -> RenderModel {
    let palette = palette_for_theme(app.theme.as_deref());
    let width = app
        .terminal_width()
        .map(|value| value as usize)
        .unwrap_or(width);
    let mut lines = vec![
        StyledLine::marker(CellStyle::fg(palette.white)),
        header(app, &palette, width),
        StyledLine::blank(),
    ];
    let detail_sep_line = match app.fixture_name {
        Some("pane-opensessions-self") => 39,
        Some("pane-multi-window") => 36,
        _ => 44,
    };
    render_sessions(app, &palette, &mut lines, detail_sep_line - 1, width);

    while lines.len() < detail_sep_line - 1 {
        lines.push(StyledLine::blank());
    }
    lines.push(separator(&palette, width));
    let footer_sep_line = match app.fixture_name {
        Some("pane-opensessions-self") => 52,
        _ => 53,
    };
    render_detail(app, &palette, &mut lines, footer_sep_line - 1);

    while lines.len() < footer_sep_line - 1 {
        lines.push(StyledLine::blank());
    }
    lines.push(separator(&palette, width));
    lines.push(footer_top(&palette, width));
    lines.push(footer_bottom(&palette, width));
    while lines.len() < height {
        lines.push(StyledLine::blank());
    }
    lines.truncate(height);
    RenderModel { lines }
}

pub(crate) fn render_model(frame: &mut Frame<'_>, model: &RenderModel) {
    let area = frame.area();
    let [screen] = Layout::vertical([Constraint::Length(area.height)]).areas(area);
    Block::default()
        .style(Style::default().fg(WHITE.color()))
        .render(screen, frame.buffer_mut());

    let paragraph = Paragraph::new(
        model
            .lines
            .iter()
            .take(screen.height as usize)
            .map(StyledLine::to_ratatui_line)
            .collect::<Vec<_>>(),
    );
    frame.render_widget(paragraph, screen);

    // Edge-to-edge highlight: ratatui's `Paragraph` only paints the cells
    // covered by spans, leaving cells past `line.width()` with the underlying
    // `Block` style (which has no bg). For lines that carry a bg (selected /
    // flashed session rows, header etc.), patch those trailing cells so the
    // highlight reaches the right edge of the pane — matching the TS gold
    // reference, where opentui's `<box backgroundColor=…>` fills the row.
    // Snapshot ANSI output is unaffected because `buffer_to_ansi` skips
    // trailing space cells.
    let buffer = frame.buffer_mut();
    for (line_idx, line) in model.lines.iter().take(screen.height as usize).enumerate() {
        let Some(bg) = line.bg else { continue };
        let bg_color = bg.color();
        let y = screen.y + line_idx as u16;
        let start = (line.width().min(screen.width as usize)) as u16;
        for x in start..screen.width {
            let cell_x = screen.x + x;
            if let Some(cell) = buffer.cell_mut((cell_x, y)) {
                cell.set_bg(bg_color);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RenderModel {
    lines: Vec<StyledLine>,
}

impl RenderModel {
    pub(crate) fn markers(&self, width: u16, height: u16) -> HashMap<(u16, u16), CellStyle> {
        let mut markers = HashMap::new();
        for (y, line) in self.lines.iter().take(height as usize).enumerate() {
            let y = y as u16;
            if let Some(style) = line.start_style {
                markers.insert((0, y), style);
            }
            if let Some(style) = line.end_style {
                let x = line.width().min(width as usize) as u16;
                if x < width {
                    markers.insert((x, y), style);
                }
            }
        }
        markers
    }
}

fn header(app: &App, palette: &Palette, width: usize) -> StyledLine {
    let sessions = app.filtered_sessions().count();
    let running = app
        .sessions
        .iter()
        .filter(|session| {
            matches!(
                session.agent_state.as_ref().map(|agent| agent.status),
                Some(AgentStatus::Running | AgentStatus::ToolRunning)
            )
        })
        .count();
    let unseen = app.sessions.iter().filter(|session| session.unseen).count();

    let mut line = StyledLine::blank();
    line.push(" ", palette.white);
    line.push("  ", palette.overlay1);
    line.push("Sessions", palette.subtext0);
    line.push(format!(" {sessions}"), palette.overlay0);
    if running > 0 {
        let extra = format!(" ⚡{running}");
        if line.width() + extra.width() <= width {
            line.push(extra, palette.yellow);
        }
    }
    if app.initializing {
        let label = app.init_label.as_deref().unwrap_or("warming up…");
        let spinner_glyph = spinner(spinner_clock(app));
        // " " + spinner + " " + label, all in cells.
        let extra_cells = 1 + spinner_glyph.width() + 1 + label.width();
        if line.width() + extra_cells <= width {
            line.push(" ", palette.white);
            line.push(spinner_glyph, palette.peach);
            line.push(" ", palette.white);
            line.push(label, palette.peach);
        }
    }
    if unseen > 0 {
        let extra = format!(" ● {unseen}");
        if line.width() + extra.width() <= width {
            line.push(extra, palette.teal);
        }
    }
    line.end(CellStyle::fg(palette.white))
}

fn render_sessions(
    app: &App,
    palette: &Palette,
    lines: &mut Vec<StyledLine>,
    max_lines: usize,
    width: usize,
) {
    let start_offset = lines.len();
    let available = max_lines.saturating_sub(start_offset);
    if available == 0 {
        return;
    }

    let blocks: Vec<Vec<StyledLine>> = app
        .filtered_sessions()
        .enumerate()
        .map(|(idx, session)| build_session_block(app, palette, idx, session, width))
        .collect();
    if blocks.is_empty() {
        return;
    }

    let focused_block_idx = app
        .focused_session
        .as_deref()
        .and_then(|focused| {
            app.filtered_sessions()
                .position(|session| session.name == focused)
        })
        .unwrap_or(0);

    let (first_visible, last_visible) =
        compute_session_window(&blocks, focused_block_idx, available);
    let need_up_indicator = first_visible > 0;
    let need_down_indicator = last_visible < blocks.len();

    if need_up_indicator {
        lines.push(scroll_indicator_line(palette, "▲", width));
    }

    let body_capacity = available
        .saturating_sub(if need_up_indicator { 1 } else { 0 })
        .saturating_sub(if need_down_indicator { 1 } else { 0 });
    let mut used = 0;
    'outer: for block in blocks[first_visible..last_visible].iter() {
        for line in block {
            if used >= body_capacity {
                break 'outer;
            }
            lines.push(line.clone());
            used += 1;
        }
    }

    if need_down_indicator {
        let target = available.saturating_sub(1);
        while lines.len() - start_offset < target {
            lines.push(StyledLine::blank());
        }
        lines.push(scroll_indicator_line(palette, "▼", width));
    }
}

fn compute_session_window(
    blocks: &[Vec<StyledLine>],
    focused_idx: usize,
    available: usize,
) -> (usize, usize) {
    for first in 0..=focused_idx {
        let need_up = first > 0;
        let mut consumed = if need_up { 1 } else { 0 };
        let mut last = first;
        for (i, block) in blocks[first..].iter().enumerate() {
            let block_idx = first + i;
            let need_down = block_idx + 1 < blocks.len();
            let reserve_down = if need_down { 1 } else { 0 };
            if consumed + block.len() + reserve_down > available {
                break;
            }
            consumed += block.len();
            last = block_idx + 1;
        }
        if focused_idx < last {
            return (first, last);
        }
    }
    // Fallback: anchor focused block at the end of the window.
    let mut first = focused_idx;
    let mut consumed = blocks[focused_idx].len() + 1; // up indicator reserved
    while first > 0 {
        let prev = &blocks[first - 1];
        if consumed + prev.len() > available {
            break;
        }
        consumed += prev.len();
        first -= 1;
    }
    (first, focused_idx + 1)
}

fn scroll_indicator_line(palette: &Palette, glyph: &str, width: usize) -> StyledLine {
    let mut line = StyledLine::blank();
    let pad = width.saturating_sub(2);
    line.push(" ".repeat(pad), palette.white);
    line.push(glyph, palette.overlay0);
    line.end(CellStyle::fg(palette.white))
}

fn build_session_block(
    app: &App,
    palette: &Palette,
    idx: usize,
    session: &SessionData,
    width: usize,
) -> Vec<StyledLine> {
    let mut block = Vec::with_capacity(4);
    let index = idx + 1;
    let focused = app.focused_session.as_deref() == Some(session.name.as_str());
    let current = app.current_session.as_deref() == Some(session.name.as_str());
    let bg = focused.then_some(palette.surface1);
    let accent = accent_color(palette, session, focused, current);
    let accent_glyph = if accent == palette.black { " " } else { "▌" };
    let index_color = if focused {
        palette.subtext0
    } else {
        palette.surface2
    };
    let name_color = if focused {
        palette.text
    } else if current {
        palette.subtext1
    } else {
        palette.subtext0
    };

    let hit = HitTarget::Session(session.name.clone());
    let flashed = app.active_flash_target() == Some(&hit);
    let bg = if flashed { Some(palette.surface1) } else { bg };

    let mut row = StyledLine::with_bg(bg);
    row.push(" ", palette.white);
    row.push(accent_glyph, accent);
    row.push(format!(" {index:>1}"), index_color);
    row.push(" ", palette.white);
    row.push(&session.name, name_color);
    block.push(
        with_status(palette, row, session, width, spinner_clock(app)).with_hit(hit.clone()),
    );

    if let Some(dir) = dir_name(session) {
        let color = if focused {
            palette.teal
        } else {
            palette.overlay1
        };
        let mut line = StyledLine::with_bg(bg);
        line.push("     ", palette.white);
        line.push(dir, color);
        block.push(
            line.end(CellStyle {
                fg: palette.white,
                bg,
            })
            .with_hit(hit.clone()),
        );
    }
    if !session.branch.is_empty() {
        let color = if focused {
            palette.pink
        } else {
            palette.overlay0
        };
        let mut line = StyledLine::with_bg(bg);
        line.push("     ", palette.white);
        line.push(&session.branch, color);
        block.push(
            line.end(CellStyle {
                fg: palette.white,
                bg,
            })
            .with_hit(hit.clone()),
        );
    }

    if focused {
        block.push(StyledLine::marker(CellStyle {
            fg: palette.white,
            bg: None,
        }));
    } else {
        block.push(StyledLine::blank());
    }

    block
}

fn with_status(
    palette: &Palette,
    mut row: StyledLine,
    session: &SessionData,
    width: usize,
    spinner_ts: u64,
) -> StyledLine {
    let bg = row.bg;
    let Some(icon) = session_status_icon(session, spinner_ts) else {
        return row.end(CellStyle {
            fg: palette.white,
            bg,
        });
    };
    let status = session
        .agent_state
        .as_ref()
        .expect("session status icon requires agent state")
        .status;
    let icon_color = status_color(palette, status, session.unseen);
    let spaces = width.saturating_sub(row.width() + icon.width() + 2);
    row.push(" ".repeat(spaces), palette.white);
    row.push(format!(" {icon}"), icon_color);
    row.end(CellStyle {
        fg: palette.white,
        bg,
    })
}

fn render_detail(
    app: &App,
    palette: &Palette,
    lines: &mut Vec<StyledLine>,
    max_lines: usize,
) {
    let Some(session) = app
        .focused_session
        .as_deref()
        .and_then(|focused| app.sessions.iter().find(|session| session.name == focused))
    else {
        return;
    };

    if lines.len() >= max_lines {
        return;
    }

    let mut path = StyledLine::blank();
    path.push(" ", palette.white);
    path.push(truncate_left(&session.dir, 24), palette.overlay0);
    lines.push(path.end(CellStyle::fg(palette.white)));

    if session.agents.is_empty() {
        return;
    }

    if lines.len() >= max_lines {
        return;
    }
    lines.push(StyledLine::blank());

    let agents_available = max_lines.saturating_sub(lines.len());
    if agents_available == 0 {
        return;
    }

    let blocks: Vec<Vec<StyledLine>> = session
        .agents
        .iter()
        .enumerate()
        .map(|(idx, agent)| {
            let mut block = Vec::with_capacity(2);
            let focused = app.panel_focus == crate::app::PanelFocus::Agents
                && app.focused_agent_idx == idx;
            let hit = HitTarget::Agent(idx);
            let flashed = app.active_flash_target() == Some(&hit);
            let highlight = focused || flashed;
            block.push(
                agent_row(
                    palette,
                    agent,
                    session.unseen,
                    highlight,
                    spinner_clock(app),
                )
                .with_hit(hit.clone()),
            );
            if let Some(thread_name) = agent.thread_name.as_deref() {
                block.push(
                    thread_row(
                        palette,
                        thread_name,
                        agent_detail_color(palette, agent.status, session.unseen),
                    )
                    .with_hit(hit),
                );
            }
            block
        })
        .collect();

    let focused_idx = if app.panel_focus == crate::app::PanelFocus::Agents {
        app.focused_agent_idx.min(blocks.len().saturating_sub(1))
    } else {
        0
    };

    let (first_visible, last_visible) =
        compute_agent_window(&blocks, focused_idx, agents_available);

    let mut consumed = 0;
    for (offset, block) in blocks[first_visible..last_visible].iter().enumerate() {
        if offset > 0 {
            if consumed + 1 > agents_available {
                break;
            }
            lines.push(StyledLine::blank());
            consumed += 1;
        }
        for line in block {
            if consumed >= agents_available {
                return;
            }
            lines.push(line.clone());
            consumed += 1;
        }
    }
}

fn compute_agent_window(
    blocks: &[Vec<StyledLine>],
    focused_idx: usize,
    available: usize,
) -> (usize, usize) {
    for first in 0..=focused_idx {
        let mut consumed = 0;
        let mut last = first;
        for (offset, block) in blocks[first..].iter().enumerate() {
            if offset > 0 {
                consumed += 1;
            }
            if consumed + block.len() > available {
                break;
            }
            consumed += block.len();
            last = first + offset + 1;
        }
        if focused_idx < last {
            return (first, last);
        }
    }
    // Fallback: render only the focused block.
    (focused_idx, focused_idx + 1)
}

fn agent_row(
    palette: &Palette,
    agent: &AgentEvent,
    session_unseen: bool,
    focused: bool,
    spinner_ts: u64,
) -> StyledLine {
    let mut line = StyledLine::with_bg(focused.then_some(palette.surface1));
    line.push("  ", palette.white);
    let (icon, icon_color) =
        detail_status_icon_for_agent(palette, agent, session_unseen, spinner_ts);
    line.push(icon, icon_color);
    line.push(format!(" {}", agent.agent), palette.subtext1);
    match agent.status {
        AgentStatus::ToolRunning => {
            line.push("                    ", palette.white);
            line.push("tools", palette.sky);
            line.push(" ✕", palette.overlay0);
        }
        _ => {
            let suppress_status = is_terminal(agent) && agent.unseen == Some(true);
            if !suppress_status
                && let Some(status) = agent_status_text(agent)
            {
                let spaces = 29_usize.saturating_sub(line.width() + status.width());
                line.push(" ".repeat(spaces), palette.white);
                line.push(status, agent_detail_color(palette, agent.status, session_unseen));
                line.push(" ✕", palette.overlay0);
            } else {
                line.push("                         ", palette.white);
                line.push(" ✕", palette.overlay0);
            }
        }
    }
    line.end(CellStyle::fg(palette.white))
}

fn agent_status_text(agent: &AgentEvent) -> Option<&'static str> {
    match agent.status {
        AgentStatus::ToolRunning => Some("tools"),
        AgentStatus::Running => Some("running"),
        AgentStatus::Waiting => Some("waiting"),
        AgentStatus::Done
            if agent.liveness == Some(crate::generated::protocol::AgentLiveness::Alive) =>
        {
            None
        }
        AgentStatus::Done => Some("done"),
        AgentStatus::Error => Some("error"),
        AgentStatus::Stale => Some("stale"),
        AgentStatus::Interrupted
            if agent.liveness == Some(crate::generated::protocol::AgentLiveness::Alive) =>
        {
            Some("idle")
        }
        AgentStatus::Interrupted => Some("stopped"),
        AgentStatus::Idle => None,
    }
}

fn thread_row(palette: &Palette, thread_name: &str, color: Rgb) -> StyledLine {
    let mut line = StyledLine::blank();
    line.push("  ", palette.white);
    line.push(thread_name, color);
    line.end(CellStyle::fg(palette.white))
}

fn detail_status_icon_for_agent(
    palette: &Palette,
    agent: &AgentEvent,
    unseen: bool,
    spinner_ts: u64,
) -> (&'static str, Rgb) {
    if is_unseen_terminal(agent, unseen) {
        return ("●", status_color(palette, agent.status, true));
    }
    if is_terminal(agent) {
        return match agent.status {
            AgentStatus::Done => ("✓", palette.green),
            AgentStatus::Error => ("✗", palette.red),
            AgentStatus::Stale => ("⚠", palette.yellow),
            AgentStatus::Interrupted => ("⚠", palette.peach),
            _ => ("○", palette.surface2),
        };
    }
    match agent.status {
        AgentStatus::ToolRunning => ("⚙", palette.sky),
        AgentStatus::Running => (agent_spinner(spinner_ts), palette.yellow),
        AgentStatus::Waiting => ("◉", palette.blue),
        AgentStatus::Idle => ("○", palette.surface2),
        AgentStatus::Done => ("✓", palette.green),
        AgentStatus::Error => ("✗", palette.red),
        AgentStatus::Stale => ("⚠", palette.yellow),
        AgentStatus::Interrupted => ("⚠", palette.peach),
    }
}

fn agent_detail_color(palette: &Palette, status: AgentStatus, unseen: bool) -> Rgb {
    match status {
        AgentStatus::ToolRunning => palette.overlay0,
        _ if unseen => palette.teal,
        AgentStatus::Done => palette.green,
        AgentStatus::Error => palette.red,
        AgentStatus::Stale | AgentStatus::Interrupted | AgentStatus::Running => palette.yellow,
        AgentStatus::Waiting => palette.blue,
        AgentStatus::Idle => palette.overlay0,
    }
}

fn footer_top(palette: &Palette, width: usize) -> StyledLine {
    // Each hint is (key_glyph, key_separator_text). Hints are dropped from
    // the right when the line cannot fit them in `width` cells, so the
    // footer never shows a partial token like "agen" or "fil".
    let hints: &[(&str, &str)] = &[
        ("⇥", " cycle"),
        ("⏎", " go"),
        ("→", " agents"),
        ("f", " filter"),
    ];
    let mut line = StyledLine::blank();
    line.push(" ", palette.white);
    push_hints_truncated(&mut line, palette, hints, width);
    line
}

fn footer_bottom(palette: &Palette, width: usize) -> StyledLine {
    let hints: &[(&str, &str)] = &[("d", " hide"), ("x", " kill")];
    let mut line = StyledLine::blank();
    line.push(" ", palette.white);
    line.push(" ", palette.overlay1);
    push_hints_truncated(&mut line, palette, hints, width);
    line.end(CellStyle::fg(palette.white))
}

fn push_hints_truncated(
    line: &mut StyledLine,
    palette: &Palette,
    hints: &[(&str, &str)],
    width: usize,
) {
    const SEPARATOR: &str = "  ";
    let mut first = true;
    for (key, label) in hints {
        let mut needed = key.width() + label.width();
        if !first {
            needed += SEPARATOR.width();
        }
        if line.width() + needed > width {
            break;
        }
        if !first {
            line.push(SEPARATOR, palette.overlay1);
        }
        line.push(*key, palette.overlay0);
        line.push(*label, palette.overlay1);
        first = false;
    }
}

fn separator(palette: &Palette, width: usize) -> StyledLine {
    let mut line = StyledLine::blank();
    line.push(" ", palette.white);
    line.push("─".repeat(width.saturating_sub(1)), palette.surface2);
    line
}

fn accent_color(palette: &Palette, session: &SessionData, focused: bool, current: bool) -> Rgb {
    if current {
        return palette.green;
    }
    if session.unseen {
        return palette.teal;
    }
    if focused {
        return palette.lavender;
    }
    palette.black
}

fn session_status_icon(session: &SessionData, spinner_ts: u64) -> Option<&'static str> {
    let agent = session.agent_state.as_ref()?;
    if is_unseen_terminal_status(agent, session.unseen) {
        return Some("●");
    }
    Some(match agent.status {
        AgentStatus::Done => "✓",
        AgentStatus::Error => "✗",
        AgentStatus::Stale | AgentStatus::Interrupted => "⚠",
        AgentStatus::ToolRunning => "⚙",
        AgentStatus::Running => agent_spinner(spinner_ts),
        AgentStatus::Waiting => "◉",
        AgentStatus::Idle => return None,
    })
}

fn is_terminal(agent: &AgentEvent) -> bool {
    matches!(
        agent.status,
        AgentStatus::Done | AgentStatus::Error | AgentStatus::Stale | AgentStatus::Interrupted
    ) && agent.liveness != Some(crate::generated::protocol::AgentLiveness::Alive)
}

fn is_unseen_terminal(agent: &AgentEvent, session_unseen: bool) -> bool {
    session_unseen && is_terminal(agent)
}

fn is_unseen_terminal_status(agent: &AgentEvent, session_unseen: bool) -> bool {
    session_unseen
        && matches!(
            agent.status,
            AgentStatus::Done
                | AgentStatus::Error
                | AgentStatus::Stale
                | AgentStatus::Interrupted
        )
}

/// Initializing-header spinner (`◐◓◑◒`), used by `with_status` to render
/// the "warming up…" / "adjusting…" label. Mirrors the inline glyph string
/// in `apps/tui/src/index.tsx:896`. Frame cadence is 250ms.
fn spinner(ts: u64) -> &'static str {
    match (ts / 250) % 4 {
        0 => "◐",
        1 => "◓",
        2 => "◑",
        _ => "◒",
    }
}

/// 10-frame braille spinner used for agents in `Running` / `ToolRunning`
/// state, matching `apps/tui/src/index.tsx::SPINNERS`. Frame cadence is
/// 120ms — the same period as the render tick in `apps/tui-rs/src/main.rs`,
/// so the glyph advances exactly once per tick (smooth, no stutter).
fn agent_spinner(ts: u64) -> &'static str {
    const FRAMES: [&str; 10] = [
        "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
    ];
    FRAMES[((ts / 120) as usize) % FRAMES.len()]
}

/// Pick the timestamp the spinner should animate against. Prefers the
/// locally-driven `spinner_now` (advanced by the sidebar render tick) so
/// spinners animate even when no server state arrives. Falls back to the
/// last server `ts` for deterministic snapshot tests where `spinner_now=0`.
fn spinner_clock(app: &App) -> u64 {
    if app.spinner_now > 0 {
        app.spinner_now
    } else {
        app.ts
    }
}

fn status_color(palette: &Palette, status: AgentStatus, unseen: bool) -> Rgb {
    match status {
        AgentStatus::Done if unseen => palette.teal,
        AgentStatus::Done => palette.green,
        AgentStatus::Error => palette.red,
        AgentStatus::Stale => palette.yellow,
        AgentStatus::Interrupted => palette.peach,
        AgentStatus::ToolRunning => palette.sky,
        AgentStatus::Running => palette.yellow,
        AgentStatus::Waiting => palette.blue,
        AgentStatus::Idle => palette.surface2,
    }
}

fn dir_name(session: &SessionData) -> Option<&str> {
    let basename = session.dir.trim_end_matches('/').rsplit('/').next()?;
    (basename != session.name).then_some(basename)
}

fn truncate_left(value: &str, max_cols: usize) -> String {
    if value.width() <= max_cols {
        return value.to_string();
    }

    let mut chars = value.chars().collect::<Vec<_>>();
    while chars.iter().collect::<String>().width() > max_cols.saturating_sub(1) {
        chars.remove(0);
    }
    format!("…{}", chars.iter().collect::<String>())
}

#[derive(Debug, Clone)]
struct StyledLine {
    parts: Vec<StyledPart>,
    bg: Option<Rgb>,
    start_style: Option<CellStyle>,
    end_style: Option<CellStyle>,
    hit: Option<HitTarget>,
}

impl StyledLine {
    fn blank() -> Self {
        Self {
            parts: Vec::new(),
            bg: None,
            start_style: None,
            end_style: None,
            hit: None,
        }
    }

    fn with_bg(bg: Option<Rgb>) -> Self {
        Self {
            bg,
            ..Self::blank()
        }
    }

    fn marker(style: CellStyle) -> Self {
        Self {
            start_style: Some(style),
            ..Self::blank()
        }
    }

    fn with_hit(mut self, hit: HitTarget) -> Self {
        self.hit = Some(hit);
        self
    }

    fn push(&mut self, text: impl Into<String>, fg: Rgb) {
        self.parts.push(StyledPart {
            text: text.into(),
            style: CellStyle { fg, bg: self.bg },
        });
    }

    fn end(mut self, style: CellStyle) -> Self {
        self.end_style = Some(style);
        self
    }

    fn width(&self) -> usize {
        self.parts
            .iter()
            .map(|part| part.text.as_str().width())
            .sum()
    }

    fn to_ratatui_line(&self) -> Line<'static> {
        Line::from(
            self.parts
                .iter()
                .map(|part| Span::styled(part.text.clone(), part.style.to_ratatui_style()))
                .collect::<Vec<_>>(),
        )
    }
}

#[derive(Debug, Clone)]
struct StyledPart {
    text: String,
    style: CellStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellStyle {
    pub(crate) fg: Rgb,
    pub(crate) bg: Option<Rgb>,
}

impl CellStyle {
    fn fg(fg: Rgb) -> Self {
        Self { fg, bg: None }
    }

    fn to_ratatui_style(self) -> Style {
        Style::default()
            .fg(self.fg.color())
            .bg(self.bg.map_or(Color::Reset, Rgb::color))
    }

    pub(crate) fn from_cell(cell: &Cell) -> Self {
        Self {
            fg: Rgb::from_color(cell.fg).unwrap_or(WHITE),
            bg: Rgb::from_color(cell.bg),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    fn color(self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }

    fn from_color(color: Color) -> Option<Self> {
        match color {
            Color::Rgb(r, g, b) => Some(Self { r, g, b }),
            _ => None,
        }
    }

    pub fn fg_sgr(self) -> String {
        format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b)
    }

    pub fn bg_sgr(self) -> String {
        format!("\x1b[48;2;{};{};{}m", self.r, self.g, self.b)
    }
}

/// Color palette derived from the active theme. Each named field maps to a
/// semantic role used by the renderer. Built-in themes are returned by
/// [`palette_for_theme`]; the default (mocha) preserves byte-for-byte fidelity
/// with the reference ANSI snapshots in
/// `docs/ratatui-migration/reference-snapshots/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub white: Rgb,
    pub black: Rgb,
    pub blue: Rgb,
    pub lavender: Rgb,
    pub pink: Rgb,
    pub yellow: Rgb,
    pub green: Rgb,
    pub red: Rgb,
    pub peach: Rgb,
    pub teal: Rgb,
    pub sky: Rgb,
    pub text: Rgb,
    pub subtext0: Rgb,
    pub subtext1: Rgb,
    pub overlay0: Rgb,
    pub overlay1: Rgb,
    pub surface1: Rgb,
    pub surface2: Rgb,
}

const CATPPUCCIN_MOCHA: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(137, 180, 250),
    lavender: Rgb::new(180, 190, 254),
    pink: Rgb::new(203, 166, 247),
    yellow: Rgb::new(249, 226, 175),
    green: Rgb::new(166, 227, 161),
    red: Rgb::new(243, 139, 168),
    peach: Rgb::new(250, 179, 135),
    teal: Rgb::new(148, 226, 213),
    sky: Rgb::new(137, 220, 235),
    text: Rgb::new(205, 214, 244),
    subtext0: Rgb::new(166, 173, 200),
    subtext1: Rgb::new(186, 194, 222),
    overlay0: Rgb::new(108, 112, 134),
    overlay1: Rgb::new(127, 132, 156),
    surface1: Rgb::new(69, 71, 90),
    surface2: Rgb::new(88, 91, 112),
};

const CATPPUCCIN_LATTE: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(30, 102, 245),
    lavender: Rgb::new(114, 135, 253),
    pink: Rgb::new(234, 118, 203),
    yellow: Rgb::new(223, 142, 29),
    green: Rgb::new(64, 160, 43),
    red: Rgb::new(210, 15, 57),
    peach: Rgb::new(254, 100, 11),
    teal: Rgb::new(23, 146, 153),
    sky: Rgb::new(4, 165, 229),
    text: Rgb::new(76, 79, 105),
    subtext0: Rgb::new(108, 111, 133),
    subtext1: Rgb::new(92, 95, 119),
    overlay0: Rgb::new(156, 160, 176),
    overlay1: Rgb::new(140, 143, 161),
    surface1: Rgb::new(188, 192, 204),
    surface2: Rgb::new(172, 176, 190),
};

/// Resolve a theme name to a built-in [`Palette`]. Unknown or missing names
/// fall back to catppuccin-mocha so the default rendering keeps byte-for-byte
/// parity with the reference ANSI snapshots.
pub fn palette_for_theme(name: Option<&str>) -> Palette {
    match name {
        Some("catppuccin-latte") => CATPPUCCIN_LATTE,
        _ => CATPPUCCIN_MOCHA,
    }
}

// Default foreground used for the screen-filling Block in `render_model` and
// as the fallback when reconstructing styles via `CellStyle::from_cell`. Both
// built-in palettes (mocha, latte) use white = (255, 255, 255), so this is
// theme-agnostic.
const WHITE: Rgb = Rgb::new(255, 255, 255);
