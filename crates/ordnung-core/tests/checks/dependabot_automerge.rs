// Tests for `src/checks/dependabot_automerge.rs`.
use crate::support::*;

#[test]
fn dependabot_automerge_requires_every_safety_gate() {
    let mut repository = facts();
    repository.workflows.push(GithubWorkflowFacts {
        id: 3,
        name: "Dependabot auto-merge".into(),
        path: ".github/workflows/dependabot-automerge.yml".into(),
        state: "active".into(),
        latest_run: None,
        dependabot_automerge: DependabotAutomergeWorkflowFacts {
            pull_request_trigger: true,
            dependabot_only: true,
            fetches_metadata: true,
            excludes_major_updates: false,
            enables_auto_merge: true,
        },
    });
    let settings = GithubSettings {
        allow_auto_merge: Some(true),
        ..GithubSettings::default()
    };
    let report = run_github_checks_with_settings(&repository, &settings);
    let automerge = report
        .results
        .iter()
        .find(|result| result.check == "dependabot-automerge")
        .unwrap();
    assert_eq!(automerge.status, CheckStatus::Fail);
    assert!(automerge.message.contains("major-update exclusion"));

    repository
        .workflows
        .last_mut()
        .unwrap()
        .dependabot_automerge
        .excludes_major_updates = true;
    let report = run_github_checks_with_settings(&repository, &settings);
    assert!(report.results.iter().any(|result| {
        result.check == "dependabot-automerge" && result.status == CheckStatus::Pass
    }));
}
