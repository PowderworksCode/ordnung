use std::fs;
use std::path::Path;

use ordnung_core::check::Severity;
use ordnung_core::fleet::{ManagedEntry, ManagedState, ProjectSelector, RelativeTo, Stage};
use ordnung_core::{
    ChangeKind, FleetConfig, InventoryOptions, LanguageId, ResolvedManaged, apply_changes,
    inspect_repository, plan_managed_changes, plan_managed_changes_for_member,
};

fn entry(source: &str, destination: &str) -> ManagedEntry {
    ManagedEntry {
        name: "test".into(),
        source: Some(source.into()),
        destination: destination.into(),
        state: ManagedState::Present,
        relative_to: RelativeTo::Repo,
        when: None,
        only: Vec::new(),
        substitute: false,
    }
}

fn resolved(root: &Path, entries: Vec<ManagedEntry>) -> Vec<ResolvedManaged> {
    entries
        .into_iter()
        .map(|entry| ResolvedManaged {
            root: root.to_path_buf(),
            entry,
        })
        .collect()
}

#[test]
fn directory_mirror_updates_and_deletes() {
    let fleet = tempfile::tempdir().unwrap();
    let member = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet.path().join("managed/styles")).unwrap();
    fs::write(fleet.path().join("managed/styles/Terms.yml"), "new\n").unwrap();
    fs::create_dir_all(member.path().join(".vale/styles")).unwrap();
    fs::write(member.path().join(".vale/styles/Terms.yml"), "old\n").unwrap();
    fs::write(member.path().join(".vale/styles/OldRule.yml"), "retired\n").unwrap();
    let inventory = inspect_repository(member.path(), &InventoryOptions::default()).unwrap();

    let changes = plan_managed_changes(
        "owner/member",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![entry("managed/styles", ".vale/styles")]),
    )
    .unwrap();
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().any(|change| {
        change.path == Path::new(".vale/styles/Terms.yml") && change.kind == ChangeKind::Update
    }));
    assert!(changes.iter().any(|change| {
        change.path == Path::new(".vale/styles/OldRule.yml") && change.kind == ChangeKind::Delete
    }));

    apply_changes(member.path(), &changes).unwrap();
    assert_eq!(
        fs::read_to_string(member.path().join(".vale/styles/Terms.yml")).unwrap(),
        "new\n"
    );
    assert!(!member.path().join(".vale/styles/OldRule.yml").exists());
    let clean = plan_managed_changes(
        "owner/member",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![entry("managed/styles", ".vale/styles")]),
    )
    .unwrap();
    assert!(clean.is_empty());
}

#[test]
fn project_relative_file_applies_to_detected_typescript_projects() {
    let fleet = tempfile::tempdir().unwrap();
    let member = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet.path().join("managed")).unwrap();
    fs::write(fleet.path().join("managed/biome.json"), "{}\n").unwrap();
    fs::create_dir_all(member.path().join("site")).unwrap();
    fs::write(member.path().join("site/package.json"), "{}").unwrap();
    fs::write(member.path().join("site/tsconfig.json"), "{}").unwrap();
    let inventory = inspect_repository(member.path(), &InventoryOptions::default()).unwrap();
    let mut managed = entry("managed/biome.json", "biome.base.json");
    managed.relative_to = RelativeTo::Project;
    managed.when = Some(ProjectSelector {
        language: Some(LanguageId::from("typescript")),
        capability: None,
        ecosystem: None,
    });

    let changes = plan_managed_changes(
        "owner/member",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![managed]),
    )
    .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, Path::new("site/biome.base.json"));
    assert_eq!(changes[0].kind, ChangeKind::Create);
}

#[test]
fn tombstone_is_an_explicit_delete() {
    let fleet = tempfile::tempdir().unwrap();
    let member = tempfile::tempdir().unwrap();
    fs::write(member.path().join(".old-tool.toml"), "old\n").unwrap();
    let inventory = inspect_repository(member.path(), &InventoryOptions::default()).unwrap();
    let managed = ManagedEntry {
        name: "retire-old-config".into(),
        source: None,
        destination: ".old-tool.toml".into(),
        state: ManagedState::Absent,
        relative_to: RelativeTo::Repo,
        when: None,
        only: Vec::new(),
        substitute: false,
    };

    let changes = plan_managed_changes(
        "owner/member",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![managed]),
    )
    .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ChangeKind::Delete);
}

#[test]
fn file_and_directory_can_replace_each_other() {
    let fleet = tempfile::tempdir().unwrap();
    let member = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet.path().join("managed/tree")).unwrap();
    fs::write(fleet.path().join("managed/tree/file.txt"), "managed\n").unwrap();
    fs::write(member.path().join("owned"), "was a file\n").unwrap();
    let inventory = inspect_repository(member.path(), &InventoryOptions::default()).unwrap();

    let directory_changes = plan_managed_changes(
        "owner/member",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![entry("managed/tree", "owned")]),
    )
    .unwrap();
    apply_changes(member.path(), &directory_changes).unwrap();
    assert_eq!(
        fs::read_to_string(member.path().join("owned/file.txt")).unwrap(),
        "managed\n"
    );

    fs::write(fleet.path().join("managed/single.txt"), "single\n").unwrap();
    let file_changes = plan_managed_changes(
        "owner/member",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![entry("managed/single.txt", "owned")]),
    )
    .unwrap();
    apply_changes(member.path(), &file_changes).unwrap();
    assert_eq!(
        fs::read_to_string(member.path().join("owned")).unwrap(),
        "single\n"
    );
}

/// Writes a `.ordnung` directory and returns its path.
fn config_dir(root: &Path, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = root.join(".ordnung");
    for (name, contents) in files {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    dir
}

const MEMBER: &str = "name = \"test\"\n\n[[member]]\nrepo = \"owner/repo\"\n";

#[test]
fn overlapping_managed_ownership_is_rejected() {
    let fleet = tempfile::tempdir().unwrap();
    let dir = config_dir(
        fleet.path(),
        &[
            ("managed/tree/file.txt", "managed\n"),
            ("managed/file.txt", "managed\n"),
            (
                "fleet.toml",
                &format!(
                    "{MEMBER}\n\
                     [[managed]]\nname = \"tree\"\nsource = \"managed/tree\"\ndestination = \".config\"\n\n\
                     [[managed]]\nname = \"file\"\nsource = \"managed/file.txt\"\ndestination = \".config/file.txt\"\n"
                ),
            ),
        ],
    );

    let error = FleetConfig::load(&dir.join("fleet.toml"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("already owned by entry"), "{error}");
}

#[test]
fn downstream_overrides_inherited_check_severity_and_managed_content() {
    let upstream = tempfile::tempdir().unwrap();
    let up = config_dir(
        upstream.path(),
        &[
            ("managed/rustfmt.toml", "upstream\n"),
            (
                "policy.toml",
                "name = \"base\"\n\n\
                 [policy.checks]\nvale = { severity = \"required\", allow_override = false }\n\
                 codespell = { severity = \"required\", allow_override = false }\n\n\
                 [[managed]]\nname = \"rustfmt\"\nsource = \"managed/rustfmt.toml\"\ndestination = \"rustfmt.toml\"\n",
            ),
        ],
    );

    let fleet = tempfile::tempdir().unwrap();
    let down = config_dir(
        fleet.path(),
        &[
            ("managed/rustfmt.toml", "downstream\n"),
            (
                "fleet.toml",
                &format!(
                    "{MEMBER}\n\
                     [[extends]]\npath = {:?}\n\n\
                     [policy.checks]\nvale = {{ severity = \"off\", allow_override = true }}\n\n\
                     [[managed]]\nname = \"rustfmt\"\nsource = \"managed/rustfmt.toml\"\ndestination = \"rustfmt.toml\"\n",
                    up.to_str().unwrap()
                ),
            ),
        ],
    );

    let config = FleetConfig::load(&down.join("fleet.toml")).unwrap();

    // Downstream wins on the check it redefines, and inherits the one it does not.
    assert_eq!(config.policy.checks["vale"].severity, Severity::Off);
    assert_eq!(
        config.policy.checks["codespell"].severity,
        Severity::Required
    );

    // Same name replaces the upstream entry rather than colliding with it.
    let managed = config.effective_managed();
    assert_eq!(managed.len(), 1);
    assert_eq!(managed[0].root, down);
}

#[test]
fn unmanaged_drops_an_inherited_entry_without_deleting_files() {
    let upstream = tempfile::tempdir().unwrap();
    let up = config_dir(
        upstream.path(),
        &[
            ("managed/editorconfig", "root = true\n"),
            (
                "policy.toml",
                "name = \"base\"\n\n\
                 [[managed]]\nname = \"editorconfig\"\nsource = \"managed/editorconfig\"\ndestination = \".editorconfig\"\n",
            ),
        ],
    );

    let fleet = tempfile::tempdir().unwrap();
    let down = config_dir(
        fleet.path(),
        &[(
            "fleet.toml",
            &format!(
                "{MEMBER}\n[[extends]]\npath = {:?}\n\n\
                 [[managed]]\nname = \"editorconfig\"\nstate = \"unmanaged\"\ndestination = \".editorconfig\"\n",
                up.to_str().unwrap()
            ),
        )],
    );

    let config = FleetConfig::load(&down.join("fleet.toml")).unwrap();
    assert!(config.effective_managed().is_empty());

    // A member that already has the file is left alone, unlike a tombstone.
    let member = tempfile::tempdir().unwrap();
    fs::write(member.path().join(".editorconfig"), "local\n").unwrap();
    let inventory = inspect_repository(member.path(), &InventoryOptions::default()).unwrap();
    let changes = plan_managed_changes(
        "owner/repo",
        member.path(),
        &inventory,
        config.effective_managed(),
    )
    .unwrap();
    assert!(changes.is_empty());
    assert_eq!(
        fs::read_to_string(member.path().join(".editorconfig")).unwrap(),
        "local\n"
    );
}

#[test]
fn unmanaged_without_an_inherited_entry_is_rejected() {
    let fleet = tempfile::tempdir().unwrap();
    let dir = config_dir(
        fleet.path(),
        &[(
            "fleet.toml",
            &format!(
                "{MEMBER}\n[[managed]]\nname = \"typo\"\nstate = \"unmanaged\"\ndestination = \".x\"\n"
            ),
        )],
    );

    let error = FleetConfig::load(&dir.join("fleet.toml"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("does not match any inherited"), "{error}");
}

#[test]
fn extends_cannot_reference_a_fleet_instance() {
    let upstream = tempfile::tempdir().unwrap();
    let up = config_dir(upstream.path(), &[("fleet.toml", MEMBER)]);

    let fleet = tempfile::tempdir().unwrap();
    let down = config_dir(
        fleet.path(),
        &[(
            "fleet.toml",
            &format!("{MEMBER}\n[[extends]]\npath = {:?}\n", up.to_str().unwrap()),
        )],
    );

    let error = FleetConfig::load(&down.join("fleet.toml"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("members are never inherited"), "{error}");
}

#[test]
fn git_extends_requires_a_full_pinned_revision() {
    let fleet = tempfile::tempdir().unwrap();
    let dir = config_dir(
        fleet.path(),
        &[(
            "fleet.toml",
            &format!(
                "{MEMBER}\n[[extends]]\ngit = \"https://example.invalid/conf\"\nrev = \"main\"\n"
            ),
        )],
    );

    let error = FleetConfig::load(&dir.join("fleet.toml"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("40-character commit revision"), "{error}");
}

/// Exercises the publishing path end to end against a real local repository,
/// including a subpath, which is how Ordnung ships its own baseline.
#[test]
fn a_pinned_git_layer_is_fetched_from_a_subdirectory() {
    fn git(dir: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let upstream = tempfile::tempdir().unwrap();
    let tier = upstream.path().join("confs/recommended");
    fs::create_dir_all(&tier).unwrap();
    fs::write(
        tier.join("policy.toml"),
        "name = \"recommended\"\n\n[policy.checks]\nvale = { severity = \"off\" }\n",
    )
    .unwrap();
    git(upstream.path(), &["init", "--quiet", "-b", "main"]);
    git(
        upstream.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    git(upstream.path(), &["config", "user.name", "Test"]);
    // Allow a client to fetch an arbitrary revision from this local repository.
    git(
        upstream.path(),
        &["config", "uploadpack.allowAnySHA1InWant", "true"],
    );
    git(upstream.path(), &["add", "-A"]);
    git(upstream.path(), &["commit", "--quiet", "-m", "init"]);
    let rev = String::from_utf8(
        std::process::Command::new("git")
            .current_dir(upstream.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();

    let cache = tempfile::tempdir().unwrap();
    // SAFETY: the cache location is process-wide configuration for this test.
    unsafe { std::env::set_var("ORDNUNG_CACHE_DIR", cache.path()) };

    let fleet = tempfile::tempdir().unwrap();
    let dir = config_dir(
        fleet.path(),
        &[(
            "fleet.toml",
            &format!(
                "{MEMBER}\n[[extends]]\ngit = {:?}\nrev = \"{rev}\"\npath = \"confs/recommended\"\n",
                upstream.path().to_str().unwrap()
            ),
        )],
    );

    let config = FleetConfig::load(&dir.join("fleet.toml")).unwrap();
    assert_eq!(config.policy.checks["vale"].severity, Severity::Off);

    // The pinned revision is immutable, so a second load reuses the cache.
    let again = FleetConfig::load(&dir.join("fleet.toml")).unwrap();
    assert_eq!(again.policy.checks["vale"].severity, Severity::Off);

    unsafe { std::env::remove_var("ORDNUNG_CACHE_DIR") };
}

/// Three layers deep, which is the shape Ordnung's own tiers use: a fleet extends
/// `paranoid`, which extends `recommended`. The nearest layer to the fleet wins.
#[test]
fn a_three_level_chain_resolves_nearest_layer_last() {
    let base = tempfile::tempdir().unwrap();
    let base_dir = config_dir(
        base.path(),
        &[(
            "policy.toml",
            "name = \"base\"\n\n[policy.checks]\n\
             vale = { severity = \"off\" }\n\
             stale = { severity = \"off\" }\n\
             license = { severity = \"off\" }\n",
        )],
    );

    // Middle tier escalates two of them and leaves the third alone.
    let middle = tempfile::tempdir().unwrap();
    let middle_dir = config_dir(
        middle.path(),
        &[(
            "policy.toml",
            &format!(
                "name = \"middle\"\n\n[[extends]]\npath = {:?}\n\n\
                 [policy.checks]\nvale = {{ severity = \"required\" }}\n\
                 stale = {{ severity = \"required\" }}\n",
                base_dir.to_str().unwrap()
            ),
        )],
    );

    // The fleet overrides one of the middle tier's escalations back down.
    let fleet = tempfile::tempdir().unwrap();
    let dir = config_dir(
        fleet.path(),
        &[(
            "fleet.toml",
            &format!(
                "{MEMBER}\n[[extends]]\npath = {:?}\n\n\
                 [policy.checks]\nstale = {{ severity = \"recommended\" }}\n",
                middle_dir.to_str().unwrap()
            ),
        )],
    );

    let config = FleetConfig::load(&dir.join("fleet.toml")).unwrap();
    let checks = &config.policy.checks;
    // Middle escalated it and nobody overrode it.
    assert_eq!(checks["vale"].severity, Severity::Required);
    // The fleet is nearest, so its value wins over the middle tier's.
    assert_eq!(checks["stale"].severity, Severity::Recommended);
    // Only the base layer mentioned it, so it survives untouched three levels down.
    assert_eq!(checks["license"].severity, Severity::Off);
}

/// A stage relaxes ceremony for one member without touching the rest of the fleet,
/// and without relaxing hygiene or security for anyone.
#[test]
fn a_member_stage_relaxes_only_that_member() {
    let base = tempfile::tempdir().unwrap();
    let base_dir = config_dir(
        base.path(),
        &[(
            "policy.toml",
            "name = \"base\"\n\n\
             [policy.checks]\n\
             branch-protection = { severity = \"required\" }\n\
             changelog = { severity = \"required\" }\n\
             secret-scanning = { severity = \"required\" }\n\n\
             [policy.stages.incubating.checks]\n\
             branch-protection = { severity = \"off\" }\n\
             changelog = { severity = \"off\" }\n",
        )],
    );

    let fleet = tempfile::tempdir().unwrap();
    let dir = config_dir(
        fleet.path(),
        &[(
            "fleet.toml",
            &format!(
                "name = \"test\"\n\n[[extends]]\npath = {:?}\n\n\
                 [[member]]\nrepo = \"owner/young\"\nstage = \"incubating\"\n\n\
                 [[member]]\nrepo = \"owner/mature\"\n",
                base_dir.to_str().unwrap()
            ),
        )],
    );

    let config = FleetConfig::load(&dir.join("fleet.toml")).unwrap();
    assert_eq!(config.stage_of("owner/young"), Stage::Incubating);
    // Omitting the field means supported, so existing fleets are unaffected.
    assert_eq!(config.stage_of("owner/mature"), Stage::Supported);

    let young = config.checks_for("owner/young");
    let mature = config.checks_for("owner/mature");
    assert_eq!(young["branch-protection"].severity, Severity::Off);
    assert_eq!(mature["branch-protection"].severity, Severity::Required);
    // Security is never part of what a stage relaxes.
    assert_eq!(young["secret-scanning"].severity, Severity::Required);
}

#[test]
fn an_unknown_stage_name_in_policy_is_rejected() {
    let fleet = tempfile::tempdir().unwrap();
    let dir = config_dir(
        fleet.path(),
        &[(
            "fleet.toml",
            &format!(
                "{MEMBER}\n[policy.stages.experimental.checks]\nchangelog = {{ severity = \"off\" }}\n"
            ),
        )],
    );
    let error = FleetConfig::load(&dir.join("fleet.toml"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown stage"), "{error}");
}

/// A substituting entry writes the member into its placeholders, leaves GitHub
/// expressions alone, and refuses a placeholder it does not know — a mistake
/// should fail the plan, not ship literally to every member.
#[test]
fn substituting_entry_fills_member_placeholders_and_rejects_unknown_ones() {
    let fleet = tempfile::tempdir().unwrap();
    let member = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet.path().join("managed")).unwrap();
    fs::write(
        fleet.path().join("managed/release.yml"),
        "# {{name}} release for {{repo}}\nenv: {{NAME}}_VERSION\nexpr: ${{ matrix.target }}\n",
    )
    .unwrap();
    let inventory = inspect_repository(member.path(), &InventoryOptions::default()).unwrap();

    let mut substituting = entry("managed/release.yml", ".github/workflows/release.yml");
    substituting.substitute = true;
    let changes = plan_managed_changes(
        "PowderworksCode/herdr-muster",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![substituting.clone()]),
    )
    .unwrap();
    assert_eq!(changes.len(), 1);
    let content = String::from_utf8(changes[0].content().unwrap().to_vec()).unwrap();
    assert_eq!(
        content,
        "# herdr-muster release for PowderworksCode/herdr-muster\n\
         env: HERDR_MUSTER_VERSION\nexpr: ${{ matrix.target }}\n"
    );

    // Applying makes the plan converge: the substituted bytes are the state.
    apply_changes(member.path(), &changes).unwrap();
    let clean = plan_managed_changes(
        "PowderworksCode/herdr-muster",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![substituting.clone()]),
    )
    .unwrap();
    assert!(clean.is_empty());

    // A different member plans different bytes from the same source.
    let other = tempfile::tempdir().unwrap();
    let other_inventory = inspect_repository(other.path(), &InventoryOptions::default()).unwrap();
    let changes = plan_managed_changes(
        "PowderworksCode/straitjacket",
        other.path(),
        &other_inventory,
        &resolved(fleet.path(), vec![substituting.clone()]),
    )
    .unwrap();
    let content = String::from_utf8(changes[0].content().unwrap().to_vec()).unwrap();
    assert!(content.contains("STRAITJACKET_VERSION"));

    // An unrecognized placeholder, spelled correctly: the plan refuses what it
    // cannot fill in, rather than shipping the braces to every member.
    fs::write(
        fleet.path().join("managed/release.yml"),
        "unknown: {{repository}}\n",
    )
    .unwrap();
    let error = plan_managed_changes(
        "PowderworksCode/straitjacket",
        other.path(),
        &other_inventory,
        &resolved(fleet.path(), vec![substituting]),
    )
    .unwrap_err();
    assert!(error.to_string().contains("{{repository}}"), "{error}");
}

/// substitute is a per-file contract; a directory source cannot carry it.
#[test]
fn substituting_directory_source_is_rejected() {
    let fleet = tempfile::tempdir().unwrap();
    let member = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet.path().join("managed/styles")).unwrap();
    fs::write(fleet.path().join("managed/styles/a.yml"), "a\n").unwrap();
    let inventory = inspect_repository(member.path(), &InventoryOptions::default()).unwrap();
    let mut substituting = entry("managed/styles", ".vale/styles");
    substituting.substitute = true;
    let error = plan_managed_changes(
        "owner/member",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![substituting]),
    )
    .unwrap_err();
    assert!(error.to_string().contains("file source"), "{error}");
}

/// A member's website is the fleet's fact, not the repository's: the two need
/// not agree, and a file that hands out an install URL has to hand out one that
/// answers. A member without a declared website is told so, by name.
#[test]
fn substituting_entry_takes_the_website_from_the_fleet() {
    let fleet = tempfile::tempdir().unwrap();
    let member = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet.path().join("managed")).unwrap();
    fs::write(
        fleet.path().join("managed/install.sh"),
        "# curl -fsSL {{website}}/install | sh\n",
    )
    .unwrap();
    let inventory = inspect_repository(member.path(), &InventoryOptions::default()).unwrap();
    let mut substituting = entry("managed/install.sh", "install.sh");
    substituting.substitute = true;

    let changes = plan_managed_changes_for_member(
        "PowderworksCode/straitjacket",
        Some("https://straitjacket.dev"),
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![substituting.clone()]),
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(changes[0].content().unwrap().to_vec()).unwrap(),
        "# curl -fsSL https://straitjacket.dev/install | sh\n"
    );

    let error = plan_managed_changes_for_member(
        "PowderworksCode/straitjacket",
        None,
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![substituting]),
    )
    .unwrap_err();
    assert!(error.to_string().contains("declares no website"), "{error}");
}

/// A managed hook arrives executable, or Git silently declines to run it —
/// which looks exactly like a repository whose hooks all pass.
#[test]
fn managed_files_keep_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt;
    let fleet = tempfile::tempdir().unwrap();
    let member = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet.path().join("managed/githooks")).unwrap();
    let hook = fleet.path().join("managed/githooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(fleet.path().join("managed/githooks/README.md"), "# hooks\n").unwrap();
    let inventory = inspect_repository(member.path(), &InventoryOptions::default()).unwrap();

    let changes = plan_managed_changes(
        "owner/member",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![entry("managed/githooks", ".githooks")]),
    )
    .unwrap();
    apply_changes(member.path(), &changes).unwrap();

    let written = member.path().join(".githooks/pre-commit");
    let mode = fs::metadata(&written).unwrap().permissions().mode();
    assert!(mode & 0o111 != 0, "hook is not executable: {mode:o}");
    let prose = member.path().join(".githooks/README.md");
    let prose_mode = fs::metadata(&prose).unwrap().permissions().mode();
    assert!(
        prose_mode & 0o111 == 0,
        "prose became executable: {prose_mode:o}"
    );

    // Converged: the mode is part of what "already correct" means.
    let clean = plan_managed_changes(
        "owner/member",
        member.path(),
        &inventory,
        &resolved(fleet.path(), vec![entry("managed/githooks", ".githooks")]),
    )
    .unwrap();
    assert!(clean.is_empty(), "{clean:?}");
}
