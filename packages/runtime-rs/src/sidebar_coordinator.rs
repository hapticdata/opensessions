#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarLifecycle {
    Idle,
    Warming,
    Ready,
    Closing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarResizeAuthority {
    None,
    UserDrag,
    ClientResizeSync,
    ProgrammaticAdjust,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarCoordinatorState {
    pub mode: String,
    pub visible: bool,
    pub initializing: bool,
    pub init_label: String,
    pub width: u32,
    pub lifecycle: SidebarLifecycle,
    pub resize_authority: SidebarResizeAuthority,
    pub suppress_width_reports_until: u64,
    pub client_resize_report_guard_until: u64,
    pub last_width_report_decision: Option<SidebarWidthReportDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarWidthReportDecision {
    pub accepted: bool,
    pub reason: String,
    pub previous_width: u32,
    pub next_width: u32,
    pub continued_drag: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarWidthReportInput {
    pub width: u32,
    pub session: Option<String>,
    pub window_id: Option<String>,
    pub is_active_session: bool,
    pub is_foreground_client: bool,
    pub is_current_window: bool,
    pub now: u64,
    pub suppress_ms: u64,
}

#[derive(Debug, Clone)]
pub struct SidebarCoordinator {
    width: u32,
    visible: bool,
    lifecycle: SidebarLifecycle,
    authority: SidebarResizeAuthority,
    suppress_width_reports_until: u64,
    client_resize_report_guard_until: u64,
    drag_owner_session: Option<String>,
    drag_owner_window_id: Option<String>,
    last_width_report_decision: Option<SidebarWidthReportDecision>,
    last_user_drag_at: Option<u64>,
    warmup_until: Option<u64>,
    adjustment_until: Option<u64>,
}

impl SidebarCoordinator {
    pub fn new(width: u32) -> Self {
        Self {
            width,
            visible: false,
            lifecycle: SidebarLifecycle::Idle,
            authority: SidebarResizeAuthority::None,
            suppress_width_reports_until: 0,
            client_resize_report_guard_until: 0,
            drag_owner_session: None,
            drag_owner_window_id: None,
            last_width_report_decision: None,
            last_user_drag_at: None,
            warmup_until: None,
            adjustment_until: None,
        }
    }

    pub fn state(&self) -> SidebarCoordinatorState {
        let mode = match (self.visible, self.lifecycle, self.authority) {
            (true, SidebarLifecycle::Closing, _) => "closing",
            (_, _, SidebarResizeAuthority::ClientResizeSync)
            | (_, _, SidebarResizeAuthority::ProgrammaticAdjust) => "resizing",
            (false, _, _) => "hidden",
            (true, SidebarLifecycle::Warming, _) => "warming",
            (true, SidebarLifecycle::Ready, _) => "ready",
            (true, SidebarLifecycle::Idle, _) => "ready",
        };
        let init_label = match mode {
            "resizing" => "adjusting…",
            "warming" => "warming up…",
            "closing" => "closing…",
            _ => "",
        };

        SidebarCoordinatorState {
            mode: mode.to_string(),
            visible: self.visible,
            initializing: !init_label.is_empty(),
            init_label: init_label.to_string(),
            width: self.width,
            lifecycle: self.lifecycle,
            resize_authority: self.authority,
            suppress_width_reports_until: self.suppress_width_reports_until,
            client_resize_report_guard_until: self.client_resize_report_guard_until,
            last_width_report_decision: self.last_width_report_decision.clone(),
        }
    }

    pub fn begin_warmup(&mut self) {
        if self.is_closing() {
            return;
        }
        self.visible = true;
        self.lifecycle = SidebarLifecycle::Warming;
        self.authority = SidebarResizeAuthority::None;
        self.warmup_until = None;
        self.clear_drag_owner();
    }

    pub fn begin_warmup_until(&mut self, until: u64) {
        self.begin_warmup();
        self.warmup_until = Some(until);
    }

    pub fn warmup_done(&mut self) {
        if self.is_closing() {
            return;
        }
        self.visible = true;
        self.lifecycle = SidebarLifecycle::Ready;
        self.warmup_until = None;
        if self.authority == SidebarResizeAuthority::None {
            self.clear_drag_owner();
        }
    }

    pub fn mark_ready(&mut self) {
        if self.is_closing() {
            return;
        }
        self.visible = true;
        self.lifecycle = SidebarLifecycle::Ready;
        self.warmup_until = None;
    }

    pub fn acknowledge_sidebar_connected(&mut self) {
        if self.is_closing() {
            return;
        }
        self.visible = true;
        if self.lifecycle != SidebarLifecycle::Warming {
            self.lifecycle = SidebarLifecycle::Ready;
        }
    }

    pub fn hide(&mut self) {
        if self.is_closing() {
            return;
        }
        self.visible = false;
        self.lifecycle = SidebarLifecycle::Idle;
        self.authority = SidebarResizeAuthority::None;
        self.warmup_until = None;
        self.adjustment_until = None;
        self.clear_drag_owner();
    }

    pub fn begin_closing(&mut self) {
        self.visible = true;
        self.lifecycle = SidebarLifecycle::Closing;
        self.authority = SidebarResizeAuthority::None;
        self.warmup_until = None;
        self.adjustment_until = None;
        self.clear_drag_owner();
    }

    pub fn begin_client_resize_sync(&mut self, suppress_until: u64, guard_until: u64) {
        if self.is_closing() {
            return;
        }
        self.visible = true;
        self.authority = SidebarResizeAuthority::ClientResizeSync;
        self.suppress_width_reports_until = self.suppress_width_reports_until.max(suppress_until);
        self.client_resize_report_guard_until =
            self.client_resize_report_guard_until.max(guard_until);
        self.adjustment_until = Some(self.adjustment_until.unwrap_or(0).max(guard_until));
        self.clear_drag_owner();
    }

    pub fn finish_client_resize_sync(&mut self) {
        if self.authority == SidebarResizeAuthority::ClientResizeSync {
            self.authority = SidebarResizeAuthority::None;
        }
        self.adjustment_until = None;
    }

    /// Begin a programmatic (server-driven) width adjustment. Mirrors the TS
    /// `startProgrammaticAdjustment` guard in `packages/runtime/src/server/index.ts`,
    /// which early-returns when the sidebar is hidden or while a user drag /
    /// client-resize-sync is in flight. Returning `false` (instead of blindly
    /// overwriting the authority) is what prevents a background enforcement pass
    /// from clobbering an active `UserDrag` and snapping the sidebar back to its
    /// previous width.
    pub fn begin_programmatic_adjustment(&mut self) -> bool {
        if self.is_closing() {
            return false;
        }
        if !self.visible {
            return false;
        }
        match self.authority {
            SidebarResizeAuthority::None => {
                self.authority = SidebarResizeAuthority::ProgrammaticAdjust;
                self.adjustment_until = None;
                self.clear_drag_owner();
                true
            }
            // Already adjusting — let the caller extend the settle deadline.
            SidebarResizeAuthority::ProgrammaticAdjust => true,
            // Never preempt a live user drag or an in-flight client resize sync.
            SidebarResizeAuthority::UserDrag | SidebarResizeAuthority::ClientResizeSync => false,
        }
    }

    pub fn begin_programmatic_adjustment_until(&mut self, until: u64) -> bool {
        if !self.begin_programmatic_adjustment() {
            return false;
        }
        self.adjustment_until = Some(until);
        true
    }

    pub fn finish_programmatic_adjustment(&mut self) {
        if self.authority == SidebarResizeAuthority::ProgrammaticAdjust {
            self.authority = SidebarResizeAuthority::None;
        }
        self.adjustment_until = None;
    }

    pub fn finish_user_drag(&mut self) {
        if self.authority == SidebarResizeAuthority::UserDrag {
            self.authority = SidebarResizeAuthority::None;
        }
        self.last_user_drag_at = None;
        self.clear_drag_owner();
    }

    pub fn tick_timers(&mut self, now: u64) -> bool {
        let before = self.state();

        if self.lifecycle == SidebarLifecycle::Warming
            && self.warmup_until.is_some_and(|until| now >= until)
        {
            self.lifecycle = SidebarLifecycle::Ready;
            self.warmup_until = None;
        }

        if self.adjustment_until.is_some_and(|until| now >= until) {
            match self.authority {
                SidebarResizeAuthority::ClientResizeSync => self.finish_client_resize_sync(),
                SidebarResizeAuthority::ProgrammaticAdjust => self.finish_programmatic_adjustment(),
                SidebarResizeAuthority::None | SidebarResizeAuthority::UserDrag => {
                    self.adjustment_until = None;
                }
            }
        }

        before != self.state()
    }

    pub fn suppress_width_reports(&mut self, until: u64) {
        self.suppress_width_reports_until = self.suppress_width_reports_until.max(until);
    }

    pub fn note_client_resize_guard(&mut self, until: u64) {
        self.client_resize_report_guard_until = self.client_resize_report_guard_until.max(until);
    }

    pub fn focus_context_changed(&mut self) {
        if self.authority != SidebarResizeAuthority::UserDrag {
            self.clear_drag_owner();
        }
    }

    pub fn apply_width_report(
        &mut self,
        report: SidebarWidthReportInput,
    ) -> SidebarWidthReportDecision {
        if self.is_closing() {
            let decision = self.reject("closing");
            self.last_width_report_decision = Some(decision.clone());
            return decision;
        }
        let decision = self.decide_width_report(&report);
        if decision.accepted {
            self.width = decision.next_width;
            self.authority = SidebarResizeAuthority::UserDrag;
            self.suppress_width_reports_until = self
                .suppress_width_reports_until
                .max(report.now.saturating_add(report.suppress_ms));
            self.drag_owner_session = report.session;
            self.drag_owner_window_id = report.window_id;
            self.last_user_drag_at = Some(report.now);
        }
        self.last_width_report_decision = Some(decision.clone());
        decision
    }

    /// Clear the UserDrag authority when no further width reports have arrived
    /// for `settle_ms` after the most recent accepted drag report. Mirrors
    /// `startTransientSidebarResize` + the FINISH_USER_DRAG `setTimeout` in
    /// `packages/runtime/src/server/index.ts` so the sidebar does not get
    /// stuck showing "adjusting…" forever once the user stops resizing.
    pub fn tick_user_drag_settle(&mut self, now: u64, settle_ms: u64) {
        if self.authority != SidebarResizeAuthority::UserDrag {
            return;
        }
        let Some(last) = self.last_user_drag_at else {
            return;
        };
        if now < last.saturating_add(settle_ms) {
            return;
        }
        self.authority = SidebarResizeAuthority::None;
        self.last_user_drag_at = None;
        self.clear_drag_owner();
    }

    fn decide_width_report(&self, report: &SidebarWidthReportInput) -> SidebarWidthReportDecision {
        let continued_drag = self.is_continuing_drag(report);
        if !self.visible {
            return self.reject("hidden");
        }
        if self.lifecycle != SidebarLifecycle::Ready {
            return self.reject("warming");
        }
        if !continued_drag && !report.is_active_session {
            return self.reject("inactive-session");
        }
        if !continued_drag && !report.is_foreground_client {
            return self.reject("background-sidebar");
        }
        if self.client_resize_report_guard_until > report.now {
            return self.reject("client-resize-guard");
        }
        if self.authority == SidebarResizeAuthority::ClientResizeSync {
            return self.reject("client-resize-sync");
        }
        if self.authority == SidebarResizeAuthority::ProgrammaticAdjust {
            return self.reject("programmatic-adjust");
        }
        if self.suppress_width_reports_until > report.now
            && !continued_drag
            && (self.authority == SidebarResizeAuthority::UserDrag || !report.is_foreground_client)
        {
            return self.reject("suppressed");
        }
        if report.width == self.width {
            return self.reject("same-width");
        }

        SidebarWidthReportDecision {
            accepted: true,
            reason: "accepted".to_string(),
            previous_width: self.width,
            next_width: report.width,
            continued_drag,
        }
    }

    fn is_continuing_drag(&self, report: &SidebarWidthReportInput) -> bool {
        self.authority == SidebarResizeAuthority::UserDrag
            && self.drag_owner_session.as_ref() == report.session.as_ref()
            && self.drag_owner_window_id.as_ref() == report.window_id.as_ref()
            && self.drag_owner_session.is_some()
            && self.drag_owner_window_id.is_some()
    }

    fn reject(&self, reason: &str) -> SidebarWidthReportDecision {
        SidebarWidthReportDecision {
            accepted: false,
            reason: reason.to_string(),
            previous_width: self.width,
            next_width: self.width,
            continued_drag: false,
        }
    }

    fn clear_drag_owner(&mut self) {
        self.drag_owner_session = None;
        self.drag_owner_window_id = None;
    }

    fn is_closing(&self) -> bool {
        self.lifecycle == SidebarLifecycle::Closing
    }
}
