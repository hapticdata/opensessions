use crate::generated::protocol::{AgentEvent, AgentLiveness, AgentStatus, LocalLink, SessionData};

pub fn fixture_static_name(name: &str) -> Option<&'static str> {
    match name {
        "pane-attached-session-list" => Some("pane-attached-session-list"),
        "pane-opensessions-self" => Some("pane-opensessions-self"),
        "pane-multi-window" => Some("pane-multi-window"),
        _ => None,
    }
}

pub fn reference_sessions() -> Vec<SessionData> {
    vec![
        session("_os_stash", "/tmp/_os_stash", "", None, Vec::new()),
        session(
            "plane-feat-edit-pages-from-pi",
            "/Users/palanikannanm/Documents/work/feat-edit-pages-from-pi",
            "feat/edit-pages-from-pi",
            None,
            Vec::new(),
        ),
        session(
            "plane-feat-background-exports",
            "/Users/palanikannanm/Documents/work/feat-background-exports",
            "feat-background-exports",
            None,
            Vec::new(),
        ),
        session(
            "learning",
            "/Users/palanikannanm/Documents/work/learning",
            "main",
            None,
            Vec::new(),
        ),
        session(
            "opensessions",
            "/Users/palanikannanm/Documents/work/opensessions",
            "devpulse",
            Some(agent(
                "amp",
                "opensessions",
                AgentStatus::ToolRunning,
                Some("Query tmux for open sessions"),
                None,
            )),
            vec![
                agent(
                    "amp",
                    "opensessions",
                    AgentStatus::ToolRunning,
                    Some("Query tmux for open sessions"),
                    None,
                ),
                agent("amp", "opensessions", AgentStatus::Idle, None, None),
            ],
        ),
        session(
            "plane-pdf-word-formatting",
            "/Users/palanikannanm/Documents/work/plane-ee-wt/pdf-word-formatting",
            "chore-relation-pqls",
            Some(agent_with_liveness(
                "amp",
                "plane-pdf-word-formatting",
                AgentStatus::Done,
                Some("Review GitHub PR for Plane"),
                Some(true),
                None,
            )),
            vec![
                agent_with_liveness(
                    "amp",
                    "plane-pdf-word-formatting",
                    AgentStatus::Done,
                    Some("Review GitHub PR for Plane"),
                    Some(true),
                    None,
                ),
                agent(
                    "amp",
                    "plane-pdf-word-formatting",
                    AgentStatus::Idle,
                    None,
                    None,
                ),
            ],
        ),
        session(
            "dotfiles_public",
            "/Users/palanikannanm/Documents/work/dotfiles.public",
            "main",
            None,
            Vec::new(),
        ),
    ]
}

fn session(
    name: &str,
    dir: &str,
    branch: &str,
    agent_state: Option<AgentEvent>,
    agents: Vec<AgentEvent>,
) -> SessionData {
    SessionData {
        name: name.to_string(),
        created_at: 0,
        dir: dir.to_string(),
        branch: branch.to_string(),
        dirty: false,
        changed_files: 0,
        insertions: 0,
        deletions: 0,
        is_worktree: false,
        unseen: name == "plane-pdf-word-formatting",
        panes: 1,
        ports: Vec::new(),
        local_links: Vec::<LocalLink>::new(),
        windows: 1,
        uptime: String::new(),
        agent_state,
        agents,
        event_timestamps: Vec::new(),
        metadata: None,
    }
}

fn agent(
    agent_name: &str,
    session: &str,
    status: AgentStatus,
    thread_name: Option<&str>,
    unseen: Option<bool>,
) -> AgentEvent {
    agent_with_liveness(
        agent_name,
        session,
        status,
        thread_name,
        unseen,
        Some(AgentLiveness::Alive),
    )
}

fn agent_with_liveness(
    agent_name: &str,
    session: &str,
    status: AgentStatus,
    thread_name: Option<&str>,
    unseen: Option<bool>,
    liveness: Option<AgentLiveness>,
) -> AgentEvent {
    AgentEvent {
        agent: agent_name.to_string(),
        session: session.to_string(),
        status,
        ts: 0,
        thread_id: None,
        thread_name: thread_name.map(str::to_string),
        unseen,
        pane_id: None,
        liveness,
    }
}
