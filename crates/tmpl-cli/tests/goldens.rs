//! Golden tests of the public surface, captured from the built binary as it
//! ships (STD-01 R24): every command's `--help`, and the plain, table and
//! JSON forms of `note list` and `note show`.
//!
//! A mismatch fails. After an intentional surface change, regenerate with
//! `make goldens UPDATE=1` (which sets `UPDATE_GOLDENS=1`) and review the
//! diff in the PR as a contract change.
#![allow(clippy::expect_used, clippy::unwrap_used)]

mod support;

use std::path::PathBuf;
use support::{Fixture, redact_timestamps, text};

const HELP: &[(&str, &[&str])] = &[
    ("root", &["--help"]),
    ("note", &["note", "--help"]),
    ("note-add", &["note", "add", "--help"]),
    ("note-list", &["note", "list", "--help"]),
    ("note-show", &["note", "show", "--help"]),
];

const OUTPUT: &[(&str, &[&str])] = &[
    ("note-list.plain.txt", &["note", "list"]),
    (
        "note-list.table.txt",
        &["note", "list", "--format", "table"],
    ),
    ("note-list.json", &["note", "list", "--json"]),
    ("note-show.plain.txt", &["note", "show", "3"]),
    (
        "note-show.table.txt",
        &["note", "show", "3", "--format", "table"],
    ),
    ("note-show.json", &["note", "show", "3", "--json"]),
];

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("goldens")
}

fn updating() -> bool {
    std::env::var_os("UPDATE_GOLDENS").is_some_and(|v| v == "1")
}

/// Compare `actual` to the golden at `rel`, or rewrite it when updating.
/// Returns a failure description instead of panicking so one run reports
/// every stale golden at once.
fn check(rel: &str, actual: &str) -> Option<String> {
    let path = golden_dir().join(rel);
    if updating() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return None;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    (expected != actual).then(|| {
        format!(
            "golden {rel} differs\n--- expected ({})\n{expected}\n--- actual\n{actual}",
            path.display()
        )
    })
}

fn report(failures: &[String]) {
    assert!(
        failures.is_empty(),
        "{}\n\nIf the change is intended: make goldens UPDATE=1, then review the diff.",
        failures.join("\n\n")
    );
}

#[test]
fn help_matches_goldens() {
    let fx = Fixture::new();
    let failures: Vec<String> = HELP
        .iter()
        .filter_map(|(name, args)| {
            let out = fx.ok(args);
            check(&format!("help/{name}.txt"), &text(&out.stdout))
        })
        .collect();
    report(&failures);
}

#[test]
fn output_matches_goldens() {
    let fx = Fixture::new();
    fx.seed();
    let failures: Vec<String> = OUTPUT
        .iter()
        .filter_map(|(name, args)| {
            let out = fx.ok(args);
            check(
                &format!("output/{name}"),
                &redact_timestamps(&text(&out.stdout)),
            )
        })
        .collect();
    report(&failures);
}
