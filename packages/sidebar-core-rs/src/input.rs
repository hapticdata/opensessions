use crate::app::{App, PanelFocus};
use crate::renderer::{HitTarget, compute_hit_map};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKey {
    Char(char),
    Up,
    Down,
    Tab { shift: bool },
    Enter,
    Esc,
    CtrlJ,
    CtrlK,
    AltUp,
    AltDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMouse {
    ScrollUp { x: u16, y: u16 },
    ScrollDown { x: u16, y: u16 },
    Click { x: u16, y: u16, width: u16, height: u16 },
}

pub fn apply_ui_key(app: &mut App, key: UiKey) {
    match key {
        UiKey::AltUp => app.reorder_focused_session(-1),
        UiKey::AltDown => app.reorder_focused_session(1),
        UiKey::CtrlJ => app.focus_agents_panel(),
        UiKey::CtrlK => app.focus_sessions_panel(),
        UiKey::Down => {
            if app.panel_focus == PanelFocus::Agents {
                app.move_agent_focus(1);
            } else {
                app.move_focus(1);
            }
        }
        UiKey::Up => {
            if app.panel_focus == PanelFocus::Agents {
                app.move_agent_focus(-1);
            } else {
                app.move_focus(-1);
            }
        }
        UiKey::Tab { shift } => app.handle_tab(shift),
        UiKey::Enter => app.activate_focused_item(),
        UiKey::Esc => app.focus_sessions_panel(),
        UiKey::Char(ch) => app.handle_key_char(ch),
    }
}

pub fn apply_ui_mouse(app: &mut App, event: UiMouse) {
    match event {
        UiMouse::ScrollUp { .. } => {
            if app.panel_focus == PanelFocus::Agents {
                app.move_agent_focus(-1);
            } else {
                app.move_focus(-1);
            }
        }
        UiMouse::ScrollDown { .. } => {
            if app.panel_focus == PanelFocus::Agents {
                app.move_agent_focus(1);
            } else {
                app.move_focus(1);
            }
        }
        UiMouse::Click { x: _, y, width, height } => {
            let hits = compute_hit_map(app, width, height);
            let target = hits.get(y as usize).cloned().flatten();
            match target {
                Some(HitTarget::Session(name)) => {
                    app.click_session(name);
                }
                Some(HitTarget::Agent(idx)) => {
                    app.click_agent(idx);
                }
                None => {}
            }
        }
    }
}
