use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use ordnung_core::fleet::RelativeTo;
use ordnung_core::{
    FileChangeSource, FleetConfig, Inventory, InventoryOptions, RemediationPlan, RepoConfig,
    Report, apply_file_changes, build_remediation_plan, default_policy, inspect_repository,
    plan_github_settings, resolve_github_settings, resolve_policy, run_github_checks_with_settings,
    run_repository_checks_for_state, run_repository_checks_with_repo_config,
};
use serde::Serialize;

use ordnung_cli::gh::GhClient;
use ordnung_cli::instructions::{InstructionContext, inject, render};
use ordnung_cli::render::{
    display_scope, file_operation_name, retain_reported, severity_name, status_name,
};
use ordnung_cli::sync::{
    GithubSyncOutcome, check_fleet_members, ensure_explicit_member, plan_fleet_member_settings,
    plan_local_sync, sync_fleet_member, sync_fleet_members,
};

#[derive(Debug, Parser)]
#[command(
    name = "ordnung",
    version,
    about = "Keep repositories structurally in order"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Serialize)]
struct JsonEnvelope<'a, T> {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    data: &'a T,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inventory supported projects in a repository.
    Inspect(RepositoryArgs),
    /// Run structural checks against a repository.
    Check(CheckArgs),
    /// Run local and GitHub-backed checks as one repository audit.
    RepoCheck(RepoCheckArgs),
    /// Plan or apply exact, non-guessing repository fixes.
    Fix(RepositoryMutationArgs),
    /// Print or inject concise repository rules for coding agents.
    Instructions(InstructionsArgs),
    /// Validate and apply centralized fleet configuration.
    Fleet(FleetArgs),
    /// Inspect GitHub repository settings through the gh CLI.
    Github(GithubArgs),
}

#[derive(Debug, Args)]
struct RepositoryArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct CheckArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    json: bool,
    /// Include checks the effective policy has switched off.
    #[arg(long)]
    all: bool,
}

#[derive(Debug, Args)]
struct RepositoryMutationArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    apply: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RepoCheckArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    repo: String,
    #[arg(long)]
    json: bool,
    /// Include checks the effective policy has switched off.
    #[arg(long)]
    all: bool,
}

#[derive(Debug, Args)]
struct InstructionsArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Fleet manifest supplying centralized policy for this repository.
    #[arg(long, requires = "repo")]
    fleet: Option<PathBuf>,
    /// Fleet repository name in owner/name form.
    #[arg(long, requires = "fleet")]
    repo: Option<String>,
    /// Inject the generated marker block into a repository-relative Markdown file.
    #[arg(long, value_name = "FILE")]
    write: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct FleetArgs {
    #[command(subcommand)]
    command: FleetCommand,
}

#[derive(Debug, Args)]
struct GithubArgs {
    #[command(subcommand)]
    command: GithubCommand,
}

#[derive(Debug, Subcommand)]
enum GithubCommand {
    /// Fetch typed repository, branch, security, and workflow facts.
    Inspect {
        repo: String,
        #[arg(long)]
        json: bool,
    },
    /// Run GitHub-backed checks against one repository.
    Check {
        repo: String,
        #[arg(long)]
        repo_root: Option<PathBuf>,
        #[arg(long)]
        json: bool,
        /// Include checks the effective policy has switched off.
        #[arg(long)]
        all: bool,
    },
    /// Plan or explicitly apply repository-level GitHub settings.
    SyncSettings {
        repo: String,
        #[arg(long)]
        repo_root: Option<PathBuf>,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum FleetCommand {
    /// Validate a fleet manifest and its canonical sources.
    Check {
        fleet_toml: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Plan or apply managed configuration to one local fleet member.
    Sync {
        fleet_toml: PathBuf,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Audit GitHub facts for every explicit fleet member.
    GithubCheck {
        fleet_toml: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Plan or apply fleet GitHub settings for one explicit member.
    GithubSyncSettings {
        fleet_toml: PathBuf,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Plan or apply one consolidated remediation pull request.
    GithubSync {
        fleet_toml: PathBuf,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Plan or apply remediation for every explicit fleet member.
    GithubSyncAll {
        fleet_toml: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
}

/// Rust masks `SIGPIPE` at startup so a closed pipe surfaces as a write error
/// instead of killing the process. Nothing here handles that error — `println!`
/// panics on it — so `ordnung check | head` died with a panic message and exit
/// 101. Restoring the default disposition makes a closed pipe end the process
/// quietly, the way every other command-line tool behaves.
#[cfg(unix)]
fn restore_default_sigpipe() {
    // SAFETY: called once, on the main thread, before any output or any thread is
    // spawned. Resetting a disposition to `SIG_DFL` touches no memory Rust owns.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn restore_default_sigpipe() {}

fn main() -> ExitCode {
    restore_default_sigpipe();
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    match Cli::parse().command {
        Command::Inspect(args) => inspect(args),
        Command::Check(args) => check(args),
        Command::RepoCheck(args) => repo_check(args),
        Command::Fix(args) => fix(args),
        Command::Instructions(args) => instructions(args),
        Command::Fleet(args) => fleet(args.command),
        Command::Github(args) => github(args.command),
    }
}

fn instructions(args: InstructionsArgs) -> Result<ExitCode> {
    let local = RepoConfig::load_optional(&args.path)?;
    let inventory = inspect_repository(
        &args.path,
        &InventoryOptions {
            ignore: local.ignore.clone(),
        },
    )?;
    let fleet = args.fleet.as_deref().map(FleetConfig::load).transpose()?;
    if let (Some(fleet), Some(repo)) = (&fleet, &args.repo) {
        if !fleet.members.iter().any(|member| member.repo == *repo) {
            bail!(
                "repository {repo:?} is not an explicit member of fleet {:?}",
                fleet.name
            );
        }
    }
    // A member's stage can relax checks, so policy is resolved per member.
    let member_checks = match (&fleet, &args.repo) {
        (Some(fleet), Some(repo)) => Some(fleet.checks_for(repo)),
        _ => None,
    };
    let policy = resolve_policy(&default_policy(), member_checks.as_ref(), &local)?;
    let github = resolve_github_settings(fleet.as_ref().map(|fleet| &fleet.policy.github), &local)?;
    let managed = fleet
        .iter()
        .flat_map(|fleet| &fleet.managed)
        .filter(|entry| {
            entry.only.is_empty()
                || args
                    .repo
                    .as_ref()
                    .is_some_and(|repo| entry.only.contains(repo))
        })
        .filter(|entry| {
            entry.relative_to == RelativeTo::Repo
                || entry.when.as_ref().is_some_and(|selector| {
                    inventory
                        .projects
                        .iter()
                        .any(|project| selector.matches(project))
                })
        })
        .collect::<Vec<_>>();
    let stage = match (&fleet, &args.repo) {
        (Some(fleet), Some(repo)) => Some(fleet.stage_of(repo).as_str()),
        _ => None,
    };
    let generated = render(&InstructionContext {
        inventory: &inventory,
        stage,
        policy: &policy,
        github: &github,
        test_layout: &local.test_layout,
        local: &local,
        fleet_name: fleet.as_ref().map(|fleet| fleet.name.as_str()),
        managed: &managed,
    });

    if args.write.is_empty() {
        println!("{generated}");
    } else {
        for destination in &args.write {
            write_instructions(&inventory.root, destination, &generated)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn write_instructions(repo_root: &Path, destination: &Path, generated: &str) -> Result<()> {
    if destination.as_os_str().is_empty()
        || destination.is_absolute()
        || destination
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        bail!(
            "instruction destination {} must be a safe repository-relative path",
            destination.display()
        );
    }
    let path = repo_root.join(destination);
    ensure_no_symlink_path(repo_root, destination)?;
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if !metadata.is_file() {
            bail!("instruction destination {} must be a file", path.display());
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let existing = if path.is_file() {
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?
    } else {
        String::new()
    };
    let updated = inject(&existing, generated).map_err(anyhow::Error::msg)?;
    if updated == existing {
        println!("current {}", destination.display());
    } else {
        fs::write(&path, updated).with_context(|| format!("cannot write {}", path.display()))?;
        println!("updated {}", destination.display());
    }
    Ok(())
}

fn ensure_no_symlink_path(repo_root: &Path, destination: &Path) -> Result<()> {
    let mut current = repo_root.to_path_buf();
    for component in destination.components() {
        let Component::Normal(component) = component else {
            continue;
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!(
                    "instruction destination traverses symlink {}",
                    current.display()
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("cannot inspect {}", current.display()));
            }
        }
    }
    Ok(())
}

fn inspect(args: RepositoryArgs) -> Result<ExitCode> {
    let config = RepoConfig::load_optional(&args.path)?;
    let inventory = inspect_repository(
        &args.path,
        &InventoryOptions {
            ignore: config.ignore,
        },
    )?;

    if args.json {
        print_json("inspect", true, &inventory)?;
    } else {
        print_inventory(&inventory);
    }
    Ok(ExitCode::SUCCESS)
}

fn check(args: CheckArgs) -> Result<ExitCode> {
    let config = RepoConfig::load_optional(&args.path)?;
    let policy = resolve_policy(&default_policy(), None, &config)?;
    let inventory = inspect_repository(
        &args.path,
        &InventoryOptions {
            ignore: config.ignore.clone(),
        },
    )?;
    let mut report = run_repository_checks_with_repo_config(&args.path, &inventory, &config);
    report.apply_policy(&policy);

    let clean = report.is_clean();
    retain_reported(&mut report, args.all);
    if args.json {
        print_json("check", clean, &report)?;
    } else {
        print_report(&report);
    }

    Ok(if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

#[derive(Serialize)]
struct RepoCheckOutcome {
    repository: String,
    local: Report,
    github: Report,
}

fn repo_check(args: RepoCheckArgs) -> Result<ExitCode> {
    let config = RepoConfig::load_optional(&args.path)?;
    let policy = resolve_policy(&default_policy(), None, &config)?;
    let settings = resolve_github_settings(None, &config)?;
    let inventory = inspect_repository(
        &args.path,
        &InventoryOptions {
            ignore: config.ignore.clone(),
        },
    )?;
    // Fetched first: whether the repository is archived decides whether any local
    // finding is actionable, and archived state is only visible from GitHub.
    let facts = GhClient::new().fetch_repository(&args.repo)?;
    let mut local = run_repository_checks_for_state(
        &args.path,
        &inventory,
        &config,
        &config.dependencies,
        facts.archived,
    );
    local.apply_policy(&policy);

    let mut github = run_github_checks_with_settings(&facts, &settings);
    github.apply_policy(&policy);
    let clean = local.is_clean() && github.is_clean();
    retain_reported(&mut local, args.all);
    retain_reported(&mut github, args.all);
    let outcome = RepoCheckOutcome {
        repository: facts.repository,
        local,
        github,
    };

    if args.json {
        print_json("repo-check", clean, &outcome)?;
    } else {
        println!("local checks");
        print_report(&outcome.local);
        println!("\nGitHub checks");
        print_report(&outcome.github);
    }
    Ok(if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn fix(args: RepositoryMutationArgs) -> Result<ExitCode> {
    let config = RepoConfig::load_optional(&args.path)?;
    let policy = resolve_policy(&default_policy(), None, &config)?;
    let inventory = inspect_repository(
        &args.path,
        &InventoryOptions {
            ignore: config.ignore.clone(),
        },
    )?;
    let mut report = run_repository_checks_with_repo_config(&args.path, &inventory, &config);
    report.apply_policy(&policy);
    let plan = build_remediation_plan(
        inventory.root.display().to_string(),
        &[report],
        &[],
        Vec::new(),
    )?;

    let clean = if args.apply {
        apply_file_changes(&inventory.root, &plan.file_changes)?;
        let inventory = inspect_repository(
            &args.path,
            &InventoryOptions {
                ignore: config.ignore.clone(),
            },
        )?;
        let mut report = run_repository_checks_with_repo_config(&args.path, &inventory, &config);
        report.apply_policy(&policy);
        report.is_clean()
    } else {
        plan.is_clean()
    };
    print_remediation_plan(&plan, args.apply, args.json, "fix", clean)?;
    Ok(if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn fleet(command: FleetCommand) -> Result<ExitCode> {
    match command {
        FleetCommand::Check { fleet_toml, json } => {
            let config = FleetConfig::load(&fleet_toml)?;
            resolve_policy(
                &default_policy(),
                Some(&config.policy.checks),
                &RepoConfig::default(),
            )?;
            if json {
                print_json("fleet-check", true, &config)?;
            } else {
                println!("fleet: {}", config.name);
                println!("members: {}", config.members.len());
                println!("policies: {}", config.policy.checks.len());
                println!(
                    "GitHub settings: {}",
                    [
                        config.policy.github.allow_auto_merge.is_some(),
                        config.policy.github.delete_branch_on_merge.is_some(),
                        config.policy.github.allow_update_branch.is_some(),
                    ]
                    .into_iter()
                    .filter(|configured| *configured)
                    .count()
                );
                println!("managed entries: {}", config.effective_managed().len());
                println!(
                    "dependency requirements: {}",
                    config.effective_dependencies().len()
                );
                let stages = config
                    .stage_counts()
                    .into_iter()
                    .map(|(stage, count)| format!("{stage} {count}"))
                    .collect::<Vec<_>>();
                println!("stages: {}", stages.join(", "));
            }
            Ok(ExitCode::SUCCESS)
        }
        FleetCommand::Sync {
            fleet_toml,
            repo,
            repo_root,
            apply,
            json,
        } => sync_fleet(&fleet_toml, &repo, &repo_root, apply, json),
        FleetCommand::GithubCheck { fleet_toml, json } => github_check_fleet(&fleet_toml, json),
        FleetCommand::GithubSyncSettings {
            fleet_toml,
            repo,
            apply,
            json,
        } => github_sync_fleet_settings(&fleet_toml, &repo, apply, json),
        FleetCommand::GithubSync {
            fleet_toml,
            repo,
            apply,
            json,
        } => github_sync_fleet(&fleet_toml, &repo, apply, json),
        FleetCommand::GithubSyncAll {
            fleet_toml,
            apply,
            json,
        } => github_sync_fleet_all(&fleet_toml, apply, json),
    }
}

fn github(command: GithubCommand) -> Result<ExitCode> {
    let client = GhClient::new();
    match command {
        GithubCommand::Inspect { repo, json } => {
            let facts = client.fetch_repository(&repo)?;
            if json {
                print_json("github-inspect", true, &facts)?;
            } else {
                println!("repository: {}", facts.repository);
                println!("default branch: {}", facts.default_branch);
                println!("visibility: {}", facts.visibility);
                println!("protected: {}", facts.branch.protected);
                match &facts.branch.required_checks {
                    ordnung_core::GithubValue::Known { value } => {
                        println!("required checks: {}", value.len())
                    }
                    ordnung_core::GithubValue::Unavailable { reason } => {
                        println!("required checks: unavailable ({reason})")
                    }
                }
                println!("workflows: {}", facts.workflows.len());
            }
            Ok(ExitCode::SUCCESS)
        }
        GithubCommand::Check {
            repo,
            repo_root,
            json,
            all,
        } => {
            let facts = client.fetch_repository(&repo)?;
            let config = match repo_root {
                Some(root) => RepoConfig::load_optional(&root)?,
                None => client.fetch_repo_config(&facts)?,
            };
            let policy = resolve_policy(&default_policy(), None, &config)?;
            let settings = resolve_github_settings(None, &config)?;
            let mut report = run_github_checks_with_settings(&facts, &settings);
            report.apply_policy(&policy);
            let clean = report.is_clean();
            retain_reported(&mut report, all);
            print_or_serialize_report(&report, json, "github-check", clean)?;
            Ok(if clean {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        GithubCommand::SyncSettings {
            repo,
            repo_root,
            apply,
            json,
        } => {
            let facts = client.fetch_repository(&repo)?;
            if facts.archived {
                bail!(
                    "repository {:?} is archived and cannot be changed",
                    facts.repository
                );
            }
            let config = match repo_root {
                Some(root) => RepoConfig::load_optional(&root)?,
                None => client.fetch_repo_config(&facts)?,
            };
            let desired = resolve_github_settings(None, &config)?;
            let changes = plan_github_settings(&facts, &desired);
            if apply {
                client.apply_setting_changes(&facts.repository, &changes)?;
            }
            let settled = changes.is_empty() || apply;
            print_github_setting_changes(&changes, apply, json, "github-sync-settings", settled)?;
            Ok(if settled {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}

fn github_check_fleet(fleet_toml: &Path, json: bool) -> Result<ExitCode> {
    let fleet = FleetConfig::load(fleet_toml)?;
    let outcomes = check_fleet_members(&GhClient::new(), &fleet);

    let clean = outcomes.iter().all(|outcome| {
        outcome.error.is_none()
            && outcome
                .report
                .as_ref()
                .is_some_and(ordnung_core::Report::is_clean)
    });
    let has_errors = outcomes.iter().any(|outcome| outcome.error.is_some());
    if json {
        print_json("fleet-github-check", clean, &outcomes)?;
    } else {
        for outcome in &outcomes {
            println!("\n{}", outcome.repository);
            if let Some(error) = &outcome.error {
                println!("error: {error}");
            } else if let Some(report) = &outcome.report {
                print_report(report);
            }
        }
    }
    Ok(if has_errors {
        ExitCode::from(2)
    } else if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn github_sync_fleet_settings(
    fleet_toml: &Path,
    repository: &str,
    apply: bool,
    json: bool,
) -> Result<ExitCode> {
    let fleet = FleetConfig::load(fleet_toml)?;
    ensure_explicit_member(&fleet, repository)?;
    let client = GhClient::new();
    let (facts, changes) = plan_fleet_member_settings(&client, &fleet, repository)?;
    if apply {
        client.apply_setting_changes(&facts.repository, &changes)?;
    }
    let settled = changes.is_empty() || apply;
    print_github_setting_changes(&changes, apply, json, "fleet-github-sync-settings", settled)?;
    Ok(if settled {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn github_sync_fleet(
    fleet_toml: &Path,
    repository: &str,
    apply: bool,
    json: bool,
) -> Result<ExitCode> {
    let fleet = FleetConfig::load(fleet_toml)?;
    ensure_explicit_member(&fleet, repository)?;
    let outcome = sync_fleet_member(&GhClient::new(), &fleet, repository, apply)?;
    if json {
        print_json("fleet-github-sync", outcome.ok, &outcome)?;
    } else {
        print_github_sync_outcome(&outcome)?;
    }
    Ok(if outcome.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn github_sync_fleet_all(fleet_toml: &Path, apply: bool, json: bool) -> Result<ExitCode> {
    let fleet = FleetConfig::load(fleet_toml)?;
    let members = sync_fleet_members(&GhClient::new(), &fleet, apply);
    let has_errors = members.iter().any(|member| member.error.is_some());
    let clean = members.iter().all(|member| {
        member.error.is_none() && member.outcome.as_ref().is_some_and(|outcome| outcome.ok)
    });
    if json {
        print_json("fleet-github-sync-all", clean, &members)?;
    } else {
        for member in &members {
            println!("\n{}", member.repository);
            if let Some(error) = &member.error {
                println!("error: {error}");
            } else if let Some(outcome) = &member.outcome {
                print_github_sync_outcome(outcome)?;
            }
        }
    }
    Ok(if has_errors {
        ExitCode::from(2)
    } else if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn print_github_sync_outcome(outcome: &GithubSyncOutcome) -> Result<()> {
    print_remediation_plan(
        &outcome.plan,
        outcome.applied,
        false,
        "fleet-github-sync",
        outcome.ok,
    )?;
    if let Some(pull_request) = &outcome.pull_request {
        println!(
            "{} pull request #{}: {}",
            match pull_request.status {
                ordnung_cli::gh::PullRequestStatus::Created => "created",
                ordnung_cli::gh::PullRequestStatus::Updated => "updated",
                ordnung_cli::gh::PullRequestStatus::Unchanged => "reused",
            },
            pull_request.number,
            pull_request.url
        );
    }
    Ok(())
}

fn sync_fleet(
    fleet_toml: &Path,
    repo: &str,
    repo_root: &Path,
    apply: bool,
    json: bool,
) -> Result<ExitCode> {
    let config = FleetConfig::load(fleet_toml)?;
    ensure_explicit_member(&config, repo)?;
    let plan = plan_local_sync(&config, repo, repo_root)?;

    if apply {
        apply_file_changes(repo_root, &plan.file_changes)?;
    }
    let settled = plan.file_changes.is_empty() || apply;
    print_remediation_plan(&plan, apply, json, "fleet-sync", settled)?;
    Ok(if settled {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn print_inventory(inventory: &Inventory) {
    println!("repository: {}", inventory.root.display());
    println!(
        "github actions: {}",
        if inventory.github.has_workflows() {
            "present"
        } else {
            "absent"
        }
    );
    for package in &inventory.packages {
        let workspace = package
            .workspace_root
            .as_deref()
            .map(display_scope)
            .unwrap_or_else(|| "standalone".into());
        let lockfile = package
            .lockfile
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "missing".into());
        println!(
            "package {}: ecosystem {}, manifest {}, workspace {}, lockfile owner {}, lockfile {}",
            display_scope(&package.root),
            package.ecosystem,
            package.manifest.display(),
            workspace,
            display_scope(&package.lockfile_owner),
            lockfile
        );
    }
    for project in &inventory.projects {
        let languages = project
            .languages
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let capabilities = project
            .capabilities
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let ecosystems = project
            .ecosystems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let evidence = project
            .evidence
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "project {}: languages [{languages}], capabilities [{capabilities}], ecosystems [{ecosystems}] [{evidence}]",
            display_scope(&project.root)
        );
    }
    for issue in &inventory.issues {
        println!("issue {}: {}", issue.path.display(), issue.message);
    }
}

fn print_or_serialize_report(
    report: &Report,
    json: bool,
    command: &'static str,
    ok: bool,
) -> Result<()> {
    if json {
        print_json(command, ok, report)?;
    } else {
        print_report(report);
    }
    Ok(())
}

fn print_report(report: &Report) {
    for result in &report.results {
        println!(
            "{:<5} {:<11} {:<22} {}: {}",
            status_name(result.status),
            severity_name(result.severity),
            result.check,
            display_scope(&result.scope),
            result.message
        );
    }
}

fn print_github_setting_changes(
    changes: &[ordnung_core::GithubSettingChange],
    apply: bool,
    json: bool,
    command: &'static str,
    ok: bool,
) -> Result<()> {
    if json {
        print_json(command, ok, &changes)?;
    } else if changes.is_empty() {
        println!("GitHub repository settings are in order");
    } else {
        for change in changes {
            println!(
                "set {:<24} {} -> {}",
                change.setting.as_str(),
                change.current,
                change.desired
            );
        }
        println!(
            "{} {} GitHub setting change(s)",
            if apply { "applied" } else { "planned" },
            changes.len()
        );
    }
    Ok(())
}

fn print_remediation_plan(
    plan: &RemediationPlan,
    apply: bool,
    json: bool,
    command: &'static str,
    ok: bool,
) -> Result<()> {
    if json {
        print_json(command, ok, plan)?;
        return Ok(());
    }
    if plan.file_changes.is_empty() && plan.github_setting_changes.is_empty() {
        println!("no automatic remediations are available");
        return Ok(());
    }

    for change in &plan.file_changes {
        let sources = change
            .sources
            .iter()
            .map(|source| match source {
                FileChangeSource::Check { check } => format!("check:{check}"),
                FileChangeSource::Managed { entry } => format!("managed:{entry}"),
            })
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{:<6} {:<30} {}",
            file_operation_name(change.operation),
            sources,
            change.path.display()
        );
    }
    for change in &plan.github_setting_changes {
        println!(
            "set    {:<30} {} -> {}",
            change.setting.as_str(),
            change.current,
            change.desired
        );
    }
    println!(
        "{} {} file change(s) and {} GitHub setting change(s)",
        if apply { "applied" } else { "planned" },
        plan.file_changes.len(),
        plan.github_setting_changes.len()
    );
    Ok(())
}

fn print_json<T: Serialize>(command: &'static str, ok: bool, data: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&JsonEnvelope {
            schema_version: 1,
            command,
            ok,
            data,
        })?
    );
    Ok(())
}
