//! Integration coverage for the README validation script.
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
fn check_readme_refuses_escape_links() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    fs::write(temp.path().join("outside.txt"), "secret\n").unwrap();
    std::os::unix::fs::symlink(temp.path().join("outside.txt"), root.join("escape.txt")).unwrap();

    let script = copy_script("check-readme.py", &root);
    fs::write(
        root.join("Cargo.toml"),
        "[workspace.package]\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("README.md"),
        "# Demo\n\n\
         ![version](https://img.shields.io/badge/version-0.1.0-blue)\n\n\
         [rel-up](../outside.txt)\n\
         [root-up](/../outside.txt)\n\
         [root-link](/escape.txt)\n\
         [rel-link](escape.txt)\n",
    )
    .unwrap();

    let out = temp.command("python3").arg(&script).output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    for expected in [
        "link leaves the repository: ../outside.txt",
        "link leaves the repository: /../outside.txt",
        "link leaves the repository: /escape.txt",
        "link leaves the repository: escape.txt",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
}
