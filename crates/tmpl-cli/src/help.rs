//! The root command groups use the summaries from the clap declarations.

use clap::Command;
use clap::builder::StyledStr;
use std::fmt::Write;

const GROUPS: &[(&str, &[&str])] = &[
    // example:begin
    ("Notes", &["note"]),
    // example:end
    ("Store", &["status"]),
    ("Help", &["help"]),
];

const COMMANDS_HEADING: &str = "__ROOT_SUBCOMMANDS__";

/// Replace the root command section and keep clap's usage, flags and footer.
pub(crate) fn with_groups(cmd: Command, width: Option<usize>) -> Command {
    let mut preview = cmd.clone().subcommand_help_heading(COMMANDS_HEADING);
    let help = preview.render_help().to_string();
    let marker = format!("\n{COMMANDS_HEADING}:\n");
    let Some((before, commands_and_options)) = help.split_once(&marker) else {
        return cmd;
    };
    let Some((_, after)) = commands_and_options.split_once("\n\n") else {
        return cmd;
    };
    let mut rendered = StyledStr::new();
    rendered.push_str(before);
    rendered.push_str("\n");
    rendered.push_str(&render_groups(&preview, width));
    rendered.push_str(after);
    cmd.override_help(rendered)
}

fn render_groups(cmd: &Command, width: Option<usize>) -> String {
    let longest_name = GROUPS
        .iter()
        .flat_map(|(_, names)| names.iter())
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for (heading, names) in GROUPS {
        let _ = writeln!(out, "{heading}:");
        for name in *names {
            if let Some(summary) = cmd
                .find_subcommand(name)
                .filter(|command| !command.is_hide_set())
                .and_then(Command::get_about)
            {
                write_row(&mut out, name, &summary.to_string(), longest_name, width);
            }
        }
        out.push('\n');
    }
    out
}

fn write_row(
    out: &mut String,
    name: &str,
    summary: &str,
    longest_name: usize,
    width: Option<usize>,
) {
    let column = longest_name + 4;
    let mut prefix = format!("  {name:longest_name$}  ");
    let Some(width) = width else {
        let _ = writeln!(out, "{prefix}{summary}");
        return;
    };
    let mut indent = column;
    let longest_word = summary
        .split_whitespace()
        .map(|word| word.chars().count())
        .max();
    if longest_word.is_some_and(|word| word > width.saturating_sub(column)) {
        let _ = writeln!(out, "  {name}");
        indent = 2;
        prefix = "  ".to_owned();
    }
    out.push_str(&prefix);
    let mut used = indent;
    for word in summary.split_whitespace() {
        let len = word.chars().count();
        if used > indent {
            if used + 1 + len > width {
                let _ = write!(out, "\n{:indent$}", "");
                used = indent;
            } else {
                out.push(' ');
                used += 1;
            }
        }
        out.push_str(word);
        used += len;
    }
    out.push('\n');
}
