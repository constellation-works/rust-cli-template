//! End-to-end tests of the status app against a real store (STD-02 R20), run
//! through `test-support`. Every test works in its own temporary directory
//! and never touches the real home (STD-03 R20).
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]

use time::OffsetDateTime;
use tmpl_cli_core::app::status::{Command, Outcome};
use tmpl_cli_core::test_support::{Ctx, status_run};
use tmpl_cli_store::Store;

#[test]
fn status_reports_a_missing_store_without_creating_it() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path().join("data");
    let store = Store::open(&root);
    let clock = || OffsetDateTime::UNIX_EPOCH;
    let outcome = status_run(&Ctx::new(&store, None, &clock), Command::Show).unwrap();
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
    let store = Store::open(&root);
    let clock = || OffsetDateTime::UNIX_EPOCH;
    let err = status_run(&Ctx::new(&store, None, &clock), Command::Show).unwrap_err();
    assert_eq!(err.code(), "store_permissions", "got {err:?}");
}
