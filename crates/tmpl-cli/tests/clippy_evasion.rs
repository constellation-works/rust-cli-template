//! Integration coverage for clippy policy enforcement.
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
use support::script::{isolated_path, repo_root};

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
