//! Helpers for integration tests that copy and run repository gate scripts.
#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "integration-test helpers set up fixtures and run copied scripts"
)]

use super::CommandDeadline;
use cw_testkit::TempRoot;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The repository root containing the `scripts/` directory.
pub(crate) fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has a workspace parent")
        .parent()
        .expect("workspace has a repository parent")
        .to_path_buf()
}

/// Copy a gate script into the fixture and make it executable. A Python script
/// is copied with the packages beside it, which `python3 -I` imports from there.
pub(crate) fn copy_script(name: &str, dest_root: &Path) -> PathBuf {
    let source = repo_root().join("scripts").join(name);
    let destination = dest_root.join("scripts").join(name);
    fs::create_dir_all(destination.parent().expect("script has a parent directory"))
        .expect("create fixture script directory");
    fs::copy(&source, &destination).expect("copy fixture script");
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o755))
        .expect("make fixture script executable");
    if name.ends_with(".py") {
        copy_packages(&repo_root().join("scripts"), &dest_root.join("scripts"));
    }
    destination
}

/// Copy each Python package (a directory holding `__init__.py`) of `from` into `to`.
fn copy_packages(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).expect("read the scripts directory") {
        let package = entry.expect("read a scripts entry").path();
        if !package.join("__init__.py").is_file() {
            continue;
        }
        let target = to.join(package.file_name().expect("package has a name"));
        fs::create_dir_all(&target).expect("create fixture package directory");
        for file in fs::read_dir(&package).expect("read the package directory") {
            let file = file.expect("read a package entry").path();
            if file.extension().is_some_and(|extension| extension == "py") {
                fs::copy(
                    &file,
                    target.join(file.file_name().expect("file has a name")),
                )
                .expect("copy fixture package file");
            }
        }
    }
}

/// Build a child `PATH` from the fixture bin directory, Cargo's directory and system directories.
pub(crate) fn isolated_path(temp: &TempRoot) -> std::ffi::OsString {
    let cargo_dir = Path::new(env!("CARGO"))
        .parent()
        .expect("Cargo executable has a parent directory");
    std::env::join_paths([
        temp.bin(),
        cargo_dir.to_path_buf(),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ])
    .expect("join isolated PATH entries")
}

/// Run `git` in the fixture with its isolated environment and require success.
pub(crate) fn git(temp: &TempRoot, root: &Path, args: &[&str]) {
    let output = temp
        .command("git")
        .args(args)
        .current_dir(root)
        .output_with_deadline()
        .expect("run fixture git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
