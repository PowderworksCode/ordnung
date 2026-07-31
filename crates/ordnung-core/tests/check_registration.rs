use std::collections::BTreeSet;

use ordnung_core::{Severity, check_definition, check_definitions, check_ids, default_policy};

#[test]
fn registered_checks_are_complete_unique_and_sorted() {
    let definitions = check_definitions();
    assert_eq!(definitions.len(), 42);
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
    assert_eq!(policy["test-layout"], Severity::Off);
}
