use std::collections::BTreeSet;

use ordnung_core::{Severity, check_definition, check_definitions, check_ids, default_policy};

#[test]
fn registered_checks_are_complete_unique_and_sorted() {
    let definitions = check_definitions();
    assert_eq!(definitions.len(), 50);
    assert!(definitions.windows(2).all(|pair| pair[0].id < pair[1].id));
    assert!(
        definitions
            .iter()
            .all(|definition| !definition.instructions.trim().is_empty())
    );

    let ids = check_ids();
    assert_eq!(ids.len(), ids.iter().collect::<BTreeSet<_>>().len());
    for definition in definitions {
        assert!(std::ptr::eq(
            check_definition(definition.id).unwrap(),
            *definition
        ));
    }
}

#[test]
fn default_policy_comes_from_registered_definitions() {
    let policy = default_policy();
    assert_eq!(policy.len(), check_definitions().len());
    for definition in check_definitions() {
        assert_eq!(policy[definition.id], definition.default_severity);
    }
    assert_eq!(policy["test-inline"], Severity::Off);
    assert_eq!(policy["test-mirror"], Severity::Off);
}

/// Policy that selects directories can only apply to project-scoped checks, so the
/// scope must be declared rather than inferred.
#[test]
fn every_check_declares_a_scope_and_github_checks_are_repository_scoped() {
    use ordnung_core::CheckScope;
    for definition in check_definitions() {
        if definition.github_runner.is_some() {
            assert_eq!(
                definition.scope,
                CheckScope::Repository,
                "{} reads GitHub facts, which describe one repository",
                definition.id
            );
        }
    }
    let project = check_definitions()
        .iter()
        .filter(|definition| definition.scope == CheckScope::Project)
        .count();
    assert_eq!(project, 12, "project-scoped checks");
}
