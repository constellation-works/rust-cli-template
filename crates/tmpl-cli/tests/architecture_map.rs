//! Integration coverage for the architecture-map script.
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
use std::path::{Path, PathBuf};
use support::script::{copy_script, isolated_path};

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

/// Run the script in the kit's isolated environment and return its exit code
/// and stderr. `cargo` comes from the toolchain on an explicit `PATH`.
fn run_architecture_map(temp: &cw_testkit::TempRoot, script: &Path) -> (Option<i32>, String) {
    let path = isolated_path(temp);
    let out = temp.command(script).env("PATH", &path).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_architecture_map_passes_on_a_complete_map() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let script = architecture_fixture(temp.path());
    let (code, stderr) = run_architecture_map(&temp, &script);
    assert_eq!(code, Some(0), "{stderr}");
}

#[test]
fn check_architecture_map_fails_on_a_crate_missing_from_the_doc() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
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
    let (code, stderr) = run_architecture_map(&temp, &script);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.contains("ARCHITECTURE.md: crate `extra` (crates/extra) has no section"),
        "{stderr}"
    );
}

#[test]
fn check_architecture_map_fails_on_a_module_missing_from_its_section() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    let script = architecture_fixture(root);
    fs::write(root.join("crates/demo/src/help.rs"), "//! The help.\n").unwrap();
    let (code, stderr) = run_architecture_map(&temp, &script);
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
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
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
    let (code, stderr) = run_architecture_map(&temp, &script);
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
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    let script = architecture_fixture(root);
    fs::write(root.join("crates/demo/src/cli.rs"), "// The flags.\n").unwrap();
    fs::write(root.join("crates/demo/src/output/mod.rs"), "mod sink;\n").unwrap();
    fs::write(root.join("crates/demo/src/main.rs"), "fn main() {}\n").unwrap();
    let (code, stderr) = run_architecture_map(&temp, &script);
    assert_eq!(code, Some(1), "{stderr}");
    for expected in [
        "crates/demo/src/cli.rs: `demo::cli` does not begin with a `//!` doc line",
        "crates/demo/src/output/mod.rs: `demo::output` does not begin with a `//!` doc line",
        "crates/demo/src/main.rs: `demo: crate root main.rs` does not begin with a `//!` doc line",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
}
