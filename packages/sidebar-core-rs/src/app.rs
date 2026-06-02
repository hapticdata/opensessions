use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::fixtures::{fixture_static_name, reference_sessions};
use crate::generated::protocol::{
    AgentEvent, AgentStatus, ClientCommand, ServerMessage, ServerState, SessionData,
    SessionFilterMode,
};
use crate::renderer::HitTarget;
pub use crate::session_display::DisplaySessionEntry;
use crate::session_display::{session_display_entries, worktree_group_key};

pub const SESSION_CARD_HEIGHT: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    Sessions,
    Agents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentPanelScope {
    Current,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchTarget {
    /// Open lazydiffs in a tmux popup.
    LazydiffTmux,
    /// Open lazydiff in a new terminal window.
    LazydiffTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarFocus {
    Session(String),
    WorktreeGroup(String),
}

impl SidebarFocus {
    pub fn session_name(&self) -> Option<&str> {
        match self {
            Self::Session(name) => Some(name),
            Self::WorktreeGroup(_) => None,
        }
    }

    pub fn worktree_group_key(&self) -> Option<&str> {
        match self {
            Self::Session(_) => None,
            Self::WorktreeGroup(key) => Some(key),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    ThemePicker {
        query: String,
        selected: usize,
        original_theme: Option<String>,
    },
    KillConfirm {
        session_name: String,
    },
}

#[derive(Debug)]
pub struct App {
    pub sessions: Vec<SessionData>,
    pub sidebar_focus: Option<SidebarFocus>,
    pub current_session: Option<String>,
    pub my_session: Option<String>,
    pub initializing: bool,
    pub init_label: Option<String>,
    pub theme: Option<String>,
    pub ts: u64,
    /// Locally-driven spinner clock in ms. Advances on every render tick
    /// (see `main.rs` event loop) so spinners animate even when no server
    /// state arrives. Starts at 0 so deterministic snapshot tests are
    /// unaffected (`spinner()` falls back to `ts` when this is 0).
    pub spinner_now: u64,
    pub session_filter: SessionFilterMode,
    pub panel_focus: PanelFocus,
    pub agent_panel_scope: AgentPanelScope,
    pub focused_agent_idx: usize,
    pub quit_deadline: Option<Instant>,
    pub flash_target: Option<HitTarget>,
    pub flash_deadline: Option<Instant>,
    pub hover_target: Option<HitTarget>,
    pub modal: Modal,
    pub detail_panel_height: usize,
    pub session_scroll_offset: usize,
    session_scroll_follows_focus: bool,
    pub resize_drag_state: Option<(u16, usize)>,
    collapsed_worktree_groups: HashSet<String>,
    pub fixture_name: Option<&'static str>,
    terminal_width: Option<u16>,
    pane_identity: Option<PaneIdentity>,
    optimistic_current_session: Option<String>,
    commands: Vec<ClientCommand>,
    pending_launches: Vec<LaunchTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneIdentity {
    pub pane_id: String,
    pub session_name: String,
    pub window_id: Option<String>,
}

impl App {
    pub fn from_state(state: ServerState) -> Self {
        let focused_session = state.focused_session.clone();
        let mut app = Self {
            sessions: state.sessions,
            sidebar_focus: None,
            current_session: state.current_session,
            my_session: None,
            initializing: state.initializing,
            init_label: state.init_label,
            theme: state.theme,
            ts: state.ts,
            spinner_now: 0,
            session_filter: state.session_filter.unwrap_or_default(),
            panel_focus: PanelFocus::Sessions,
            agent_panel_scope: AgentPanelScope::Current,
            focused_agent_idx: 0,
            quit_deadline: None,
            flash_target: None,
            flash_deadline: None,
            hover_target: None,
            modal: Modal::None,
            detail_panel_height: 10,
            session_scroll_offset: 0,
            session_scroll_follows_focus: true,
            resize_drag_state: None,
            collapsed_worktree_groups: state.collapsed_worktree_groups.into_iter().collect(),
            fixture_name: None,
            terminal_width: None,
            pane_identity: None,
            optimistic_current_session: None,
            commands: Vec::new(),
            pending_launches: Vec::new(),
        };
        app.sidebar_focus = focused_session
            .as_deref()
            .and_then(|name| app.visible_focus_for_session(name))
            .or_else(|| app.display_session_entries().first().map(entry_focus));
        app
    }

    pub fn set_terminal_width(&mut self, width: u16) {
        self.terminal_width = Some(width);
    }

    pub fn terminal_width(&self) -> Option<u16> {
        self.terminal_width
    }

    /// Record the running pane's identity and queue an `IdentifyPane` command.
    /// Calling again replaces the stored identity so subsequent `ReIdentify`
    /// requests use the freshest values, matching `apps/tui/src/index.tsx`'s
    /// `reIdentify()` behavior.
    pub fn identify_pane(
        &mut self,
        pane_id: String,
        session_name: String,
        window_id: Option<String>,
    ) {
        let identity = PaneIdentity {
            pane_id,
            session_name,
            window_id,
        };
        self.commands.push(ClientCommand::IdentifyPane {
            pane_id: identity.pane_id.clone(),
            session_name: identity.session_name.clone(),
            window_id: identity.window_id.clone(),
        });
        self.pane_identity = Some(identity);
    }

    /// Store the pane identity without queuing an `IdentifyPane` command.
    /// Used after main.rs has already sent the initial identify, so future
    /// `ReIdentify` requests can resend without doubling the first call.
    pub fn set_pane_identity(
        &mut self,
        pane_id: String,
        session_name: String,
        window_id: Option<String>,
    ) {
        self.pane_identity = Some(PaneIdentity {
            pane_id,
            session_name,
            window_id,
        });
    }

    pub fn pane_identity(&self) -> Option<&PaneIdentity> {
        self.pane_identity.as_ref()
    }

    pub fn apply_server_message(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::State(state) => {
                let previous_focus = self.sidebar_focus.clone();
                self.sessions = state.sessions;
                self.current_session = match self.optimistic_current_session.as_deref() {
                    Some(optimistic)
                        if state.current_session.as_deref() != Some(optimistic)
                            && self
                                .sessions
                                .iter()
                                .any(|session| session.name == optimistic) =>
                    {
                        Some(optimistic.to_string())
                    }
                    _ => {
                        if self.optimistic_current_session.as_deref()
                            == state.current_session.as_deref()
                        {
                            self.optimistic_current_session = None;
                        }
                        state.current_session
                    }
                };
                self.initializing = state.initializing;
                self.init_label = state.init_label;
                self.theme = state.theme;
                self.ts = state.ts;
                self.session_filter = state.session_filter.unwrap_or_default();
                self.collapsed_worktree_groups =
                    state.collapsed_worktree_groups.into_iter().collect();
                let focus_still_exists = previous_focus
                    .as_ref()
                    .is_some_and(|focus| self.focus_exists(focus));
                if !focus_still_exists {
                    self.sidebar_focus = state
                        .focused_session
                        .as_deref()
                        .and_then(|focused| self.visible_focus_for_session(focused))
                        .or_else(|| self.display_session_entries().first().map(entry_focus));
                    self.session_scroll_follows_focus = true;
                }
                self.clamp_session_scroll_offset(0);
            }
            ServerMessage::Focus(update) => {
                let next_focus = update
                    .focused_session
                    .as_deref()
                    .and_then(|focused| self.visible_focus_for_session(focused));
                self.current_session = update.current_session;
                if self.optimistic_current_session.as_deref() == self.current_session.as_deref() {
                    self.optimistic_current_session = None;
                }
                if let Some(next_focus) = next_focus {
                    self.set_sidebar_focus(next_focus);
                }
            }
            ServerMessage::YourSession { name, .. } => {
                self.my_session = Some(name);
            }
            ServerMessage::ReIdentify => {
                if let Some(identity) = self.pane_identity.clone() {
                    self.commands.push(ClientCommand::IdentifyPane {
                        pane_id: identity.pane_id,
                        session_name: identity.session_name,
                        window_id: identity.window_id,
                    });
                }
            }
            ServerMessage::Hello(_) | ServerMessage::Resize { .. } | ServerMessage::Quit => {}
        }
    }

    pub fn reference_fixture(name: &str) -> Self {
        let (focused_session, current_session) = match name {
            "pane-opensessions-self" => (Some("opensessions"), Some("opensessions")),
            "pane-multi-window" => (Some("plane-feat-background-exports"), Some("opensessions")),
            _ => (Some("plane-pdf-word-formatting"), Some("opensessions")),
        };

        let mut app = Self {
            sessions: reference_sessions(),
            sidebar_focus: focused_session.map(|name| SidebarFocus::Session(name.to_string())),
            current_session: current_session.map(str::to_string),
            my_session: current_session.map(str::to_string),
            initializing: false,
            init_label: None,
            theme: None,
            ts: 0,
            spinner_now: 0,
            session_filter: SessionFilterMode::All,
            panel_focus: PanelFocus::Sessions,
            agent_panel_scope: AgentPanelScope::Current,
            focused_agent_idx: 0,
            quit_deadline: None,
            flash_target: None,
            flash_deadline: None,
            hover_target: None,
            modal: Modal::None,
            detail_panel_height: 10,
            session_scroll_offset: 0,
            session_scroll_follows_focus: true,
            resize_drag_state: None,
            collapsed_worktree_groups: HashSet::new(),
            fixture_name: fixture_static_name(name),
            terminal_width: None,
            pane_identity: None,
            optimistic_current_session: None,
            commands: Vec::new(),
            pending_launches: Vec::new(),
        };

        if name == "pane-opensessions-self" {
            app.focused_agent_idx = 0;
        }

        app
    }

    pub fn resolve_synced_focus(
        next_focused_session: Option<&str>,
        next_current_session: Option<&str>,
        local_session_name: Option<&str>,
    ) -> Option<String> {
        if let (Some(local), Some(current)) = (local_session_name, next_current_session)
            && current != local
        {
            return Some(local.to_string());
        }

        next_focused_session
            .or(local_session_name)
            .map(str::to_string)
    }

    pub fn filtered_sessions(&self) -> impl Iterator<Item = &SessionData> {
        let mode = self.session_filter;
        self.sessions.iter().filter(move |session| {
            if session.name == "_os_stash" {
                return false;
            }

            match mode {
                SessionFilterMode::All => true,
                SessionFilterMode::Active => {
                    !session.agents.is_empty() || session.agent_state.is_some()
                }
                SessionFilterMode::Running => matches!(
                    session.agent_state.as_ref().map(|agent| agent.status),
                    Some(AgentStatus::Running | AgentStatus::ToolRunning | AgentStatus::Waiting),
                ),
            }
        })
    }

    pub fn display_session_entries(&self) -> Vec<DisplaySessionEntry<'_>> {
        session_display_entries(
            self.filtered_sessions().collect(),
            &self.collapsed_worktree_groups,
        )
    }

    pub fn display_sessions(&self) -> Vec<&SessionData> {
        self.display_session_entries()
            .into_iter()
            .filter_map(|entry| match entry {
                DisplaySessionEntry::Session { session, .. } => Some(session),
                DisplaySessionEntry::Group { .. } => None,
            })
            .collect()
    }

    pub fn focused_session_name(&self) -> Option<&str> {
        self.sidebar_focus.as_ref()?.session_name()
    }

    pub fn focused_group_key(&self) -> Option<&str> {
        self.sidebar_focus.as_ref()?.worktree_group_key()
    }

    pub fn set_sidebar_focus(&mut self, focus: SidebarFocus) {
        self.sidebar_focus = Some(focus);
        self.panel_focus = PanelFocus::Sessions;
        self.focused_agent_idx = 0;
        self.session_scroll_follows_focus = true;
    }

    pub fn set_focused_session(&mut self, name: impl Into<String>) {
        self.set_sidebar_focus(SidebarFocus::Session(name.into()));
    }

    fn focus_exists(&self, focus: &SidebarFocus) -> bool {
        self.display_session_entries()
            .iter()
            .any(|entry| &entry_focus(entry) == focus)
    }

    fn visible_focus_for_session(&self, name: &str) -> Option<SidebarFocus> {
        let session_focus = SidebarFocus::Session(name.to_string());
        if self.focus_exists(&session_focus) {
            return Some(session_focus);
        }
        let session = self.sessions.iter().find(|session| session.name == name)?;
        let group_focus = SidebarFocus::WorktreeGroup(worktree_group_key(session)?);
        self.focus_exists(&group_focus).then_some(group_focus)
    }

    pub fn handle_key_char(&mut self, key: char) {
        match key {
            '1'..='9' => {
                let index = key.to_digit(10).expect("digit key must parse") as usize;
                if let Some(session) = self.display_sessions().get(index.saturating_sub(1)) {
                    self.switch_to_session(session.name.clone());
                }
            }
            'q' => {
                self.commands.push(ClientCommand::Quit);
                self.quit_deadline = Some(Instant::now() + Duration::from_millis(500));
            }
            'r' => self.commands.push(ClientCommand::Refresh),
            'n' | 'c' => self.commands.push(ClientCommand::NewSession),
            'u' => self.commands.push(ClientCommand::ShowAllSessions),
            'd' => {
                if self.panel_focus == PanelFocus::Agents {
                    self.dismiss_focused_agent();
                } else if let Some(name) = self.focused_session_name().map(str::to_string) {
                    self.commands.push(ClientCommand::HideSession { name });
                }
            }
            'x' => {
                if self.panel_focus == PanelFocus::Agents {
                    self.kill_focused_agent_pane();
                } else if let Some(name) = self.focused_session_name().map(str::to_string) {
                    self.modal = Modal::KillConfirm { session_name: name };
                }
            }
            'l' => self.pending_launches.push(LaunchTarget::LazydiffTmux),
            'L' => self.pending_launches.push(LaunchTarget::LazydiffTerminal),
            't' => self.open_theme_picker(),
            'f' => self.cycle_filter(),
            'a' => self.toggle_agent_panel_scope(),
            _ => {}
        }
    }

    pub fn handle_tab(&mut self, shift: bool) {
        let names: Vec<String> = self
            .display_sessions()
            .into_iter()
            .map(|session| session.name.clone())
            .collect();
        if names.is_empty() {
            return;
        }

        let current = self.current_session.as_deref();
        let current_idx = current
            .and_then(|name| names.iter().position(|candidate| candidate == name))
            .unwrap_or(0);
        let next_idx = if shift {
            (current_idx + names.len() - 1) % names.len()
        } else {
            (current_idx + 1) % names.len()
        };
        self.switch_to_session_debounced(names[next_idx].clone());
    }

    pub fn drain_commands(&mut self) -> Vec<ClientCommand> {
        self.commands.drain(..).collect()
    }

    pub fn drain_launches(&mut self) -> Vec<LaunchTarget> {
        self.pending_launches.drain(..).collect()
    }

    pub fn commands_push(&mut self, command: ClientCommand) {
        self.commands.push(command);
    }

    pub fn move_focus(&mut self, delta: i8) {
        let Some((current_idx, len)) = self.focused_filtered_index_and_len() else {
            return;
        };
        let max_idx = len - 1;
        let next_idx = (current_idx as i16 + delta as i16).clamp(0, max_idx as i16) as usize;
        if next_idx == current_idx {
            return;
        }
        let Some(target) = self.filtered_focus_target_at(next_idx) else {
            return;
        };
        self.set_sidebar_focus(target);
    }

    pub fn session_scroll_offset(&self) -> usize {
        self.session_scroll_offset
    }

    pub fn session_scroll_follows_focus(&self) -> bool {
        self.session_scroll_follows_focus
    }

    pub fn scroll_sessions(&mut self, delta: i8, viewport_rows: usize) {
        let entries = self.display_session_entries();
        let len = entries.len();
        if len == 0 || viewport_rows == 0 {
            self.session_scroll_offset = 0;
            return;
        }

        let max_offset = max_scroll_offset(&entries, viewport_rows);
        let next_offset = if delta < 0 {
            self.session_scroll_offset
                .saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.session_scroll_offset.saturating_add(delta as usize)
        }
        .min(max_offset);

        self.session_scroll_offset = next_offset;
        self.session_scroll_follows_focus = false;
        self.panel_focus = PanelFocus::Sessions;
    }

    pub fn ensure_focused_session_visible(&mut self, viewport_rows: usize) {
        let Some((focused_idx, len)) = self.focused_filtered_index_and_len() else {
            self.session_scroll_offset = 0;
            return;
        };
        if viewport_rows == 0 {
            return;
        }

        let visible_cards = visible_session_cards(viewport_rows);
        let max_offset = len.saturating_sub(visible_cards);
        if focused_idx < self.session_scroll_offset {
            self.session_scroll_offset = focused_idx;
        } else if focused_idx >= self.session_scroll_offset.saturating_add(visible_cards) {
            self.session_scroll_offset =
                focused_idx.saturating_add(1).saturating_sub(visible_cards);
        }
        self.session_scroll_offset = self.session_scroll_offset.min(max_offset);
    }

    pub fn focus_sessions_panel(&mut self) {
        self.panel_focus = PanelFocus::Sessions;
    }

    pub fn focus_agents_panel(&mut self) {
        let agent_count = self.focused_agents_len();
        if agent_count == 0 {
            return;
        }
        self.panel_focus = PanelFocus::Agents;
        self.focused_agent_idx = self.focused_agent_idx.min(agent_count - 1);
    }

    pub fn toggle_agent_panel_scope(&mut self) {
        self.agent_panel_scope = match self.agent_panel_scope {
            AgentPanelScope::Current => AgentPanelScope::All,
            AgentPanelScope::All => AgentPanelScope::Current,
        };
        self.focused_agent_idx = self
            .focused_agent_idx
            .min(self.focused_agents_len().saturating_sub(1));
        if self.focused_agents_len() == 0 {
            self.panel_focus = PanelFocus::Sessions;
        }
    }

    pub fn move_agent_focus(&mut self, delta: i8) {
        let agent_count = self.focused_agents_len();
        if agent_count == 0 {
            return;
        }
        let max_idx = agent_count - 1;
        self.focused_agent_idx =
            (self.focused_agent_idx as i16 + delta as i16).clamp(0, max_idx as i16) as usize;
    }

    pub fn activate_focused_item(&mut self) {
        if self.panel_focus == PanelFocus::Agents {
            self.activate_focused_agent();
            return;
        }
        match self.sidebar_focus.clone() {
            Some(SidebarFocus::Session(name)) => self.switch_to_session(name),
            Some(SidebarFocus::WorktreeGroup(key)) => self.toggle_worktree_group(&key),
            None => {}
        }
    }

    pub fn activate_focused_session(&mut self) {
        if let Some(name) = self.focused_session_name().map(str::to_string) {
            self.switch_to_session(name);
        }
    }

    /// Click on a session row in the list. Mirrors the TS
    /// `onSelect={() => switchToSession(session.name)}` handler in
    /// `apps/tui/src/index.tsx::SessionCard`.
    pub fn click_session(&mut self, name: String) {
        self.trigger_flash(HitTarget::Session(name.clone()));
        self.switch_to_session(name);
    }

    pub fn click_group(&mut self, key: String) {
        self.trigger_flash(HitTarget::Group(key.clone()));
        if self.focused_group_key() == Some(key.as_str()) {
            self.toggle_worktree_group(&key);
            return;
        }
        self.set_sidebar_focus(SidebarFocus::WorktreeGroup(key));
    }

    pub fn click_diff_count(&mut self, name: String) {
        self.trigger_flash(HitTarget::DiffCount(name.clone()));
        self.set_sidebar_focus(SidebarFocus::Session(name));
        self.pending_launches.push(LaunchTarget::LazydiffTmux);
    }

    /// Click on an agent row in the detail panel. Mirrors the TS
    /// `onFocusPane`/`onFocusAgentPane` flow that switches to the agent's
    /// session and sends `focus-agent-pane`.
    pub fn click_agent(&mut self, idx: usize) {
        let agent_count = self.focused_agents_len();
        if idx >= agent_count {
            return;
        }
        self.trigger_flash(HitTarget::Agent(idx));
        self.panel_focus = PanelFocus::Agents;
        self.focused_agent_idx = idx;
        self.activate_focused_agent();
    }

    /// Queue a `SetTheme` command for the server. Mirrors the TS
    /// `applyTheme(themeName) => send({ type: "set-theme", theme: themeName })`
    /// in `apps/tui/src/index.tsx`. The server replies with a fresh `State`
    /// broadcast carrying the new theme name, which `apply_server_message`
    /// stores on `self.theme`.
    pub fn set_theme_request(&mut self, theme: String) {
        self.commands.push(ClientCommand::SetTheme { theme });
    }

    /// Arm a 150ms click-flash highlight on the given target. Mirrors the TS
    /// `triggerFlash()` helper which sets `flashUntil = Date.now() + 150`.
    pub fn trigger_flash(&mut self, target: HitTarget) {
        self.flash_target = Some(target);
        self.flash_deadline = Some(Instant::now() + Duration::from_millis(150));
    }

    /// Returns the currently active flash target, or `None` if the flash has
    /// expired or was never armed.
    pub fn active_flash_target(&self) -> Option<&HitTarget> {
        let deadline = self.flash_deadline?;
        if Instant::now() >= deadline {
            return None;
        }
        self.flash_target.as_ref()
    }

    pub fn set_hover_target(&mut self, target: Option<HitTarget>) {
        self.hover_target = target;
    }

    pub fn is_modal_open(&self) -> bool {
        !matches!(self.modal, Modal::None)
    }

    pub fn open_theme_picker(&mut self) {
        self.modal = Modal::ThemePicker {
            query: String::new(),
            selected: 0,
            original_theme: self.theme.clone(),
        };
    }

    pub fn close_theme_picker(&mut self) {
        if let Modal::ThemePicker { original_theme, .. } = &self.modal {
            self.theme = original_theme.clone();
        }
        self.modal = Modal::None;
    }

    pub fn confirm_theme_picker(&mut self) {
        if let Some(name) = self.theme.clone() {
            self.commands.push(ClientCommand::SetTheme { theme: name });
        }
        self.modal = Modal::None;
    }

    pub fn resize_detail_panel(&mut self, delta: i8) {
        let new_height = (self.detail_panel_height as i16 + delta as i16).max(4) as usize;
        self.detail_panel_height = new_height;
    }

    pub fn activate_focused_agent(&mut self) {
        let Some((session, agent)) = self
            .focused_agent()
            .map(|(session, agent)| (session.name.clone(), agent.clone()))
        else {
            return;
        };
        self.current_session = Some(session.clone());
        self.optimistic_current_session = Some(session.clone());
        self.commands.push(ClientCommand::SwitchSession {
            name: session.clone(),
            client_tty: None,
            debounce: None,
        });
        self.commands.push(ClientCommand::FocusAgentPane {
            session,
            agent: agent.agent,
            thread_id: agent.thread_id,
            thread_name: agent.thread_name,
            pane_id: agent.pane_id,
        });
    }

    pub fn dismiss_focused_agent(&mut self) {
        let agent_count = self.focused_agents_len();
        let Some((session, agent)) = self
            .focused_agent()
            .map(|(session, agent)| (session.name.clone(), agent.clone()))
        else {
            return;
        };
        self.commands.push(ClientCommand::DismissAgent {
            session,
            agent: agent.agent,
            thread_id: agent.thread_id,
        });
        if self.focused_agent_idx >= agent_count.saturating_sub(1) && agent_count > 1 {
            self.focused_agent_idx = agent_count - 2;
        }
        if agent_count <= 1 {
            self.panel_focus = PanelFocus::Sessions;
        }
    }

    pub fn kill_focused_agent_pane(&mut self) {
        let Some((session, agent)) = self
            .focused_agent()
            .map(|(session, agent)| (session.name.clone(), agent.clone()))
        else {
            return;
        };
        self.commands.push(ClientCommand::KillAgentPane {
            session,
            agent: agent.agent,
            thread_id: agent.thread_id,
            thread_name: agent.thread_name,
            pane_id: agent.pane_id,
        });
    }

    pub fn reorder_focused_session(&mut self, delta: i8) {
        if let Some(name) = self.focused_session_name().map(str::to_string) {
            self.commands
                .push(ClientCommand::ReorderSession { name, delta });
        }
    }

    fn switch_to_session(&mut self, name: String) {
        self.my_session = Some(name.clone());
        self.current_session = Some(name.clone());
        self.optimistic_current_session = Some(name.clone());
        self.set_sidebar_focus(SidebarFocus::Session(name.clone()));
        self.commands.push(ClientCommand::SwitchSession {
            name,
            client_tty: None,
            debounce: None,
        });
    }

    fn switch_to_session_debounced(&mut self, name: String) {
        self.my_session = Some(name.clone());
        self.current_session = Some(name.clone());
        self.optimistic_current_session = Some(name.clone());
        self.set_sidebar_focus(SidebarFocus::Session(name.clone()));
        self.commands.push(ClientCommand::SwitchSession {
            name,
            client_tty: None,
            debounce: Some(true),
        });
    }

    fn cycle_filter(&mut self) {
        self.session_filter = match self.session_filter {
            SessionFilterMode::All => SessionFilterMode::Active,
            SessionFilterMode::Active => SessionFilterMode::Running,
            SessionFilterMode::Running => SessionFilterMode::All,
        };
        self.clamp_session_scroll_offset(0);
        self.commands.push(ClientCommand::SetFilter {
            filter: self.session_filter,
        });
    }

    fn clamp_session_scroll_offset(&mut self, viewport_rows: usize) {
        let entries = self.display_session_entries();
        self.session_scroll_offset = self
            .session_scroll_offset
            .min(max_scroll_offset(&entries, viewport_rows));
    }

    pub fn is_group_collapsed(&self, key: &str) -> bool {
        self.collapsed_worktree_groups.contains(key)
    }

    fn toggle_worktree_group(&mut self, key: &str) {
        self.commands.push(ClientCommand::ToggleWorktreeGroup {
            key: key.to_string(),
        });
    }

    fn focused_filtered_index_and_len(&self) -> Option<(usize, usize)> {
        let focused = self.sidebar_focus.as_ref();
        let mut focused_idx = None;
        let mut len = 0;
        for (idx, entry) in self.display_session_entries().into_iter().enumerate() {
            if Some(&entry_focus(&entry)) == focused {
                focused_idx = Some(idx);
            }
            len += 1;
        }
        (len > 0).then_some((focused_idx.unwrap_or(0), len))
    }

    fn filtered_focus_target_at(&self, index: usize) -> Option<SidebarFocus> {
        self.display_session_entries().get(index).map(entry_focus)
    }

    fn focused_session_data(&self) -> Option<&SessionData> {
        let focused = self.focused_session_name()?;
        self.sessions.iter().find(|session| session.name == focused)
    }

    fn focused_agents_len(&self) -> usize {
        match self.agent_panel_scope {
            AgentPanelScope::Current => self
                .focused_session_data()
                .map(|session| session.agents.len())
                .unwrap_or(0),
            AgentPanelScope::All => self
                .sessions
                .iter()
                .map(|session| session.agents.len())
                .sum(),
        }
    }

    fn focused_agent(&self) -> Option<(&SessionData, &AgentEvent)> {
        match self.agent_panel_scope {
            AgentPanelScope::Current => {
                let session = self.focused_session_data()?;
                let agent = session.agents.get(self.focused_agent_idx)?;
                Some((session, agent))
            }
            AgentPanelScope::All => self
                .sessions
                .iter()
                .flat_map(|session| session.agents.iter().map(move |agent| (session, agent)))
                .nth(self.focused_agent_idx),
        }
    }
}

fn visible_session_cards(viewport_rows: usize) -> usize {
    viewport_rows.div_ceil(SESSION_CARD_HEIGHT).max(1)
}

fn max_scroll_offset(entries: &[DisplaySessionEntry<'_>], viewport_rows: usize) -> usize {
    if entries.is_empty() || viewport_rows == 0 {
        return 0;
    }
    let total_rows = entries
        .iter()
        .map(DisplaySessionEntry::row_height)
        .sum::<usize>();
    if total_rows <= viewport_rows {
        return 0;
    }
    let mut offset = 0;
    let mut remaining_rows = total_rows;
    while offset < entries.len() && remaining_rows > viewport_rows {
        remaining_rows = remaining_rows.saturating_sub(entries[offset].row_height());
        offset += 1;
    }
    offset.min(entries.len().saturating_sub(1))
}

fn entry_focus(entry: &DisplaySessionEntry<'_>) -> SidebarFocus {
    match entry {
        DisplaySessionEntry::Session { session, .. } => SidebarFocus::Session(session.name.clone()),
        DisplaySessionEntry::Group { key, .. } => SidebarFocus::WorktreeGroup(key.clone()),
    }
}
