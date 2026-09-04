// Tests for `src/checks/allow_auto_merge.rs`.
use crate::support::*;

#[test]
fn allow_auto_merge_uses_the_effective_setting_policy() {
    let facts = facts();
    let disabled = GithubSettings {
        allow_auto_merge: Some(false),
        ..GithubSettings::default()
    };
    let report = run_github_checks_with_settings(&facts, &disabled);
    let check = report
        .results
        .iter()
        .find(|result| result.check == "allow-auto-merge")
        .unwrap();
    assert_eq!(check.status, CheckStatus::Fail);

    let enabled = GithubSettings {
        allow_auto_merge: Some(true),
        ..GithubSettings::default()
    };
    let report = run_github_checks_with_settings(&facts, &enabled);
    assert!(report.results.iter().any(|result| {
        result.check == "allow-auto-merge" && result.status == CheckStatus::Pass
    }));
}
