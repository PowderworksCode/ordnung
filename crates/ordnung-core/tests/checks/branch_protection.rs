// Tests for `src/checks/branch_protection.rs`.
use crate::support::*;

#[test]
fn branch_protection_reports_each_missing_safeguard() {
    let mut facts = facts();
    facts.branch.protection = GithubValue::known(GithubBranchProtectionFacts {
        pull_requests_required: false,
        force_pushes_blocked: false,
        deletion_blocked: false,
    });

    let report = run_github_checks(&facts);
    let protection = report
        .results
        .iter()
        .find(|result| result.check == "branch-protection")
        .unwrap();
    assert_eq!(protection.status, CheckStatus::Fail);
    assert!(
        protection
            .message
            .contains("pull requests are not required")
    );
    assert!(protection.message.contains("force pushes are allowed"));
    assert!(protection.message.contains("branch deletion is allowed"));
}

#[test]
fn unavailable_private_branch_protection_is_skipped() {
    let mut facts = facts();
    facts.visibility = "private".into();
    facts.branch.protection = GithubValue::unavailable("HTTP 403");

    let report = run_github_checks(&facts);
    let protection = report
        .results
        .iter()
        .find(|result| result.check == "branch-protection")
        .unwrap();
    assert_eq!(protection.status, CheckStatus::Skip);
}
