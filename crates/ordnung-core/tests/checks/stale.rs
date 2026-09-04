// Tests for `src/checks/stale.rs`.
use crate::support::*;

#[test]
fn stale_reports_idle_pulls_merged_branches_and_cleanup_setting() {
    let mut repository = facts();
    repository.delete_branch_on_merge = false;
    repository.stale = GithubValue::known(GithubStaleFacts {
        open_pull_requests: vec![GithubPullRequestAgeFacts {
            number: 17,
            updated_at: "2020-01-01T00:00:00Z".into(),
            idle_days: 45,
        }],
        merged_branches: vec!["finished".into()],
        examined_branches: 20,
        non_default_branches: 25,
        ..GithubStaleFacts::default()
    });
    let report = run_github_checks(&repository);
    let stale = report
        .results
        .iter()
        .find(|result| result.check == "stale")
        .unwrap();
    assert_eq!(stale.status, CheckStatus::Fail);
    assert!(stale.message.contains("#17 (45d)"));
    assert!(stale.message.contains("finished"));
    assert!(stale.message.contains("automatic branch deletion"));
    assert!(stale.message.contains("20 of 25"));
}
