//! Golden tests of the public surface, captured from the built binary as it
//! ships (STD-01 R24): every command's `--help`, and the plain, table and
//! JSON forms of `note list` and `note show`.
//!
//! A mismatch fails. After an intentional surface change, regenerate with
//! `make goldens UPDATE=1` (which sets `UPDATE_GOLDENS=1`) and review the
//! diff in the PR as a contract change.
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]

mod support;

use std::path::PathBuf;
use support::{Fixture, redact_timestamps, text};

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
/// every stale or missing golden at once.
fn check(rel: &str, actual: &str) -> Option<String> {
    let path = golden_dir().join(rel);
    if updating() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return None;
    }
    let Ok(expected) = std::fs::read_to_string(&path) else {
        return Some(format!(
            "missing golden {rel} ({})\n--- actual\n{actual}",
            path.display()
        ));
    };
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
    let commands = tmpl_cli::every_command();
    assert!(!commands.is_empty(), "every_command() returned no commands");
    let failures: Vec<String> = commands
        .iter()
        .filter_map(|(path, _cmd)| {
            let subcommands: Vec<&str> = path.split_whitespace().skip(1).collect();
            let name = if subcommands.is_empty() {
                String::from("root")
            } else {
                subcommands.join("-")
            };
            let mut args = subcommands;
            args.push("--help");
            let out = fx.ok(&args);
            check(&format!("help/{name}.txt"), &text(&out.stdout))
        })
        .collect();
    report(&failures);
}

#[test]
fn command_summaries_fit_on_one_line_and_root_commands_are_listed_once() {
    const MAX_SUMMARY_CHARS: usize = 70;
    let commands = tmpl_cli::every_command();
    let (_, root) = commands
        .first()
        .expect("the command tree includes the root");
    let fx = Fixture::new();
    let help = text(&fx.ok(&["--help"]).stdout);

    for (path, command) in commands.iter().skip(1) {
        assert_summary(path, command, MAX_SUMMARY_CHARS);
    }
    for command in root
        .get_subcommands()
        .filter(|command| !command.is_hide_set())
    {
        let name = command.get_name();
        let summary = assert_summary(name, command, MAX_SUMMARY_CHARS);
        let rows = help.lines().filter(|line| {
            line.trim_start()
                .split_once("  ")
                .is_some_and(|(label, text)| label == name && text.trim() == summary)
        });
        assert_eq!(rows.count(), 1, "root help must list {name} once");
    }
}

fn assert_summary(path: &str, command: &clap::Command, limit: usize) -> String {
    let summary = command
        .get_about()
        .unwrap_or_else(|| panic!("{path} needs a summary"))
        .to_string();
    assert!(
        !summary.trim().is_empty()
            && !summary.contains(['\r', '\n'])
            && summary.chars().count() <= limit,
        "{path} needs a one-line summary of at most {limit} characters: {summary:?}"
    );
    summary
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
