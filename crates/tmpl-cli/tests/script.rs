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
#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "a test sets up fixture files and runs the built binary; production code may not"
)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

/// A `PATH` with the toolchain directory that holds `CARGO` first, so child
/// scripts find `cargo` and `rustfmt` reliably.
fn path_with_cargo() -> std::ffi::OsString {
    let cargo_dir = Path::new(env!("CARGO")).parent().unwrap();
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![cargo_dir.to_path_buf()];
    paths.extend(std::env::split_paths(&current));
    std::env::join_paths(paths).unwrap()
}

#[test]
fn check_doc_frontmatter_refuses_invalid_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = copy_script("check-doc-frontmatter.py", root);

    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join("docs/colon.md"),
        "---\ntype: guide\nsummary: How to run demo: its commands.\n\
         created_date: 2026-01-01\nupdated_date: 2026-01-01\n---\n\n# Guide\n",
    )
    .unwrap();
    fs::write(
        root.join("docs/unclosed.md"),
        "---\ntype: guide\nsummary: \"unclosed quote\n\
         created_date: 2026-01-01\nupdated_date: 2026-01-01\n---\n\n# Unclosed\n",
    )
    .unwrap();

    // Run once with the default Python environment (PyYAML if installed) and
    // once with `yaml` blocked so the built-in stdlib parser is exercised too.
    let default_out = Command::new("python3").arg(&script).output().unwrap();
    let default_stderr = String::from_utf8_lossy(&default_out.stderr);
    assert_eq!(default_out.status.code(), Some(1), "{default_stderr}");
    assert!(
        default_stderr.contains("docs/colon.md:") && default_stderr.contains("YAML"),
        "{default_stderr}"
    );
    assert!(
        default_stderr.contains("docs/unclosed.md:") && default_stderr.contains("YAML"),
        "{default_stderr}"
    );

    let fallback_out = Command::new("python3")
        .args([
            "-I",
            "-c",
            "import runpy, sys; sys.modules['yaml'] = None; runpy.run_path(sys.argv[1], run_name='__main__')",
        ])
        .arg(&script)
        .output()
        .unwrap();
    let fallback_stderr = String::from_utf8_lossy(&fallback_out.stderr);
    assert_eq!(fallback_out.status.code(), Some(1), "{fallback_stderr}");
    assert!(
        fallback_stderr.contains("docs/colon.md:") && fallback_stderr.contains("YAML"),
        "{fallback_stderr}"
    );
    assert!(
        fallback_stderr.contains("docs/unclosed.md:") && fallback_stderr.contains("YAML"),
        "{fallback_stderr}"
    );
}

#[test]
fn check_doc_links_refuses_root_relative_and_symlink_escape() {
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("repo");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(outer.path().join("outside.txt"), "secret\n").unwrap();
    std::os::unix::fs::symlink(outer.path().join("outside.txt"), root.join("escape.txt")).unwrap();

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

    let out = Command::new("python3").arg(&script).output().unwrap();
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

#[test]
fn check_readme_refuses_escape_links() {
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    fs::write(outer.path().join("outside.txt"), "secret\n").unwrap();
    std::os::unix::fs::symlink(outer.path().join("outside.txt"), root.join("escape.txt")).unwrap();

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

    let out = Command::new("python3").arg(&script).output().unwrap();
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

#[test]
fn check_module_cycles_catches_grouped_imports_and_deep_sibling_cycles() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = copy_script("check-module-cycles.sh", root);

    let src = root.join("crates/demo/src");
    fs::create_dir_all(src.join("driver/sqlite")).unwrap();
    fs::write(
        src.join("lib.rs"),
        "//! Demo.\nmod alpha;\nmod beta;\nmod driver;\nmod gamma;\n",
    )
    .unwrap();
    // Grouped import cycle at top level.
    fs::write(src.join("alpha.rs"), "use crate::{beta, gamma};\n").unwrap();
    fs::write(src.join("beta.rs"), "use crate::alpha::Item;\n").unwrap();
    fs::write(src.join("gamma.rs"), "//! Leaf.\n").unwrap();
    // Third-level sibling cycle under driver/sqlite/.
    fs::write(src.join("driver/mod.rs"), "mod sqlite;\n").unwrap();
    fs::write(
        src.join("driver/sqlite/mod.rs"),
        "mod first;\nmod second;\n",
    )
    .unwrap();
    fs::write(
        src.join("driver/sqlite/first.rs"),
        "use super::second::Row;\n",
    )
    .unwrap();
    fs::write(
        src.join("driver/sqlite/second.rs"),
        "use crate::driver::sqlite::{first};\n",
    )
    .unwrap();

    let out = Command::new(&script).output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    for expected in [
        "demo: top-level modules: alpha -> beta -> alpha",
        "demo: modules of driver/sqlite/: first -> second -> first",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
}

#[test]
fn empty_workspace_exemption_decides_from_cargo_metadata_not_crates_glob() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let path_env = path_with_cargo();

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

    let arch_out = Command::new(&arch_script)
        .env("PATH", &path_env)
        .output()
        .unwrap();
    let arch_err = String::from_utf8_lossy(&arch_out.stderr);
    assert_eq!(arch_out.status.code(), Some(1), "{arch_err}");
    assert!(
        arch_err.contains("ARCHITECTURE.md: crate `demo` (packages/demo) has no section"),
        "{arch_err}"
    );

    let dep_out = Command::new(&dep_script)
        .env("PATH", &path_env)
        .output()
        .unwrap();
    let dep_err = String::from_utf8_lossy(&dep_out.stderr);
    assert_eq!(dep_out.status.code(), Some(1), "{dep_err}");
    assert!(dep_err.contains("crate 'demo' has no policy"), "{dep_err}");

    let make_out = Command::new("make")
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

#[test]
fn check_terminal_guard_catches_imported_stdout_and_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
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

    let out = Command::new(&script).output().unwrap();
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

#[test]
fn clippy_refuses_an_aliased_env_read_and_a_direct_file_read() {
    // A scratch crate under the repository's clippy.toml, with the two lints
    // denied as the workspace denies them.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::copy(repo_root().join("clippy.toml"), root.join("clippy.toml")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"evasion\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
         [lints.clippy]\ndisallowed_methods = \"deny\"\ndisallowed_types = \"deny\"\n\n\
         [workspace]\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.rs"),
        "use std::env;\nuse std::path::Path;\nuse std::{fmt, fs};\n\n\
         fn main() {\n    let _ = fmt::Error;\n    let _value = env::var(\"X\");\n    \
         let _bytes = fs::read(\"f\");\n    let _exists = Path::new(\"f\").exists();\n    \
         let _try_exists = Path::new(\"f\").try_exists();\n}\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO"))
        .args(["clippy", "--quiet", "--offline"])
        .env("CARGO_TARGET_DIR", root.join("target"))
        .current_dir(root)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0), "clippy passed: {stderr}");
    for refused in [
        "use of a disallowed method `std::env::var`",
        "use of a disallowed method `std::fs::read`",
        "use of a disallowed method `std::path::Path::exists`",
        "use of a disallowed method `std::path::Path::try_exists`",
    ] {
        assert!(stderr.contains(refused), "{refused}: {stderr}");
    }
}

/// A task id, built at run time so that this file names none: the gate scans
/// every tracked file, this one included.
const TASK_ID: &str = concat!("ORB-", "26767");

/// Run `git` in `root`, which must succeed.
fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Copy the private-id gate into `root` and run it there. Git's search for a
/// repository stops at `root`'s parent, so an enclosing checkout cannot make
/// `root` look like a work tree.
fn private_task_ids(root: &Path) -> Output {
    let script = copy_script("check-private-task-ids.sh", root);
    Command::new(&script)
        .env("GIT_CEILING_DIRECTORIES", root.parent().unwrap())
        .output()
        .unwrap()
}

#[test]
fn private_task_ids_gate_passes_a_copy_outside_a_work_tree() {
    // Git sees none of this copy's files, so the gate scans them. The vendored
    // canon and target/ may name task ids; nothing else may.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
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

    let out = private_task_ids(root);
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
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("crates/demo/src")).unwrap();
    fs::write(
        root.join("crates/demo/src/lib.rs"),
        format!("//! Tracked in {TASK_ID}.\n"),
    )
    .unwrap();

    let out = private_task_ids(root);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("crates/demo/src/lib.rs"), "{stdout}");
    assert!(stderr.contains("replace private task IDs"), "{stderr}");
}

#[test]
fn private_task_ids_gate_inside_a_work_tree_scans_tracked_files_only() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    fs::create_dir_all(root.join("crates/demo/src")).unwrap();
    fs::write(
        root.join("crates/demo/src/lib.rs"),
        format!("//! Tracked in {TASK_ID}.\n"),
    )
    .unwrap();
    git(root, &["add", "crates/demo/src/lib.rs"]);
    // Untracked, so git grep does not see it.
    fs::write(root.join("untracked.md"), format!("{TASK_ID}\n")).unwrap();

    let out = private_task_ids(root);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("crates/demo/src/lib.rs"), "{stdout}");
    assert!(!stdout.contains("untracked.md"), "{stdout}");
}
