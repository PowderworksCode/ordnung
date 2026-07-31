use std::collections::BTreeMap;
use std::path::Path;

use ordnung_core::{
    BooleanSettingOverride, BooleanSettingPolicy, CheckPolicy, GithubSettingsOverrides,
    GithubSettingsPolicy, LanguageTestLayout, LocalOverride, RepoConfig, Severity,
    TestLayoutConfig, resolve_github_settings, resolve_policy,
};

#[test]
fn scripts_configuration_uses_exact_safe_paths() {
    let config = RepoConfig::parse(
        "ordnung.toml",
        "[scripts]\ndirectory = 'bin'\ndevelopment = 'setup'\nallow = ['install.sh']\nignore_directories = ['vendor']\n",
    )
    .unwrap();
    assert_eq!(config.scripts.development_path(), Path::new("bin/setup"));
    assert_eq!(config.scripts.allow, [Path::new("install.sh")]);

    for invalid in [
        "[scripts]\ndirectory = '../bin'\n",
        "[scripts]\nallow = ['tool.sh', 'tool.sh']\n",
        "[scripts]\nignore_directories = ['generated/output']\n",
    ] {
        assert!(RepoConfig::parse("ordnung.toml", invalid).is_err());
    }
}

#[test]
fn codegen_declarations_are_explicit_and_relative() {
    let config = RepoConfig::parse(
        "ordnung.toml",
        r#"[[codegen]]
name = "bindings"
root = "crates/bindings"
command = "bun run generate"
outputs = ["src/generated/**"]
"#,
    )
    .unwrap();
    assert_eq!(config.codegen[0].name, "bindings");
    assert_eq!(config.codegen[0].root, Path::new("crates/bindings"));

    for invalid in [
        r#"[[codegen]]
name = "bindings"
root = "../bindings"
command = "bun run generate"
outputs = ["src/generated/**"]
"#,
        r#"[[codegen]]
name = "bindings"
command = "bun run generate && git diff --exit-code"
outputs = ["src/generated/**"]
"#,
        r#"[[codegen]]
name = "bindings"
command = "bun run generate"
outputs = []
"#,
    ] {
        assert!(RepoConfig::parse("ordnung.toml", invalid).is_err());
    }
}

#[test]
fn test_layout_uses_registered_language_profiles() {
    let config = RepoConfig::parse(
        "ordnung.toml",
        "[test_layout.rust]\ntest_root = \"checks\"\n",
    )
    .unwrap();
    assert_eq!(
        config.test_layout.languages["rust"].test_root,
        Path::new("checks")
    );

    let unknown = RepoConfig::parse(
        "ordnung.toml",
        "[test_layout.brainfuck]\ntest_root = \"checks\"\n",
    );
    assert!(unknown.is_err());
}

#[test]
fn test_layout_rejects_languages_without_conventions() {
    let config = TestLayoutConfig {
        languages: [("python".into(), LanguageTestLayout::default())]
            .into_iter()
            .collect(),
        ..TestLayoutConfig::default()
    };

    assert!(
        config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("unsupported test-layout language profile \"python\"")
    );
}

#[test]
fn ci_exists_ignore_requires_valid_relative_patterns() {
    let config =
        RepoConfig::parse("ordnung.toml", "[ci_exists]\nignore = ['spikes/**']\n").unwrap();
    assert_eq!(config.ci_exists.ignore, ["spikes/**"]);

    assert!(RepoConfig::parse("ordnung.toml", "[ci_exists]\nignore = ['../src']\n").is_err());
    assert!(RepoConfig::parse("ordnung.toml", "[ci_exists]\nignore = ['[']\n").is_err());
}

#[test]
fn stray_file_configuration_uses_exact_paths_and_valid_globs() {
    let config = RepoConfig::parse(
        "ordnung.toml",
        "[stray_files]\nnotes = 'knowledge'\nallow = ['ROADMAP.md']\n",
    )
    .unwrap();
    assert_eq!(config.stray_files.allow, [Path::new("ROADMAP.md")]);
    for invalid in [
        "[stray_files]\nallow = ['docs/ROADMAP.md']\n",
        "[stray_files]\nnotes = '../notes'\n",
    ] {
        assert!(RepoConfig::parse("ordnung.toml", invalid).is_err());
    }
}

#[test]
fn fleet_override_requires_permission_and_reason() {
    let defaults = BTreeMap::from([("website".into(), Severity::Required)]);
    let fleet = BTreeMap::from([(
        "website".into(),
        CheckPolicy {
            severity: Severity::Required,
            allow_override: true,
        },
    )]);
    let local = RepoConfig {
        overrides: BTreeMap::from([(
            "website".into(),
            LocalOverride {
                severity: Severity::Off,
                reason: "internal repository".into(),
            },
        )]),
        ..RepoConfig::default()
    };

    let resolved = resolve_policy(&defaults, Some(&fleet), &local).unwrap();
    assert_eq!(resolved["website"], Severity::Off);

    let mut denied = fleet;
    denied.get_mut("website").unwrap().allow_override = false;
    assert!(resolve_policy(&defaults, Some(&denied), &local).is_err());
}

#[test]
fn unknown_checks_are_rejected() {
    let defaults = BTreeMap::from([("website".into(), Severity::Required)]);
    let local = RepoConfig {
        checks: BTreeMap::from([(
            "mystery".into(),
            CheckPolicy {
                severity: Severity::Off,
                allow_override: false,
            },
        )]),
        ..RepoConfig::default()
    };

    assert!(resolve_policy(&defaults, None, &local).is_err());
}

#[test]
fn github_setting_override_requires_permission_and_reason() {
    let fleet = GithubSettingsPolicy {
        allow_auto_merge: Some(BooleanSettingPolicy {
            value: false,
            allow_override: true,
        }),
        ..GithubSettingsPolicy::default()
    };
    let local = RepoConfig {
        github_overrides: GithubSettingsOverrides {
            allow_auto_merge: Some(BooleanSettingOverride {
                value: true,
                reason: "trusted automatic dependency updates".into(),
            }),
            ..GithubSettingsOverrides::default()
        },
        ..RepoConfig::default()
    };
    let resolved = resolve_github_settings(Some(&fleet), &local).unwrap();
    assert_eq!(resolved.allow_auto_merge, Some(true));

    let denied = GithubSettingsPolicy {
        allow_auto_merge: Some(BooleanSettingPolicy {
            value: false,
            allow_override: false,
        }),
        ..GithubSettingsPolicy::default()
    };
    assert!(resolve_github_settings(Some(&denied), &local).is_err());
}
