use ordnung_core::profile::registry;
use ordnung_core::{
    EcosystemProfile, EcosystemRegistration, EcosystemRole, LanguageConventions, LanguageProfile,
    LanguageRegistration, ManifestSelection, TestLayoutDefaults, ecosystem_profile,
    language_conventions, language_profile, language_profiles,
};

static FIXTURE_LANGUAGE: LanguageProfile = LanguageProfile {
    id: "fixture-language",
    display_name: "Fixture Language",
    extensions: &["fixture"],
    source_extensions: &["fixture"],
    filenames: &[],
    shebangs: &[],
    role: ordnung_core::LanguageRole::Programming,
    comments: None,
    facets: &[],
    conventions: Some(LanguageConventions {
        typecheck: None,
        test_layout: TestLayoutDefaults {
            source_roots: &["src"],
            test_root: "tests",
            test_suffixes: &[""],
        },
        inline_test_detector: |_| None,
    }),
    config_files: &["fixture.toml"],
    package_dependencies: &[],
    supersedes: &[],
};

static FIXTURE_ECOSYSTEM: EcosystemProfile = EcosystemProfile {
    id: "fixture-ecosystem",
    display_name: "Fixture Ecosystem",
    roles: &[EcosystemRole::BuildSystem],
    implied_languages: &[&FIXTURE_LANGUAGE],
    manifest: None,
    lockfiles: &[],
    selector_files: &[],
    gitignore_patterns: &[],
    manifest_selection: ManifestSelection::Default,
    dependency_pins: None,
};

registry::submit! {
    LanguageRegistration(&FIXTURE_LANGUAGE)
}

registry::submit! {
    EcosystemRegistration(&FIXTURE_ECOSYSTEM)
}

#[test]
fn downstream_modules_can_register_profiles() {
    let profile = language_profile("fixture-language").unwrap();
    assert_eq!(profile.display_name, "Fixture Language");
    assert_eq!(
        language_conventions(profile)
            .unwrap()
            .test_layout
            .source_roots,
        ["src"]
    );
    assert!(
        ecosystem_profile("fixture-ecosystem")
            .unwrap()
            .implies_language(profile)
    );
    assert!(
        language_profiles()
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id)
    );
}
