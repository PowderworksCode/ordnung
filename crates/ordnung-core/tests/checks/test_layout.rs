// Tests for `src/checks/test_layout.rs`.
use crate::support::*;

#[test]
fn test_layout_checks_are_optional_by_default() {
    assert_eq!(default_policy()["test-inline"], Severity::Off);
    assert_eq!(default_policy()["test-mirror"], Severity::Off);
}
