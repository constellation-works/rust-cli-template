use crate::cli::{Cli, Invocation, every_command, raw_requested_format};
use crate::command::Command;
use clap::{CommandFactory, Parser};
use cw_cli::Format;
use cw_cli::help::{SUMMARY_LIMIT, check_summaries};
use std::ffi::OsString;

#[test]
fn the_command_tree_is_internally_consistent() {
    // Catches colliding long flags and other declaration errors (STD-01 R3).
    Cli::command().debug_assert();
}

#[test]
fn global_flags_parse_before_and_after_the_subcommand() {
    // STD-01 R4.
    let before =
        Cli::try_parse_from(["tmpl-cli", "--format", "json", "--root", "/r", "status"]).unwrap();
    let after =
        Cli::try_parse_from(["tmpl-cli", "status", "--format", "json", "--root", "/r"]).unwrap();
    for cli in [before, after] {
        assert_eq!(cli.global.requested_format(), Some(Format::Json));
        assert_eq!(cli.global.root.as_deref(), Some(std::path::Path::new("/r")));
        assert!(matches!(cli.command, Invocation::Run(Command::Status(_))));
    }
}

#[test]
fn json_is_shorthand_for_format_json_and_conflicts_with_format() {
    let cli = Cli::try_parse_from(["tmpl-cli", "status", "--json"]).unwrap();
    assert_eq!(cli.global.requested_format(), Some(Format::Json));
    for conflicting in [
        &["tmpl-cli", "--json", "status", "--format", "table"][..],
        &["tmpl-cli", "status", "--json", "--format", "auto"][..],
    ] {
        let args: Vec<OsString> = conflicting.iter().map(OsString::from).collect();
        let cli = Cli::parse_args(&args, None).unwrap();
        let err = cli.global.check().unwrap_err();
        assert_eq!(
            err.kind(),
            tmpl_cli_core::Kind::Usage,
            "{conflicting:?} should be a usage error"
        );
    }
    let redundant: Vec<OsString> = ["tmpl-cli", "--json", "status", "--format", "json"]
        .iter()
        .map(OsString::from)
        .collect();
    let cli = Cli::parse_args(&redundant, None).unwrap();
    assert!(cli.global.check().is_ok());
}

/// Every subcommand path in the unbuilt tree, including the root.
fn all_commands(
    cmd: &clap::Command,
    path: &mut Vec<String>,
    out: &mut Vec<(String, clap::Command)>,
) {
    out.push((path.join(" "), cmd.clone()));
    for sub in cmd.get_subcommands() {
        path.push(sub.get_name().to_owned());
        all_commands(sub, path, out);
        path.pop();
    }
}

#[test]
fn every_command_and_flag_has_help_text() {
    // STD-01 R22.
    for (path, cmd) in every_command() {
        if path != "tmpl-cli" {
            assert!(cmd.get_about().is_some(), "`{path}` has no help text");
        }
        for arg in cmd.get_arguments() {
            let builtin = matches!(arg.get_id().as_str(), "help" | "version");
            assert!(
                builtin || arg.get_help().is_some(),
                "`{path}` argument `{}` has no help text",
                arg.get_id()
            );
            if !builtin && !arg.is_positional() {
                let help = arg.get_help().unwrap().to_string();
                assert!(
                    help.contains("[default:"),
                    "`{path}` flag `--{}` help states no default: {help:?}",
                    arg.get_long().unwrap_or(arg.get_id().as_str())
                );
            }
        }
    }
}

/// True when `text` contains something shaped like a tracker id: `ABC-123`
/// or `F2026-09-042`.
fn has_tracker_id(text: &str) -> Option<String> {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-'))
        .find(|word| {
            let Some((prefix, rest)) = word.split_once('-') else {
                return false;
            };
            let prefix_ok = prefix.len() >= 2 && prefix.chars().all(|ch| ch.is_ascii_uppercase());
            let dated = prefix.len() == 5
                && prefix.starts_with('F')
                && prefix[1..].chars().all(|ch| ch.is_ascii_digit());
            let digits = |text: &str| {
                !text.is_empty() && text.chars().all(|ch| ch.is_ascii_digit() || ch == '-')
            };
            (prefix_ok || dated) && digits(rest)
        })
        .map(str::to_owned)
}

#[test]
fn help_text_carries_no_tracker_ids() {
    // STD-01 R23: placeholders like <id>, never real task or friction numbers.
    assert!(
        has_tracker_id("see ABC-123").is_some(),
        "detector must work"
    );
    assert!(
        has_tracker_id("see F2026-09-042").is_some(),
        "detector must work"
    );
    for (path, mut cmd) in every_command() {
        let help = cmd.render_long_help().to_string();
        assert_eq!(
            has_tracker_id(&help),
            None,
            "`{path} --help` mentions a tracker id"
        );
    }
}

#[test]
fn no_subcommand_argument_shares_an_id_with_a_global_one() {
    // STD-01 R26: clap merges arguments that share an id, so a subcommand
    // field named like a global flag would silently feed the global. Walk the
    // tree before `build()` propagates the globals into every subcommand.
    let root = Cli::command();
    let globals: Vec<String> = root
        .get_arguments()
        .filter(|arg| arg.is_global_set())
        .map(|arg| arg.get_id().to_string())
        .collect();
    assert!(globals.len() >= 3, "the walk must see the global flags");
    let mut unbuilt = Vec::new();
    all_commands(&root, &mut vec![String::from("tmpl-cli")], &mut unbuilt);
    for (path, cmd) in unbuilt.into_iter().skip(1) {
        for arg in cmd.get_arguments() {
            let id = arg.get_id().as_str();
            assert!(
                !globals.iter().any(|global| global == id),
                "`{path}` argument `{id}` shares its id with a global argument"
            );
        }
    }
}

#[test]
fn every_command_summary_is_one_short_line() {
    // STD-01 R22: the grouped root help keeps one line per command.
    let (_, root) = every_command().remove(0);
    check_summaries(&root, SUMMARY_LIMIT).unwrap();
}

#[test]
fn a_command_line_that_does_not_parse_still_names_its_format() {
    // STD-01 R19: the usage error is reported in the format asked for.
    let requested = |args: &[&str]| {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        raw_requested_format(&args)
    };
    assert_eq!(
        requested(&["tmpl-cli", "bogus", "--json"]),
        Some(Format::Json)
    );
    assert_eq!(
        requested(&["tmpl-cli", "--format=json", "x"]),
        Some(Format::Json)
    );
    assert_eq!(
        requested(&["tmpl-cli", "--format", "json", "--format", "table"]),
        Some(Format::Table)
    );
    assert_eq!(
        requested(&["tmpl-cli", "--format", "nonsense"]),
        Some(Format::Auto)
    );
    assert_eq!(requested(&["tmpl-cli", "x", "--", "--json"]), None);
    assert_eq!(requested(&["tmpl-cli", "x"]), None);
}
