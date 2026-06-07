#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInfo {
    pub branch: String,
    pub dirty: bool,
    pub is_worktree: bool,
    pub changed_files: u32,
    pub insertions: u32,
    pub deletions: u32,
}

impl GitInfo {
    pub fn empty() -> Self {
        Self {
            branch: String::new(),
            dirty: false,
            is_worktree: false,
            changed_files: 0,
            insertions: 0,
            deletions: 0,
        }
    }
}

pub fn parse_git_info_output(output: &str) -> GitInfo {
    let output = output.trim();
    if output.is_empty() {
        return GitInfo::empty();
    }

    let (header, rest) = output
        .split_once("---")
        .map(|(header, rest)| (header.trim(), rest.trim()))
        .unwrap_or((output, ""));
    let (status, numstat) = rest
        .split_once("---NUMSTAT---")
        .map(|(status, numstat)| (status.trim(), numstat.trim()))
        .unwrap_or((rest, ""));
    let mut lines = header.lines();
    let branch = lines.next().unwrap_or_default().trim().to_string();
    let git_dir = lines.next().unwrap_or_default().trim();
    let changed_files = status
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count() as u32;
    let (insertions, deletions) = parse_numstat_totals(numstat);

    GitInfo {
        branch,
        dirty: changed_files > 0,
        is_worktree: git_dir.contains("/worktrees/"),
        changed_files,
        insertions,
        deletions,
    }
}

fn parse_numstat_totals(numstat: &str) -> (u32, u32) {
    numstat
        .lines()
        .fold((0, 0), |(insertions, deletions), line| {
            let mut fields = line.split_whitespace();
            let added = fields
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            let removed = fields
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            (insertions + added, deletions + removed)
        })
}
