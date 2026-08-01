use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use entl_codebase::{
    EcosystemRole, JAVASCRIPT_LANGUAGE, STYLELINT, TYPESCRIPT_LANGUAGE, ToolProfile,
};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};
use crate::inventory::PackageInstance;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "stylelint",
    default_severity: Severity::Off,
    category: CheckCategory::BuildToolchain,
    scope: CheckScope::Project,
    instructions: "For each package containing CSS, SCSS, Sass, or Less, keep a Stylelint configuration in that package or an ancestor and run Stylelint on pushes or pull requests.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let package_scopes = context
        .inventory
        .packages
        .iter()
        .filter(|package| is_javascript_package(package))
        .map(|package| package.root.clone())
        .collect::<BTreeSet<_>>();
    let mut stylesheet_scopes = BTreeSet::new();
    for path in context
        .inventory
        .files
        .iter()
        .filter(|path| is_stylesheet(path, &STYLELINT))
    {
        stylesheet_scopes.insert(owning_scope(path, &package_scopes));
    }

    let mut configurations = BTreeMap::new();
    for scope in std::iter::once(PathBuf::new()).chain(package_scopes.iter().cloned()) {
        if let Some(configuration) = configuration_at(context, &scope, &STYLELINT) {
            configurations.insert(scope, configuration);
        }
    }
    let mut applicable = stylesheet_scopes;
    applicable.extend(configurations.keys().cloned());
    if applicable.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no stylesheet or Stylelint configuration found",
        ));
        return;
    }

    for scope in applicable {
        let inherited_configuration = configurations
            .iter()
            .filter(|(owner, _)| scope.starts_with(owner))
            .max_by_key(|(owner, _)| owner.components().count())
            .map(|(_, configuration)| configuration);
        let Some(configuration) = inherited_configuration else {
            results.push(result(
                definition,
                CheckStatus::Fail,
                scope,
                "stylesheets are present but no Stylelint configuration applies",
            ));
            continue;
        };
        let wired = context
            .inventory
            .github
            .workflows
            .iter()
            .filter(|workflow| workflow.runs_on_changes())
            .flat_map(|workflow| &workflow.tasks)
            .any(|task| {
                task.tool.as_str() == STYLELINT.id
                    && (task.package_roots.is_empty()
                        || task
                            .package_roots
                            .iter()
                            .any(|root| scope.starts_with(root)))
            });
        results.push(if wired {
            result(
                definition,
                CheckStatus::Pass,
                scope,
                format!(
                    "Stylelint configuration {} runs on repository changes",
                    configuration.display()
                ),
            )
        } else {
            result(
                definition,
                CheckStatus::Fail,
                scope,
                format!(
                    "Stylelint configuration {} is not run on pushes or pull requests",
                    configuration.display()
                ),
            )
        });
    }
}

fn is_javascript_package(package: &PackageInstance) -> bool {
    let ecosystem = package.ecosystem_profile();
    ecosystem.has_role(EcosystemRole::PackageManager)
        && (ecosystem.implies_language(&JAVASCRIPT_LANGUAGE)
            || ecosystem.implies_language(&TYPESCRIPT_LANGUAGE))
}

fn is_stylesheet(path: &Path, tool: &ToolProfile) -> bool {
    tool.languages
        .iter()
        .any(|language| language.detects_source(path))
}

fn owning_scope(path: &Path, scopes: &BTreeSet<PathBuf>) -> PathBuf {
    scopes
        .iter()
        .filter(|scope| path.starts_with(scope))
        .max_by_key(|scope| scope.components().count())
        .cloned()
        .unwrap_or_default()
}

fn configuration_at(
    context: &RepositoryCheckContext<'_>,
    scope: &Path,
    tool: &ToolProfile,
) -> Option<PathBuf> {
    for filename in tool.configuration_files {
        let path = scope.join(filename);
        if context.inventory.files.contains(&path) {
            return Some(path);
        }
    }
    let package = scope.join("package.json");
    if !context.inventory.files.contains(&package) {
        return None;
    }
    let text = fs::read_to_string(context.root.join(&package)).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    tool.package_json_keys
        .iter()
        .any(|key| value.get(key).is_some())
        .then_some(package)
}
