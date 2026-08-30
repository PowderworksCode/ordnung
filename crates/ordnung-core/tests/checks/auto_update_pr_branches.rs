// Tests for `src/checks/auto_update_pr_branches.rs`.
use crate::support::*;
use crate::support_github::*;

/// The setting only means something where the default branch requires its
/// checks to be current; without strict status checks there is nothing for an
/// automatic update to keep current.
#[test]
fn skips_when_strict_status_checks_are_off() {
    let mut facts = facts();
    facts.branch.strict_status_checks = GithubValue::known(false);
    assert_eq!(
        github_status(&facts, "auto-update-pr-branches"),
        CheckStatus::Skip
    );
}

#[test]
fn passes_when_branches_update_themselves() {
    assert_eq!(
        github_status(&facts(), "auto-update-pr-branches"),
        CheckStatus::Pass
    );
}

#[test]
fn fails_when_the_setting_is_off() {
    let mut facts = facts();
    facts.allow_update_branch = false;
    assert_eq!(
        github_status(&facts, "auto-update-pr-branches"),
        CheckStatus::Fail
    );
}
