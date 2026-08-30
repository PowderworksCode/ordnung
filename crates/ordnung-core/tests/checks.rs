// The mirrored tests for `src/checks/`, one file per check module.
//
// A test target resolves `mod` against its own directory rather than a
// subdirectory named after it, so each module states its path. Cargo builds
// only top-level files under tests/ as targets, which is why they arrive
// through this one.
#[path = "checks/support.rs"]
mod support;

#[path = "checks/artifacts_built.rs"]
mod artifacts_built;
#[path = "checks/builds.rs"]
mod builds;
#[path = "checks/changelog.rs"]
mod changelog;
#[path = "checks/ci_continue_on_error.rs"]
mod ci_continue_on_error;
#[path = "checks/ci_exists.rs"]
mod ci_exists;
#[path = "checks/ci_job_timeout.rs"]
mod ci_job_timeout;
#[path = "checks/ci_matrix_scoped.rs"]
mod ci_matrix_scoped;
#[path = "checks/ci_scoped.rs"]
mod ci_scoped;
#[path = "checks/codegen_drift.rs"]
mod codegen_drift;
#[path = "checks/codeowners.rs"]
mod codeowners;
#[path = "checks/codespell.rs"]
mod codespell;
#[path = "checks/conventional_commits.rs"]
mod conventional_commits;
#[path = "checks/dependabot.rs"]
mod dependabot;
#[path = "checks/field_guide.rs"]
mod field_guide;
#[path = "checks/git_hooks.rs"]
mod git_hooks;
#[path = "checks/gitignore.rs"]
mod gitignore;
#[path = "checks/license.rs"]
mod license;
#[path = "checks/lockfiles.rs"]
mod lockfiles;
#[path = "checks/pinned_actions.rs"]
mod pinned_actions;
#[path = "checks/readme.rs"]
mod readme;
#[path = "checks/readme_quality.rs"]
mod readme_quality;
#[path = "checks/reproducible_toolchain.rs"]
mod reproducible_toolchain;
#[path = "checks/required_dependencies.rs"]
mod required_dependencies;
#[path = "checks/scripts.rs"]
mod scripts;
#[path = "checks/stray_files.rs"]
mod stray_files;
#[path = "checks/stylelint.rs"]
mod stylelint;
#[path = "checks/test_inline.rs"]
mod test_inline;
#[path = "checks/test_layout.rs"]
mod test_layout;
#[path = "checks/test_mirror.rs"]
mod test_mirror;
#[path = "checks/test_retry_masking.rs"]
mod test_retry_masking;
#[path = "checks/typecheck.rs"]
mod typecheck;
