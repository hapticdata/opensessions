use std::collections::{HashMap, HashSet};

use crate::generated::protocol::SessionData;

#[derive(Debug, Clone)]
pub enum DisplaySessionEntry<'a> {
    Group {
        label: String,
        count: usize,
    },
    Session {
        index: usize,
        session: &'a SessionData,
        indented: bool,
    },
}

pub(crate) fn session_display_entries(sessions: Vec<&SessionData>) -> Vec<DisplaySessionEntry<'_>> {
    let grouped_keys = grouped_worktree_keys(&sessions);
    let mut emitted = HashSet::<String>::new();
    let mut entries = Vec::with_capacity(sessions.len());
    let mut display_index = 0;

    for session in sessions.iter().copied() {
        let Some(key) = worktree_group_key(session).filter(|key| grouped_keys.contains(key)) else {
            display_index += 1;
            entries.push(DisplaySessionEntry::Session {
                index: display_index,
                session,
                indented: false,
            });
            continue;
        };

        if !emitted.insert(key.clone()) {
            continue;
        }

        let members = sessions
            .iter()
            .copied()
            .filter(|candidate| worktree_group_key(candidate).as_deref() == Some(key.as_str()))
            .collect::<Vec<_>>();
        entries.push(DisplaySessionEntry::Group {
            label: group_label(&key),
            count: members.len(),
        });
        for member in members {
            display_index += 1;
            entries.push(DisplaySessionEntry::Session {
                index: display_index,
                session: member,
                indented: true,
            });
        }
    }

    entries
}

fn grouped_worktree_keys(sessions: &[&SessionData]) -> HashSet<String> {
    let mut counts = HashMap::<String, usize>::new();
    for session in sessions {
        if let Some(key) = worktree_group_key(session) {
            *counts.entry(key).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .filter_map(|(key, count)| (count >= 2).then_some(key))
        .collect()
}

fn worktree_group_key(session: &SessionData) -> Option<String> {
    if !session.is_worktree {
        return None;
    }
    let dir = session.dir.trim_end_matches('/');
    let (parent, _) = dir.rsplit_once('/')?;
    (!parent.is_empty()).then(|| parent.to_string())
}

fn group_label(key: &str) -> String {
    key.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(key)
        .to_string()
}
