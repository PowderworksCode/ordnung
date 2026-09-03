// Tests for `src/lib.rs`: the CLI's public surface.
//
// main.rs is a thin front for these modules, so what lib.rs exposes is what the
// binary can do. A module dropped from here is a subcommand that stops working.

/// Naming them is the test: each line stops compiling if the module goes.
#[test]
fn every_module_the_binary_needs_is_exposed() {
    let _ = ordnung_cli::manifest::SCHEMA;
    let _ = ordnung_cli::render::status_name(ordnung_core::CheckStatus::Pass);
    let _: fn() -> ordnung_cli::manifest::Manifest = ordnung_cli::manifest::Manifest::build;
}
