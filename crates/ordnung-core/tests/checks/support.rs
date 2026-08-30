// Fixtures shared by the mirrored check tests. A check's file imports this
// one and nothing else, so it reads as the check it belongs to.
//
// No single file needs every fixture, and one target compiles them all.
#![allow(dead_code)]

pub use std::fs;

pub use ordnung_core::fleet::ManagedState;
pub use ordnung_core::{
    CheckStatus, CiExistsConfig, DependabotAutomergeWorkflowFacts, DependencyRequirement,
    GithubActionPublicationFacts, GithubActionsPermissionsFacts, GithubBranchFacts,
    GithubBranchProtectionFacts, GithubDefaultWorkflowPermissions, GithubLicenseFacts,
    GithubPullRequestAgeFacts, GithubRepositoryFacts, GithubRulesetBypassActor, GithubRulesetFacts,
    GithubSecurityFacts, GithubSettings, GithubStaleFacts, GithubValue, GithubWorkflowFacts,
    GithubWorkflowRun, InventoryOptions, LanguageTestLayout, RepoConfig, Severity,
    TestLayoutConfig, default_policy, inspect_repository, run_github_checks,
    run_github_checks_with_settings, run_repository_checks_with_config,
    run_repository_checks_with_repo_config, run_repository_checks_with_requirements,
};

pub fn complete_readme(link: &str) -> String {
    format!(
        "# Demo\n\nA repository that demonstrates the README quality floor.\n\n\
         ## Getting Started\n\nRun the development setup command.\n\n\
         ## Usage\n\nUse the command and see [the guide]({link}).\n\n\
         ### Contributions\n\nChanges are welcome through pull requests.\n\n\
         ## Licensing\n\nReleased under the MIT license.\n\n{}",
        "Additional documentation explains the project purpose, behavior, maintenance, and supported workflows clearly. ".repeat(20)
    )
}

pub fn requirement(name: &str, language: &str, require: &[&str]) -> DependencyRequirement {
    DependencyRequirement {
        name: name.into(),
        language: Some(language.into()),
        ecosystem: None,
        require: require
            .iter()
            .map(|package| (*package).to_owned())
            .collect(),
        kind: None,
        state: ManagedState::Present,
    }
}

pub fn dependency_result(
    repo: &std::path::Path,
    manifest: &str,
    requirements: &[DependencyRequirement],
) -> ordnung_core::CheckResult {
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("Cargo.toml"), manifest).unwrap();
    fs::write(repo.join("src/lib.rs"), "pub fn value() {}\n").unwrap();
    let inventory = inspect_repository(repo, &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_requirements(
        repo,
        &inventory,
        &RepoConfig::default(),
        requirements,
    );
    report
        .results
        .into_iter()
        .find(|result| result.check == "required-dependencies")
        .expect("required-dependencies runs")
}

pub fn hooks_result(repo: &std::path::Path) -> ordnung_core::CheckResult {
    let inventory = inspect_repository(repo, &InventoryOptions::default()).unwrap();
    run_repository_checks_with_repo_config(repo, &inventory, &RepoConfig::default())
        .results
        .into_iter()
        .find(|result| result.check == "git-hooks")
        .expect("git-hooks runs")
}

#[cfg(unix)]
pub fn write_hook(repo: &std::path::Path, name: &str, executable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo.join(".githooks");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
}

pub fn write_dev_script(repo: &std::path::Path, body: &str) {
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/dev.sh"), body).unwrap();
}

pub use std::io::{Read, Write};
pub use std::net::TcpListener;
pub use std::sync::LazyLock;

pub static WEBSITE_SERVER: LazyLock<String> = LazyLock::new(|| {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut request = [0; 2048];
            let read = stream.read(&mut request).unwrap();
            let path = String::from_utf8_lossy(&request[..read])
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_owned();
            let (status, body) = if path == "/missing" {
                ("404 Not Found", "missing")
            } else {
                ("200 OK", "ok")
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    format!("http://{address}")
});

pub fn facts() -> GithubRepositoryFacts {
    GithubRepositoryFacts {
        repository: "owner/repo".into(),
        default_branch: "main".into(),
        visibility: "public".into(),
        archived: false,
        description: Some("A repository".into()),
        homepage: Some(format!("{}/ok", WEBSITE_SERVER.as_str())),
        license: Some(GithubLicenseFacts {
            key: "mit".into(),
            name: "MIT License".into(),
            spdx_id: "MIT".into(),
        }),
        topics: vec!["rust".into()],
        has_issues: true,
        allow_auto_merge: true,
        delete_branch_on_merge: true,
        allow_update_branch: true,
        branch: GithubBranchFacts {
            protected: true,
            protection: GithubValue::known(GithubBranchProtectionFacts {
                pull_requests_required: true,
                force_pushes_blocked: true,
                deletion_blocked: true,
            }),
            required_checks: GithubValue::known(vec!["CI".into()]),
            strict_status_checks: GithubValue::known(true),
        },
        security: GithubValue::known(GithubSecurityFacts {
            secret_scanning: true,
            push_protection: true,
        }),
        vulnerability_alerts: GithubValue::known(true),
        automated_security_fixes: GithubValue::known(true),
        actions_permissions: GithubValue::known(GithubActionsPermissionsFacts {
            default_workflow_permissions: GithubDefaultWorkflowPermissions::Read,
            can_approve_pull_request_reviews: false,
        }),
        rulesets: GithubValue::known(Vec::new()),
        pull_request_checks: GithubValue::known(vec!["CI".into()]),
        workflows: vec![
            GithubWorkflowFacts {
                id: 1,
                name: "CI".into(),
                path: ".github/workflows/ci.yml".into(),
                state: "active".into(),
                latest_run: Some(GithubWorkflowRun {
                    id: 10,
                    conclusion: Some("success".into()),
                    html_url: "https://example.test/run/10".into(),
                }),
                dependabot_automerge: Default::default(),
            },
            GithubWorkflowFacts {
                id: 2,
                name: "Update branches".into(),
                path: ".github/workflows/auto-update-pr-branches.yml".into(),
                state: "active".into(),
                latest_run: Some(GithubWorkflowRun {
                    id: 11,
                    conclusion: Some("success".into()),
                    html_url: "https://example.test/run/11".into(),
                }),
                dependabot_automerge: Default::default(),
            },
        ],
        action_publication: GithubValue::known(None),
        stale: GithubValue::known(GithubStaleFacts::default()),
    }
}
