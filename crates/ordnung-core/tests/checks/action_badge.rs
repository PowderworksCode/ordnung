// Tests for `src/checks/action_badge.rs`.
use crate::support::*;

#[test]
fn public_actions_link_their_exact_marketplace_listing() {
    let mut repository = facts();
    repository.action_publication = GithubValue::known(Some(GithubActionPublicationFacts {
        manifest_path: "action.yml".into(),
        name: "Setup Powderworks".into(),
        marketplace_slug: "setup-powderworks".into(),
        marketplace_url: "https://github.com/marketplace/actions/setup-powderworks".into(),
        readme_path: Some("README.md".into()),
        marketplace_linked: false,
    }));
    let report = run_github_checks(&repository);
    let badge = report
        .results
        .iter()
        .find(|result| result.check == "action-badge")
        .unwrap();
    assert_eq!(badge.status, CheckStatus::Fail);
    assert!(badge.message.contains("setup-powderworks"));

    let GithubValue::Known {
        value: Some(action),
    } = &mut repository.action_publication
    else {
        unreachable!();
    };
    action.marketplace_linked = true;
    let report = run_github_checks(&repository);
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "action-badge" && result.status == CheckStatus::Pass })
    );
}
