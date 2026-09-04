// Tests for `src/checks/ci_green.rs`.
use crate::support::*;

#[test]
fn dynamic_workflows_do_not_affect_ci_health() {
    let mut facts = facts();
    facts.workflows.push(GithubWorkflowFacts {
        id: 3,
        name: "Dependabot".into(),
        path: "dynamic/dependabot/dependabot-updates".into(),
        state: "active".into(),
        latest_run: Some(GithubWorkflowRun {
            id: 12,
            conclusion: Some("failure".into()),
            html_url: "https://example.test/run/12".into(),
        }),
        dependabot_automerge: Default::default(),
    });
    let report = run_github_checks(&facts);
    let ci = report
        .results
        .iter()
        .find(|result| result.check == "ci-green")
        .unwrap();
    assert_eq!(ci.status, CheckStatus::Pass);
}

#[test]
fn quiet_workflows_are_not_treated_as_red() {
    let mut facts = facts();
    facts.workflows.push(GithubWorkflowFacts {
        id: 3,
        name: "PR only".into(),
        path: ".github/workflows/pr.yml".into(),
        state: "active".into(),
        latest_run: None,
        dependabot_automerge: Default::default(),
    });

    let report = run_github_checks(&facts);
    let ci = report
        .results
        .iter()
        .find(|result| result.check == "ci-green")
        .unwrap();
    assert_eq!(ci.status, CheckStatus::Pass);
    assert!(
        ci.message
            .contains("no completed main runs yet for: PR only")
    );
}

#[test]
fn ci_green_excludes_self_audit_family_workflows() {
    let mut facts = facts();
    facts.workflows = vec![GithubWorkflowFacts {
        id: 3,
        name: "housekeeping".into(),
        path: ".github/workflows/housekeeping.yml".into(),
        state: "active".into(),
        latest_run: Some(GithubWorkflowRun {
            id: 13,
            conclusion: Some("failure".into()),
            html_url: "https://example.test/run/13".into(),
        }),
        dependabot_automerge: Default::default(),
    }];

    let report = run_github_checks(&facts);
    let ci = report
        .results
        .iter()
        .find(|result| result.check == "ci-green")
        .unwrap();
    assert_eq!(ci.status, CheckStatus::Skip);
    assert!(ci.message.contains("housekeeping"));
}

#[test]
fn ci_green_skips_when_every_workflow_is_quiet() {
    let mut facts = facts();
    facts
        .workflows
        .iter_mut()
        .for_each(|workflow| workflow.latest_run = None);

    let report = run_github_checks(&facts);
    let ci = report
        .results
        .iter()
        .find(|result| result.check == "ci-green")
        .unwrap();
    assert_eq!(ci.status, CheckStatus::Skip);
}
