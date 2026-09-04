// Fixtures for the GitHub-backed checks. `facts()` in support.rs describes a
// healthy repository; these tests each spoil one thing about it and read the
// check that notices.
#![allow(dead_code)]

use crate::support::*;

pub fn github_status(facts: &GithubRepositoryFacts, check: &str) -> CheckStatus {
    run_github_checks(facts)
        .results
        .iter()
        .find(|result| result.check == check)
        .unwrap_or_else(|| panic!("{check} reports a result"))
        .status
}

pub fn github_message(facts: &GithubRepositoryFacts, check: &str) -> String {
    run_github_checks(facts)
        .results
        .iter()
        .find(|result| result.check == check)
        .unwrap_or_else(|| panic!("{check} reports a result"))
        .message
        .clone()
}
