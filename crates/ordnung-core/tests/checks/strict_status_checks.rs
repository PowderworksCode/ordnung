// Tests for `src/checks/strict_status_checks.rs`.
use crate::support::*;

#[test]
fn unavailable_strict_policy_is_an_error() {
    let mut facts = facts();
    facts.branch.strict_status_checks = GithubValue::unavailable("HTTP 403");
    let report = run_github_checks(&facts);
    // An unreadable setting is reported as an error rather than a silent pass. The
    // check is advisory by default, so it does not by itself make a report unclean.
    let result = report
        .results
        .iter()
        .find(|result| result.check == "strict-status-checks")
        .expect("strict-status-checks runs");
    assert_eq!(result.status, CheckStatus::Error);
    assert_eq!(result.severity, Severity::Recommended);
}

#[test]
fn unavailable_private_strict_policy_is_skipped() {
    let mut facts = facts();
    facts.visibility = "private".into();
    facts.branch.strict_status_checks = GithubValue::unavailable("HTTP 403");
    let report = run_github_checks(&facts);
    assert!(report.results.iter().any(|result| {
        result.check == "strict-status-checks" && result.status == CheckStatus::Skip
    }));
}

#[test]
fn strict_status_checks_distinguish_missing_required_checks() {
    let mut facts = facts();
    facts.allow_update_branch = false;
    facts.branch.required_checks = GithubValue::known(Vec::new());
    facts.branch.strict_status_checks = GithubValue::known(false);

    let report = run_github_checks(&facts);
    let strict = report
        .results
        .iter()
        .find(|result| result.check == "strict-status-checks")
        .unwrap();
    assert_eq!(strict.status, CheckStatus::Fail);
    assert!(strict.message.contains("no required status checks"));
    assert!(strict.message.contains("suggestions are also disabled"));
}

#[test]
fn strict_status_checks_recommend_update_branch_without_failing() {
    let mut facts = facts();
    facts.allow_update_branch = false;

    let report = run_github_checks(&facts);
    let strict = report
        .results
        .iter()
        .find(|result| result.check == "strict-status-checks")
        .unwrap();
    assert_eq!(strict.status, CheckStatus::Pass);
    assert!(strict.message.contains("enable update-branch suggestions"));
}
