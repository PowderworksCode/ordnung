use std::fs;
use std::path::Path;

use ordnung_core::fleet::{
    FleetMember, FleetPolicy, ManagedEntry, ManagedState, ProjectSelector, RelativeTo,
};
use ordnung_core::{
    ChangeKind, FleetConfig, InventoryOptions, LanguageId, apply_changes, inspect_repository,
    plan_managed_changes,
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
    }
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
        fleet.path(),
        "owner/member",
        member.path(),
        &inventory,
        &[entry("managed/styles", ".vale/styles")],
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
        fleet.path(),
        "owner/member",
        member.path(),
        &inventory,
        &[entry("managed/styles", ".vale/styles")],
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
        fleet.path(),
        "owner/member",
        member.path(),
        &inventory,
        &[managed],
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
    };

    let changes = plan_managed_changes(
        fleet.path(),
        "owner/member",
        member.path(),
        &inventory,
        &[managed],
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
        fleet.path(),
        "owner/member",
        member.path(),
        &inventory,
        &[entry("managed/tree", "owned")],
    )
    .unwrap();
    apply_changes(member.path(), &directory_changes).unwrap();
    assert_eq!(
        fs::read_to_string(member.path().join("owned/file.txt")).unwrap(),
        "managed\n"
    );

    fs::write(fleet.path().join("managed/single.txt"), "single\n").unwrap();
    let file_changes = plan_managed_changes(
        fleet.path(),
        "owner/member",
        member.path(),
        &inventory,
        &[entry("managed/single.txt", "owned")],
    )
    .unwrap();
    apply_changes(member.path(), &file_changes).unwrap();
    assert_eq!(
        fs::read_to_string(member.path().join("owned")).unwrap(),
        "single\n"
    );
}

#[test]
fn overlapping_managed_ownership_is_rejected() {
    let fleet = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet.path().join("managed/tree")).unwrap();
    fs::write(fleet.path().join("managed/tree/file.txt"), "managed\n").unwrap();
    fs::write(fleet.path().join("managed/file.txt"), "managed\n").unwrap();
    let config = FleetConfig {
        name: "test".into(),
        members: vec![FleetMember {
            repo: "owner/repo".into(),
            note: String::new(),
        }],
        policy: FleetPolicy::default(),
        managed: vec![
            entry("managed/tree", ".config"),
            entry("managed/file.txt", ".config/file.txt"),
        ],
    };

    assert!(config.validate(fleet.path()).is_err());
}
