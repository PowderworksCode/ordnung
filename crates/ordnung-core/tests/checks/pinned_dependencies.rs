// Tests for `src/checks/pinned_dependencies.rs`.
use crate::support::*;
use crate::support_lint::*;

/// Cargo resolves through a lockfile, so a range in Cargo.toml is not the thing
/// that decides what gets built. This check is about the ecosystems where the
/// manifest is the decision.
#[test]
fn a_cargo_range_is_not_a_finding() {
    let repo = repo_with(&[
        (
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n\n[dependencies]\nserde = \"1\"\n",
        ),
        ("src/lib.rs", "pub fn fixture() {}\n"),
    ]);
    assert_ne!(
        status(repo.path(), "pinned-dependencies"),
        CheckStatus::Fail
    );
}

#[test]
fn fails_on_a_floating_npm_range() {
    let repo = repo_with(&[(
        "package.json",
        "{\n  \"name\": \"fixture\",\n  \"dependencies\": { \"left-pad\": \"^1.3.0\" }\n}\n",
    )]);
    assert_eq!(
        status(repo.path(), "pinned-dependencies"),
        CheckStatus::Fail
    );
}

#[test]
fn passes_on_an_exact_npm_version() {
    let repo = repo_with(&[(
        "package.json",
        "{\n  \"name\": \"fixture\",\n  \"dependencies\": { \"left-pad\": \"1.3.0\" }\n}\n",
    )]);
    assert_eq!(
        status(repo.path(), "pinned-dependencies"),
        CheckStatus::Pass
    );
}

/// A dependency on a path in the same repository has no version to pin, so a
/// manifest holding only those leaves the check nothing to assess — it skips
/// rather than passing, which is the honest answer to "are your versions exact"
/// when none of them are versions.
#[test]
fn a_local_dependency_is_exempt() {
    let repo = repo_with(&[(
        "package.json",
        "{\n  \"name\": \"fixture\",\n  \"dependencies\": { \"sibling\": \"file:../sibling\" }\n}\n",
    )]);
    assert_eq!(
        status(repo.path(), "pinned-dependencies"),
        CheckStatus::Skip
    );
}
