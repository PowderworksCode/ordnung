// Tests for `src/render.rs`: how a report is put into words.
use ordnung_cli::render::{SeverityFloor, severity_name, status_name};
use ordnung_core::{CheckStatus, Severity};

#[test]
fn every_status_and_severity_has_a_name() {
    for status in [
        CheckStatus::Pass,
        CheckStatus::Fail,
        CheckStatus::Skip,
        CheckStatus::Error,
    ] {
        assert!(!status_name(status).is_empty());
    }
    for severity in [Severity::Required, Severity::Recommended, Severity::Off] {
        assert!(!severity_name(severity).is_empty());
    }
}

/// The names are what a reader greps for, so they are lowercase words rather
/// than the Rust spelling of the variant.
#[test]
fn the_names_are_the_words_the_output_uses() {
    assert_eq!(status_name(CheckStatus::Pass), "pass");
    assert_eq!(status_name(CheckStatus::Fail), "fail");
    assert_eq!(severity_name(Severity::Required), "required");
    assert_eq!(severity_name(Severity::Off), "off");
}

#[test]
fn the_floor_widens_from_required_to_all() {
    assert_ne!(SeverityFloor::Required, SeverityFloor::All);
    assert_ne!(SeverityFloor::Recommended, SeverityFloor::All);
}
