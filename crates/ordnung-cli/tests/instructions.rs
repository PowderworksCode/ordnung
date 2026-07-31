use std::collections::BTreeMap;
use std::path::PathBuf;

use ordnung_cli::instructions::{END_MARKER, InstructionContext, START_MARKER, inject, render};
use ordnung_core::{
    CodegenConfig, GithubSettings, Inventory, LanguageId, Project, ProjectCapability, RepoConfig,
    Severity, check_definitions,
};

fn inventory() -> Inventory {
    Inventory {
        root: PathBuf::from("/repo"),
        files: Default::default(),
        shell_scripts: Default::default(),
        projects: vec![Project {
            root: PathBuf::new(),
            languages: [LanguageId::from("rust")].into_iter().collect(),
            capabilities: [ProjectCapability::CargoWorkspace].into_iter().collect(),
            ecosystems: [ordnung_core::EcosystemId::from("cargo")]
                .into_iter()
                .collect(),
            evidence: [PathBuf::from("Cargo.toml")].into_iter().collect(),
        }],
        artifacts: Vec::new(),
        packages: Vec::new(),
        github: ordnung_core::GithubInventory::default(),
        issues: Vec::new(),
    }
}

#[test]
fn renders_concise_effective_rules() {
    let inventory = inventory();
    let policy = BTreeMap::from([
        ("ci-continue-on-error".into(), Severity::Required),
        ("ci-exists".into(), Severity::Required),
        ("field-guide".into(), Severity::Recommended),
        ("test-layout".into(), Severity::Off),
        ("website".into(), Severity::Recommended),
    ]);
    let local = RepoConfig::default();
    let output = render(&InstructionContext {
        inventory: &inventory,
        policy: &policy,
        github: &GithubSettings::default(),
        test_layout: &local.test_layout,
        local: &local,
        fleet_name: None,
        managed: &[],
    });

    assert!(output.starts_with(START_MARKER));
    assert!(output.ends_with(END_MARKER));
    assert!(output.contains("languages `rust`"));
    assert!(output.contains("### Required rules"));
    assert!(output.contains("**CI safety**"));
    assert!(output.contains(
        "`ci-exists`: Keep a push or pull-request workflow with test, lint, and format tasks for every detected language; exempt scratch project paths explicitly with ci_exists.ignore."
    ));
    assert!(output.contains(
        "`ci-continue-on-error`: Do not let jobs or gating test, lint, format, typecheck, and build steps hide failures with continue-on-error."
    ));
    assert!(output.contains("### Recommended rules"));
    assert!(output.contains("**Documentation and text**"));
    assert!(output.contains(
        "`website`: Keep the repository's GitHub homepage setting pointed at its reachable HTTP(S) website."
    ));
    assert!(output.contains(
        "`field-guide`: At the start of work, find and read `field_guide.md`; append concise, durable discoveries that will help future agents."
    ));
    assert!(!output.contains("Rust tests:"));
}

#[test]
fn renders_effective_script_locations_and_exact_exceptions() {
    let inventory = inventory();
    let policy = BTreeMap::from([("scripts".into(), Severity::Required)]);
    let local = RepoConfig::parse(
        "ordnung.toml",
        "[scripts]\ndirectory = 'bin'\ndevelopment = 'setup'\nallow = ['install.sh']\n",
    )
    .unwrap();
    let output = render(&InstructionContext {
        inventory: &inventory,
        policy: &policy,
        github: &GithubSettings::default(),
        test_layout: &local.test_layout,
        local: &local,
        fleet_name: None,
        managed: &[],
    });

    assert!(output.contains("keep them under `bin`; development entry `bin/setup`"));
    assert!(output.contains("Shell-script path exceptions: `install.sh`"));
}

#[test]
fn renders_effective_note_corral() {
    let inventory = inventory();
    let local = RepoConfig::parse(
        "ordnung.toml",
        "[stray_files]\nnotes = 'knowledge'\nallow = ['ROADMAP.md']\n",
    )
    .unwrap();
    let policy = BTreeMap::from([("stray-files".into(), Severity::Required)]);
    let output = render(&InstructionContext {
        inventory: &inventory,
        policy: &policy,
        github: &GithubSettings::default(),
        test_layout: &local.test_layout,
        local: &local,
        fleet_name: None,
        managed: &[],
    });
    assert!(output.contains("Working notes belong under `knowledge`"));
    assert!(output.contains("Allowed root text files: `ROADMAP.md`"));
}

#[test]
fn every_registered_check_supplies_its_own_instructions() {
    let inventory = inventory();
    let local = RepoConfig::default();
    let policy = check_definitions()
        .iter()
        .map(|definition| (definition.id.to_owned(), Severity::Required))
        .collect();
    let output = render(&InstructionContext {
        inventory: &inventory,
        policy: &policy,
        github: &GithubSettings::default(),
        test_layout: &local.test_layout,
        local: &local,
        fleet_name: None,
        managed: &[],
    });

    for definition in check_definitions() {
        assert!(
            output.contains(&format!("`{}`: {}", definition.id, definition.instructions)),
            "missing instruction text for {}",
            definition.id
        );
    }
    assert!(!output.contains("Other configured rules"));
}

#[test]
fn injection_preserves_prose_and_replaces_one_owned_block() {
    let first = inject("# Agent notes\n", "generated one").unwrap();
    assert_eq!(first, "# Agent notes\n\ngenerated one\n");

    let marked = format!("# Agent notes\n\n{START_MARKER}\nold\n{END_MARKER}\nTail\n");
    let second = inject(&marked, &format!("{START_MARKER}\nnew\n{END_MARKER}")).unwrap();
    assert_eq!(
        second,
        format!("# Agent notes\n\n{START_MARKER}\nnew\n{END_MARKER}\nTail\n")
    );
}

#[test]
fn injection_rejects_malformed_markers() {
    assert!(inject(START_MARKER, "generated").is_err());
    assert!(
        inject(
            &format!("{START_MARKER}\n{END_MARKER}\n{START_MARKER}\n{END_MARKER}"),
            "generated"
        )
        .is_err()
    );
}

#[test]
fn renders_declared_codegen_commands_and_outputs() {
    let inventory = inventory();
    let local = RepoConfig {
        codegen: vec![CodegenConfig {
            name: "bindings".into(),
            root: PathBuf::from("crates/bindings"),
            command: "bun run generate".into(),
            outputs: vec!["src/generated/**".into()],
        }],
        ..RepoConfig::default()
    };
    let output = render(&InstructionContext {
        inventory: &inventory,
        policy: &BTreeMap::new(),
        github: &GithubSettings::default(),
        test_layout: &local.test_layout,
        local: &local,
        fleet_name: None,
        managed: &[],
    });
    assert!(output.contains(
        "Codegen `bindings` at `crates/bindings`: run `bun run generate`; committed outputs `src/generated/**`."
    ));
}
