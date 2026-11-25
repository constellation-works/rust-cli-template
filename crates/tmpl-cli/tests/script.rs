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
    // The Makefile runs its Cargo targets only once a crate exists.
    fs::create_dir_all(root.join("crates/app")).unwrap();
    fs::write(root.join("crates/app/Cargo.toml"), "").unwrap();
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
        root.join("crates/tmpl-cli-core/src/flow.rs"),
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

/// A one-crate workspace whose `ARCHITECTURE.md` maps every unit, with the
/// architecture-map script copied in.
fn architecture_fixture(root: &Path) -> PathBuf {
    let script = copy_script("check-architecture-map.sh", root);
    let files = [
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/demo\"]\nresolver = \"2\"\n",
        ),
        (
            "crates/demo/Cargo.toml",
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "crates/demo/src/main.rs",
            "//! The binary.\nmod cli;\nmod output;\nfn main() {}\n",
        ),
        ("crates/demo/src/cli.rs", "//! The flags.\n"),
        (
            "crates/demo/src/output/mod.rs",
            "//! The output.\nmod sink;\n",
        ),
        ("crates/demo/src/output/sink.rs", "//! The sink.\n"),
        ("crates/demo/src/tests/mod.rs", ""),
        (
            "ARCHITECTURE.md",
            "# Architecture\n\n## Crates\n\n`demo` is the binary; \
             `scripts/check-architecture-map.sh` checks this map.\n\n\
             ## `demo` (binary)\n\n```\nmain.rs      the entry\ncli.rs       the flags\n\
             output/\n  sink.rs    the sink\n```\n\n`output/` alone writes.\n",
        ),
    ];
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    script
}

/// Run the script and return its exit code and stderr.
fn run_architecture_map(script: &Path) -> (Option<i32>, String) {
    let out = Command::new(script).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_architecture_map_passes_on_a_complete_map() {
    let dir = tempfile::tempdir().unwrap();
    let script = architecture_fixture(dir.path());
    let (code, stderr) = run_architecture_map(&script);
    assert_eq!(code, Some(0), "{stderr}");
}

#[test]
fn check_architecture_map_fails_on_a_crate_missing_from_the_doc() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = architecture_fixture(root);
    // A new crate and its member entry; the script is not edited.
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/demo\", \"crates/extra\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("crates/extra/src")).unwrap();
    fs::write(
        root.join("crates/extra/Cargo.toml"),
        "[package]\nname = \"extra\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(root.join("crates/extra/src/lib.rs"), "//! Extra.\n").unwrap();
    let (code, stderr) = run_architecture_map(&script);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.contains("ARCHITECTURE.md: crate `extra` (crates/extra) has no section"),
        "{stderr}"
    );
}

#[test]
fn check_architecture_map_fails_on_a_module_missing_from_its_section() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = architecture_fixture(root);
    fs::write(root.join("crates/demo/src/help.rs"), "//! The help.\n").unwrap();
    let (code, stderr) = run_architecture_map(&script);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.contains(
            "ARCHITECTURE.md: the `demo` section does not name module `help` (crates/demo/src/help.rs)"
        ),
        "{stderr}"
    );
}

#[test]
fn check_architecture_map_fails_on_a_documented_path_that_does_not_exist() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = architecture_fixture(root);
    let doc = fs::read_to_string(root.join("ARCHITECTURE.md")).unwrap();
    let doc = doc
        .replace(
            "cli.rs       the flags\n",
            "cli.rs       the flags\nerror.rs     gone\n",
        )
        .replace(
            "`output/` alone",
            "`crates/demo/src/render.rs` and `output/` alone",
        );
    fs::write(root.join("ARCHITECTURE.md"), doc).unwrap();
    let (code, stderr) = run_architecture_map(&script);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.contains(
            "ARCHITECTURE.md:12: `demo`: the tree names `error.rs`, which does not exist"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "ARCHITECTURE.md:17: `demo` section names `crates/demo/src/render.rs`, which does not exist"
        ),
        "{stderr}"
    );
}

#[test]
fn check_architecture_map_fails_on_a_unit_without_a_first_line_doc() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = architecture_fixture(root);
    fs::write(root.join("crates/demo/src/cli.rs"), "// The flags.\n").unwrap();
    fs::write(root.join("crates/demo/src/output/mod.rs"), "mod sink;\n").unwrap();
    fs::write(root.join("crates/demo/src/main.rs"), "fn main() {}\n").unwrap();
    let (code, stderr) = run_architecture_map(&script);
    assert_eq!(code, Some(1), "{stderr}");
    for expected in [
        "crates/demo/src/cli.rs: `demo::cli` does not begin with a `//!` doc line",
        "crates/demo/src/output/mod.rs: `demo::output` does not begin with a `//!` doc line",
        "crates/demo/src/main.rs: `demo: crate root main.rs` does not begin with a `//!` doc line",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
}
