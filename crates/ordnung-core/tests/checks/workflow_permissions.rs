// Tests for `src/checks/workflow_permissions.rs`.
use crate::support::*;

#[test]
fn workflow_permissions_require_read_only_without_pr_approval() {
    let mut repository = facts();
    repository.actions_permissions = GithubValue::known(GithubActionsPermissionsFacts {
        default_workflow_permissions: GithubDefaultWorkflowPermissions::Write,
        can_approve_pull_request_reviews: true,
    });

    let report = run_github_checks(&repository);
    let permissions = report
        .results
        .iter()
        .find(|result| result.check == "workflow-permissions")
        .unwrap();
    assert_eq!(permissions.status, CheckStatus::Fail);
    assert!(permissions.message.contains("read-write"));
    assert!(permissions.message.contains("approve"));

    repository.actions_permissions = GithubValue::unavailable("HTTP 403");
    let report = run_github_checks(&repository);
    let permissions = report
        .results
        .iter()
        .find(|result| result.check == "workflow-permissions")
        .unwrap();
    assert_eq!(permissions.status, CheckStatus::Skip);
}
