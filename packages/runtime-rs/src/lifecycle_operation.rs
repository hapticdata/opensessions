use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleServer {
    phase: ServerPhase,
    connected_clients: BTreeMap<ClientId, ClientViewState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleChannel {
    server: LifecycleServer,
    broadcasting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerPhase {
    Running(RunningGeneration),
    Closing(ClosingGeneration),
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningGeneration {
    pub lifecycle: SidebarLifecycle,
    pub presence: Option<SidebarPresenceReconciliation>,
    pub resize: Option<ResizeAdjustment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosingGeneration {
    pub requested_by: ClientId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientViewState {
    pub current_session: Option<String>,
    pub sidebar_focus: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarLifecycle {
    Hidden,
    Warming,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResizeAdjustment {
    pub owner: ResizeOwner,
    pub target_width: SidebarWidth,
    pub pending_targets: BTreeSet<ResizeTarget>,
    pub acknowledged_targets: BTreeSet<ResizeTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarPresenceReconciliation {
    pub pending_windows: BTreeSet<SidebarPresenceTarget>,
    pub connected_windows: BTreeSet<SidebarPresenceTarget>,
    pub failed_windows: BTreeMap<SidebarPresenceTarget, PresenceFailureReason>,
    pub spawn_queue: VecDeque<SidebarPresenceTarget>,
    pub in_flight_spawn: Option<SidebarPresenceTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SidebarPresenceTarget {
    pub session: String,
    pub window_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxWindowTarget {
    pub session: String,
    pub window_id: String,
    pub stash: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceFailureReason {
    SpawnFailed,
    ConnectTimeout,
    WindowVanished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarWidth(pub u16);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResizeOwner {
    pub client_id: ClientId,
    pub target: ResizeTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResizeTarget {
    pub session: String,
    pub window_id: String,
    pub pane_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClientId(String);

impl ClientId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleOperation {
    BeginWarmup {
        origin_session: Option<String>,
        windows: Vec<TmuxWindowTarget>,
    },
    WarmupComplete,
    RequestNextSpawn,
    SidebarConnected {
        client_id: ClientId,
        window_id: Option<String>,
        current_session: Option<String>,
    },
    PresenceTargetFailed {
        window_id: String,
        reason: PresenceFailureReason,
    },
    BeginResize {
        owner: ResizeOwner,
        target_width: SidebarWidth,
        targets: Vec<ResizeTarget>,
    },
    AckResize {
        target: ResizeTarget,
        observed_width: SidebarWidth,
    },
    SwitchSession {
        client_id: ClientId,
        target_session: String,
    },
    RequestQuit {
        requested_by: ClientId,
    },
    DrainComplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEffect {
    BroadcastState(LifecycleSnapshot),
    SendQuit {
        client_id: ClientId,
    },
    SpawnSidebar {
        session: String,
        window_id: String,
    },
    SendClientView {
        client_id: ClientId,
        view: ClientViewState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleSubmitError {
    BroadcastInProgress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleSnapshot {
    pub phase: SnapshotPhase,
    pub visible: bool,
    pub initializing: bool,
    pub init_label: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotPhase {
    Hidden,
    Warming,
    Adjusting,
    Ready,
    Closing,
    Closed,
}

impl Default for LifecycleServer {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleServer {
    pub fn new() -> Self {
        Self {
            phase: ServerPhase::Running(RunningGeneration {
                lifecycle: SidebarLifecycle::Hidden,
                presence: None,
                resize: None,
            }),
            connected_clients: BTreeMap::new(),
        }
    }

    pub fn phase(&self) -> &ServerPhase {
        &self.phase
    }

    pub fn client_view(&self, client_id: &ClientId) -> Option<&ClientViewState> {
        self.connected_clients.get(client_id)
    }

    pub fn snapshot(&self) -> LifecycleSnapshot {
        match &self.phase {
            ServerPhase::Running(generation) if generation.resize.is_some() => LifecycleSnapshot {
                phase: SnapshotPhase::Adjusting,
                visible: true,
                initializing: true,
                init_label: Some("adjusting…"),
            },
            ServerPhase::Running(generation) if generation.presence.is_some() => {
                LifecycleSnapshot {
                    phase: SnapshotPhase::Warming,
                    visible: true,
                    initializing: true,
                    init_label: Some("warming up…"),
                }
            }
            ServerPhase::Running(generation) => match generation.lifecycle {
                SidebarLifecycle::Hidden => LifecycleSnapshot {
                    phase: SnapshotPhase::Hidden,
                    visible: false,
                    initializing: false,
                    init_label: None,
                },
                SidebarLifecycle::Warming => LifecycleSnapshot {
                    phase: SnapshotPhase::Warming,
                    visible: true,
                    initializing: true,
                    init_label: Some("warming up…"),
                },
                SidebarLifecycle::Ready => LifecycleSnapshot {
                    phase: SnapshotPhase::Ready,
                    visible: true,
                    initializing: false,
                    init_label: None,
                },
            },
            ServerPhase::Closing(_) => LifecycleSnapshot {
                phase: SnapshotPhase::Closing,
                visible: true,
                initializing: true,
                init_label: Some("closing…"),
            },
            ServerPhase::Closed => LifecycleSnapshot {
                phase: SnapshotPhase::Closed,
                visible: false,
                initializing: false,
                init_label: None,
            },
        }
    }

    pub fn apply(&mut self, operation: LifecycleOperation) -> Vec<LifecycleEffect> {
        match (&mut self.phase, operation) {
            (
                ServerPhase::Running(generation),
                LifecycleOperation::BeginWarmup {
                    origin_session,
                    windows,
                },
            ) => {
                generation.lifecycle = SidebarLifecycle::Warming;
                let targets = ordered_presence_targets(windows, origin_session.as_deref());
                generation.presence = Some(SidebarPresenceReconciliation {
                    pending_windows: targets.iter().cloned().collect(),
                    connected_windows: BTreeSet::new(),
                    failed_windows: BTreeMap::new(),
                    spawn_queue: targets.into_iter().collect(),
                    in_flight_spawn: None,
                });
                vec![LifecycleEffect::BroadcastState(self.snapshot())]
            }
            (ServerPhase::Running(generation), LifecycleOperation::WarmupComplete) => {
                if generation.presence.is_some() {
                    return Vec::new();
                }
                if generation.lifecycle == SidebarLifecycle::Warming {
                    generation.lifecycle = SidebarLifecycle::Ready;
                    vec![LifecycleEffect::BroadcastState(self.snapshot())]
                } else {
                    Vec::new()
                }
            }
            (ServerPhase::Running(generation), LifecycleOperation::RequestNextSpawn) => {
                let Some(presence) = generation.presence.as_mut() else {
                    return Vec::new();
                };
                if presence.in_flight_spawn.is_some() {
                    return Vec::new();
                }
                let Some(target) = presence.spawn_queue.pop_front() else {
                    return Vec::new();
                };
                presence.in_flight_spawn = Some(target.clone());
                vec![LifecycleEffect::SpawnSidebar {
                    session: target.session,
                    window_id: target.window_id,
                }]
            }
            (
                ServerPhase::Running(generation),
                LifecycleOperation::SidebarConnected {
                    client_id,
                    window_id,
                    current_session,
                },
            ) => {
                self.connected_clients.insert(
                    client_id,
                    ClientViewState {
                        current_session: current_session.clone(),
                        sidebar_focus: current_session,
                    },
                );
                if let Some(window_id) = window_id
                    && let Some(presence) = generation.presence.as_mut()
                {
                    let Some(target) = presence_target_for_window(presence, &window_id) else {
                        return Vec::new();
                    };
                    if presence.pending_windows.remove(&target) {
                        if presence.in_flight_spawn.as_ref() == Some(&target) {
                            presence.in_flight_spawn = None;
                        }
                        presence.connected_windows.insert(target);
                    }
                    if presence.pending_windows.is_empty() {
                        generation.presence = None;
                        generation.lifecycle = SidebarLifecycle::Ready;
                        return vec![LifecycleEffect::BroadcastState(self.snapshot())];
                    }
                }
                if generation.lifecycle == SidebarLifecycle::Hidden {
                    generation.lifecycle = SidebarLifecycle::Ready;
                    vec![LifecycleEffect::BroadcastState(self.snapshot())]
                } else {
                    Vec::new()
                }
            }
            (
                ServerPhase::Running(generation),
                LifecycleOperation::PresenceTargetFailed { window_id, reason },
            ) => {
                let Some(presence) = generation.presence.as_mut() else {
                    return Vec::new();
                };
                let Some(target) = presence_target_for_window(presence, &window_id) else {
                    return Vec::new();
                };
                if presence.pending_windows.remove(&target) {
                    if presence.in_flight_spawn.as_ref() == Some(&target) {
                        presence.in_flight_spawn = None;
                    }
                    presence.failed_windows.insert(target, reason);
                }
                if presence.pending_windows.is_empty() {
                    generation.presence = None;
                    generation.lifecycle = SidebarLifecycle::Ready;
                    vec![LifecycleEffect::BroadcastState(self.snapshot())]
                } else {
                    Vec::new()
                }
            }
            (
                ServerPhase::Running(generation),
                LifecycleOperation::BeginResize {
                    owner,
                    target_width,
                    targets,
                },
            ) => {
                if generation.resize.is_some() {
                    return Vec::new();
                }
                generation.lifecycle = SidebarLifecycle::Ready;
                let pending_targets = targets.into_iter().collect::<BTreeSet<_>>();
                generation.resize = Some(ResizeAdjustment {
                    owner,
                    target_width,
                    pending_targets,
                    acknowledged_targets: BTreeSet::new(),
                });
                vec![LifecycleEffect::BroadcastState(self.snapshot())]
            }
            (
                ServerPhase::Running(generation),
                LifecycleOperation::AckResize {
                    target,
                    observed_width,
                },
            ) => {
                let Some(resize) = generation.resize.as_mut() else {
                    return Vec::new();
                };
                if observed_width != resize.target_width || !resize.pending_targets.remove(&target)
                {
                    return Vec::new();
                }
                resize.acknowledged_targets.insert(target);
                if resize.pending_targets.is_empty() {
                    generation.resize = None;
                    vec![LifecycleEffect::BroadcastState(self.snapshot())]
                } else {
                    Vec::new()
                }
            }
            (
                ServerPhase::Running(_),
                LifecycleOperation::SwitchSession {
                    client_id,
                    target_session,
                },
            ) => {
                let view = ClientViewState {
                    current_session: Some(target_session.clone()),
                    sidebar_focus: Some(target_session),
                };
                self.connected_clients
                    .insert(client_id.clone(), view.clone());
                vec![LifecycleEffect::SendClientView { client_id, view }]
            }
            (ServerPhase::Running(_), LifecycleOperation::RequestQuit { requested_by }) => {
                self.phase = ServerPhase::Closing(ClosingGeneration { requested_by });
                let mut effects = vec![LifecycleEffect::BroadcastState(self.snapshot())];
                effects.extend(
                    self.connected_clients
                        .keys()
                        .filter(|client_id| {
                            !matches!(
                                &self.phase,
                                ServerPhase::Closing(closing)
                                    if closing.requested_by == **client_id
                            )
                        })
                        .cloned()
                        .map(|client_id| LifecycleEffect::SendQuit { client_id }),
                );
                effects
            }
            (ServerPhase::Closing(_), LifecycleOperation::DrainComplete) => {
                self.phase = ServerPhase::Closed;
                vec![LifecycleEffect::BroadcastState(self.snapshot())]
            }
            (ServerPhase::Closing(_), LifecycleOperation::RequestQuit { .. }) => Vec::new(),
            (ServerPhase::Closing(_), _) => Vec::new(),
            (ServerPhase::Closed, _) => Vec::new(),
            (ServerPhase::Running(_), LifecycleOperation::DrainComplete) => Vec::new(),
        }
    }
}

fn ordered_presence_targets(
    windows: Vec<TmuxWindowTarget>,
    origin_session: Option<&str>,
) -> Vec<SidebarPresenceTarget> {
    let mut by_window = BTreeMap::<String, SidebarPresenceTarget>::new();
    for window in windows.into_iter().filter(|window| !window.stash) {
        by_window
            .entry(window.window_id.clone())
            .or_insert_with(|| SidebarPresenceTarget {
                session: window.session,
                window_id: window.window_id,
            });
    }

    let mut targets = by_window.into_values().collect::<Vec<_>>();
    targets.sort_by_key(|target| {
        (
            origin_session.is_some_and(|origin| target.session != origin),
            target.session.clone(),
            target.window_id.clone(),
        )
    });
    targets
}

fn presence_target_for_window(
    presence: &SidebarPresenceReconciliation,
    window_id: &str,
) -> Option<SidebarPresenceTarget> {
    presence
        .pending_windows
        .iter()
        .chain(presence.connected_windows.iter())
        .chain(presence.failed_windows.keys())
        .find(|target| target.window_id == window_id)
        .cloned()
}

impl Default for LifecycleChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleChannel {
    pub fn new() -> Self {
        Self {
            server: LifecycleServer::new(),
            broadcasting: false,
        }
    }

    pub fn server(&self) -> &LifecycleServer {
        &self.server
    }

    pub fn submit(
        &mut self,
        operation: LifecycleOperation,
        mut deliver: impl FnMut(&LifecycleEffect, &mut Self),
    ) -> Result<Vec<LifecycleEffect>, LifecycleSubmitError> {
        if self.broadcasting {
            return Err(LifecycleSubmitError::BroadcastInProgress);
        }

        let effects = self.server.apply(operation);
        self.broadcasting = true;
        for effect in &effects {
            deliver(effect, self);
        }
        self.broadcasting = false;
        Ok(effects)
    }
}
