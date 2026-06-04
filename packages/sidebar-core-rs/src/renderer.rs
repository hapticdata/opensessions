use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{AgentPanelScope, App, DisplaySessionEntry, Modal};
use crate::generated::protocol::{AgentEvent, AgentStatus, MetadataTone, SessionData};
use crate::session_display::worktree_group_key;

pub fn render_app(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let model = build_model(app, area.width as usize, area.height as usize);
    render_model(frame, &model);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitTarget {
    Session(String),
    Group(String),
    DiffCount(String),
    Agent(usize),
    AgentScopeToggle,
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

pub fn compute_hit_target(app: &App, x: u16, y: u16, width: u16, height: u16) -> Option<HitTarget> {
    let model = build_model(app, width as usize, height as usize);
    let line = model.lines.get(y as usize)?;
    line.hit_at(x as usize).or_else(|| line.hit.clone())
}

pub fn detail_separator_row(app: &App, width: u16, height: u16) -> u16 {
    sidebar_layout(app, width, height).detail_separator.y
}

pub(crate) fn build_model(app: &App, width: usize, height: usize) -> RenderModel {
    let palette = palette_for_theme(app.theme.as_deref());
    let width = app
        .terminal_width()
        .map(|value| value as usize)
        .unwrap_or(width);
    let layout = sidebar_layout(app, width as u16, height as u16);
    let mut lines = vec![header(app, &palette, width), StyledLine::blank()];
    let detail_separator_row = layout.detail_separator.y as usize;
    let session_scrollbar = render_sessions(app, &palette, &mut lines, detail_separator_row, width);

    while lines.len() < detail_separator_row {
        lines.push(StyledLine::blank());
    }
    lines.push(separator(&palette, width));
    let footer_separator_row = layout.footer_separator.y as usize;
    render_detail(app, &palette, &mut lines, footer_separator_row, width);

    while lines.len() < footer_separator_row {
        lines.push(StyledLine::blank());
    }
    lines.push(separator(&palette, width));
    let [footer_top, footer_bottom] = footer(&palette, width);
    lines.push(footer_top);
    lines.push(footer_bottom);
    while lines.len() < height {
        lines.push(StyledLine::blank());
    }
    lines.truncate(height);

    if app.is_modal_open() {
        render_modal_overlay(app, &palette, &mut lines, width, height);
    }

    RenderModel {
        lines,
        layout,
        session_scrollbar,
    }
}

#[derive(Debug, Clone, Copy)]
struct SidebarLayout {
    header_rows: u16,
    detail_separator: Rect,
    footer_separator: Rect,
}

fn sidebar_layout(app: &App, width: u16, height: u16) -> SidebarLayout {
    const HEADER_ROWS: u16 = 2;
    const DETAIL_SEPARATOR_ROWS: u16 = 1;
    const FOOTER_SEPARATOR_ROWS: u16 = 1;
    const FOOTER_ROWS: u16 = 2;

    let detail_separator_row = height
        .saturating_sub(FOOTER_SEPARATOR_ROWS + FOOTER_ROWS + app.detail_panel_height as u16)
        .saturating_sub(DETAIL_SEPARATOR_ROWS)
        .max(HEADER_ROWS);
    let footer_separator_row = height
        .saturating_sub(FOOTER_ROWS + FOOTER_SEPARATOR_ROWS)
        .max(detail_separator_row + DETAIL_SEPARATOR_ROWS);

    let session_rows = detail_separator_row.saturating_sub(HEADER_ROWS);
    let detail_rows =
        footer_separator_row.saturating_sub(detail_separator_row + DETAIL_SEPARATOR_ROWS);
    let area = Rect::new(0, 0, width, height);
    let [
        _header,
        _sessions,
        detail_separator,
        _detail,
        footer_separator,
        _footer,
    ] = Layout::vertical([
        Constraint::Length(HEADER_ROWS),
        Constraint::Length(session_rows),
        Constraint::Length(DETAIL_SEPARATOR_ROWS),
        Constraint::Length(detail_rows),
        Constraint::Length(FOOTER_SEPARATOR_ROWS),
        Constraint::Length(FOOTER_ROWS),
    ])
    .areas(area);

    SidebarLayout {
        header_rows: HEADER_ROWS,
        detail_separator,
        footer_separator,
    }
}

pub(crate) fn render_model(frame: &mut Frame<'_>, model: &RenderModel) {
    let area = frame.area();
    let [screen] = Layout::vertical([Constraint::Length(area.height)]).areas(area);
    Block::default()
        .style(Style::default().fg(WHITE.color()))
        .render(screen, frame.buffer_mut());

    let layout = model.layout;
    let header = Rect::new(screen.x, screen.y, screen.width, layout.header_rows);
    let sessions = Rect::new(
        screen.x,
        screen.y + layout.header_rows,
        screen.width,
        layout.detail_separator.y.saturating_sub(layout.header_rows),
    );
    let detail_separator = Rect::new(
        screen.x,
        screen.y + layout.detail_separator.y,
        screen.width,
        layout.detail_separator.height,
    );
    let detail = Rect::new(
        screen.x,
        screen.y + layout.detail_separator.y + layout.detail_separator.height,
        screen.width,
        layout
            .footer_separator
            .y
            .saturating_sub(layout.detail_separator.y + layout.detail_separator.height),
    );
    let footer_separator = Rect::new(
        screen.x,
        screen.y + layout.footer_separator.y,
        screen.width,
        layout.footer_separator.height,
    );
    let footer = Rect::new(
        screen.x,
        screen.y + layout.footer_separator.y + layout.footer_separator.height,
        screen.width,
        screen
            .height
            .saturating_sub(layout.footer_separator.y + layout.footer_separator.height),
    );

    render_lines(frame, &model.lines, 0, header);
    render_lines(frame, &model.lines, header.height as usize, sessions);
    render_lines(
        frame,
        &model.lines,
        layout.detail_separator.y as usize,
        detail_separator,
    );
    render_lines(
        frame,
        &model.lines,
        (layout.detail_separator.y + layout.detail_separator.height) as usize,
        detail,
    );
    render_lines(
        frame,
        &model.lines,
        layout.footer_separator.y as usize,
        footer_separator,
    );
    render_lines(
        frame,
        &model.lines,
        (layout.footer_separator.y + layout.footer_separator.height) as usize,
        footer,
    );

    if let Some(scrollbar) = model.session_scrollbar {
        let mut state = ScrollbarState::new(scrollbar.content_length)
            .position(scrollbar.position)
            .viewport_content_length(scrollbar.viewport_length);
        let widget = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .track_style(Style::default().fg(scrollbar.track.color()))
            .thumb_symbol("┃")
            .thumb_style(Style::default().fg(scrollbar.thumb.color()));
        widget.render(scrollbar.area, frame.buffer_mut(), &mut state);
    }
}

fn render_lines(frame: &mut Frame<'_>, lines: &[StyledLine], start: usize, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let visible = lines
        .iter()
        .skip(start)
        .take(area.height as usize)
        .map(StyledLine::to_ratatui_line)
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(visible), area);

    // Edge-to-edge highlight: ratatui's `Paragraph` only paints cells covered
    // by spans. Patch trailing cells for rows that intentionally carry a bg so
    // selection / flash highlights fill the full component width like OpenTUI.
    let buffer = frame.buffer_mut();
    for (offset, line) in lines
        .iter()
        .skip(start)
        .take(area.height as usize)
        .enumerate()
    {
        let Some(bg) = line.bg else { continue };
        let bg_color = bg.color();
        let y = area.y + offset as u16;
        let start_x = (line.width().min(area.width as usize)) as u16;
        for x in start_x..area.width {
            let cell_x = area.x + x;
            if let Some(cell) = buffer.cell_mut((cell_x, y)) {
                cell.set_bg(bg_color);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RenderModel {
    lines: Vec<StyledLine>,
    layout: SidebarLayout,
    session_scrollbar: Option<ScrollbarSpec>,
}

#[derive(Debug, Clone, Copy)]
struct ScrollbarSpec {
    area: Rect,
    content_length: usize,
    position: usize,
    viewport_length: usize,
    track: Rgb,
    thumb: Rgb,
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
    line.push("Sessions", palette.subtext0);

    if app.initializing {
        let label = app.init_label.as_deref().unwrap_or("warming up…");
        let spinner_glyph = agent_spinner(spinner_clock(app));
        let init_cells = 1 + spinner_glyph.width() + 1 + label.width();
        let count = format!(" {sessions}");
        if line.width() + count.width() + init_cells <= width {
            line.push(count, palette.overlay0);
        }
        if line.width() + init_cells <= width {
            line.push(" ", palette.white);
            line.push(spinner_glyph, palette.peach);
            line.push(" ", palette.white);
            line.push(label, palette.peach);
        }
    } else {
        let count = sessions.to_string();
        let spaces = width.saturating_sub(line.width() + count.width());
        if spaces > 0 {
            line.push(" ".repeat(spaces), palette.white);
        }
        line.push(count, palette.overlay0);
    }

    if running > 0 {
        let extra = format!(" ⚡{running}");
        if line.width() + extra.width() <= width {
            line.push(extra, palette.yellow);
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
) -> Option<ScrollbarSpec> {
    let start_offset = lines.len();
    let available = max_lines.saturating_sub(start_offset);
    if available == 0 {
        return None;
    }

    let entries = app.display_session_entries();
    if entries.is_empty() && app.initializing {
        push_loader_rows(
            lines,
            palette,
            width,
            available,
            app.init_label.as_deref().unwrap_or("warming up…"),
            "reading tmux + git state",
            spinner_clock(app),
        );
        return None;
    }

    let rows = flatten_session_rows(&entries);
    if rows.is_empty() {
        return None;
    }

    let focused_row_idx = rows
        .iter()
        .position(|row| row.is_focus_row(app))
        .unwrap_or(0);

    let total_rows = rows.len();
    let visible_cards = available.div_ceil(2).max(1);
    let max_first_visible = entries.len().saturating_sub(visible_cards);
    let first_visible = if total_rows <= available {
        0
    } else if app.session_scroll_follows_focus() {
        compute_session_window_start_for_focus(&rows, focused_row_idx, available)
    } else {
        row_index_for_entry(&rows, app.session_scroll_offset().min(max_first_visible))
    };

    let body_capacity = available;
    for row in rows.iter().skip(first_visible).take(body_capacity) {
        lines.push(row.render(app, palette, width));
    }

    while lines.len() - start_offset < available {
        lines.push(rail_blank(palette));
    }

    if total_rows > available {
        Some(ScrollbarSpec {
            area: Rect::new(0, start_offset as u16, width as u16, available as u16),
            content_length: total_rows,
            position: first_visible,
            viewport_length: available,
            track: palette.surface2,
            thumb: palette.overlay1,
        })
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum SessionListRow<'a> {
    Group {
        entry_idx: usize,
        key: &'a str,
        label: &'a str,
        count: usize,
        collapsed: bool,
        summary: &'a crate::session_display::GroupSummary,
    },
    SessionName {
        entry_idx: usize,
        index: usize,
        session: &'a SessionData,
        indented: bool,
    },
    SessionDetail {
        entry_idx: usize,
        session: &'a SessionData,
        indented: bool,
    },
    Spacer {
        entry_idx: usize,
        group_rail: bool,
    },
}

impl<'a> SessionListRow<'a> {
    fn entry_idx(self) -> usize {
        match self {
            Self::Group { entry_idx, .. }
            | Self::SessionName { entry_idx, .. }
            | Self::SessionDetail { entry_idx, .. }
            | Self::Spacer { entry_idx, .. } => entry_idx,
        }
    }

    fn is_focus_row(self, app: &App) -> bool {
        match self {
            Self::Group { key, .. } => app.focused_group_key() == Some(key),
            Self::SessionName { session, .. } => {
                app.focused_session_name() == Some(session.name.as_str())
            }
            Self::SessionDetail { .. } | Self::Spacer { .. } => false,
        }
    }

    fn render(self, app: &App, palette: &Palette, width: usize) -> StyledLine {
        match self {
            Self::Group {
                key,
                label,
                count,
                collapsed,
                summary,
                ..
            } => build_group_row(app, palette, key, label, count, collapsed, summary, width),
            Self::SessionName {
                index,
                session,
                indented,
                ..
            } => build_session_name_row(app, palette, index, session, indented),
            Self::SessionDetail {
                session, indented, ..
            } => build_session_detail_row(app, palette, session, indented, width),
            Self::Spacer {
                group_rail: true, ..
            } => inner_rail_blank(palette),
            Self::Spacer {
                group_rail: false, ..
            } => rail_blank(palette),
        }
    }
}

fn flatten_session_rows<'a>(entries: &'a [DisplaySessionEntry<'a>]) -> Vec<SessionListRow<'a>> {
    let mut rows = Vec::new();
    for (entry_idx, entry) in entries.iter().enumerate() {
        match entry {
            DisplaySessionEntry::Group {
                key,
                label,
                count,
                collapsed,
                summary,
            } => rows.push(SessionListRow::Group {
                entry_idx,
                key,
                label,
                count: *count,
                collapsed: *collapsed,
                summary,
            }),
            DisplaySessionEntry::Session {
                index,
                session,
                indented,
            } => {
                rows.push(SessionListRow::SessionName {
                    entry_idx,
                    index: *index,
                    session,
                    indented: *indented,
                });
                rows.push(SessionListRow::SessionDetail {
                    entry_idx,
                    session,
                    indented: *indented,
                });
                let next_is_group_child = matches!(
                    entries.get(entry_idx + 1),
                    Some(DisplaySessionEntry::Session { indented: true, .. })
                );
                rows.push(SessionListRow::Spacer {
                    entry_idx,
                    group_rail: *indented && next_is_group_child,
                });
            }
        }
    }
    rows
}

fn row_index_for_entry(rows: &[SessionListRow<'_>], entry_idx: usize) -> usize {
    rows.iter()
        .position(|row| row.entry_idx() >= entry_idx)
        .unwrap_or(0)
}

fn build_group_row(
    app: &App,
    palette: &Palette,
    key: &str,
    label: &str,
    count: usize,
    collapsed: bool,
    summary: &crate::session_display::GroupSummary,
    width: usize,
) -> StyledLine {
    let hit = HitTarget::Group(key.to_string());
    let focused =
        app.panel_focus == crate::app::PanelFocus::Sessions && app.focused_group_key() == Some(key);
    let active_surrogate = collapsed
        && app
            .current_session
            .as_deref()
            .and_then(|name| app.sessions.iter().find(|session| session.name == name))
            .and_then(worktree_group_key)
            .as_deref()
            == Some(key);
    let flashed = app.active_flash_target() == Some(&hit);
    let bg = (focused || flashed).then_some(palette.surface1);

    let mut row = StyledLine::with_bg(bg);
    let marker = if active_surrogate {
        "▌"
    } else if focused {
        "›"
    } else {
        " "
    };
    let marker_color = if active_surrogate {
        palette.green
    } else {
        palette.lavender
    };
    row.push(marker, marker_color);
    row.push("  ", palette.surface2);
    row.push(if collapsed { "▸" } else { "▾" }, palette.overlay0);
    row.push(" ", palette.white);
    row.push(
        label,
        if focused {
            palette.text
        } else {
            palette.subtext1
        },
    );
    let count_text = format!("{count}wt");
    let spaces = 25usize.saturating_sub(row.width());
    if spaces > 0 {
        row.push(" ".repeat(spaces), palette.white);
    }
    if row.width() + count_text.width() <= width {
        row.push(count_text, palette.overlay0);
    }
    push_group_summary(&mut row, palette, summary, collapsed, width);

    let mut line = row.end(CellStyle {
        fg: palette.white,
        bg,
    });
    let remaining = width.saturating_sub(line.width());
    if remaining > 0 {
        line.push(" ".repeat(remaining), palette.white);
    }
    line.with_hit(hit)
}

fn push_loader_rows(
    lines: &mut Vec<StyledLine>,
    palette: &Palette,
    width: usize,
    available: usize,
    label: &str,
    detail: &str,
    spinner_ts: u64,
) {
    if available == 0 {
        return;
    }

    let start_len = lines.len();
    let mut primary = StyledLine::blank();
    primary.push("   ", palette.white);
    primary.push(agent_spinner(spinner_ts), palette.yellow);
    primary.push("  ", palette.white);
    let available_label = width.saturating_sub(primary.width());
    primary.push(truncate_right(label, available_label), palette.subtext0);
    lines.push(primary.end(CellStyle::fg(palette.white)));

    if available > 1 {
        let mut secondary = StyledLine::blank();
        secondary.push("      ", palette.white);
        let available_detail = width.saturating_sub(secondary.width());
        secondary.push(truncate_right(detail, available_detail), palette.overlay0);
        lines.push(secondary.end(CellStyle::fg(palette.white)));
    }

    while lines.len() - start_len < available {
        lines.push(StyledLine::blank());
    }
}

fn push_group_summary(
    line: &mut StyledLine,
    palette: &Palette,
    summary: &crate::session_display::GroupSummary,
    collapsed: bool,
    width: usize,
) {
    let mut wrote = false;
    if collapsed && summary.running_agents > 0 {
        let text = format!("⚡{}", summary.running_agents);
        if line.width() + text.width() <= width {
            line.push(text, palette.yellow);
            wrote = true;
        }
    }
    if collapsed && summary.unseen > 0 {
        let text = if wrote { " ●" } else { "●" };
        if line.width() + text.width() <= width {
            line.push(text, palette.teal);
            wrote = true;
        }
    }
    if collapsed && (summary.insertions > 0 || summary.deletions > 0) {
        let text = format!(" +{} -{}", summary.insertions, summary.deletions);
        if line.width() + text.width() <= width {
            line.push(text, palette.green);
            wrote = true;
        }
    }
    if collapsed && let Some(port) = summary.first_port {
        let text = if summary.extra_ports == 0 {
            format!(" ⌁{port}")
        } else {
            format!(" ⌁{port}+{}", summary.extra_ports)
        };
        if line.width() + text.width() <= width {
            line.push(text, palette.sky);
            wrote = true;
        }
    }
    let _ = wrote;
}

fn compute_session_window_start_for_focus(
    rows: &[SessionListRow<'_>],
    focused_idx: usize,
    available_rows: usize,
) -> usize {
    let mut start = focused_idx;
    let mut used = 1;
    while start > 0 {
        if used + 1 > available_rows {
            break;
        }
        start -= 1;
        used += 1;
    }
    while start < rows.len()
        && matches!(
            rows[start],
            SessionListRow::SessionDetail { .. } | SessionListRow::Spacer { .. }
        )
    {
        start += 1;
    }
    start
}

fn build_session_name_row(
    app: &App,
    palette: &Palette,
    idx: usize,
    session: &SessionData,
    indented: bool,
) -> StyledLine {
    let index = idx;
    let focused = app.focused_session_name() == Some(session.name.as_str());
    let current = app.current_session.as_deref() == Some(session.name.as_str());
    let bg = focused.then_some(palette.surface1);
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
    let marker_color = if current {
        palette.green
    } else {
        palette.lavender
    };
    let marker = if current {
        "▌"
    } else if focused {
        "›"
    } else {
        " "
    };
    if indented {
        row.push("   │", palette.surface2);
        row.push(marker, marker_color);
    } else {
        row.push(format!("{marker}  "), marker_color);
    }
    row.push(format!("{index:02}  "), index_color);
    row.push(&session.name, name_color);
    row.end(CellStyle {
        fg: palette.white,
        bg,
    })
    .with_hit(hit)
}

fn build_session_detail_row(
    app: &App,
    palette: &Palette,
    session: &SessionData,
    indented: bool,
    width: usize,
) -> StyledLine {
    let focused = app.focused_session_name() == Some(session.name.as_str());
    let bg = focused.then_some(palette.surface1);
    let hit = HitTarget::Session(session.name.clone());
    let flashed = app.active_flash_target() == Some(&hit);
    let bg = if flashed { Some(palette.surface1) } else { bg };
    let has_ports = !session.ports.is_empty();
    let has_diff_stats = session.insertions > 0 || session.deletions > 0;
    let detail_text = if !session.branch.is_empty() {
        Some(session.branch.as_str())
    } else {
        dir_name(session)
    };
    let branch_color = if focused {
        palette.pink
    } else {
        palette.overlay0
    };
    let port_color = if focused {
        palette.sky
    } else {
        palette.overlay0
    };
    let mut line = StyledLine::with_bg(bg);
    line.push(
        if indented { "   │     " } else { "       " },
        palette.surface2,
    );
    let mut suffix_width = 0;
    if has_ports {
        suffix_width += if session.ports.len() == 1 {
            format!("  ⌁{}", session.ports[0]).width()
        } else {
            format!("  ⌁{}+{}", session.ports[0], session.ports.len() - 1).width()
        };
    }
    if has_diff_stats {
        suffix_width += diff_stats_width(session);
    }
    if let Some(detail_text) = detail_text {
        let available = width.saturating_sub(line.width() + suffix_width);
        line.push(truncate_right(detail_text, available), branch_color);
    }
    if has_ports {
        let port_text = if session.ports.len() == 1 {
            format!("  ⌁{}", session.ports[0])
        } else {
            format!("  ⌁{}+{}", session.ports[0], session.ports.len() - 1)
        };
        line.push(port_text, port_color);
    }
    if has_diff_stats {
        let spaces = width.saturating_sub(line.width() + diff_stats_width(session));
        line.push_hit(
            " ".repeat(spaces),
            palette.white,
            HitTarget::DiffCount(session.name.clone()),
        );
        push_diff_stats(&mut line, palette, session, app);
    }
    line.end(CellStyle {
        fg: palette.white,
        bg,
    })
    .with_hit(hit)
}

fn rail_blank(palette: &Palette) -> StyledLine {
    StyledLine::blank().end(CellStyle::fg(palette.white))
}

fn inner_rail_blank(palette: &Palette) -> StyledLine {
    let mut line = StyledLine::blank();
    line.push("   │", palette.surface2);
    line.end(CellStyle::fg(palette.white))
}

fn diff_stats_width(session: &SessionData) -> usize {
    let insertions = format!(" +{}", session.insertions);
    let deletions = format!(" -{}", session.deletions);
    insertions.width() + deletions.width()
}

fn push_diff_stats(line: &mut StyledLine, palette: &Palette, session: &SessionData, app: &App) {
    let hit = HitTarget::DiffCount(session.name.clone());
    let hovered = app.hover_target.as_ref() == Some(&hit);
    let additions_bg = hovered.then_some(palette.surface2);
    let deletions_bg = hovered.then_some(palette.surface2);
    line.push_hit_with_bg(
        format!(" +{}", session.insertions),
        palette.green,
        additions_bg,
        hit.clone(),
    );
    line.push_hit_with_bg(
        format!(" -{}", session.deletions),
        palette.red,
        deletions_bg,
        hit,
    );
}

fn render_detail(
    app: &App,
    palette: &Palette,
    lines: &mut Vec<StyledLine>,
    max_lines: usize,
    width: usize,
) {
    let Some(session) = app
        .focused_session_name()
        .and_then(|focused| app.sessions.iter().find(|session| session.name == focused))
    else {
        return;
    };

    if lines.len() >= max_lines {
        return;
    }

    render_agent_panel_header(app, palette, lines, max_lines, width);

    if app.agent_panel_scope == AgentPanelScope::All {
        render_agent_blocks(app, palette, lines, max_lines, all_agent_entries(app));
        return;
    }

    let mut path = StyledLine::blank();
    path.push(" ", palette.white);
    path.push(truncate_left(&session.dir, 24), palette.overlay0);
    lines.push(path.end(CellStyle::fg(palette.white)));

    for (i, link) in session.local_links.iter().enumerate() {
        if lines.len() >= max_lines {
            break;
        }
        let mut line = StyledLine::blank();
        line.push(" ", palette.white);
        if i == 0 {
            line.push("local ", palette.overlay0);
        } else {
            line.push("      ", palette.white);
        }
        let label = if link.label.is_empty() {
            &link.url
        } else {
            &link.label
        };
        line.push(label, palette.sky);
        lines.push(line.end(CellStyle::fg(palette.white)));
    }

    render_agent_blocks(
        app,
        palette,
        lines,
        max_lines,
        current_agent_entries(session),
    );

    render_metadata(session, palette, lines, max_lines);
}

fn render_agent_panel_header(
    app: &App,
    palette: &Palette,
    lines: &mut Vec<StyledLine>,
    max_lines: usize,
    width: usize,
) {
    if lines.len() >= max_lines {
        return;
    }

    let scope = match app.agent_panel_scope {
        AgentPanelScope::Current => "current",
        AgentPanelScope::All => "all",
    };
    let agent_count = match app.agent_panel_scope {
        AgentPanelScope::Current => app
            .focused_session_name()
            .and_then(|focused| app.sessions.iter().find(|session| session.name == focused))
            .map(|session| session.agents.len())
            .unwrap_or(0),
        AgentPanelScope::All => app
            .sessions
            .iter()
            .map(|session| session.agents.len())
            .sum(),
    };

    let mut line = StyledLine::blank();
    line.push(" ", palette.white);
    line.push("agents", palette.subtext0);
    if agent_count > 0 {
        line.push(format!(" {agent_count}"), palette.overlay0);
    }
    let right = format!("a:{scope}");
    if line.width() + 1 + right.width() <= width {
        let spaces = width.saturating_sub(line.width() + right.width());
        line.push(" ".repeat(spaces), palette.white);
        line.push_hit(right, palette.overlay0, HitTarget::AgentScopeToggle);
    } else if line.width() + 1 + scope.width() <= width {
        let spaces = width.saturating_sub(line.width() + scope.width());
        line.push(" ".repeat(spaces), palette.white);
        line.push_hit(scope, palette.overlay0, HitTarget::AgentScopeToggle);
    }
    lines.push(line.end(CellStyle::fg(palette.white)));
}

#[derive(Debug, Clone, Copy)]
struct AgentPanelEntry<'a> {
    session: &'a SessionData,
    agent: &'a AgentEvent,
    global_idx: usize,
}

fn current_agent_entries(session: &SessionData) -> Vec<AgentPanelEntry<'_>> {
    session
        .agents
        .iter()
        .enumerate()
        .map(|(global_idx, agent)| AgentPanelEntry {
            session,
            agent,
            global_idx,
        })
        .collect()
}

fn all_agent_entries(app: &App) -> Vec<AgentPanelEntry<'_>> {
    app.sessions
        .iter()
        .flat_map(|session| session.agents.iter().map(move |agent| (session, agent)))
        .enumerate()
        .map(|(global_idx, (session, agent))| AgentPanelEntry {
            session,
            agent,
            global_idx,
        })
        .collect()
}

fn render_agent_blocks(
    app: &App,
    palette: &Palette,
    lines: &mut Vec<StyledLine>,
    max_lines: usize,
    entries: Vec<AgentPanelEntry<'_>>,
) {
    if entries.is_empty() || lines.len() >= max_lines {
        if app.initializing && lines.len() + 2 <= max_lines {
            lines.push(StyledLine::blank());
            let mut primary = StyledLine::blank();
            primary.push("  ", palette.white);
            primary.push(agent_spinner(spinner_clock(app)), palette.yellow);
            primary.push("  ", palette.white);
            primary.push("checking agents", palette.subtext0);
            lines.push(primary.end(CellStyle::fg(palette.white)));
        }
        return;
    }

    lines.push(StyledLine::blank());

    let agents_available = max_lines.saturating_sub(lines.len());
    if agents_available == 0 {
        return;
    }

    let blocks: Vec<Vec<StyledLine>> = entries
        .into_iter()
        .map(|entry| agent_panel_block(app, palette, entry))
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
                break;
            }
            lines.push(line.clone());
            consumed += 1;
        }
    }
}

fn render_metadata(
    session: &SessionData,
    palette: &Palette,
    lines: &mut Vec<StyledLine>,
    max_lines: usize,
) {
    let Some(metadata) = &session.metadata else {
        return;
    };
    let has_status = metadata.status.is_some();
    let has_progress = metadata.progress.is_some();
    let has_logs = !metadata.logs.is_empty();
    if !has_status && !has_progress && !has_logs {
        return;
    }

    if lines.len() >= max_lines {
        return;
    }
    lines.push(StyledLine::blank());

    if let Some(status) = &metadata.status {
        if lines.len() >= max_lines {
            return;
        }
        let tone = status.tone;
        let mut line = StyledLine::blank();
        line.push("  ", palette.white);
        line.push(tone_icon(tone), tone_color(palette, tone));
        line.push(format!(" {}", status.text), tone_color(palette, tone));
        if let Some(progress) = &metadata.progress {
            if let (Some(current), Some(total)) = (progress.current, progress.total) {
                line.push(format!(" · {current}/{total}"), palette.sky);
            } else if let Some(percent) = progress.percent {
                line.push(format!(" · {percent:.0}%"), palette.sky);
            }
        }
        lines.push(line.end(CellStyle::fg(palette.white)));
    } else if let Some(progress) = &metadata.progress {
        if lines.len() >= max_lines {
            return;
        }
        let mut line = StyledLine::blank();
        line.push("  ", palette.white);
        if let (Some(current), Some(total)) = (progress.current, progress.total) {
            line.push(format!("{current}/{total}"), palette.sky);
        } else if let Some(percent) = progress.percent {
            line.push(format!("{percent:.0}%"), palette.sky);
        }
        lines.push(line.end(CellStyle::fg(palette.white)));
    }

    let log_start = metadata.logs.len().saturating_sub(8);
    for entry in &metadata.logs[log_start..] {
        if lines.len() >= max_lines {
            return;
        }
        let tone = entry.tone;
        let mut line = StyledLine::blank();
        line.push("  ", palette.white);
        line.push(tone_icon(tone), tone_color(palette, tone));
        if let Some(source) = &entry.source {
            line.push(format!(" [{source}]"), palette.surface2);
        }
        line.push(format!(" {}", entry.message), palette.overlay0);
        lines.push(line.end(CellStyle::fg(palette.white)));
    }
}

fn render_modal_overlay(
    app: &App,
    palette: &Palette,
    lines: &mut [StyledLine],
    width: usize,
    height: usize,
) {
    match &app.modal {
        Modal::ThemePicker {
            query, selected, ..
        } => render_theme_picker_overlay(palette, lines, width, height, query, *selected),
        Modal::KillConfirm { session_name } => {
            render_kill_confirm_overlay(palette, lines, width, height, session_name)
        }
        Modal::None => {}
    }
}

fn render_theme_picker_overlay(
    palette: &Palette,
    lines: &mut [StyledLine],
    width: usize,
    height: usize,
    query: &str,
    selected: usize,
) {
    let box_width: usize = 28;
    let visible_items: usize = 12;
    // title + search + blank + items + blank + footer
    let box_height = 4 + visible_items + 1;
    if height < box_height + 2 || width < box_width + 2 {
        return;
    }

    let filtered: Vec<&str> = THEME_NAMES
        .iter()
        .copied()
        .filter(|name| query.is_empty() || name.contains(&query.to_lowercase()))
        .collect();

    let start_y = (height.saturating_sub(box_height)) / 2;
    let start_x = (width.saturating_sub(box_width)) / 2;
    let border_color = palette.blue;
    let inner_width = box_width - 2;

    // Top border
    let mut top = StyledLine::blank();
    top.push(" ".repeat(start_x), palette.white);
    top.push("╭", border_color);
    top.push("─".repeat(inner_width), border_color);
    top.push("╮", border_color);
    if start_y < lines.len() {
        lines[start_y] = top;
    }

    let mut row = start_y + 1;

    // Title row
    let title = "Select Theme";
    let title_pad = inner_width.saturating_sub(title.width());
    let left_pad = title_pad / 2;
    let right_pad = title_pad - left_pad;
    let mut title_line = StyledLine::blank();
    title_line.push(" ".repeat(start_x), palette.white);
    title_line.push("│", border_color);
    title_line.push(" ".repeat(left_pad), palette.white);
    title_line.push(title, palette.blue);
    title_line.push(" ".repeat(right_pad), palette.white);
    title_line.push("│", border_color);
    if row < lines.len() {
        lines[row] = title_line;
    }
    row += 1;

    // Search row
    let search_display = if query.is_empty() {
        "search…".to_string()
    } else {
        query.to_string()
    };
    let search_color = if query.is_empty() {
        palette.overlay0
    } else {
        palette.text
    };
    let search_pad = inner_width.saturating_sub(search_display.width() + 2);
    let mut search_line = StyledLine::blank();
    search_line.push(" ".repeat(start_x), palette.white);
    search_line.push("│", border_color);
    search_line.push(" ", palette.white);
    search_line.push(&search_display, search_color);
    search_line.push(" ".repeat(search_pad + 1), palette.white);
    search_line.push("│", border_color);
    if row < lines.len() {
        lines[row] = search_line;
    }
    row += 1;

    // Blank separator
    let mut blank = StyledLine::blank();
    blank.push(" ".repeat(start_x), palette.white);
    blank.push("│", border_color);
    blank.push(" ".repeat(inner_width), palette.white);
    blank.push("│", border_color);
    if row < lines.len() {
        lines[row] = blank;
    }
    row += 1;

    // Items
    let total = filtered.len();
    let scroll_offset = if selected >= visible_items {
        selected - visible_items + 1
    } else {
        0
    };
    let visible_end = (scroll_offset + visible_items).min(total);

    for i in 0..visible_items {
        let idx = scroll_offset + i;
        let mut item_line = StyledLine::blank();
        item_line.push(" ".repeat(start_x), palette.white);
        item_line.push("│", border_color);
        if idx < total {
            let name = filtered[idx];
            let is_selected = idx == selected;
            let prefix = if is_selected { "▸ " } else { "  " };
            let name_color = if is_selected {
                palette.text
            } else {
                palette.subtext0
            };
            let display = format!("{prefix}{name}");
            let pad = inner_width.saturating_sub(display.width());
            item_line.push(display, name_color);
            item_line.push(" ".repeat(pad), palette.white);
        } else {
            item_line.push(" ".repeat(inner_width), palette.white);
        }
        item_line.push("│", border_color);
        if row < lines.len() {
            lines[row] = item_line;
        }
        row += 1;
    }

    // More indicator / blank
    let mut more_line = StyledLine::blank();
    more_line.push(" ".repeat(start_x), palette.white);
    more_line.push("│", border_color);
    let hidden_above = scroll_offset;
    let hidden_below = total.saturating_sub(visible_end);
    if hidden_above > 0 || hidden_below > 0 {
        let indicator = format!("↕ {} more", hidden_above + hidden_below);
        let pad = inner_width.saturating_sub(indicator.width() + 1);
        more_line.push(" ", palette.white);
        more_line.push(indicator, palette.overlay0);
        more_line.push(" ".repeat(pad), palette.white);
    } else {
        more_line.push(" ".repeat(inner_width), palette.white);
    }
    more_line.push("│", border_color);
    if row < lines.len() {
        lines[row] = more_line;
    }
    row += 1;

    // Bottom border
    let mut bottom = StyledLine::blank();
    bottom.push(" ".repeat(start_x), palette.white);
    bottom.push("╰", border_color);
    bottom.push("─".repeat(inner_width), border_color);
    bottom.push("╯", border_color);
    if row < lines.len() {
        lines[row] = bottom;
    }
}

fn render_kill_confirm_overlay(
    palette: &Palette,
    lines: &mut [StyledLine],
    width: usize,
    height: usize,
    session_name: &str,
) {
    let box_width: usize = 30.max(session_name.width() + 6);
    let box_height: usize = 5;
    if height < box_height + 2 || width < box_width + 2 {
        return;
    }

    let start_y = (height.saturating_sub(box_height)) / 2;
    let start_x = (width.saturating_sub(box_width)) / 2;
    let border_color = palette.red;
    let inner_width = box_width - 2;

    let make_inner = |content: &str, color: Rgb| -> StyledLine {
        let pad = inner_width.saturating_sub(content.width());
        let left = pad / 2;
        let right = pad - left;
        let mut line = StyledLine::blank();
        line.push(" ".repeat(start_x), palette.white);
        line.push("│", border_color);
        line.push(" ".repeat(left), palette.white);
        line.push(content, color);
        line.push(" ".repeat(right), palette.white);
        line.push("│", border_color);
        line
    };

    // Top border
    let mut top = StyledLine::blank();
    top.push(" ".repeat(start_x), palette.white);
    top.push("╭", border_color);
    top.push("─".repeat(inner_width), border_color);
    top.push("╮", border_color);

    let title_line = make_inner("Kill session?", palette.red);
    let name_line = make_inner(session_name, palette.text);
    let hint_line = make_inner("y / n", palette.overlay0);

    // Bottom border
    let mut bottom = StyledLine::blank();
    bottom.push(" ".repeat(start_x), palette.white);
    bottom.push("╰", border_color);
    bottom.push("─".repeat(inner_width), border_color);
    bottom.push("╯", border_color);

    let rows = [top, title_line, name_line, hint_line, bottom];
    for (i, row) in rows.into_iter().enumerate() {
        let y = start_y + i;
        if y < lines.len() {
            lines[y] = row;
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

fn agent_panel_block(app: &App, palette: &Palette, entry: AgentPanelEntry<'_>) -> Vec<StyledLine> {
    if app.agent_panel_scope == AgentPanelScope::All {
        return compact_agent_panel_block(app, palette, entry);
    }

    let focused = app.panel_focus == crate::app::PanelFocus::Agents
        && app.focused_agent_idx == entry.global_idx;
    let hit = HitTarget::Agent(entry.global_idx);
    let flashed = app.active_flash_target() == Some(&hit);
    let highlight = focused || flashed;
    let bg = highlight.then_some(palette.surface1);

    let mut primary = StyledLine::with_bg(bg);
    primary.push("  ", palette.white);
    let (icon, icon_color) = detail_status_icon_for_agent(
        palette,
        entry.agent,
        entry.session.unseen,
        spinner_clock(app),
    );
    primary.push(icon, icon_color);
    primary.push(" ", palette.white);
    if let Some(thread_name) = entry.agent.thread_name.as_deref() {
        primary.push(
            truncate_right(thread_name, 30),
            if highlight {
                palette.text
            } else {
                palette.subtext1
            },
        );
    } else {
        primary.push(
            &entry.agent.agent,
            if highlight {
                palette.text
            } else {
                palette.subtext1
            },
        );
    }

    let mut secondary = StyledLine::with_bg(bg);
    secondary.push("    ", palette.white);
    secondary.push(
        semantic_agent_status_text(entry.agent, entry.session.unseen),
        agent_detail_color(palette, entry.agent.status, entry.session.unseen),
    );
    secondary.push(" · ", palette.overlay0);
    secondary.push(&entry.agent.agent, palette.overlay0);

    vec![
        primary
            .end(CellStyle {
                fg: palette.white,
                bg,
            })
            .with_hit(hit.clone()),
        secondary
            .end(CellStyle {
                fg: palette.white,
                bg,
            })
            .with_hit(hit),
    ]
}

fn compact_agent_panel_block(
    app: &App,
    palette: &Palette,
    entry: AgentPanelEntry<'_>,
) -> Vec<StyledLine> {
    let focused = app.panel_focus == crate::app::PanelFocus::Agents
        && app.focused_agent_idx == entry.global_idx;
    let hit = HitTarget::Agent(entry.global_idx);
    let bg = focused.then_some(palette.surface1);
    let mut line = StyledLine::with_bg(bg);
    line.push("  ", palette.white);
    let (icon, icon_color) = detail_status_icon_for_agent(
        palette,
        entry.agent,
        entry.session.unseen,
        spinner_clock(app),
    );
    line.push(icon, icon_color);
    line.push(" ", palette.white);
    if let Some(thread_name) = entry.agent.thread_name.as_deref() {
        line.push(truncate_right(thread_name, 26), palette.subtext1);
    } else {
        line.push(&entry.session.name, palette.subtext1);
        line.push(" · ", palette.overlay0);
        line.push(entry.agent.agent.as_str(), palette.overlay1);
        return vec![
            line.end(CellStyle {
                fg: palette.white,
                bg,
            })
            .with_hit(hit),
        ];
    }
    vec![
        line.end(CellStyle {
            fg: palette.white,
            bg,
        })
        .with_hit(hit),
    ]
}

fn semantic_agent_status_text(agent: &AgentEvent, session_unseen: bool) -> &'static str {
    match agent.status {
        AgentStatus::Waiting => "blocked",
        AgentStatus::Running | AgentStatus::ToolRunning => "working",
        AgentStatus::Done if session_unseen || agent.unseen == Some(true) => "done",
        AgentStatus::Done | AgentStatus::Idle => "idle",
        AgentStatus::Error => "error",
        AgentStatus::Stale => "stale",
        AgentStatus::Interrupted => "stopped",
    }
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

fn footer(palette: &Palette, width: usize) -> [StyledLine; 2] {
    let hints: &[(&str, Rgb)] = &[
        (" ", palette.white),
        ("⇥", palette.overlay0),
        (" cycle  ", palette.overlay1),
        ("⏎", palette.overlay0),
        (" go  ", palette.overlay1),
        ("→", palette.overlay0),
        (" agents  ", palette.overlay1),
        ("f", palette.overlay0),
        (" filter  ", palette.overlay1),
        ("d", palette.overlay0),
        (" hide  ", palette.overlay1),
        ("x", palette.overlay0),
        (" kill", palette.overlay1),
    ];
    let mut line = StyledLine::blank();
    let mut wrapped = StyledLine::blank();
    let mut on_wrapped_line = false;
    for &(text, color) in hints {
        if !on_wrapped_line && line.width() + text.width() > width {
            on_wrapped_line = true;
        }
        if on_wrapped_line {
            if wrapped.width() + text.width() <= width {
                wrapped.push(text, color);
            }
        } else {
            line.push(text, color);
        }
    }
    [line, wrapped.end(CellStyle::fg(palette.white))]
}

fn separator(palette: &Palette, width: usize) -> StyledLine {
    let mut line = StyledLine::blank();
    line.push(" ", palette.white);
    line.push("─".repeat(width.saturating_sub(1)), palette.surface2);
    line
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

/// 10-frame braille spinner used for agents in `Running` / `ToolRunning`
/// state, matching `apps/tui/src/index.tsx::SPINNERS`. Frame cadence is
/// 120ms — the same period as the render tick in `apps/tui-rs/src/main.rs`,
/// so the glyph advances exactly once per tick (smooth, no stutter).
fn agent_spinner(ts: u64) -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
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

fn truncate_right(value: &str, max_cols: usize) -> String {
    if value.width() <= max_cols {
        return value.to_string();
    }
    if max_cols == 0 {
        return String::new();
    }

    let mut result = String::new();
    for ch in value.chars() {
        let next = format!("{result}{ch}…");
        if next.width() > max_cols {
            break;
        }
        result.push(ch);
    }
    format!("{result}…")
}

fn tone_icon(tone: Option<MetadataTone>) -> &'static str {
    match tone {
        Some(MetadataTone::Info) => "ℹ",
        Some(MetadataTone::Success) => "✓",
        Some(MetadataTone::Warn) => "⚠",
        Some(MetadataTone::Error) => "✗",
        _ => "·",
    }
}

fn tone_color(palette: &Palette, tone: Option<MetadataTone>) -> Rgb {
    match tone {
        Some(MetadataTone::Success) => palette.green,
        Some(MetadataTone::Error) => palette.red,
        Some(MetadataTone::Warn) => palette.yellow,
        Some(MetadataTone::Info) => palette.blue,
        _ => palette.overlay0,
    }
}

#[derive(Debug, Clone)]
struct StyledLine {
    parts: Vec<StyledPart>,
    bg: Option<Rgb>,
    end_style: Option<CellStyle>,
    hit: Option<HitTarget>,
}

impl StyledLine {
    fn blank() -> Self {
        Self {
            parts: Vec::new(),
            bg: None,
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

    fn with_hit(mut self, hit: HitTarget) -> Self {
        self.hit = Some(hit);
        self
    }

    fn push(&mut self, text: impl Into<String>, fg: Rgb) {
        self.parts.push(StyledPart {
            text: text.into(),
            style: CellStyle { fg, bg: self.bg },
            hit: None,
        });
    }

    fn push_hit(&mut self, text: impl Into<String>, fg: Rgb, hit: HitTarget) {
        self.parts.push(StyledPart {
            text: text.into(),
            style: CellStyle { fg, bg: self.bg },
            hit: Some(hit),
        });
    }

    fn push_hit_with_bg(
        &mut self,
        text: impl Into<String>,
        fg: Rgb,
        bg: Option<Rgb>,
        hit: HitTarget,
    ) {
        self.parts.push(StyledPart {
            text: text.into(),
            style: CellStyle {
                fg,
                bg: bg.or(self.bg),
            },
            hit: Some(hit),
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

    fn hit_at(&self, x: usize) -> Option<HitTarget> {
        let mut offset = 0;
        for part in &self.parts {
            let width = part.text.as_str().width();
            if x >= offset && x < offset + width {
                return part.hit.clone();
            }
            offset += width;
        }
        None
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
    hit: Option<HitTarget>,
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

const CATPPUCCIN_FRAPPE: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(141, 164, 226),
    lavender: Rgb::new(186, 187, 241),
    pink: Rgb::new(244, 184, 228),
    yellow: Rgb::new(229, 200, 144),
    green: Rgb::new(166, 209, 137),
    red: Rgb::new(231, 130, 132),
    peach: Rgb::new(239, 159, 118),
    teal: Rgb::new(129, 200, 190),
    sky: Rgb::new(153, 209, 219),
    text: Rgb::new(198, 208, 245),
    subtext0: Rgb::new(165, 173, 206),
    subtext1: Rgb::new(181, 191, 226),
    overlay0: Rgb::new(98, 104, 128),
    overlay1: Rgb::new(81, 87, 109),
    surface1: Rgb::new(81, 87, 109),
    surface2: Rgb::new(98, 104, 128),
};

const CATPPUCCIN_MACCHIATO: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(138, 173, 244),
    lavender: Rgb::new(183, 189, 248),
    pink: Rgb::new(245, 189, 230),
    yellow: Rgb::new(238, 212, 159),
    green: Rgb::new(166, 218, 149),
    red: Rgb::new(237, 135, 150),
    peach: Rgb::new(245, 169, 127),
    teal: Rgb::new(139, 213, 202),
    sky: Rgb::new(145, 215, 227),
    text: Rgb::new(202, 211, 245),
    subtext0: Rgb::new(165, 173, 203),
    subtext1: Rgb::new(184, 192, 224),
    overlay0: Rgb::new(91, 96, 120),
    overlay1: Rgb::new(73, 77, 100),
    surface1: Rgb::new(73, 77, 100),
    surface2: Rgb::new(91, 96, 120),
};

const TOKYO_NIGHT: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(122, 162, 247),
    lavender: Rgb::new(187, 154, 247),
    pink: Rgb::new(187, 154, 247),
    yellow: Rgb::new(224, 175, 104),
    green: Rgb::new(158, 206, 106),
    red: Rgb::new(247, 118, 142),
    peach: Rgb::new(255, 158, 100),
    teal: Rgb::new(115, 218, 202),
    sky: Rgb::new(125, 207, 255),
    text: Rgb::new(192, 202, 245),
    subtext0: Rgb::new(169, 177, 214),
    subtext1: Rgb::new(154, 165, 206),
    overlay0: Rgb::new(86, 95, 137),
    overlay1: Rgb::new(65, 72, 104),
    surface1: Rgb::new(41, 46, 66),
    surface2: Rgb::new(52, 58, 82),
};

const GRUVBOX_DARK: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(131, 165, 152),
    lavender: Rgb::new(211, 134, 155),
    pink: Rgb::new(211, 134, 155),
    yellow: Rgb::new(250, 189, 47),
    green: Rgb::new(184, 187, 38),
    red: Rgb::new(251, 73, 52),
    peach: Rgb::new(254, 128, 25),
    teal: Rgb::new(142, 192, 124),
    sky: Rgb::new(131, 165, 152),
    text: Rgb::new(235, 219, 178),
    subtext0: Rgb::new(213, 196, 161),
    subtext1: Rgb::new(189, 174, 147),
    overlay0: Rgb::new(102, 92, 84),
    overlay1: Rgb::new(124, 111, 100),
    surface1: Rgb::new(80, 73, 69),
    surface2: Rgb::new(102, 92, 84),
};

const NORD: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(129, 161, 193),
    lavender: Rgb::new(180, 142, 173),
    pink: Rgb::new(180, 142, 173),
    yellow: Rgb::new(235, 203, 139),
    green: Rgb::new(163, 190, 140),
    red: Rgb::new(191, 97, 106),
    peach: Rgb::new(208, 135, 112),
    teal: Rgb::new(143, 188, 187),
    sky: Rgb::new(136, 192, 208),
    text: Rgb::new(236, 239, 244),
    subtext0: Rgb::new(216, 222, 233),
    subtext1: Rgb::new(229, 233, 240),
    overlay0: Rgb::new(76, 86, 106),
    overlay1: Rgb::new(67, 76, 94),
    surface1: Rgb::new(67, 76, 94),
    surface2: Rgb::new(76, 86, 106),
};

const DRACULA: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(139, 233, 253),
    lavender: Rgb::new(189, 147, 249),
    pink: Rgb::new(255, 121, 198),
    yellow: Rgb::new(241, 250, 140),
    green: Rgb::new(80, 250, 123),
    red: Rgb::new(255, 85, 85),
    peach: Rgb::new(255, 184, 108),
    teal: Rgb::new(139, 233, 253),
    sky: Rgb::new(139, 233, 253),
    text: Rgb::new(248, 248, 242),
    subtext0: Rgb::new(191, 191, 191),
    subtext1: Rgb::new(98, 114, 164),
    overlay0: Rgb::new(98, 114, 164),
    overlay1: Rgb::new(86, 87, 97),
    surface1: Rgb::new(68, 71, 90),
    surface2: Rgb::new(98, 114, 164),
};

const GITHUB_DARK: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(88, 166, 255),
    lavender: Rgb::new(188, 140, 255),
    pink: Rgb::new(188, 140, 255),
    yellow: Rgb::new(227, 179, 65),
    green: Rgb::new(63, 185, 80),
    red: Rgb::new(248, 81, 73),
    peach: Rgb::new(210, 153, 34),
    teal: Rgb::new(57, 197, 207),
    sky: Rgb::new(88, 166, 255),
    text: Rgb::new(201, 209, 217),
    subtext0: Rgb::new(139, 148, 158),
    subtext1: Rgb::new(177, 186, 196),
    overlay0: Rgb::new(72, 79, 88),
    overlay1: Rgb::new(48, 54, 61),
    surface1: Rgb::new(33, 38, 45),
    surface2: Rgb::new(48, 54, 61),
};

const ONE_DARK: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(97, 175, 239),
    lavender: Rgb::new(198, 120, 221),
    pink: Rgb::new(198, 120, 221),
    yellow: Rgb::new(229, 192, 123),
    green: Rgb::new(152, 195, 121),
    red: Rgb::new(224, 108, 117),
    peach: Rgb::new(209, 154, 102),
    teal: Rgb::new(86, 182, 194),
    sky: Rgb::new(97, 175, 239),
    text: Rgb::new(171, 178, 191),
    subtext0: Rgb::new(130, 137, 151),
    subtext1: Rgb::new(92, 99, 112),
    overlay0: Rgb::new(92, 99, 112),
    overlay1: Rgb::new(75, 82, 99),
    surface1: Rgb::new(75, 82, 99),
    surface2: Rgb::new(92, 99, 112),
};

const KANAGAWA: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(126, 156, 216),
    lavender: Rgb::new(149, 127, 184),
    pink: Rgb::new(210, 126, 153),
    yellow: Rgb::new(215, 166, 87),
    green: Rgb::new(152, 187, 108),
    red: Rgb::new(232, 36, 36),
    peach: Rgb::new(255, 160, 102),
    teal: Rgb::new(122, 168, 159),
    sky: Rgb::new(127, 180, 202),
    text: Rgb::new(220, 215, 186),
    subtext0: Rgb::new(200, 192, 147),
    subtext1: Rgb::new(114, 113, 105),
    overlay0: Rgb::new(114, 113, 105),
    overlay1: Rgb::new(84, 84, 109),
    surface1: Rgb::new(84, 84, 109),
    surface2: Rgb::new(114, 113, 105),
};

const EVERFOREST: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(127, 187, 179),
    lavender: Rgb::new(214, 153, 182),
    pink: Rgb::new(214, 153, 182),
    yellow: Rgb::new(219, 188, 127),
    green: Rgb::new(167, 192, 128),
    red: Rgb::new(230, 126, 128),
    peach: Rgb::new(230, 152, 117),
    teal: Rgb::new(131, 192, 146),
    sky: Rgb::new(127, 187, 179),
    text: Rgb::new(211, 198, 170),
    subtext0: Rgb::new(157, 169, 160),
    subtext1: Rgb::new(122, 132, 120),
    overlay0: Rgb::new(122, 132, 120),
    overlay1: Rgb::new(133, 146, 137),
    surface1: Rgb::new(61, 72, 77),
    surface2: Rgb::new(71, 82, 88),
};

const MATERIAL: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(130, 170, 255),
    lavender: Rgb::new(199, 146, 234),
    pink: Rgb::new(199, 146, 234),
    yellow: Rgb::new(255, 203, 107),
    green: Rgb::new(195, 232, 141),
    red: Rgb::new(240, 113, 120),
    peach: Rgb::new(247, 140, 108),
    teal: Rgb::new(137, 221, 255),
    sky: Rgb::new(130, 170, 255),
    text: Rgb::new(238, 255, 255),
    subtext0: Rgb::new(176, 190, 197),
    subtext1: Rgb::new(84, 110, 122),
    overlay0: Rgb::new(84, 110, 122),
    overlay1: Rgb::new(55, 71, 79),
    surface1: Rgb::new(69, 90, 100),
    surface2: Rgb::new(84, 110, 122),
};

const COBALT2: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(0, 136, 255),
    lavender: Rgb::new(154, 95, 235),
    pink: Rgb::new(255, 157, 0),
    yellow: Rgb::new(255, 198, 0),
    green: Rgb::new(158, 255, 128),
    red: Rgb::new(255, 0, 136),
    peach: Rgb::new(255, 98, 140),
    teal: Rgb::new(42, 255, 223),
    sky: Rgb::new(0, 136, 255),
    text: Rgb::new(255, 255, 255),
    subtext0: Rgb::new(173, 183, 201),
    subtext1: Rgb::new(102, 136, 170),
    overlay0: Rgb::new(45, 90, 123),
    overlay1: Rgb::new(31, 70, 98),
    surface1: Rgb::new(35, 75, 107),
    surface2: Rgb::new(45, 90, 123),
};

const FLEXOKI: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(67, 133, 190),
    lavender: Rgb::new(139, 126, 200),
    pink: Rgb::new(206, 93, 151),
    yellow: Rgb::new(208, 162, 21),
    green: Rgb::new(135, 154, 57),
    red: Rgb::new(209, 77, 65),
    peach: Rgb::new(218, 112, 44),
    teal: Rgb::new(58, 169, 159),
    sky: Rgb::new(67, 133, 190),
    text: Rgb::new(206, 205, 195),
    subtext0: Rgb::new(183, 181, 172),
    subtext1: Rgb::new(135, 133, 128),
    overlay0: Rgb::new(111, 110, 105),
    overlay1: Rgb::new(87, 86, 83),
    surface1: Rgb::new(52, 51, 49),
    surface2: Rgb::new(64, 62, 60),
};

const AYU: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(89, 194, 255),
    lavender: Rgb::new(210, 166, 255),
    pink: Rgb::new(240, 113, 120),
    yellow: Rgb::new(230, 180, 80),
    green: Rgb::new(127, 217, 98),
    red: Rgb::new(217, 87, 87),
    peach: Rgb::new(255, 143, 64),
    teal: Rgb::new(149, 230, 203),
    sky: Rgb::new(57, 186, 230),
    text: Rgb::new(191, 189, 182),
    subtext0: Rgb::new(172, 182, 191),
    subtext1: Rgb::new(86, 91, 102),
    overlay0: Rgb::new(86, 91, 102),
    overlay1: Rgb::new(108, 115, 128),
    surface1: Rgb::new(15, 19, 26),
    surface2: Rgb::new(17, 21, 28),
};

const AURA: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(130, 226, 255),
    lavender: Rgb::new(162, 119, 255),
    pink: Rgb::new(246, 148, 255),
    yellow: Rgb::new(255, 202, 133),
    green: Rgb::new(157, 255, 101),
    red: Rgb::new(255, 103, 103),
    peach: Rgb::new(255, 202, 133),
    teal: Rgb::new(97, 255, 202),
    sky: Rgb::new(130, 226, 255),
    text: Rgb::new(237, 236, 238),
    subtext0: Rgb::new(189, 189, 189),
    subtext1: Rgb::new(109, 109, 109),
    overlay0: Rgb::new(109, 109, 109),
    overlay1: Rgb::new(45, 45, 45),
    surface1: Rgb::new(31, 31, 43),
    surface2: Rgb::new(45, 45, 45),
};

const MATRIX: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(48, 179, 255),
    lavender: Rgb::new(199, 112, 255),
    pink: Rgb::new(199, 112, 255),
    yellow: Rgb::new(230, 255, 87),
    green: Rgb::new(98, 255, 148),
    red: Rgb::new(255, 75, 75),
    peach: Rgb::new(255, 168, 61),
    teal: Rgb::new(36, 246, 217),
    sky: Rgb::new(48, 179, 255),
    text: Rgb::new(98, 255, 148),
    subtext0: Rgb::new(140, 163, 145),
    subtext1: Rgb::new(74, 107, 85),
    overlay0: Rgb::new(46, 74, 55),
    overlay1: Rgb::new(30, 42, 27),
    surface1: Rgb::new(24, 34, 24),
    surface2: Rgb::new(30, 42, 27),
};

// Transparent uses the same palette as mocha (background transparency is
// handled at the terminal level, not by the palette colors).
const TRANSPARENT: Palette = Palette {
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

const SHADES_OF_PURPLE: Palette = Palette {
    white: Rgb::new(255, 255, 255),
    black: Rgb::new(0, 0, 0),
    blue: Rgb::new(158, 255, 255),
    lavender: Rgb::new(179, 98, 255),
    pink: Rgb::new(255, 98, 140),
    yellow: Rgb::new(250, 208, 0),
    green: Rgb::new(165, 255, 144),
    red: Rgb::new(236, 58, 55),
    peach: Rgb::new(255, 157, 0),
    teal: Rgb::new(128, 255, 187),
    sky: Rgb::new(158, 255, 255),
    text: Rgb::new(255, 255, 255),
    subtext0: Rgb::new(165, 153, 233),
    subtext1: Rgb::new(126, 116, 179),
    overlay0: Rgb::new(77, 33, 252),
    overlay1: Rgb::new(105, 67, 255),
    surface1: Rgb::new(34, 34, 68),
    surface2: Rgb::new(45, 43, 85),
};

/// All built-in theme names, in display order. Used by the theme picker.
pub const THEME_NAMES: &[&str] = &[
    "catppuccin-mocha",
    "catppuccin-latte",
    "catppuccin-frappe",
    "catppuccin-macchiato",
    "tokyo-night",
    "gruvbox-dark",
    "nord",
    "dracula",
    "github-dark",
    "one-dark",
    "kanagawa",
    "everforest",
    "material",
    "cobalt2",
    "flexoki",
    "ayu",
    "aura",
    "matrix",
    "transparent",
    "shades-of-purple",
];

/// Resolve a theme name to a built-in [`Palette`]. Unknown or missing names
/// fall back to catppuccin-mocha so the default rendering keeps byte-for-byte
/// parity with the reference ANSI snapshots.
pub fn palette_for_theme(name: Option<&str>) -> Palette {
    match name {
        Some("catppuccin-latte") => CATPPUCCIN_LATTE,
        Some("catppuccin-frappe") => CATPPUCCIN_FRAPPE,
        Some("catppuccin-macchiato") => CATPPUCCIN_MACCHIATO,
        Some("tokyo-night") => TOKYO_NIGHT,
        Some("gruvbox-dark") => GRUVBOX_DARK,
        Some("nord") => NORD,
        Some("dracula") => DRACULA,
        Some("github-dark") => GITHUB_DARK,
        Some("one-dark") => ONE_DARK,
        Some("kanagawa") => KANAGAWA,
        Some("everforest") => EVERFOREST,
        Some("material") => MATERIAL,
        Some("cobalt2") => COBALT2,
        Some("flexoki") => FLEXOKI,
        Some("ayu") => AYU,
        Some("aura") => AURA,
        Some("matrix") => MATRIX,
        Some("transparent") => TRANSPARENT,
        Some("shades-of-purple") => SHADES_OF_PURPLE,
        _ => CATPPUCCIN_MOCHA,
    }
}

// Default foreground used for the screen-filling Block in `render_model`.
const WHITE: Rgb = Rgb::new(255, 255, 255);
