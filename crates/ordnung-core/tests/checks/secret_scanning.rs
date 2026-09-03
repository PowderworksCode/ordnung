// Tests for `src/checks/secret_scanning.rs`.
use crate::support::*;
use crate::support_github::*;

#[test]
fn passes_when_both_are_enabled() {
    assert_eq!(
        github_status(&facts(), "secret-scanning"),
        CheckStatus::Pass
    );
}

/// The two settings are reported separately, because turning one on is a
/// different piece of work from turning the other on.
#[test]
fn names_whichever_is_missing() {
    let mut without_scanning = facts();
    without_scanning.security = GithubValue::known(GithubSecurityFacts {
        secret_scanning: false,
        push_protection: true,
    });
    assert_eq!(
        github_status(&without_scanning, "secret-scanning"),
        CheckStatus::Fail
    );
    let message = github_message(&without_scanning, "secret-scanning");
    assert!(message.contains("secret scanning"), "{message}");
    assert!(!message.contains("push protection"), "{message}");

    let mut without_protection = facts();
    without_protection.security = GithubValue::known(GithubSecurityFacts {
        secret_scanning: true,
        push_protection: false,
    });
    let message = github_message(&without_protection, "secret-scanning");
    assert!(message.contains("push protection"), "{message}");
}
