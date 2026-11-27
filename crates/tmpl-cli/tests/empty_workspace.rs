//! Integration coverage for empty-workspace gate behavior.
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
use support::script::{copy_script, isolated_path, repo_root};

#[test]
fn empty_workspace_exemption_decides_from_cargo_metadata_not_crates_glob() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    let path_env = isolated_path(&temp);

    fs::copy(repo_root().join("Makefile"), root.join("Makefile")).unwrap();
    let arch_script = copy_script("check-architecture-map.sh", root);
    let dep_script = copy_script("check-dependency-direction.sh", root);
    copy_script("check-module-cycles.sh", root);

    // A crate outside `crates/*/Cargo.toml` must NOT trigger the empty-workspace skip.
    fs::create_dir_all(root.join("packages/demo/src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"packages/demo\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    fs::write(
        root.join("packages/demo/Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(
        root.join("packages/demo/src/lib.rs"),
        "pub fn   unformatted( ){}\n",
    )
    .unwrap();
    fs::write(root.join("ARCHITECTURE.md"), "# Architecture\n").unwrap();

    let arch_out = temp
        .command(&arch_script)
        .env("PATH", &path_env)
        .output()
        .unwrap();
    let arch_err = String::from_utf8_lossy(&arch_out.stderr);
    assert_eq!(arch_out.status.code(), Some(1), "{arch_err}");
    assert!(
        arch_err.contains("ARCHITECTURE.md: crate `demo` (packages/demo) has no section"),
        "{arch_err}"
    );

    let dep_out = temp
        .command(&dep_script)
        .env("PATH", &path_env)
        .output()
        .unwrap();
    let dep_err = String::from_utf8_lossy(&dep_out.stderr);
    assert_eq!(dep_out.status.code(), Some(1), "{dep_err}");
    assert!(dep_err.contains("crate 'demo' has no policy"), "{dep_err}");

    let make_out = temp
        .command("make")
        .arg("fmt-check")
        .current_dir(root)
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert_ne!(
        make_out.status.code(),
        Some(0),
        "expected make fmt-check to run on packages/demo instead of skipping, stderr: {}",
        String::from_utf8_lossy(&make_out.stderr)
    );
}
