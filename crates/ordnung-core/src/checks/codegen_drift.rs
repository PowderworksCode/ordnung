use std::path::PathBuf;

use entl_github::WorkflowCommand;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};
use crate::config::CodegenConfig;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "codegen-drift",
    default_severity: Severity::Recommended,
    category: CheckCategory::BuildToolchain,
    instructions: "Declare each committed generator under [[codegen]] with its project root, command, and output patterns; run it in CI and follow it in the same job with git diff --exit-code or git diff --quiet.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if context.codegen.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no [[codegen]] entries declared",
        ));
        return;
    }

    for declaration in context.codegen {
        let (program, arguments) = declaration.normalized_command();
        let generators = context
            .inventory
            .github
            .workflows
            .iter()
            .flat_map(|workflow| &workflow.commands)
            .filter(|command| {
                command.program == program
                    && command.arguments == arguments
                    && applies_to(command, declaration)
            })
            .collect::<Vec<_>>();
        let guarded = generators.iter().any(|generator| {
            context
                .inventory
                .github
                .workflows
                .iter()
                .flat_map(|workflow| &workflow.commands)
                .any(|command| follows(command, generator) && is_zero_diff_guard(command))
        });
        let (status, message) = if generators.is_empty() {
            (
                CheckStatus::Fail,
                format!(
                    "{}: CI never runs {:?} for outputs {}",
                    declaration.name,
                    declaration.command,
                    declaration.outputs.join(", ")
                ),
            )
        } else if !guarded {
            (
                CheckStatus::Fail,
                format!(
                    "{}: regeneration runs without a subsequent zero-drift assertion in the same job",
                    declaration.name
                ),
            )
        } else {
            (
                CheckStatus::Pass,
                format!(
                    "{} regenerates {} and subsequently asserts zero drift",
                    declaration.name,
                    declaration.outputs.join(", ")
                ),
            )
        };
        results.push(result(
            definition,
            status,
            declaration.scope_root().to_path_buf(),
            message,
        ));
    }
}

fn applies_to(command: &WorkflowCommand, declaration: &CodegenConfig) -> bool {
    command.working_directory == declaration.scope_root()
        || command.package_roots.contains(declaration.scope_root())
}

fn follows(command: &WorkflowCommand, generator: &WorkflowCommand) -> bool {
    command.workflow == generator.workflow
        && command.job == generator.job
        && (command.step > generator.step
            || (command.step == generator.step && command.segment > generator.segment))
}

fn is_zero_diff_guard(command: &WorkflowCommand) -> bool {
    if command.program != "git" {
        return false;
    }
    match command.arguments.as_slice() {
        [operation, arguments @ ..] if operation == "diff" => arguments
            .iter()
            .any(|argument| matches!(argument.as_str(), "--exit-code" | "--quiet")),
        _ => false,
    }
}
