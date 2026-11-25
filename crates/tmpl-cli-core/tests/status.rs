//! End-to-end tests of the status app against a real store (STD-02 R20), run
//! through the kit's chokepoint. Every test works in its own temporary directory
//! and never touches the real home (STD-03 R20).
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]
#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "a test sets up fixture files and runs the built binary; production code may not"
)]

use cw_base::Clock;
use std::path::Path;
use time::OffsetDateTime;
use tmpl_cli_core::app::status::{self, Command, Outcome};
use tmpl_cli_core::{Error, Origin, Runtime, Surface};
use tmpl_cli_store::Store;

/// Run `command` against the data directory `root` with the clock pinned.
fn run(root: &Path, command: Command) -> Result<Outcome, Error> {
    let runtime =
        Runtime::new(root, Store::open(root)).with_clock(Clock::fixed(OffsetDateTime::UNIX_EPOCH));
    let origin = Origin::new(Surface::CLI, None);
    let (outcome, warning) = status::exec(&runtime, &origin, command).settle();
    assert!(warning.is_none(), "status wrote an audit line: {warning:?}");
    outcome
}

#[test]
fn status_reports_a_missing_store_without_creating_it() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path().join("data");
    let outcome = run(&root, Command::Show).unwrap();
    assert_eq!(
        outcome,
        Outcome::Shown {
            root: root.clone(),
            format: None,
        }
    );
    assert!(!root.exists(), "status created the data directory");
}

#[cfg(unix)]
#[test]
fn status_refuses_a_data_directory_open_to_others() {
    use std::os::unix::fs::PermissionsExt;
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path().join("data");
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777)).unwrap();
    let err = run(&root, Command::Show).unwrap_err();
    assert_eq!(err.code(), "store_permissions", "got {err:?}");
}
