// Tests for `src/checks/test_inline.rs`.
use crate::support::*;

#[test]
fn rust_test_layout_rejects_inline_tests_and_requires_a_mirror() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("src/lib.rs"),
        "pub fn value() -> bool { true }\n#[cfg(test)]\nmod tests {}\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();

    let report =
        run_repository_checks_with_config(repo.path(), &inventory, &TestLayoutConfig::default());
    let failures: Vec<_> = report
        .results
        .iter()
        .filter(|result| {
            matches!(result.check.as_str(), "test-inline" | "test-mirror")
                && result.status == CheckStatus::Fail
        })
        .collect();
    assert_eq!(failures.len(), 2);
    assert!(
        failures
            .iter()
            .all(|result| result.severity == Severity::Off)
    );
    // Each position is now reported by its own check, so a fleet can require one
    // without the other.
    let inline = failures
        .iter()
        .find(|result| result.check == "test-inline")
        .expect("test-inline reports the inline module");
    assert!(inline.message.contains("inline"), "{}", inline.message);
    let mirror = failures
        .iter()
        .find(|result| result.check == "test-mirror")
        .expect("test-mirror reports the missing mirror");
    assert!(mirror.message.contains("mirrored"), "{}", mirror.message);

    fs::write(
        repo.path().join("src/lib.rs"),
        "pub fn value() -> bool { true }\npub const MARKER: &str = \"#[cfg(test)]\";\n",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join("tests")).unwrap();
    fs::write(repo.path().join("tests/lib.rs"), "#[test]\nfn value() {}\n").unwrap();
    let clean =
        run_repository_checks_with_config(repo.path(), &inventory, &TestLayoutConfig::default());
    for check in ["test-inline", "test-mirror"] {
        assert!(
            clean
                .results
                .iter()
                .any(|result| result.check == check && result.status == CheckStatus::Pass),
            "{check} should pass"
        );
    }
}
