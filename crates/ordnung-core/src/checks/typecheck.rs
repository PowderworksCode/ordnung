use entl_codebase::{TaskKind, language_conventions};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};
use crate::profile::language_profile;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "typecheck",
    default_severity: Severity::Required,
    category: CheckCategory::BuildToolchain,
    scope: CheckScope::Project,
    instructions: "Keep JavaScript and TypeScript projects on an explicit type layer and run their typechecker on push or pull requests.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let mut applicable = false;
    for project in &context.inventory.projects {
        for profile in project
            .languages
            .iter()
            .filter_map(|language| language_profile(language.as_str()))
        {
            let Some(typecheck) =
                language_conventions(profile).and_then(|conventions| conventions.typecheck)
            else {
                continue;
            };
            applicable = true;
            let config = typecheck
                .config_files
                .iter()
                .find(|config| context.root.join(&project.root).join(config).is_file());
            let has_typecheck = context.inventory.has_task(profile.id, TaskKind::Typecheck);
            let (status, message) = match (config, has_typecheck) {
                (None, _) => (
                    CheckStatus::Fail,
                    format!(
                        "{} has no type layer; add {}",
                        profile.display_name,
                        typecheck.config_files.join(" or ")
                    ),
                ),
                (Some(_), false) => (
                    CheckStatus::Fail,
                    format!(
                        "{} CI has no typecheck task on push or pull_request",
                        profile.display_name
                    ),
                ),
                (Some(config), true) => (
                    CheckStatus::Pass,
                    format!(
                        "{} project has {} and typechecks on changes",
                        profile.display_name, config,
                    ),
                ),
            };
            results.push(result(definition, status, project.root.clone(), message));
        }
    }
    if !applicable {
        results.push(result(
            definition,
            CheckStatus::Skip,
            std::path::PathBuf::new(),
            "no optionally typed JavaScript or TypeScript project detected",
        ));
    }
}
