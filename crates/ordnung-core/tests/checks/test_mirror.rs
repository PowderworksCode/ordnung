// Tests for `src/checks/test_mirror.rs`.
use crate::support::*;

#[test]
fn typescript_layout_accepts_configured_external_suffix() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join("checks")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"devDependencies":{"typescript":"1.0.0"}}"#,
    )
    .unwrap();
    fs::write(repo.path().join("tsconfig.json"), "{}").unwrap();
    fs::write(
        repo.path().join("src/widget.ts"),
        "export const widget = 1;\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("checks/widget.spec.ts"),
        "test('widget', () => {});\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let mut config = TestLayoutConfig::default();
    config.languages.insert(
        "typescript".into(),
        LanguageTestLayout {
            source_roots: vec!["src".into()],
            test_root: "checks".into(),
            test_suffixes: vec![".spec".into()],
        },
    );

    let report = run_repository_checks_with_config(repo.path(), &inventory, &config);
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "test-mirror" && result.status == CheckStatus::Pass })
    );
}
