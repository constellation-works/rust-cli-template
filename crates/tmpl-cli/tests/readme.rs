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

#[test]
fn check_readme_requires_description_status_section_order_and_doc_index() {
    let temp = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();
    let root = temp.path().join("repo");
    fs::create_dir_all(root.join("docs")).unwrap();
    let script = copy_script("check-readme.py", &root);
    fs::write(
        root.join("Cargo.toml"),
        "[workspace.package]\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("docs/INDEX.md"), "# Documentation\n").unwrap();
    fs::write(root.join("ARCHITECTURE.md"), "# Architecture\n").unwrap();
    fs::write(
        root.join("README.md"),
        r#"<div align="center">
  <h1>demo</h1>
  <img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version">
</div>

## Starting point

Generated from the constellation Rust CLI template.

This project is pre-1.0.

## Quick start

Run the first command.

## Learn more

- [Architecture](ARCHITECTURE.md) for the crate layout.

## Contributing

See the contributing guide.

## Licence

MIT.
"#,
    )
    .unwrap();

    let invalid = temp.command("python3").arg(&script).output().unwrap();
    let invalid_stderr = String::from_utf8_lossy(&invalid.stderr);
    assert_eq!(invalid.status.code(), Some(1), "{invalid_stderr}");
    for expected in [
        "first prose after the header must describe the project",
        "state pre-1.0 status once in or directly under the description",
        "optional section 'starting point' must sit between Quick start and Learn more",
        "Learn more section must link docs/INDEX.md",
    ] {
        assert!(
            invalid_stderr.contains(expected),
            "{expected}: {invalid_stderr}"
        );
    }

    fs::write(
        root.join("README.md"),
        r#"<div align="center">
  <h1>demo</h1>
  <img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version">
</div>

A command-line app to make a task list easier to manage.

This project is pre-1.0.

## Quick start

Run the first command.

## Starting point

More detail.

## Learn more

- [Documentation index](docs/INDEX.md) for the full docs.

## Contributing

See the contributing guide.

## Licence

MIT.
"#,
    )
    .unwrap();
    let valid = temp.command("python3").arg(&script).output().unwrap();
    let valid_stderr = String::from_utf8_lossy(&valid.stderr);
    assert_eq!(valid.status.code(), Some(0), "{valid_stderr}");
}
