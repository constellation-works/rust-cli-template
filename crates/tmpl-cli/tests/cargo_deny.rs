//! Integration coverage for the Makefile cargo-deny target.
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
    clippy::disallowed_types,
    reason = "a test sets up fixture files and runs the built binary; production code may not"
)]

mod support;

use support::CommandDeadline;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use support::script::repo_root;

#[test]
fn deny_uses_a_writable_cache_when_the_cargo_home_is_unusable() {
    // make runs in the kit's temp root and isolated environment.
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    for name in ["Makefile", "deny.toml"] {
        fs::copy(repo_root().join(name), root.join(name)).unwrap();
    }
    // The Makefile runs its Cargo targets only once a crate exists.
    fs::create_dir_all(root.join("crates/app")).unwrap();
    fs::write(root.join("crates/app/Cargo.toml"), "").unwrap();
    let cargo = root.join("cargo");
    fs::write(
        &cargo,
        r#"#!/bin/sh
set -eu
if [ "$1" = metadata ]; then
  printf '{"packages":[{"name":"app"}]}\n'
  exit 0
fi
test "$1" = deny
if [ "$2" = --version ]; then exit 0; fi
test "$2" = check
test "$3" = --config
grep -Fx 'db-path = "target/cargo-deny/advisory-dbs"' "$4"
grep -Fx 'yanked = "deny"' "$4"
grep -Fx '[licenses]' "$4"
"#,
    )
    .unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    let cargo_home = root.join("cargo-home");
    fs::write(&cargo_home, b"not a directory").unwrap();
    let out = temp
        .command("make")
        .args(["deny", "CARGO=./cargo"])
        .env("CARGO_HOME", cargo_home)
        .output_with_deadline()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
