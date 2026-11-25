//! The MCP surface: every command is also a tool, served by the kit's
//! `cw-mcp` on standard input and output from `tmpl-cli mcp`.
//!
//! A tool is a second mapping onto the same command, not a second command:
//! its input is the command's own clap `Args` struct (which also derives
//! `Deserialize` and `JsonSchema`; a text argument is read through its
//! type's `FromStr`, as the example's `input.rs` shows), its description is the
//! command's summary, and its function wraps the input in the CLI's
//! [`Command`] and calls the same handler, [`command::run`], which runs the
//! app through `app::<app>::exec` and the kit's `cw_app::exec`. So the tool
//! holds no rule, store call or file access, and its result is the value
//! `--json` prints ([`Payload::json`]). A failure is the command's own
//! error, with the template's code, which the kit answers as a tool error.
//!
//! The server runs against the runtime `run` started from the environment
//! `main` captured; this module reads no environment and never writes to
//! standard output, which carries the protocol. Warnings go to the log.
//!
//! To add a tool for a new command: give its `Args` the same derives as
//! `StatusArgs`, and add one line to [`tools`].

// example:begin
pub(crate) mod input;

// example:end
use crate::cli::Cli;
use crate::command::{self, Command, StatusArgs};
// example:begin
use crate::command::{AddArgs, ListArgs, NoteCommand, ShowArgs};
// example:end
use crate::error::{TOOL_PANICKED, encode_error, mcp_error};
use crate::output::Payload;
use clap::{Args, CommandFactory};
use cw_mcp::schemars::JsonSchema;
use cw_mcp::{Access, Server, ServerInfo, Tool};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tmpl_cli_core::{APP, Error, Origin, Runtime, State};

/// Arguments to `mcp`: none of its own; `--root` picks the data directory
/// every tool runs against.
#[derive(Debug, Args)]
pub(crate) struct McpArgs {}

pub(crate) const MCP_EXAMPLES: &str = "\
Examples:
  tmpl-cli mcp
  tmpl-cli --root ~/data mcp

Point an MCP client at the command. It serves until its input closes.";

/// Every tool, one per command, in the order `tools/list` gives them.
pub(crate) fn tools() -> Vec<Tool<State>> {
    vec![
        tool::<StatusArgs>(&["status"], Access::ReadOnly, Command::Status),
        // example:begin
        tool::<AddArgs>(&["note", "add"], Access::Mutating, |args| {
            Command::Note(NoteCommand::Add(args))
        }),
        tool::<ListArgs>(&["note", "list"], Access::ReadOnly, |args| {
            Command::Note(NoteCommand::List(args))
        }),
        tool::<ShowArgs>(&["note", "show"], Access::ReadOnly, |args| {
            Command::Note(NoteCommand::Show(args))
        }),
        // example:end
    ]
}

/// The server over `runtime`: the program's name and version, and every
/// tool.
///
/// # Errors
///
/// `mcp_tool_list` when the kit refuses the tool list.
pub fn server(runtime: Runtime) -> Result<Server<State>, Error> {
    let info = ServerInfo::new(APP, env!("CARGO_PKG_VERSION"), TOOL_PANICKED);
    Server::new(info, runtime, tools()).map_err(mcp_error)
}

/// Serve every tool on standard input and output until the input closes.
pub(crate) fn serve(runtime: Runtime) -> Result<(), Error> {
    server(runtime)?.serve_stdio().map_err(mcp_error)
}

/// The tool for the command at `path`: named by the path joined with `_`,
/// described by the command's summary, and running `wrap(input)` through
/// the command's handler.
fn tool<I>(path: &[&str], access: Access, wrap: fn(I) -> Command) -> Tool<State>
where
    I: DeserializeOwned + JsonSchema + Send + 'static,
{
    Tool::new::<I>(
        path.join("_"),
        summary(path),
        access,
        move |runtime, origin, input| call(runtime, origin, wrap(input)),
    )
}

/// The command's one-line summary, from its clap declaration (STD-01 R25).
fn summary(path: &[&str]) -> String {
    let mut cmd = Cli::command();
    for name in path {
        match cmd.find_subcommand(name) {
            Some(sub) => cmd = sub.clone(),
            None => return String::new(),
        }
    }
    cmd.get_about().map(ToString::to_string).unwrap_or_default()
}

/// Run `command` through the same handler the command line uses, settle its
/// audit record, and return what `--json` prints.
fn call(runtime: &Runtime, origin: &Origin, command: Command) -> Result<Value, Error> {
    let (outcome, warning) = command::run(command, runtime, origin, |outcome| outcome);
    if let Some(warning) = warning {
        tracing::warn!("{warning}");
    }
    let output = outcome?;
    for warning in &output.warnings {
        tracing::warn!("{warning}");
    }
    json(&output.payload)
}

/// The payload as the tool's JSON: the same value `--json` prints.
fn json(payload: &Payload) -> Result<Value, Error> {
    serde_json::to_value(payload.json()).map_err(encode_error)
}

#[cfg(test)]
mod tests;
