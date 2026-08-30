// Tests for `src/manifest.rs`.
//
// The manifest is the machine-readable list of what Ordnung grades. Consumers
// pin its schema, so its shape is a promise rather than an implementation
// detail. Its fields are private and it is read through serialisation, which is
// how a consumer reads it too.
use ordnung_cli::manifest::{Manifest, REMOVED, SCHEMA};

fn as_json() -> serde_json::Value {
    serde_json::to_value(Manifest::build()).expect("the manifest serialises")
}

#[test]
fn the_schema_is_stated_and_versioned() {
    assert_eq!(SCHEMA, "ordnung.checks/1");
    assert_eq!(as_json()["schema"], SCHEMA);
}

#[test]
fn the_manifest_lists_every_registered_check() {
    let json = as_json();
    let listed: Vec<&str> = json["checks"]
        .as_array()
        .expect("checks is an array")
        .iter()
        .map(|check| check["id"].as_str().expect("each check has an id"))
        .collect();
    for id in ordnung_core::check_ids() {
        assert!(listed.contains(&id), "manifest omits {id}");
    }
}

/// A check that goes away is named rather than silently dropped: a consumer
/// pinned to the schema needs to tell "renamed" from "never existed".
#[test]
fn removed_checks_are_named_and_never_also_registered() {
    let registered = ordnung_core::check_ids();
    for id in REMOVED {
        assert!(
            !registered.contains(id),
            "{id} is listed as removed and still registered"
        );
    }
}

/// The text rendering is what a person reads, and it names every check too.
#[test]
fn the_text_is_stable_and_complete() {
    let text = Manifest::build().to_text();
    assert_eq!(text, Manifest::build().to_text());
    for id in ordnung_core::check_ids() {
        assert!(text.contains(id), "text rendering omits {id}");
    }
}
