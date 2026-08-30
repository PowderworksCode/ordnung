// Tests for `src/lib.rs`: the crate's public surface.
//
// lib.rs declares the modules and re-exports the names a consumer is expected
// to reach for. Both are a promise: ordnung-cli and any other consumer bind to
// what is named here, so a module made private or a re-export dropped is a
// breaking change that would otherwise show up as a compile error somewhere
// else entirely. Naming them is the test — each line stops compiling if the
// name goes.
use std::path::Path;

use ordnung_core::{
    CheckStatus, InventoryOptions, RepoConfig, Severity, check_ids, default_policy,
    inspect_repository,
};

#[test]
fn the_modules_are_public() {
    let _: fn(&Path, &InventoryOptions) -> _ = inspect_repository;
    let _ = ordnung_core::config::RepoConfig::default();
    let _ = ordnung_core::fleet::CONFIG_DIR;
    let _ = ordnung_core::check::CheckCategory::ALL;
}

/// The flat re-exports are the surface most consumers use; reaching a name both
/// ways is what makes moving a type between modules a non-event.
///
/// Binding one to the other's type is the assertion: it compiles only while the
/// flat name and the nested one name the same type.
#[test]
fn the_flat_re_exports_reach_the_same_types() {
    let nested: ordnung_core::config::RepoConfig = RepoConfig::default();
    let _flat: RepoConfig = nested;
    assert_eq!(Severity::Required, ordnung_core::check::Severity::Required);
    assert_eq!(CheckStatus::Pass, ordnung_core::check::CheckStatus::Pass);
}

/// The registry is reachable from the crate root, and it is not empty. An empty
/// one would mean the inventory collection silently failed to link, which grades
/// every repository as clean.
#[test]
fn the_check_registry_is_reachable_and_populated() {
    assert!(!check_ids().is_empty());
    assert!(!default_policy().is_empty());
}
