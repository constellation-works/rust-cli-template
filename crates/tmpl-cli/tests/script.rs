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
use std::process::Output;

/// A task id, built at run time so that this file names none: the gate scans
/// every tracked file, this one included.
const TASK_ID: &str = concat!("ORB-", "26767");

/// Source that breaches STD-12 R21, R22 and R26, one breach per marked line.
const STYLE_BREACHES: &str = "\
//! Breaches.

pub struct Gadget {
    id: u32,
}

const BETWEEN: u32 = 1;

impl Gadget {
    pub fn get(&self) -> u32 {
        self.id
    }

    pub fn new() -> Self {
        Self { id: BETWEEN }
    }
}

pub struct Widget {
    id: u32,
}

impl Widget {
    pub fn get(&self) -> u32 {
        self.id
    }

    pub fn set(&mut self, id: u32) {
        self.id = id;
    }

    pub fn into_id(self) -> u32 {
        self.id
    }

    pub fn with_id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }
}

impl crate::Project for Widget {}

impl Default for Widget {
    fn default() -> Self {
        Self { id: 0 }
    }
}

mod inner {
    use super::super::Thing;

    // super::super in a comment is allowed.
    pub fn name() -> &'static str {
        \"super::super in a string is allowed\"
    }
}
";

/// Source that follows STD-12 R21, R22 and R26.
const STYLE_ALLOWED: &str = "\
//! Allowed forms.

pub struct Gadget {
    id: u32,
}

impl Gadget {
    const LIMIT: u32 = 3;

    pub fn new() -> Self {
        Self { id: Self::LIMIT }
    }

    fn private_new() -> Self {
        Self { id: 0 }
    }

    pub fn helper() -> u32 {
        0
    }

    pub fn with_id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }

    pub fn get(&self) -> u32 {
        self.id
    }

    fn private_get(&self) -> u32 {
        self.id
    }

    pub fn set(&mut self, id: u32) {
        self.id = id;
    }

    pub fn into_id(self) -> u32 {
        self.id
    }
}

impl std::fmt::Debug for Gadget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(\"Gadget\")
    }
}

impl From<Gadget> for u32 {
    fn from(gadget: Gadget) -> Self {
        gadget.id
    }
}

impl From<Other> for Gadget {
    fn from(other: Other) -> Self {
        Self { id: other.0 }
    }
}

impl crate::Project for Gadget {}

pub struct Other(u32);

impl From<Other> for cw::Error {
    fn from(other: Other) -> Self {
        Self::new(other.0)
    }
}

mod inner {
    use crate::Thing;
    use super::Other;

    impl Other {
        pub fn new() -> Self {
            Self(0)
        }
    }
}
";

/// A lib that breaches the clippy gates STD-02 R51 and STD-12 R20 and R23.
const CLIPPY_BREACHES: &str = "\
pub fn spread(one: u32, two: u32, three: u32, four: u32, five: u32, six: u32, seven: u32, eight: u32) -> u32 {
    one + two + three + four + five + six + seven + eight
}

pub struct Pair {
    pub first: u32,
    pub second: u32,
}

pub fn pair(first: u32, second: u32) -> Pair {
    Pair { second, first }
}
";

/// The same lib in the permitted forms: seven parameters, types before
/// functions, the literal in declaration order.
const CLIPPY_ALLOWED: &str = "\
pub struct Pair {
    pub first: u32,
    pub second: u32,
}

pub fn spread(one: u32, two: u32, three: u32, four: u32, five: u32, six: u32, seven: u32) -> u32 {
    one + two + three + four + five + six + seven
}

pub fn pair(first: u32, second: u32) -> Pair {
    Pair { first, second }
}
";

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
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
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

    let out = temp.command(&script).output().unwrap();
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

/// A `PATH` for a kit child that must run the toolchain: the fixture's
/// `bin/` first, then the directory holding `CARGO`, then the system
/// directories the kit already puts on `PATH`. Built from named values only;
/// the parent's `PATH` is never inherited (STD-03 R20).
fn isolated_path(temp: &cw_testkit::TempRoot) -> std::ffi::OsString {
    let cargo_dir = Path::new(env!("CARGO")).parent().unwrap();
    std::env::join_paths([
        temp.bin(),
        cargo_dir.to_path_buf(),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ])
    .unwrap()
}

#[test]
fn check_doc_frontmatter_refuses_invalid_yaml() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
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
    let default_out = temp.command("python3").arg(&script).output().unwrap();
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

    let fallback_out = temp.command("python3")
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

#[test]
fn check_module_cycles_catches_grouped_imports_and_deep_sibling_cycles() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
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

    let out = temp.command(&script).output().unwrap();
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

#[test]
fn check_terminal_guard_catches_imported_stdout_and_stderr() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
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

    let out = temp.command(&script).output().unwrap();
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

#[test]
fn clippy_refuses_an_aliased_env_read_and_a_direct_file_read() {
    // A scratch crate under the repository's clippy.toml, with the two lints
    // denied as the workspace denies them.
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
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

    let path = isolated_path(&temp);
    let out = temp
        .command(env!("CARGO"))
        .args(["clippy", "--quiet", "--offline"])
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("PATH", &path)
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

/// Run `git` in `root` with the kit's isolated environment, which must succeed.
fn git(temp: &cw_testkit::TempRoot, root: &Path, args: &[&str]) {
    let out = temp
        .command("git")
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

/// Run the source-style gate over a scratch repository holding `source` as
/// `src/lib.rs`: its exit code and standard error.
fn source_style(source: &str) -> (Option<i32>, String) {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path();
    let script = copy_script("check-source-style.sh", root);
    copy_script("check-source-style.py", root);
    git(&temp, root, &["init", "-q"]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), source).unwrap();
    let out = temp.command(&script).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_source_style_names_each_adjacency_member_order_and_super_breach() {
    let (code, stderr) = source_style(STYLE_BREACHES);
    assert_eq!(code, Some(1), "{stderr}");
    for expected in [
        // An item between a type and its impl: the impl, not the type, is named.
        "src/lib.rs:9: R21: this impl of `Gadget` does not directly follow the type",
        // Members out of order: a constructor after a `&self` method.
        "src/lib.rs:14: R22: a constructor comes after a `&self` method",
        "src/lib.rs:36: R22: a `with_*` method comes after a method taking `self` by value",
        // A project trait impl above a standard-library one.
        "src/lib.rs:44: R21: impls of `Widget`: inherent, then standard-library traits",
        "src/lib.rs:51: R26: `super::super`: name the item from `crate::`",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
    assert_eq!(
        stderr.matches(": R26:").count(),
        1,
        "a comment and a string naming the path are allowed: {stderr}"
    );
}

#[test]
fn check_source_style_accepts_adjacent_ordered_impls_and_one_level_of_super() {
    let (code, stderr) = source_style(STYLE_ALLOWED);
    assert_eq!(code, Some(0), "{stderr}");
}

#[test]
fn clippy_gates_parameter_count_item_order_and_literal_field_order() {
    // A scratch crate under the repository's clippy.toml and its clippy lint
    // table, run as CI runs clippy.
    let manifest = fs::read_to_string(repo_root().join("Cargo.toml")).unwrap();
    let table = manifest
        .split("[workspace.lints.clippy]")
        .nth(1)
        .and_then(|rest| rest.split("[workspace.lints.rust]").next())
        .unwrap();
    let clippy = |source: &str| {
        let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
        let root = temp.path();
        fs::copy(repo_root().join("clippy.toml"), root.join("clippy.toml")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"scratch\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
                 [lints.clippy]{table}\n[workspace]\n"
            ),
        )
        .unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), source).unwrap();
        let path = isolated_path(&temp);
        let out = temp
            .command(env!("CARGO"))
            .args(["clippy", "--quiet", "--offline", "--", "-D", "warnings"])
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env("PATH", &path)
            .current_dir(root)
            .output()
            .unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };

    let (code, stderr) = clippy(CLIPPY_BREACHES);
    assert_ne!(code, Some(0), "clippy passed: {stderr}");
    for refused in [
        "this function has too many arguments (8/7)",
        "incorrect ordering of items",
        "struct constructor field order is inconsistent",
    ] {
        assert!(stderr.contains(refused), "{refused}: {stderr}");
    }

    let (code, stderr) = clippy(CLIPPY_ALLOWED);
    assert_eq!(code, Some(0), "{stderr}");
}

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
