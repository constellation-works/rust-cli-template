use crate::cli::Cli;
use crate::commands::{Command, NoteCommand};
use crate::output::FormatArg;
use clap::{CommandFactory, Parser};
use std::ffi::OsString;

#[test]
fn the_command_tree_is_internally_consistent() {
    // Catches colliding long flags and other declaration errors (STD-01 R3).
    Cli::command().debug_assert();
}

#[test]
fn global_flags_parse_before_and_after_the_subcommand() {
    // STD-01 R4.
    let before = Cli::try_parse_from([
        "tmpl-cli", "--format", "json", "--root", "/r", "note", "list",
    ])
    .unwrap();
    let after = Cli::try_parse_from([
        "tmpl-cli", "note", "list", "--format", "json", "--root", "/r",
    ])
    .unwrap();
    for cli in [before, after] {
        assert_eq!(cli.global.requested_format(), Some(FormatArg::Json));
        assert_eq!(cli.global.root.as_deref(), Some(std::path::Path::new("/r")));
        assert!(matches!(cli.command, Command::Note(NoteCommand::List(_))));
    }
}

#[test]
fn json_is_shorthand_for_format_json_and_conflicts_with_format() {
    let cli = Cli::try_parse_from(["tmpl-cli", "note", "list", "--json"]).unwrap();
    assert_eq!(cli.global.requested_format(), Some(FormatArg::Json));
    for conflicting in [
        &["tmpl-cli", "--json", "note", "list", "--format", "table"][..],
        &["tmpl-cli", "note", "list", "--json", "--format", "auto"][..],
    ] {
        let args: Vec<OsString> = conflicting.iter().map(OsString::from).collect();
        let err = Cli::parse_args(&args).unwrap_err();
        assert_eq!(
            err.exit_code(),
            2,
            "{conflicting:?} should be a usage error"
        );
    }
    let redundant: Vec<OsString> = ["tmpl-cli", "--json", "note", "list", "--format", "json"]
        .iter()
        .map(OsString::from)
        .collect();
    assert!(Cli::parse_args(&redundant).is_ok());
}

/// Every subcommand path in the tree, including the root.
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

fn every_command() -> Vec<(String, clap::Command)> {
    let mut root = Cli::command();
    root.build();
    let mut out = Vec::new();
    all_commands(&root, &mut vec![String::from("tmpl-cli")], &mut out);
    out
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
        }
    }
}

/// True when `text` contains something shaped like a tracker id: `ABC-123`
/// or `F2026-09-042`.
fn has_tracker_id(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .find(|word| {
            let Some((prefix, rest)) = word.split_once('-') else {
                return false;
            };
            let prefix_ok = prefix.len() >= 2 && prefix.chars().all(|c| c.is_ascii_uppercase());
            let dated = prefix.len() == 5
                && prefix.starts_with('F')
                && prefix[1..].chars().all(|c| c.is_ascii_digit());
            let digits =
                |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '-');
            (prefix_ok || dated) && digits(rest)
        })
        .map(str::to_owned)
}

#[test]
fn help_text_carries_no_tracker_ids() {
    // STD-01 R23: placeholders like <id>, never real task or friction numbers.
    assert!(
        has_tracker_id("see ORB-123").is_some(),
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
        .filter(|a| a.is_global_set())
        .map(|a| a.get_id().to_string())
        .collect();
    assert!(globals.len() >= 3, "the walk must see the global flags");
    let mut unbuilt = Vec::new();
    all_commands(&root, &mut vec![String::from("tmpl-cli")], &mut unbuilt);
    for (path, cmd) in unbuilt.into_iter().skip(1) {
        for arg in cmd.get_arguments() {
            let id = arg.get_id().as_str();
            assert!(
                !globals.iter().any(|g| g == id),
                "`{path}` argument `{id}` shares its id with a global argument"
            );
        }
    }
}

/// Split one documented shell command line into words: whitespace outside
/// quotes separates, `'…'` and `"…"` group. Enough for documentation.
fn shell_words(line: &str) -> Vec<String> {
    let (mut words, mut word, mut in_word, mut quote) = (Vec::new(), String::new(), false, None);
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(word);
    }
    words
}

/// The `tmpl-cli …` part of a documented line: before a pipe or a comment,
/// with the `<id>` placeholder filled in.
fn documented_invocation(line: &str) -> Option<Vec<OsString>> {
    let line = line.trim_start();
    if !line.starts_with("tmpl-cli ") {
        return None;
    }
    let command = line.split(" | ").next().unwrap_or(line);
    let command = command.split("  #").next().unwrap_or(command);
    let words = shell_words(&command.replace("<id>", "1"));
    Some(words.into_iter().map(OsString::from).collect())
}

#[test]
fn every_documented_command_line_parses() {
    // STD-04 R14: examples in help and in the README are run through the real
    // parser, so an example naming a removed flag or value fails here.
    let mut documented: Vec<(String, String)> = Vec::new();
    for (path, mut cmd) in every_command() {
        let help = cmd.render_long_help().to_string();
        let examples = help.split("Examples:").nth(1).unwrap_or_default();
        documented.extend(
            examples
                .lines()
                .map(|l| (format!("`{path} --help`"), l.to_owned())),
        );
    }
    let readme = concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md");
    let readme = std::fs::read_to_string(readme).expect("README.md next to the workspace");
    documented.extend(
        readme
            .lines()
            .map(|l| ("README.md".to_owned(), l.to_owned())),
    );

    let mut parsed = 0;
    for (source, line) in &documented {
        let Some(args) = documented_invocation(line) else {
            continue;
        };
        if let Err(err) = Cli::parse_args(&args) {
            panic!("{source}: `{}` does not parse:\n{err}", line.trim());
        }
        parsed += 1;
    }
    assert!(parsed >= 10, "only {parsed} documented command lines found");
}
