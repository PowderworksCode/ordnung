use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use entl_github::{DependabotEcosystemProfile, DependabotUpdate, dependabot_ecosystem_profile};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    GithubCheckContext, RepositoryCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "dependabot",
    default_severity: Severity::Required,
    category: CheckCategory::Dependencies,
    scope: CheckScope::Repository,
    instructions: "Keep a valid .github/dependabot.yml version 2 configuration with a scheduled update covering every detected package ecosystem at its owning directory and GitHub Actions at the repository root; directory globs may be used explicitly.",
    repository_runner: Some(run_repository),
    github_runner: Some(run_github),
};

registry::submit! { CheckRegistration(&CHECK) }

struct Requirement {
    profile: Option<&'static DependabotEcosystemProfile>,
    directory: PathBuf,
}

fn run_repository(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let requirements = requirements(context);
    if requirements.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::from(".github/dependabot.yml"),
            "no supported package ecosystems or GitHub Actions workflows detected",
        ));
        return;
    }
    if !context.inventory.github.dependabot.diagnostics.is_empty() {
        for diagnostic in &context.inventory.github.dependabot.diagnostics {
            results.push(result(
                definition,
                CheckStatus::Fail,
                diagnostic.path.clone(),
                diagnostic.message.clone(),
            ));
        }
        return;
    }
    let Some(configuration) = &context.inventory.github.dependabot.configuration else {
        results.push(result(
            definition,
            CheckStatus::Fail,
            PathBuf::from(".github/dependabot.yml"),
            "no .github/dependabot.yml or .github/dependabot.yaml found",
        ));
        return;
    };

    for requirement in requirements {
        let package_ecosystem = requirement
            .profile
            .map_or("github-actions", |profile| profile.package_ecosystem);
        let covered = configuration.updates.iter().any(|update| {
            accepts(requirement.profile, update)
                && update
                    .directories
                    .iter()
                    .any(|pattern| directory_matches(pattern, &requirement.directory))
        });
        let directory = display_directory(&requirement.directory);
        results.push(result(
            definition,
            if covered {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            configuration.path.clone(),
            if covered {
                format!("Dependabot covers {package_ecosystem} at {directory}")
            } else {
                format!("Dependabot is missing {package_ecosystem} coverage at {directory}")
            },
        ));
    }
}

fn run_github(
    definition: &'static CheckDefinition,
    facts: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let settings = [
        ("vulnerability alerts", &facts.vulnerability_alerts),
        ("automated security fixes", &facts.automated_security_fixes),
    ];
    let disabled = settings
        .iter()
        .filter_map(|(name, value)| {
            matches!(value, GithubValue::Known { value: false }).then_some(*name)
        })
        .collect::<Vec<_>>();
    let unavailable = settings
        .iter()
        .filter_map(|(name, value)| match value {
            GithubValue::Unavailable { reason } => Some(format!("{name}: {reason}")),
            GithubValue::Known { .. } => None,
        })
        .collect::<Vec<_>>();
    let (status, message) = if !disabled.is_empty() {
        let mut message = format!("disabled: {}", disabled.join(", "));
        if !unavailable.is_empty() {
            message.push_str(&format!("; unavailable: {}", unavailable.join(", ")));
        }
        (CheckStatus::Fail, message)
    } else if !unavailable.is_empty() {
        (
            CheckStatus::Skip,
            format!(
                "Dependabot security settings unavailable: {}",
                unavailable.join(", ")
            ),
        )
    } else {
        (
            CheckStatus::Pass,
            "vulnerability alerts and automated security fixes are enabled".to_owned(),
        )
    };
    results.push(result(definition, status, PathBuf::new(), message));
}

fn requirements(context: &RepositoryCheckContext<'_>) -> Vec<Requirement> {
    let mut seen = BTreeSet::new();
    let mut requirements = Vec::new();
    for package in &context.inventory.packages {
        let ecosystem = package.ecosystem_profile();
        let Some(profile) = dependabot_ecosystem_profile(ecosystem) else {
            continue;
        };
        let directory = if ecosystem.id == "cargo" {
            package.lockfile_owner.clone()
        } else {
            package.root.clone()
        };
        if seen.insert((profile.package_ecosystem, directory.clone())) {
            requirements.push(Requirement {
                profile: Some(profile),
                directory,
            });
        }
    }
    if context.inventory.github.has_workflows() && seen.insert(("github-actions", PathBuf::new())) {
        requirements.push(Requirement {
            profile: None,
            directory: PathBuf::new(),
        });
    }
    requirements.sort_by(|left, right| {
        requirement_name(left)
            .cmp(requirement_name(right))
            .then(left.directory.cmp(&right.directory))
    });
    requirements
}

fn requirement_name(requirement: &Requirement) -> &'static str {
    requirement
        .profile
        .map_or("github-actions", |profile| profile.package_ecosystem)
}

fn accepts(profile: Option<&DependabotEcosystemProfile>, update: &DependabotUpdate) -> bool {
    profile.map_or(update.package_ecosystem == "github-actions", |profile| {
        profile.accepts(&update.package_ecosystem)
    })
}

fn directory_matches(pattern: &str, target: &Path) -> bool {
    let pattern = pattern.trim();
    let target = normalize_directory(target);
    if let Some(base) = pattern.strip_suffix("/**") {
        let base = normalize_directory(Path::new(base));
        let prefix = if base == "/" {
            "/".to_owned()
        } else {
            format!("{base}/")
        };
        return target == base || target.starts_with(&prefix);
    }
    if let Some(base) = pattern.strip_suffix("/*") {
        let base = normalize_directory(Path::new(base));
        let prefix = if base == "/" {
            "/".to_owned()
        } else {
            format!("{base}/")
        };
        return target
            .strip_prefix(&prefix)
            .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'));
    }
    normalize_directory(Path::new(pattern)) == target
}

fn normalize_directory(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = path.trim().trim_matches('/');
    if path.is_empty() {
        "/".to_owned()
    } else {
        format!("/{path}")
    }
}

fn display_directory(path: &Path) -> String {
    format!("`{}`", normalize_directory(path))
}
