// Tests for `src/checks/git_hooks.rs`.
use crate::support::*;

#[test]
fn git_hooks_requires_committed_hooks_or_a_manager() {
    let repo = tempfile::tempdir().unwrap();
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result.message.contains("no committed hooks"),
        "{}",
        result.message
    );
}

#[cfg(unix)]
#[test]
fn git_hooks_accepts_executable_hooks_installed_by_the_development_script() {
    let repo = tempfile::tempdir().unwrap();
    write_hook(repo.path(), "pre-commit", true);
    write_hook(repo.path(), "commit-msg", true);
    write_dev_script(
        repo.path(),
        "#!/usr/bin/env bash\ngit config core.hooksPath .githooks\n",
    );
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.message);
    assert!(result.message.contains('2'), "{}", result.message);
}

/// Git ignores a hook without the execute bit, so the gate looks present and never
/// runs. That is the worst way for this to fail, which is why it is graded.
#[cfg(unix)]
#[test]
fn git_hooks_rejects_a_hook_that_git_would_silently_ignore() {
    let repo = tempfile::tempdir().unwrap();
    write_hook(repo.path(), "pre-commit", false);
    write_dev_script(repo.path(), "git config core.hooksPath .githooks\n");
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result.message.contains("not executable"),
        "{}",
        result.message
    );
    assert!(result.message.contains("pre-commit"), "{}", result.message);
}

#[cfg(unix)]
#[test]
fn git_hooks_rejects_committed_hooks_that_nothing_installs() {
    let repo = tempfile::tempdir().unwrap();
    write_hook(repo.path(), "pre-commit", true);
    write_dev_script(repo.path(), "#!/usr/bin/env bash\ncargo build\n");
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result.message.contains("core.hooksPath"),
        "{}",
        result.message
    );
}

/// A manager installs through its own lifecycle, so requiring the development
/// script to repeat that would be wrong.
#[test]
fn git_hooks_accepts_a_declared_manager_without_a_development_script() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"name":"fixture","devDependencies":{"lefthook":"1.7.0"}}"#,
    )
    .unwrap();
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.message);
    assert!(result.message.contains("lefthook"), "{}", result.message);
}

/// A README beside the hooks is documentation, not something Git runs.
#[cfg(unix)]
#[test]
fn git_hooks_ignores_files_that_are_not_hook_names() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".githooks")).unwrap();
    fs::write(repo.path().join(".githooks/README.md"), "# hooks\n").unwrap();
    fs::write(
        repo.path().join(".githooks/run-straitjacket"),
        "#!/bin/sh\n",
    )
    .unwrap();
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result.message.contains("no committed hooks"),
        "{}",
        result.message
    );
}
