//! Integration coverage for the private-task-id script.
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
use std::path::Path;
use std::process::Output;
use support::script::{copy_script, git};

/// A task id, built at run time so that this file names none: the gate scans
/// every tracked file, this one included.
const TASK_ID: &str = concat!("ORB-", "26767");

/// Copy the private-id gate into `root` and run it there in the kit's
/// isolated environment, whose `GIT_CEILING_DIRECTORIES` stops git's search
/// above the temp root, so an enclosing checkout cannot make `root` look
/// like a work tree.
fn private_task_ids(temp: &cw_testkit::TempRoot, root: &Path) -> Output {
    let script = copy_script("check-private-task-ids.sh", root);
    temp.command(&script).output().unwrap()
}

#[test]
fn private_task_ids_gate_passes_a_copy_outside_a_work_tree() {
    // Git sees none of this copy's files, so the gate scans them. The vendored
    // canon and target/ may name task ids; nothing else may.
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "//! Clean.\n").unwrap();
    fs::create_dir_all(root.join("docs/standards/STD-01")).unwrap();
    fs::write(
        root.join("docs/standards/STD-01/STD-01.md"),
        format!("Cites {TASK_ID}.\n"),
    )
    .unwrap();
    fs::create_dir_all(root.join("target")).unwrap();
    fs::write(root.join("target/build.log"), format!("{TASK_ID}\n")).unwrap();

    let out = private_task_ids(&temp, root);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn private_task_ids_gate_names_a_file_outside_a_work_tree_with_an_id() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("crates/demo/src")).unwrap();
    fs::write(
        root.join("crates/demo/src/lib.rs"),
        format!("//! Tracked in {TASK_ID}.\n"),
    )
    .unwrap();

    let out = private_task_ids(&temp, root);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("crates/demo/src/lib.rs"), "{stdout}");
    assert!(stderr.contains("replace private task IDs"), "{stderr}");
}

#[test]
fn private_task_ids_gate_inside_a_work_tree_scans_tracked_files_only() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    git(&temp, root, &["init", "-q"]);
    fs::create_dir_all(root.join("crates/demo/src")).unwrap();
    fs::write(
        root.join("crates/demo/src/lib.rs"),
        format!("//! Tracked in {TASK_ID}.\n"),
    )
    .unwrap();
    git(&temp, root, &["add", "crates/demo/src/lib.rs"]);
    // Untracked, so git grep does not see it.
    fs::write(root.join("untracked.md"), format!("{TASK_ID}\n")).unwrap();

    let out = private_task_ids(&temp, root);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("crates/demo/src/lib.rs"), "{stdout}");
    assert!(!stdout.contains("untracked.md"), "{stdout}");
}
