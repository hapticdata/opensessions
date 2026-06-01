use opensessions_runtime::git_info::{GitInfo, parse_git_info_output};

#[test]
fn parse_git_info_output_extracts_branch_dirty_and_worktree() {
    assert_eq!(
        parse_git_info_output(
            "feature/rust\n.git/worktrees/rust\n---\n M src/lib.rs\n---NUMSTAT---\n4\t10\tsrc/lib.rs\n",
        ),
        GitInfo {
            branch: "feature/rust".to_string(),
            dirty: true,
            is_worktree: true,
            changed_files: 1,
            insertions: 4,
            deletions: 10,
        }
    );
}

#[test]
fn parse_git_info_output_handles_clean_or_empty_output() {
    assert_eq!(
        parse_git_info_output("main\n.git\n---\n"),
        GitInfo {
            branch: "main".to_string(),
            dirty: false,
            is_worktree: false,
            changed_files: 0,
            insertions: 0,
            deletions: 0,
        }
    );
    assert_eq!(parse_git_info_output(""), GitInfo::empty());
}
