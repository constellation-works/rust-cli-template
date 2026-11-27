//! Integration coverage for the documentation links script.
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
fn check_doc_links_refuses_root_relative_and_symlink_escape() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path().join("repo");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(temp.path().join("outside.txt"), "secret\n").unwrap();
    std::os::unix::fs::symlink(temp.path().join("outside.txt"), root.join("escape.txt")).unwrap();

    let script = copy_script("check-doc-links.py", &root);
    copy_script("check-readme.py", &root);
    copy_script("check-doc-frontmatter.py", &root);

    fs::write(
        root.join("docs/guide.md"),
        "---\ntype: guide\nsummary: Links that leave the repository.\n\
         created_date: 2026-01-01\nupdated_date: 2026-01-01\n---\n\n\
         [root-up](/../outside.txt)\n\
         [root-link](/escape.txt)\n\
         [rel-link](../escape.txt)\n",
    )
    .unwrap();

    let out = temp.command("python3").arg(&script).output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    for expected in [
        "link leaves the repository: /../outside.txt",
        "link leaves the repository: /escape.txt",
        "link leaves the repository: ../escape.txt",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
}
