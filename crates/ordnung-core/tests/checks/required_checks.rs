// Tests for `src/checks/required_checks.rs`.
use crate::support::*;

#[test]
fn required_checks_reports_each_unprotected_pr_job() {
    let mut facts = facts();
    facts.pull_request_checks = GithubValue::known(vec!["Build".into(), "CI".into()]);

    let report = run_github_checks(&facts);
    let required = report
        .results
        .iter()
        .find(|result| result.check == "required-checks")
        .unwrap();
    assert_eq!(required.status, CheckStatus::Fail);
    assert_eq!(
        required.message,
        "pull-request checks not required on the default branch: Build"
    );
}

#[test]
fn required_checks_skips_when_no_pr_workflow_posts_checks() {
    let mut facts = facts();
    facts.pull_request_checks = GithubValue::known(Vec::new());

    let report = run_github_checks(&facts);
    let required = report
        .results
        .iter()
        .find(|result| result.check == "required-checks")
        .unwrap();
    assert_eq!(required.status, CheckStatus::Skip);
}

#[test]
fn unreadable_required_checks_skip_for_private_repositories() {
    let mut facts = facts();
    facts.visibility = "private".into();
    facts.branch.required_checks = GithubValue::unavailable("HTTP 403");

    let report = run_github_checks(&facts);
    let required = report
        .results
        .iter()
        .find(|result| result.check == "required-checks")
        .unwrap();
    assert_eq!(required.status, CheckStatus::Skip);
}
