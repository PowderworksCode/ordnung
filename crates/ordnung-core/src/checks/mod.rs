mod action_badge;
mod allow_auto_merge;
mod artifacts_built;
mod auto_update_pr_branches;
mod branch_protection;
mod builds;
mod changelog;
mod ci_continue_on_error;
mod ci_exists;
mod ci_green;
mod ci_job_timeout;
mod ci_scheduled_run;
mod ci_scoped;
mod codegen_drift;
mod codeowners;
mod codespell;
mod conventional_commits;
mod dependabot;
mod dependabot_automerge;
mod field_guide;
mod git_hooks;
mod gitignore;
mod license;
mod lockfiles;
mod pinned_actions;
mod pinned_dependencies;
mod project_inventory;
mod readme;
mod readme_quality;
mod repo_meta;
mod reproducible_toolchain;
mod required_checks;
mod required_dependencies;
mod ruleset_bypass;
mod scripts;
mod secret_scanning;
mod stale;
mod stray_files;
mod strict_status_checks;
mod stylelint;
mod test_inline;
mod test_layout;
mod test_mirror;
mod test_retry_masking;
mod typecheck;
mod vale;
mod website;
mod workflow_permissions;

const EXAMPLE_LIMIT: usize = 8;

/// Renders a bounded sample of findings so a message stays readable when a
/// repository has many of them.
fn examples(values: &[String]) -> String {
    let shown = values
        .iter()
        .take(EXAMPLE_LIMIT)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if values.len() > EXAMPLE_LIMIT {
        format!("{shown} (+{} more)", values.len() - EXAMPLE_LIMIT)
    } else {
        shown
    }
}
