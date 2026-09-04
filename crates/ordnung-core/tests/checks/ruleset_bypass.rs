// Tests for `src/checks/ruleset_bypass.rs`.
use crate::support::*;

#[test]
fn gating_rulesets_require_an_explicit_bypass_actor() {
    let mut facts = facts();
    facts.rulesets = GithubValue::known(vec![GithubRulesetFacts {
        id: 42,
        name: "main".into(),
        target: "branch".into(),
        enforcement: "active".into(),
        rule_types: ["pull_request".into()].into(),
        bypass_actors: Vec::new(),
    }]);
    let report = run_github_checks(&facts);
    let check = report
        .results
        .iter()
        .find(|result| result.check == "ruleset-bypass")
        .unwrap();
    assert_eq!(check.status, CheckStatus::Fail);
    assert!(check.message.contains("main"));

    let GithubValue::Known { value } = &mut facts.rulesets else {
        unreachable!();
    };
    value[0].bypass_actors.push(GithubRulesetBypassActor {
        actor_id: Some(5),
        actor_type: "RepositoryRole".into(),
        bypass_mode: "always".into(),
    });
    let report = run_github_checks(&facts);
    assert!(
        report.results.iter().any(|result| {
            result.check == "ruleset-bypass" && result.status == CheckStatus::Pass
        })
    );
}
