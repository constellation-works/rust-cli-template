//! Integration coverage for the terminal ownership script.
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]
#![cfg(unix)]
#![expect(
    clippy::disallowed_methods,
    reason = "a test sets up fixture files and runs the built binary; production code may not"
)]

mod support;

use std::fs;
use support::script::copy_script;

#[test]
fn check_terminal_guard_catches_imported_stdout_and_stderr() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    let script = copy_script("check-terminal-guard.sh", root);

    fs::create_dir_all(root.join("crates/tmpl-cli/src")).unwrap();
    fs::write(
        root.join("crates/tmpl-cli/src/cli.rs"),
        "use std::io::{Write, stdout};\nfn write_out() { let _ = stdout(); }\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/tmpl-cli/src/error.rs"),
        "use std::io::{stderr as err_stream};\nfn write_err() { let _ = err_stream(); }\n",
    )
    .unwrap();

    let out = temp.command(&script).output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("crates/tmpl-cli/src/cli.rs"),
        "expected cli.rs in {stderr}"
    );
    assert!(
        stderr.contains("crates/tmpl-cli/src/error.rs"),
        "expected error.rs in {stderr}"
    );
}
