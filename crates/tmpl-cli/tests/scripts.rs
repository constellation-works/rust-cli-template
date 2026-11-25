//! End-to-end tests for the repository gates.
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn copy_script(name: &str, dest_root: &Path) -> PathBuf {
    let src = repo_root().join("scripts").join(name);
    let dst = dest_root.join("scripts").join(name);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::copy(&src, &dst).unwrap();
    fs::set_permissions(&dst, fs::Permissions::from_mode(0o755)).unwrap();
    dst
}

#[test]
fn deny_uses_a_writable_cache_when_the_cargo_home_is_unusable() {
    // make runs in the kit's temp root and isolated environment.
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    for name in ["Makefile", "deny.toml"] {
        fs::copy(repo_root().join(name), root.join(name)).unwrap();
    }
    let cargo = root.join("cargo");
    fs::write(
        &cargo,
        r#"#!/bin/sh
set -eu
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
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn check_test_modules_fails_when_parent_lacks_mod_tests() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = copy_script("check-test-modules.sh", root);

    // Parent module omits `mod tests;` even though tests/mod.rs declares its child.
    fs::create_dir_all(root.join("crates/demo/src/sub/tests")).unwrap();
    fs::write(root.join("crates/demo/src/sub/mod.rs"), "// no mod tests\n").unwrap();
    fs::write(root.join("crates/demo/src/sub/tests/mod.rs"), "mod unit;\n").unwrap();
    fs::write(
        root.join("crates/demo/src/sub/tests/unit.rs"),
        "#[test] fn t() {}\n",
    )
    .unwrap();

    let out = Command::new(&script).output().unwrap();
    assert_ne!(
        out.status.code(),
        Some(0),
        "expected failure when parent lacks `mod tests;`, got stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("crates/demo/src/sub/tests"),
        "expected error to name the tests directory, got: {stderr}"
    );
}

#[test]
fn check_dependency_direction_catches_grouped_std_fs_import() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = copy_script("check-dependency-direction.sh", root);

    fs::create_dir_all(root.join("crates/tmpl-cli-core/src/store")).unwrap();
    fs::create_dir_all(root.join("crates/tmpl-cli/src/output")).unwrap();
    fs::write(
        root.join("crates/tmpl-cli-core/Cargo.toml"),
        "[package]\nname = \"tmpl-cli-core\"\n[dependencies]\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/tmpl-cli/Cargo.toml"),
        "[package]\nname = \"tmpl-cli\"\n[dependencies]\ntmpl-cli-core = { workspace = true }\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/tmpl-cli-core/src/note.rs"),
        "use std::{fmt, fs};\n",
    )
    .unwrap();
    fs::write(root.join("crates/tmpl-cli-core/src/store/fsio.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli-core/src/store/format.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli-core/src/store/audit.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli/src/app.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli/src/audit_middleware.rs"), "").unwrap();
    fs::write(root.join("crates/tmpl-cli/src/output/mod.rs"), "").unwrap();

    let out = Command::new(&script).output().unwrap();
    assert_ne!(
        out.status.code(),
        Some(0),
        "expected grouped `use std::{{fmt, fs}};` to fail dependency-direction check"
    );
}
