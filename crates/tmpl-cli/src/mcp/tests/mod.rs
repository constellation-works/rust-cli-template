//! Unit tests for the MCP surface, one file per source file (STD-02 R19).

// example:begin
mod input;

// example:end
use crate::cli::every_command;
use crate::command::StatusArgs;
use crate::mcp::tools;

/// Every command a user can run, as a path: a leaf of the tree other than
/// `mcp` itself, which serves the others.
fn runnable_commands() -> Vec<String> {
    every_command()
        .into_iter()
        .filter(|(path, cmd)| {
            path != "tmpl-cli" && cmd.get_subcommands().next().is_none() && path != "tmpl-cli mcp"
        })
        .map(|(path, _)| path.trim_start_matches("tmpl-cli ").to_owned())
        .collect()
}

#[test]
fn every_command_has_exactly_one_tool_and_every_tool_a_command() {
    // STD-01 R25, R36: one declaration, every surface derived from it.
    let mut commands: Vec<String> = runnable_commands()
        .iter()
        .map(|path| path.replace(' ', "_"))
        .collect();
    commands.sort();
    let mut names: Vec<String> = tools().iter().map(|tool| tool.name().to_owned()).collect();
    names.sort();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "two tools share a name");
    assert_eq!(names, commands, "tools and commands differ");
}

#[test]
fn a_tools_description_is_its_commands_summary() {
    for tool in tools() {
        assert!(
            !tool.description().is_empty(),
            "`{}` has no description",
            tool.name()
        );
    }
}

#[test]
fn an_input_refuses_an_argument_its_command_does_not_have() {
    // STD-01 R36: nothing is accepted that no runtime path reads.
    assert!(serde_json::from_value::<StatusArgs>(serde_json::json!({})).is_ok());
    assert!(serde_json::from_value::<StatusArgs>(serde_json::json!({"root": "/x"})).is_err());
}
