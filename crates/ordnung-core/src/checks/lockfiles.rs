use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "lockfiles",
    default_severity: Severity::Required,
    category: CheckCategory::Dependencies,
    instructions: "Commit the correct lockfile for every detected package ecosystem.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    for package in context
        .inventory
        .packages
        .iter()
        .filter(|package| !package.is_workspace_member())
    {
        let profile = package.ecosystem_profile();
        results.push(result(
            definition,
            if package.lockfile.is_some() {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            package.lockfile_owner.clone(),
            if let Some(lockfile) = &package.lockfile {
                format!(
                    "{} package is covered by {}",
                    profile.display_name,
                    lockfile.display()
                )
            } else {
                format!(
                    "{} package has no {} at its lockfile owner {}",
                    profile.display_name,
                    profile.lockfile_description(),
                    display_root(&package.lockfile_owner)
                )
            },
        ));
    }
}

fn display_root(root: &std::path::Path) -> String {
    if root.as_os_str().is_empty() {
        ".".into()
    } else {
        root.display().to_string()
    }
}
