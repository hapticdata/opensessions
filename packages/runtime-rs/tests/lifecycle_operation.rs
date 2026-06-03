use opensessions_runtime::lifecycle_operation::PresenceFailureReason;
use opensessions_runtime::lifecycle_operation::{
    ClientId, LifecycleChannel, LifecycleEffect, LifecycleOperation, LifecycleServer,
    LifecycleSubmitError, ResizeOwner, ResizeTarget, ServerPhase, SidebarWidth, SnapshotPhase,
    TmuxWindowTarget,
};

#[test]
fn quit_broadcasts_closing_then_quit_to_connected_clients() {
    let mut server = LifecycleServer::new();
    let alpha = ClientId::new("alpha");
    let beta = ClientId::new("beta");

    server.apply(LifecycleOperation::SidebarConnected {
        client_id: alpha.clone(),
        window_id: None,
        current_session: None,
    });
    server.apply(LifecycleOperation::SidebarConnected {
        client_id: beta.clone(),
        window_id: None,
        current_session: None,
    });

    let effects = server.apply(LifecycleOperation::RequestQuit {
        requested_by: alpha.clone(),
    });

    assert!(matches!(server.phase(), ServerPhase::Closing(_)));
    assert_eq!(
        effects,
        vec![
            LifecycleEffect::BroadcastState(closing_snapshot()),
            LifecycleEffect::SendQuit { client_id: beta },
        ]
    );
}

#[test]
fn quit_to_one_hundred_clients_excludes_requester() {
    let mut server = LifecycleServer::new();
    let clients = (0..100)
        .map(|idx| ClientId::new(format!("client-{idx:03}")))
        .collect::<Vec<_>>();
    for client_id in &clients {
        server.apply(LifecycleOperation::SidebarConnected {
            client_id: client_id.clone(),
            window_id: None,
            current_session: None,
        });
    }

    let requester = clients[42].clone();
    let effects = server.apply(LifecycleOperation::RequestQuit {
        requested_by: requester.clone(),
    });
    let quit_recipients = effects
        .iter()
        .filter_map(|effect| match effect {
            LifecycleEffect::SendQuit { client_id } => Some(client_id.clone()),
            LifecycleEffect::BroadcastState(_)
            | LifecycleEffect::SpawnSidebar { .. }
            | LifecycleEffect::SendClientView { .. } => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(quit_recipients.len(), 99);
    assert!(!quit_recipients.contains(&requester));
    for client_id in clients.iter().filter(|client_id| **client_id != requester) {
        assert!(quit_recipients.contains(client_id));
    }
}

#[test]
fn lifecycle_channel_rejects_reentrant_quit_during_broadcast() {
    let mut channel = LifecycleChannel::new();
    let alpha = ClientId::new("alpha");
    let beta = ClientId::new("beta");

    channel
        .submit(
            LifecycleOperation::SidebarConnected {
                client_id: alpha.clone(),
                window_id: None,
                current_session: None,
            },
            |_, _| {},
        )
        .unwrap();
    channel
        .submit(
            LifecycleOperation::SidebarConnected {
                client_id: beta.clone(),
                window_id: None,
                current_session: None,
            },
            |_, _| {},
        )
        .unwrap();

    let mut reentrant_result = None;
    let effects = channel
        .submit(
            LifecycleOperation::RequestQuit {
                requested_by: alpha.clone(),
            },
            |effect, channel| {
                if matches!(effect, LifecycleEffect::BroadcastState(_)) {
                    reentrant_result = Some(channel.submit(
                        LifecycleOperation::RequestQuit {
                            requested_by: beta.clone(),
                        },
                        |_, _| {},
                    ));
                }
            },
        )
        .unwrap();

    assert_eq!(
        reentrant_result,
        Some(Err(LifecycleSubmitError::BroadcastInProgress))
    );
    assert_eq!(
        effects,
        vec![
            LifecycleEffect::BroadcastState(closing_snapshot()),
            LifecycleEffect::SendQuit { client_id: beta },
        ]
    );
    assert!(matches!(channel.server().phase(), ServerPhase::Closing(_)));
}

#[test]
fn resize_adjustment_ignores_competing_resize_targets() {
    let mut server = LifecycleServer::new();
    let owner = ResizeOwner {
        client_id: ClientId::new("owner"),
        target: resize_target("alpha", "@1", "%1"),
    };
    let competitor = ResizeOwner {
        client_id: ClientId::new("competitor"),
        target: resize_target("beta", "@2", "%2"),
    };

    assert_eq!(
        server.apply(LifecycleOperation::BeginResize {
            owner: owner.clone(),
            target_width: SidebarWidth(36),
            targets: vec![
                resize_target("alpha", "@1", "%1"),
                resize_target("beta", "@2", "%2")
            ],
        }),
        vec![LifecycleEffect::BroadcastState(adjusting_snapshot())]
    );

    assert_eq!(
        server.apply(LifecycleOperation::BeginResize {
            owner: competitor,
            target_width: SidebarWidth(24),
            targets: vec![resize_target("beta", "@2", "%2")],
        }),
        Vec::new(),
        "a stale pane must not overwrite the active resize target"
    );

    let resize = match server.phase() {
        ServerPhase::Running(generation) => generation.resize.as_ref().unwrap(),
        phase => panic!("expected running phase, got {phase:?}"),
    };
    assert_eq!(resize.owner, owner);
    assert_eq!(resize.target_width, SidebarWidth(36));
    assert_eq!(resize.pending_targets.len(), 2);
}

#[test]
fn resize_adjustment_stays_adjusting_until_all_targets_ack_target_width() {
    let mut server = LifecycleServer::new();
    let alpha = resize_target("alpha", "@1", "%1");
    let beta = resize_target("beta", "@2", "%2");

    server.apply(LifecycleOperation::BeginResize {
        owner: ResizeOwner {
            client_id: ClientId::new("owner"),
            target: alpha.clone(),
        },
        target_width: SidebarWidth(36),
        targets: vec![alpha.clone(), beta.clone()],
    });

    assert_eq!(
        server.apply(LifecycleOperation::AckResize {
            target: beta.clone(),
            observed_width: SidebarWidth(24),
        }),
        Vec::new(),
        "stale width observations are ignored"
    );
    assert_eq!(server.snapshot(), adjusting_snapshot());

    assert_eq!(
        server.apply(LifecycleOperation::AckResize {
            target: alpha,
            observed_width: SidebarWidth(36),
        }),
        Vec::new(),
        "one target ack is not enough to finish the adjustment"
    );
    assert_eq!(server.snapshot(), adjusting_snapshot());

    assert_eq!(
        server.apply(LifecycleOperation::AckResize {
            target: beta,
            observed_width: SidebarWidth(36),
        }),
        vec![LifecycleEffect::BroadcastState(ready_snapshot())]
    );
    assert_eq!(server.snapshot(), ready_snapshot());
}

#[test]
fn warmup_dedupes_linked_windows_and_completes_when_each_unique_window_connects() {
    let mut server = LifecycleServer::new();

    assert_eq!(
        server.apply(LifecycleOperation::BeginWarmup {
            origin_session: Some("alpha".to_string()),
            windows: vec![
                tmux_window("alpha", "@1", false),
                tmux_window("beta", "@2", false),
                tmux_window("gamma", "@2", false),
                tmux_window("_os_stash", "@stash", true),
            ],
        }),
        vec![LifecycleEffect::BroadcastState(warming_snapshot())]
    );

    let presence = match server.phase() {
        ServerPhase::Running(generation) => generation.presence.as_ref().unwrap(),
        phase => panic!("expected running phase, got {phase:?}"),
    };
    assert_eq!(
        presence
            .pending_windows
            .iter()
            .map(|target| target.window_id.as_str())
            .collect::<Vec<_>>(),
        vec!["@1", "@2"],
        "linked session windows should be targeted once by window_id and stash should be ignored"
    );

    assert_eq!(
        server.apply(LifecycleOperation::SidebarConnected {
            client_id: ClientId::new("sidebar-1"),
            window_id: Some("@1".to_string()),
            current_session: None,
        }),
        Vec::new(),
        "one connected sidebar is not enough to finish warmup"
    );
    assert_eq!(server.snapshot(), warming_snapshot());

    assert_eq!(
        server.apply(LifecycleOperation::SidebarConnected {
            client_id: ClientId::new("sidebar-2"),
            window_id: Some("@2".to_string()),
            current_session: None,
        }),
        vec![LifecycleEffect::BroadcastState(ready_snapshot())]
    );
    assert_eq!(server.snapshot(), ready_snapshot());
}

#[test]
fn closing_is_terminal_until_drain_completes() {
    let mut server = LifecycleServer::new();
    let alpha = ClientId::new("alpha");
    let late = ClientId::new("late");

    assert_eq!(
        server
            .apply(LifecycleOperation::BeginWarmup {
                origin_session: Some("alpha".to_string()),
                windows: vec![tmux_window("alpha", "@1", false)],
            })
            .first()
            .cloned(),
        Some(LifecycleEffect::BroadcastState(warming_snapshot()))
    );

    server.apply(LifecycleOperation::RequestQuit {
        requested_by: alpha.clone(),
    });
    assert_eq!(server.snapshot(), closing_snapshot());

    assert_eq!(
        server.apply(LifecycleOperation::SidebarConnected {
            client_id: late,
            window_id: Some("@1".to_string()),
            current_session: None,
        }),
        Vec::new(),
        "late client identify must not move closing back to ready"
    );
    assert_eq!(server.apply(LifecycleOperation::WarmupComplete), Vec::new());
    assert_eq!(
        server.apply(LifecycleOperation::BeginWarmup {
            origin_session: Some("alpha".to_string()),
            windows: vec![tmux_window("alpha", "@1", false)],
        }),
        Vec::new()
    );
    assert_eq!(
        server.apply(LifecycleOperation::RequestQuit {
            requested_by: alpha,
        }),
        Vec::new(),
        "quit is idempotent after closing starts"
    );
    assert_eq!(server.snapshot(), closing_snapshot());

    assert_eq!(
        server.apply(LifecycleOperation::DrainComplete),
        vec![LifecycleEffect::BroadcastState(closed_snapshot())]
    );
    assert!(matches!(server.phase(), ServerPhase::Closed));
}

#[test]
fn warmup_timer_cannot_complete_active_presence_reconciliation() {
    let mut server = LifecycleServer::new();

    server.apply(LifecycleOperation::BeginWarmup {
        origin_session: None,
        windows: vec![],
    });
    assert_eq!(server.snapshot(), warming_snapshot());

    assert_eq!(
        server.apply(LifecycleOperation::WarmupComplete),
        Vec::new(),
        "timer completion must not clear warmup while presence reconciliation is active"
    );

    let generation = match server.phase() {
        ServerPhase::Running(generation) => generation,
        phase => panic!("expected running phase, got {phase:?}"),
    };
    assert!(generation.presence.is_some());
}

#[test]
fn warmup_completion_requires_connected_presence_targets() {
    let mut server = LifecycleServer::new();

    server.apply(LifecycleOperation::BeginWarmup {
        origin_session: Some("alpha".to_string()),
        windows: vec![tmux_window("alpha", "@1", false)],
    });
    assert_eq!(server.snapshot(), warming_snapshot());

    assert_eq!(
        server.apply(LifecycleOperation::SidebarConnected {
            client_id: ClientId::new("sidebar-1"),
            window_id: Some("@1".to_string()),
            current_session: None,
        }),
        vec![LifecycleEffect::BroadcastState(ready_snapshot())]
    );
    assert_eq!(server.snapshot(), ready_snapshot());
}

#[test]
fn warmup_spawns_one_sidebar_at_a_time_origin_session_first() {
    let mut server = LifecycleServer::new();

    server.apply(LifecycleOperation::BeginWarmup {
        origin_session: Some("alpha".to_string()),
        windows: vec![
            tmux_window("beta", "@2", false),
            tmux_window("alpha", "@1", false),
            tmux_window("alpha", "@3", false),
            tmux_window("gamma", "@4", false),
        ],
    });

    assert_eq!(
        server.apply(LifecycleOperation::RequestNextSpawn),
        vec![LifecycleEffect::SpawnSidebar {
            session: "alpha".to_string(),
            window_id: "@1".to_string(),
        }]
    );
    assert_eq!(
        server.apply(LifecycleOperation::RequestNextSpawn),
        Vec::new(),
        "only one warmup spawn may be in flight at a time"
    );
    assert_eq!(
        server.apply(LifecycleOperation::SidebarConnected {
            client_id: ClientId::new("sidebar-1"),
            window_id: Some("@1".to_string()),
            current_session: None,
        }),
        Vec::new()
    );
    assert_eq!(
        server.apply(LifecycleOperation::RequestNextSpawn),
        vec![LifecycleEffect::SpawnSidebar {
            session: "alpha".to_string(),
            window_id: "@3".to_string(),
        }]
    );
    assert_eq!(
        server.apply(LifecycleOperation::PresenceTargetFailed {
            window_id: "@3".to_string(),
            reason: PresenceFailureReason::SpawnFailed,
        }),
        Vec::new()
    );
    assert_eq!(
        server.apply(LifecycleOperation::RequestNextSpawn),
        vec![LifecycleEffect::SpawnSidebar {
            session: "beta".to_string(),
            window_id: "@2".to_string(),
        }]
    );
    assert_eq!(
        server.apply(LifecycleOperation::SidebarConnected {
            client_id: ClientId::new("sidebar-2"),
            window_id: Some("@2".to_string()),
            current_session: None,
        }),
        Vec::new()
    );
    assert_eq!(
        server.apply(LifecycleOperation::RequestNextSpawn),
        vec![LifecycleEffect::SpawnSidebar {
            session: "gamma".to_string(),
            window_id: "@4".to_string(),
        }]
    );
    assert_eq!(
        server.apply(LifecycleOperation::SidebarConnected {
            client_id: ClientId::new("sidebar-3"),
            window_id: Some("@4".to_string()),
            current_session: None,
        }),
        vec![LifecycleEffect::BroadcastState(ready_snapshot())]
    );
    assert_eq!(
        server.apply(LifecycleOperation::RequestNextSpawn),
        Vec::new()
    );
}

#[test]
fn warmup_failed_targets_are_diagnosed_and_do_not_strand_warmup() {
    let mut server = LifecycleServer::new();

    server.apply(LifecycleOperation::BeginWarmup {
        origin_session: Some("alpha".to_string()),
        windows: vec![
            tmux_window("alpha", "@1", false),
            tmux_window("beta", "@2", false),
        ],
    });

    assert_eq!(
        server.apply(LifecycleOperation::SidebarConnected {
            client_id: ClientId::new("sidebar-1"),
            window_id: Some("@1".to_string()),
            current_session: None,
        }),
        Vec::new()
    );

    assert_eq!(
        server.apply(LifecycleOperation::PresenceTargetFailed {
            window_id: "@2".to_string(),
            reason: PresenceFailureReason::ConnectTimeout,
        }),
        vec![LifecycleEffect::BroadcastState(ready_snapshot())]
    );
    assert_eq!(server.snapshot(), ready_snapshot());
}

#[test]
fn switch_session_updates_only_requesting_client_view() {
    let mut server = LifecycleServer::new();
    let client_a = ClientId::new("ghostty-a");
    let client_b = ClientId::new("ghostty-b");

    server.apply(LifecycleOperation::SidebarConnected {
        client_id: client_a.clone(),
        window_id: Some("@1".to_string()),
        current_session: Some("alpha".to_string()),
    });
    server.apply(LifecycleOperation::SidebarConnected {
        client_id: client_b.clone(),
        window_id: Some("@2".to_string()),
        current_session: Some("gamma".to_string()),
    });

    let effects = server.apply(LifecycleOperation::SwitchSession {
        client_id: client_a.clone(),
        target_session: "beta".to_string(),
    });

    assert_eq!(
        effects,
        vec![LifecycleEffect::SendClientView {
            client_id: client_a.clone(),
            view: opensessions_runtime::lifecycle_operation::ClientViewState {
                current_session: Some("beta".to_string()),
                sidebar_focus: Some("beta".to_string()),
            },
        }]
    );
    assert_eq!(
        server
            .client_view(&client_a)
            .and_then(|view| view.current_session.as_deref()),
        Some("beta")
    );
    assert_eq!(
        server
            .client_view(&client_b)
            .and_then(|view| view.current_session.as_deref()),
        Some("gamma"),
        "switching one client must not mutate another client's current session"
    );
}

fn warming_snapshot() -> opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
    opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
        phase: SnapshotPhase::Warming,
        visible: true,
        initializing: true,
        init_label: Some("warming up…"),
    }
}

fn ready_snapshot() -> opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
    opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
        phase: SnapshotPhase::Ready,
        visible: true,
        initializing: false,
        init_label: None,
    }
}

fn adjusting_snapshot() -> opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
    opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
        phase: SnapshotPhase::Adjusting,
        visible: true,
        initializing: true,
        init_label: Some("adjusting…"),
    }
}

fn closing_snapshot() -> opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
    opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
        phase: SnapshotPhase::Closing,
        visible: true,
        initializing: true,
        init_label: Some("closing…"),
    }
}

fn closed_snapshot() -> opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
    opensessions_runtime::lifecycle_operation::LifecycleSnapshot {
        phase: SnapshotPhase::Closed,
        visible: false,
        initializing: false,
        init_label: None,
    }
}

fn resize_target(session: &str, window_id: &str, pane_id: &str) -> ResizeTarget {
    ResizeTarget {
        session: session.to_string(),
        window_id: window_id.to_string(),
        pane_id: pane_id.to_string(),
    }
}

fn tmux_window(session: &str, window_id: &str, stash: bool) -> TmuxWindowTarget {
    TmuxWindowTarget {
        session: session.to_string(),
        window_id: window_id.to_string(),
        stash,
    }
}
