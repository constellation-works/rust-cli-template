//! Golden tests of the public surface, captured from the built binary as it
//! ships (STD-01 R24): every command's `--help`, and the plain, table and
//! JSON forms of each command's output.
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
#![expect(
    clippy::disallowed_methods,
    reason = "the test reads and, when updating, rewrites the usage document's generated table"
)]

mod support;

use cw_testkit::Goldens;
use std::fs;
use support::Fixture;

/// Output on a fresh data directory named relative to the fixture, so the
/// path in the output is the same on every machine.
const STATUS_OUTPUT: &[(&str, &[&str])] = &[
    ("status.plain.txt", &["--root", "data", "status"]),
    (
        "status.table.txt",
        &["--root", "data", "status", "--format", "table"],
    ),
    ("status.json", &["--root", "data", "status", "--json"]),
];

// example:begin
/// Output on the seeded store.
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
// example:end

/// The usage document, whose variables table is generated.
const USAGE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/usage.md");

/// The marker line that opens the generated table in [`USAGE`].
const TABLE_BEGIN: &str = "<!-- generated: environment table -->\n";

/// The marker that closes it.
const TABLE_END: &str = "<!-- end generated -->";

/// The goldens folder. `Goldens` rewrites a file instead of comparing it
/// when `UPDATE_GOLDENS=1`, which `make goldens UPDATE=1` sets.
fn goldens() -> Goldens {
    Goldens::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/goldens"))
}

/// Compare `actual` to the golden at `rel`, or rewrite it when updating.
/// Returns the failure instead of panicking so one run reports every stale
/// or missing golden at once.
fn check(goldens: &Goldens, rel: &str, actual: &str) -> Option<String> {
    goldens.check(rel, actual).err().map(|err| err.to_string())
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
    let goldens = goldens();
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
            check(
                &goldens,
                &format!("help/{name}.txt"),
                &support::text(&out.stdout),
            )
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
    let help = support::text(&fx.ok(&["--help"]).stdout);

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
fn status_output_matches_goldens() {
    let fx = Fixture::new();
    let goldens = goldens();
    let failures: Vec<String> = STATUS_OUTPUT
        .iter()
        .filter_map(|(name, args)| {
            let out = fx.ok(args);
            check(
                &goldens,
                &format!("output/{name}"),
                &support::text(&out.stdout),
            )
        })
        .collect();
    report(&failures);
}
// example:begin

#[test]
fn output_matches_goldens() {
    let fx = Fixture::new();
    fx.seed();
    let goldens = goldens();
    let failures: Vec<String> = OUTPUT
        .iter()
        .filter_map(|(name, args)| {
            let out = fx.ok(args);
            check(
                &goldens,
                &format!("output/{name}"),
                &cw_testkit::redact_timestamps(&support::text(&out.stdout)),
            )
        })
        .collect();
    report(&failures);
}
// example:end

#[test]
fn usage_environment_table_matches_the_declarations() {
    let doc = fs::read_to_string(USAGE).unwrap();
    let start = doc
        .find(TABLE_BEGIN)
        .map(|at| at + TABLE_BEGIN.len())
        .expect("docs/usage.md has no generated environment table marker");
    let end = doc[start..]
        .find(TABLE_END)
        .map(|len| start + len)
        .expect("docs/usage.md has no end marker after the environment table");
    let table = tmpl_cli::env_table();
    if doc[start..end] == table {
        return;
    }
    if goldens().is_updating() {
        fs::write(USAGE, format!("{}{table}{}", &doc[..start], &doc[end..])).unwrap();
        return;
    }
    report(&[format!(
        "docs/usage.md: the environment table differs from the declarations.\n\
         --- in docs/usage.md\n{}--- rendered\n{table}",
        &doc[start..end]
    )]);
}
