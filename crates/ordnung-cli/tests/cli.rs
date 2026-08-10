use std::fs;
use std::process::{Command, Output};

fn ordnung(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ordnung"))
        .args(args)
        .output()
        .unwrap()
}

fn ordnung_with_gh(args: &[&str], gh: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ordnung"))
        .args(args)
        .env("ORDNUNG_GH", gh)
        .output()
        .unwrap()
}

fn healthy_readme() -> String {
    format!(
        "# Fixture\n\nA fixture repository for end-to-end Ordnung command testing.\n\n\
         ## Getting Started\n\nRun `scripts/dev.sh` to set up the development environment.\n\n\
         ## Usage\n\nUse the fixture through the Ordnung test harness. Pull request titles follow Conventional Commits.\n\n\
         ## Contributing\n\nChanges are made through reviewed pull requests.\n\n\
         ## License\n\nReleased under the MIT license.\n\n{}",
        "The fixture documents its purpose, setup, commands, expected behavior, maintenance process, and repository conventions. ".repeat(20)
    )
}

#[test]
fn inspect_and_check_a_rust_repository() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::create_dir_all(repo.path().join("scripts")).unwrap();
    fs::create_dir_all(repo.path().join("styles")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(repo.path().join("Cargo.lock"), "").unwrap();
    fs::write(repo.path().join("LICENSE"), "MIT License\n").unwrap();
    fs::write(repo.path().join("CHANGELOG.md"), "# Changelog\n").unwrap();
    fs::write(repo.path().join(".vale.ini"), "StylesPath = styles\n").unwrap();
    fs::write(repo.path().join("styles/.gitkeep"), "").unwrap();
    fs::write(repo.path().join(".gitignore"), "target/\n").unwrap();
    fs::write(repo.path().join("scripts/dev.sh"), "#!/bin/sh\n").unwrap();
    fs::write(repo.path().join("README.md"), healthy_readme()).unwrap();
    fs::write(repo.path().join(".github/CODEOWNERS"), "* @owner\n").unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "name: CI\non: [push, pull_request]\njobs:\n  changes:\n    outputs:\n      native: ${{ steps.filter.outputs.native }}\n    steps: []\n  quality:\n    needs: changes\n    if: needs.changes.outputs.native == 'true'\n    steps:\n      - run: cargo test && cargo clippy && cargo fmt --check\n      - uses: amannn/action-semantic-pull-request@0123456789012345678901234567890123456789\n      - uses: codespell-project/actions-codespell@0123456789012345678901234567890123456789\n      - uses: errata-ai/vale-action@0123456789012345678901234567890123456789\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".github/dependabot.yml"),
        "version: 2\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n    schedule: { interval: weekly }\n  - package-ecosystem: github-actions\n    directory: /\n    schedule: { interval: weekly }\n",
    )
    .unwrap();

    let inspect = ordnung(&["inspect", repo.path().to_str().unwrap(), "--json"]);
    assert!(inspect.status.success());
    let inventory: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(inventory["schema_version"], 1);
    assert_eq!(inventory["command"], "inspect");
    assert_eq!(inventory["data"]["projects"][0]["languages"][0], "rust");
    assert_eq!(inventory["data"]["projects"][0]["ecosystems"][0], "cargo");

    let check = ordnung(&["check", repo.path().to_str().unwrap(), "--json"]);
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn instructions_print_and_update_agent_files_idempotently() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(repo.path().join("Cargo.lock"), "").unwrap();

    let printed = ordnung(&["instructions", repo.path().to_str().unwrap()]);
    assert!(printed.status.success());
    let text = String::from_utf8(printed.stdout).unwrap();
    assert!(text.contains("Ordnung Repository Rules"));
    assert!(text.contains("languages `rust`"));

    fs::write(repo.path().join("AGENTS.md"), "# Existing guidance\n").unwrap();
    let args = [
        "instructions",
        repo.path().to_str().unwrap(),
        "--write",
        "AGENTS.md",
    ];
    assert!(ordnung(&args).status.success());
    assert!(ordnung(&args).status.success());
    let written = fs::read_to_string(repo.path().join("AGENTS.md")).unwrap();
    assert!(written.starts_with("# Existing guidance\n"));
    assert_eq!(written.matches("ordnung:instructions:start").count(), 1);
    assert_eq!(written.matches("ordnung:instructions:end").count(), 1);
}

#[cfg(unix)]
#[test]
fn instructions_reject_symlinked_destination_ancestors() {
    use std::os::unix::fs::symlink;

    let repo = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), repo.path().join("linked")).unwrap();
    let output = ordnung(&[
        "instructions",
        repo.path().to_str().unwrap(),
        "--write",
        "linked/AGENTS.md",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(!outside.path().join("AGENTS.md").exists());
}

#[test]
fn fleet_sync_plans_applies_and_clears_drift() {
    let fleet = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet.path().join("managed")).unwrap();
    fs::write(fleet.path().join("managed/config.toml"), "managed = true\n").unwrap();
    fs::write(
        fleet.path().join("fleet.toml"),
        concat!(
            "name = \"test\"\n",
            "[[member]]\nrepo = \"owner/repo\"\n",
            "[[managed]]\n",
            "name = \"config\"\n",
            "source = \"managed/config.toml\"\n",
            "destination = \"config.toml\"\n",
        ),
    )
    .unwrap();

    let fleet_toml = fleet.path().join("fleet.toml");
    let args = [
        "fleet",
        "sync",
        fleet_toml.to_str().unwrap(),
        "--repo",
        "owner/repo",
        "--repo-root",
        repo.path().to_str().unwrap(),
    ];
    let plan = ordnung(&args);
    assert_eq!(plan.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&plan.stdout).contains("create"));

    let mut apply_args = args.to_vec();
    apply_args.push("--apply");
    assert!(ordnung(&apply_args).status.success());
    assert_eq!(
        fs::read_to_string(repo.path().join("config.toml")).unwrap(),
        "managed = true\n"
    );
    assert!(ordnung(&args).status.success());
}

#[cfg(unix)]
#[test]
fn github_commands_shell_out_through_the_configured_gh_binary() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("fake-gh");
    fs::write(
        &script,
        r##"#!/bin/sh
endpoint=""
for argument in "$@"; do
  case "$argument" in repos/*) endpoint="$argument" ;; esac
done
case "$endpoint" in
  "repos/owner/repo")
    printf '%s' '{"full_name":"owner/repo","default_branch":"main","visibility":"public","archived":false,"description":"Fixture","homepage":null,"topics":[],"has_issues":true,"allow_auto_merge":false,"delete_branch_on_merge":false,"allow_update_branch":false}'
    ;;
  "repos/owner/repo/branches/main")
    printf '%s' '{"protected":false,"protection":{"required_status_checks":{"contexts":[],"checks":[]}}}'
    ;;
  "repos/owner/repo/actions/workflows?per_page=100")
    printf '%s' '{"total_count":0,"workflows":[]}'
    ;;
  "repos/owner/repo/vulnerability-alerts")
    exit 0
    ;;
  "repos/owner/repo/automated-security-fixes")
    printf '%s' '{"enabled":true}'
    ;;
  "repos/owner/repo/actions/permissions/workflow")
    printf '%s' '{"default_workflow_permissions":"read","can_approve_pull_request_reviews":false}'
    ;;
  "repos/owner/repo/rulesets?targets=branch&per_page=100")
    printf '%s' '[]'
    ;;
  "repos/owner/repo/contents/.ordnung/overrides.toml?ref=main")
    printf '%s\n' 'gh: Not Found (HTTP 404)' >&2
    exit 1
    ;;
  *)
    printf '%s\n' "unexpected endpoint: $endpoint" >&2
    exit 1
    ;;
esac
"##,
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).unwrap();

    let inspect = ordnung_with_gh(&["github", "inspect", "owner/repo", "--json"], &script);
    assert!(
        inspect.status.success(),
        "{}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let facts: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(facts["schema_version"], 1);
    assert_eq!(facts["data"]["repository"], "owner/repo");

    let check = ordnung_with_gh(&["github", "check", "owner/repo", "--json"], &script);
    assert_eq!(check.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(report["ok"], false);
    assert!(
        report["data"]["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|result| { result["check"] == "branch-protection" && result["status"] == "fail" })
    );

    let repository = tempfile::tempdir().unwrap();
    let combined = ordnung_with_gh(
        &[
            "repo-check",
            repository.path().to_str().unwrap(),
            "--repo",
            "owner/repo",
            "--json",
        ],
        &script,
    );
    assert_eq!(combined.status.code(), Some(1));
    let combined: serde_json::Value = serde_json::from_slice(&combined.stdout).unwrap();
    assert_eq!(combined["command"], "repo-check");
    assert!(combined["data"]["local"]["results"].is_array());
    assert!(combined["data"]["github"]["results"].is_array());
}

#[cfg(unix)]
#[test]
fn action_wrapper_uses_bounded_arguments_and_records_drift() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let binary = temp.path().join("fake-ordnung");
    let arguments = temp.path().join("arguments");
    let output = temp.path().join("github-output");
    fs::write(
        &binary,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FAKE_ARGUMENTS\"\nexit 1\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&binary).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&binary, permissions).unwrap();

    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/action.sh");
    let result = Command::new(script)
        .env("ORDNUNG_BIN", &binary)
        .env("FAKE_ARGUMENTS", &arguments)
        .env("GITHUB_OUTPUT", &output)
        .env("ORDNUNG_ACTION_MODE", "fleet-sync-all")
        .env("ORDNUNG_ACTION_FLEET", "fleet.toml")
        .env("ORDNUNG_ACTION_APPLY", "true")
        .env("ORDNUNG_ACTION_FORMAT", "json")
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(arguments).unwrap(),
        "fleet\ngithub-sync-all\nfleet.toml\n--apply\n--json\n"
    );
    assert_eq!(
        fs::read_to_string(output).unwrap(),
        "outcome=drift\nexit-code=1\n"
    );
}

#[cfg(unix)]
#[test]
fn action_wrapper_defaults_to_combined_repository_check() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let binary = temp.path().join("fake-ordnung");
    let arguments = temp.path().join("arguments");
    fs::write(
        &binary,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FAKE_ARGUMENTS\"\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&binary).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&binary, permissions).unwrap();

    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/action.sh");
    let result = Command::new(script)
        .env("ORDNUNG_BIN", &binary)
        .env("FAKE_ARGUMENTS", &arguments)
        .env("ORDNUNG_ACTION_PATH", "checkout")
        .env("ORDNUNG_ACTION_REPOSITORY", "owner/repo")
        .output()
        .unwrap();

    assert!(result.status.success());
    assert_eq!(
        fs::read_to_string(arguments).unwrap(),
        "repo-check\ncheckout\n--repo\nowner/repo\n"
    );
}

#[cfg(unix)]
#[test]
fn action_wrapper_records_validation_errors() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("github-output");
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/action.sh");
    let result = Command::new(script)
        .env("GITHUB_OUTPUT", &output)
        .env("ORDNUNG_ACTION_APPLY", "sometimes")
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        fs::read_to_string(output).unwrap(),
        "outcome=error\nexit-code=2\n"
    );
}

/// A check the effective policy has switched off still runs, but reporting it as
/// `fail` beside a real failure is what made a first run read as dozens of
/// problems. It is hidden unless `--all` asks for it, and it never moves the
/// exit code either way.
#[test]
fn disabled_checks_are_hidden_unless_all_is_requested() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(repo.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(repo.path().join("README.md"), "# fixture\n").unwrap();

    let path = repo.path().to_str().unwrap();
    let default = ordnung(&["check", path]);
    let all = ordnung(&["check", path, "--all"]);

    let default_out = String::from_utf8(default.stdout).unwrap();
    let all_out = String::from_utf8(all.stdout).unwrap();

    assert!(
        !default_out.lines().any(|line| line.contains(" off ")),
        "no disabled check appears by default:\n{default_out}"
    );
    assert!(
        all_out.lines().any(|line| line.contains(" off ")),
        "--all shows disabled checks:\n{all_out}"
    );
    assert!(
        all_out.lines().count() > default_out.lines().count(),
        "--all is a superset"
    );
    assert_eq!(
        default.status.code(),
        all.status.code(),
        "hiding disabled checks does not change the exit code"
    );

    // Every line the default prints is one --all also prints: hiding, not rewording.
    for line in default_out.lines() {
        assert!(all_out.contains(line), "--all is missing {line:?}");
    }
}

/// The JSON envelope reports the same set the human output does, so a consumer
/// filtering on `severity` is not silently handed a different population.
#[test]
fn json_output_hides_disabled_checks_too() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(repo.path().join("src/main.rs"), "fn main() {}\n").unwrap();

    let path = repo.path().to_str().unwrap();
    let default = ordnung(&["check", path, "--json"]);
    let out = String::from_utf8(default.stdout).unwrap();

    assert!(
        !out.contains("\"severity\": \"off\""),
        "no disabled check in the default JSON report:\n{out}"
    );

    let all = ordnung(&["check", path, "--json", "--all"]);
    let all_out = String::from_utf8(all.stdout).unwrap();
    assert!(
        all_out.contains("\"severity\": \"off\""),
        "--all restores them in JSON too"
    );
}
