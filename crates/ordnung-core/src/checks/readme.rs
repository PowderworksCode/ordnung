use std::path::{Component, Path, PathBuf};

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

const MIN_WORDS: usize = 150;
const MAX_WORDS: usize = 1_500;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "readme",
    default_severity: Severity::Required,
    category: CheckCategory::Documentation,
    instructions: "Keep a root README with an H1 title in its first ten nonblank lines, between 150 and 1,500 words, install/getting-started, usage/docs, contributing, and license sections, and no broken repository-relative Markdown links.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let Some(path) = root_readme(&context.inventory.files) else {
        results.push(result(
            definition,
            CheckStatus::Fail,
            PathBuf::from("README.md"),
            "no root README found",
        ));
        return;
    };
    let text = match std::fs::read_to_string(context.root.join(path)) {
        Ok(text) => text,
        Err(error) => {
            results.push(result(
                definition,
                CheckStatus::Error,
                path.clone(),
                format!("could not read {}: {error}", path.display()),
            ));
            return;
        }
    };
    let facts = inspect(&text);
    let mut problems = Vec::new();
    if !facts.has_early_title {
        problems.push("no H1 title heading in the first 10 nonblank lines".to_owned());
    }
    if facts.words < MIN_WORDS {
        problems.push(format!("under {MIN_WORDS} words ({})", facts.words));
    }
    if facts.words > MAX_WORDS {
        problems.push(format!("over {MAX_WORDS} words ({})", facts.words));
    }
    if !facts.has_install {
        problems.push("no install/getting-started section".to_owned());
    }
    if !facts.has_usage {
        problems.push("no usage/docs section".to_owned());
    }
    if !facts.has_license {
        problems.push("no License section heading".to_owned());
    }
    if !facts.has_contributing {
        problems.push("no Contributing section heading".to_owned());
    }
    let broken = facts
        .relative_links
        .iter()
        .filter(|target| !relative_target_exists(target, &context.inventory.files))
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    if !broken.is_empty() {
        problems.push(format!("broken relative links: {}", broken.join(", ")));
    }

    results.push(result(
        definition,
        if problems.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        path.clone(),
        if problems.is_empty() {
            format!("{} passes the README floor", path.display())
        } else {
            problems.join("; ")
        },
    ));
}

struct ReadmeFacts {
    words: usize,
    has_early_title: bool,
    has_install: bool,
    has_usage: bool,
    has_license: bool,
    has_contributing: bool,
    relative_links: Vec<String>,
}

struct Heading {
    level: HeadingLevel,
    start: usize,
    text: String,
}

fn inspect(text: &str) -> ReadmeFacts {
    let mut headings = Vec::new();
    let mut current_heading: Option<Heading> = None;
    let mut relative_links = Vec::new();
    for (event, range) in Parser::new_ext(text, Options::ENABLE_GFM).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some(Heading {
                    level,
                    start: range.start,
                    text: String::new(),
                });
            }
            Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                if let Some(heading) = current_heading.take() {
                    headings.push(heading);
                }
            }
            Event::Text(value) | Event::Code(value) => {
                if let Some(heading) = &mut current_heading {
                    if !heading.text.is_empty() {
                        heading.text.push(' ');
                    }
                    heading.text.push_str(&value);
                }
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                let target = dest_url.trim();
                if is_repository_relative(target) {
                    relative_links.push(target.to_owned());
                }
            }
            _ => {}
        }
    }
    relative_links.sort();
    relative_links.dedup();
    let early_cutoff = first_nonblank_lines_end(text, 10);
    let heading_text = headings
        .iter()
        .map(|heading| heading.text.to_ascii_lowercase())
        .collect::<Vec<_>>();
    ReadmeFacts {
        words: text.split_whitespace().count(),
        has_early_title: headings
            .iter()
            .any(|heading| heading.level == HeadingLevel::H1 && heading.start < early_cutoff),
        has_install: heading_text.iter().any(|heading| {
            contains_word(heading, "install")
                || contains_word(heading, "setup")
                || heading.contains("getting started")
                || heading.contains("quick start")
                || heading.contains("quickstart")
        }),
        has_usage: heading_text.iter().any(|heading| {
            [
                "usage",
                "use",
                "example",
                "examples",
                "how",
                "docs",
                "documentation",
            ]
            .iter()
            .any(|word| contains_word(heading, word))
        }),
        has_license: heading_text.iter().any(|heading| {
            words(heading).any(|word| word.starts_with("licens") || word.starts_with("licenc"))
        }),
        has_contributing: heading_text
            .iter()
            .any(|heading| words(heading).any(|word| word.starts_with("contribut"))),
        relative_links,
    }
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

fn first_nonblank_lines_end(text: &str, limit: usize) -> usize {
    let mut nonblank = 0;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        offset += line.len();
        if !line.trim().is_empty() {
            nonblank += 1;
            if nonblank == limit {
                return offset;
            }
        }
    }
    text.len().saturating_add(1)
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

fn contains_word(text: &str, expected: &str) -> bool {
    words(text).any(|word| word == expected)
}

fn is_repository_relative(target: &str) -> bool {
    if target.is_empty() || target.starts_with('#') || target.starts_with("//") {
        return false;
    }
    !target.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
    })
}

fn relative_target_exists(target: &str, files: &std::collections::BTreeSet<PathBuf>) -> bool {
    let target = target.split(['#', '?']).next().unwrap_or_default().trim();
    if target.is_empty() {
        return true;
    }
    let path = Path::new(target);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return false;
    }
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect::<PathBuf>();
    files.contains(&normalized)
        || files
            .iter()
            .any(|candidate| candidate.starts_with(&normalized))
}
