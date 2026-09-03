// Tests for `src/checks/repo_meta.rs`.
use crate::support::*;
use crate::support_github::*;

#[test]
fn passes_on_a_described_repository_with_issues_open() {
    assert_eq!(github_status(&facts(), "repo-meta"), CheckStatus::Pass);
}

/// A repository with no description is one whose purpose is only discoverable
/// by reading it.
#[test]
fn fails_without_a_description() {
    let mut facts = facts();
    facts.description = None;
    assert_eq!(github_status(&facts, "repo-meta"), CheckStatus::Fail);
}

/// Issues are where a reader reports what the repository got wrong. Turning
/// them off is a decision, and this is where it gets stated out loud.
#[test]
fn fails_with_the_issue_tracker_closed() {
    let mut facts = facts();
    facts.has_issues = false;
    assert_eq!(github_status(&facts, "repo-meta"), CheckStatus::Fail);
}
