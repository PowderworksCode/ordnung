use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "scripts",
    default_severity: Severity::Recommended,
    category: CheckCategory::RepositoryShape,
    instructions: "Keep detected shell scripts under the configured scripts.directory, except exact scripts.allow paths; provide the configured development script there and name its repository-relative path in the root README.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let config = context.scripts;
    let development = config.development_path();
    let mut problems = Vec::new();
    let strays = context
        .inventory
        .shell_scripts
        .iter()
        .filter(|path| !path.starts_with(&config.directory))
        .filter(|path| !config.allow.contains(path))
        .filter(|path| !ignored(path, &config.ignore_directories))
        .cloned()
        .collect::<Vec<_>>();
    if !strays.is_empty() {
        problems.push(format!(
            "shell scripts outside {}/: {}",
            display(&config.directory),
            strays
                .iter()
                .map(|path| display(path))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if !context.inventory.files.contains(&development) {
        problems.push(format!(
            "no {} to stand up the development environment",
            display(&development)
        ));
    } else if !context.inventory.shell_scripts.contains(&development) {
        problems.push(format!(
            "{} is not detected as a shell script",
            display(&development)
        ));
    } else {
        match root_readme(&context.inventory.files) {
            None => problems.push(format!(
                "{} is not mentioned in a root README",
                display(&development)
            )),
            Some(readme) => match fs::read_to_string(context.root.join(readme)) {
                Ok(text) if !text.contains(&display(&development)) => problems.push(format!(
                    "{} is not mentioned in {}",
                    display(&development),
                    display(readme)
                )),
                Ok(_) => {}
                Err(error) => {
                    results.push(result(
                        definition,
                        CheckStatus::Error,
                        readme.clone(),
                        format!("could not read {}: {error}", display(readme)),
                    ));
                    return;
                }
            },
        }
    }

    let status = if problems.is_empty() {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail
    };
    let message = if problems.is_empty() {
        format!(
            "shell scripts are corralled in {}/ and {} is documented",
            display(&config.directory),
            display(&development)
        )
    } else {
        problems.join("; ")
    };
    results.push(result(definition, status, development, message));
}

fn ignored(path: &Path, ignored_directories: &[String]) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => {
            let name = name.to_string_lossy();
            name.starts_with('.') || ignored_directories.iter().any(|ignored| ignored == &name)
        }
        _ => false,
    })
}

fn root_readme(files: &std::collections::BTreeSet<PathBuf>) -> Option<&PathBuf> {
    files.iter().find(|path| {
        path.parent()
            .is_some_and(|parent| parent.as_os_str().is_empty())
            && path
                .file_stem()
                .is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case("readme"))
    })
}

fn display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
