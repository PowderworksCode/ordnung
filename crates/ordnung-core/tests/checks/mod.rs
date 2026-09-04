// Tests for `src/checks/mod.rs`: the module that declares every check.
//
// The declarations there and the registry are two lists of the same thing kept
// by hand, and a check that is written but never declared is invisible — it
// compiles, ships, and grades nothing.
use ordnung_core::check_ids;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn checks_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/checks")
}

/// Module names, as check ids would spell them.
fn declared() -> BTreeSet<String> {
    std::fs::read_to_string(checks_dir().join("mod.rs"))
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("mod ")?.strip_suffix(';'))
        .map(|module| module.replace('_', "-"))
        .collect()
}

/// Not every module under checks/ is a check. `test_layout` is the shared
/// machinery test-inline and test-mirror both read, and it registers nothing.
fn registers_a_check(module: &str) -> bool {
    let path = checks_dir().join(format!("{}.rs", module.replace('-', "_")));
    std::fs::read_to_string(path)
        .map(|source| source.contains("registry::submit!"))
        .unwrap_or(false)
}

#[test]
fn every_module_that_registers_a_check_is_reachable_by_its_id() {
    let registered: BTreeSet<String> = check_ids().into_iter().map(str::to_owned).collect();
    let silent: Vec<_> = declared()
        .into_iter()
        .filter(|module| registers_a_check(module) && !registered.contains(module))
        .collect();
    assert!(
        silent.is_empty(),
        "these modules submit a registration the registry does not carry: {silent:?}"
    );
}

/// The other direction: a registered check with no module named for it means
/// the convention has drifted, and the next reader looks in the wrong file.
#[test]
fn every_registered_check_has_a_module_named_for_it() {
    let declared = declared();
    let orphans: Vec<_> = check_ids()
        .into_iter()
        .filter(|id| !declared.contains(*id))
        .collect();
    assert!(
        orphans.is_empty(),
        "these checks have no module named for them: {orphans:?}"
    );
}

/// Shared machinery is allowed, but it should stay rare enough to name.
#[test]
fn the_only_module_that_is_not_a_check_is_the_shared_test_layout() {
    let helpers: Vec<_> = declared()
        .into_iter()
        .filter(|module| !registers_a_check(module))
        .collect();
    assert_eq!(helpers, vec!["test-layout".to_string()]);
}
