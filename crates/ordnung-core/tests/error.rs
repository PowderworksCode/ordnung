// Tests for `src/error.rs`.
//
// An error here is read by someone at a terminal deciding what to fix, so what
// it says is part of the interface: every variant names the file it is about.
use std::path::PathBuf;

use ordnung_core::error::Error;

#[test]
fn an_io_error_names_the_path_and_the_cause() {
    let error = Error::Io {
        path: PathBuf::from("fleet.toml"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "no such file"),
    };
    let text = error.to_string();
    assert!(text.contains("fleet.toml"), "{text}");
    assert!(text.contains("no such file"), "{text}");
}

#[test]
fn a_parse_error_names_the_path_and_the_message() {
    let error = Error::Parse {
        path: PathBuf::from(".ordnung/overrides.toml"),
        message: "expected a table".into(),
    };
    let text = error.to_string();
    assert!(text.contains(".ordnung/overrides.toml"), "{text}");
    assert!(text.contains("expected a table"), "{text}");
}

/// A configuration error has no path: it is about what the configuration says
/// rather than where it lives, and the message carries the whole of it.
#[test]
fn a_config_error_is_its_message() {
    let error = Error::Config("managed entry has no destination".into());
    assert!(
        error
            .to_string()
            .contains("managed entry has no destination"),
        "{error}"
    );
}

#[test]
fn errors_implement_the_std_trait() {
    fn assert_error<E: std::error::Error>(_: &E) {}
    assert_error(&Error::Config("x".into()));
}
