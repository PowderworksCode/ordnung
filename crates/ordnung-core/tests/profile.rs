// Tests for `src/profile.rs`: the language and ecosystem profiles.
//
// Registration lives in profile_registration.rs and stays a target of its
// own: it submits a fixture profile to the inventory registry, which every
// test in the same binary would then see.
use std::collections::BTreeSet;
use std::path::Path;

use ordnung_core::{
    ManifestSelection, ecosystem_profile, ecosystem_profiles, language_conventions,
    language_profile, language_profiles,
};

#[test]
fn built_in_registrations_are_complete_and_deterministic() {
    let language_ids = language_profiles()
        .iter()
        .map(|profile| profile.id)
        .collect::<Vec<_>>();
    let ecosystem_ids = ecosystem_profiles()
        .iter()
        .map(|profile| profile.id)
        .collect::<Vec<_>>();

    assert!(language_ids.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(language_ids.contains(&"javascript"));
    assert!(language_ids.contains(&"rust"));
    assert!(language_ids.contains(&"typescript"));
    assert_eq!(ecosystem_ids, ["bun", "cargo", "npm", "pnpm", "yarn"]);
}

#[test]
fn profile_ids_are_unique_and_ecosystems_reference_languages() {
    let languages: BTreeSet<_> = language_profiles()
        .iter()
        .map(|profile| profile.id)
        .collect();
    let ecosystems: BTreeSet<_> = ecosystem_profiles()
        .iter()
        .map(|profile| profile.id)
        .collect();

    assert_eq!(languages.len(), language_profiles().len());
    assert_eq!(ecosystems.len(), ecosystem_profiles().len());
    assert!(ecosystem_profiles().iter().all(|profile| {
        language_profiles()
            .iter()
            .any(|language| profile.implies_language(language))
    }));
    for manifest in ecosystem_profiles()
        .iter()
        .filter_map(|profile| profile.manifest)
        .collect::<BTreeSet<_>>()
    {
        assert_eq!(
            ecosystem_profiles()
                .iter()
                .filter(|profile| {
                    profile.manifest == Some(manifest)
                        && matches!(profile.manifest_selection, ManifestSelection::Default)
                })
                .count(),
            1,
            "{manifest} needs exactly one unlocked fallback"
        );
    }
}

#[test]
fn profiles_separate_detection_from_language_conventions() {
    let rust = language_profile("rust").unwrap();
    let rust_conventions = language_conventions(rust).unwrap();
    assert!(rust.detects_source(Path::new("src/lib.rs")));
    assert!(rust.accepts_source(Path::new("src/lib.rs")));
    assert!(!rust.accepts_source(Path::new("src/lib.ts")));
    assert_eq!(
        rust_conventions.inline_test_indicator("#[cfg(test)]\nmod tests {}\n"),
        Some("#[cfg(test)]")
    );

    let typescript = language_profile("typescript").unwrap();
    let javascript = language_profile("javascript").unwrap();
    let conventions = language_conventions(typescript).unwrap();
    assert!(typescript.accepts_source(Path::new("src/component.tsx")));
    assert!(typescript.supersedes(javascript));
    assert_eq!(
        conventions.typecheck.unwrap().config_files,
        ["tsconfig.json"]
    );
    assert_eq!(conventions.test_layout.source_roots, ["src"]);
    assert_eq!(conventions.test_layout.test_root, "tests");
}

#[test]
fn ecosystems_own_manifest_and_lockfile_conventions() {
    let bun = ecosystem_profile("bun").unwrap();
    let javascript = language_profile("javascript").unwrap();
    assert_eq!(bun.manifest, Some("package.json"));
    assert_eq!(bun.lockfiles, ["bun.lock", "bun.lockb"]);
    assert!(bun.implies_language(javascript));
}
