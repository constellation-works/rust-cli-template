---
type: architecture
summary: "The tmpl-cli binary crate as it is: the command line, each command's view, and the MCP server."
created_date: 2026-11-25
updated_date: 2026-11-25
---

# tmpl-cli

The binary crate. Its module tree and the rules it is held to are in
[ARCHITECTURE.md](../../ARCHITECTURE.md#tmpl-cli-binary).
Why it is shaped this way is in the feature design docs listed on the
[feature board](../design/FEATURES.md).
<!-- example:begin -->
For the template's own example, start with
[2_design.md](../design/tmpl-cli/2_design.md).
<!-- example:end -->

## What it owns

- The one read of the arguments, in `main`.
- The command tree: every command's arguments, help and examples, declared
  once with clap.
- The mapping from parsed arguments to an app's `Command`, and from its
  `Outcome` to a payload.
- Each command's human view (a table or a detail text) and its JSON.
- The remedy printed for an error, and the surface's own failures.
- The MCP server: one tool per command, with protocol output handled by
  `cw-mcp`.
- CLI payloads and error output go through the kit's `cw-cli` sink.
  `cw-cli` initializes logging separately, outside the sink.

## Main modules

- [main.rs](../../crates/tmpl-cli/src/main.rs): reads the arguments, has
  the core capture the environment, calls `run`.
- [lib.rs](../../crates/tmpl-cli/src/lib.rs): `run`, the wiring from the
  arguments to the rendered result.
- [cli.rs](../../crates/tmpl-cli/src/cli.rs): the root command, the global
  flags and the grouped root help.
- [command/](../../crates/tmpl-cli/src/command/): one module per noun, its
  arguments and its handlers.
- [mcp/](../../crates/tmpl-cli/src/mcp/): the tool list and the server.
- [output/](../../crates/tmpl-cli/src/output/): the payloads and each
  command's view.
- [error.rs](../../crates/tmpl-cli/src/error.rs): the remedies and the
  surface's own error codes.

## Dependencies

`tmpl-cli-core`, and the kit's `cw-cli` (the terminal, the sink, error
rendering, logging and help) and `cw-mcp` (the MCP server). It names no
store, database or file primitive; the full list of what it may and may not
depend on is the table in [ARCHITECTURE.md](../../ARCHITECTURE.md#crates).

## Entry points

- The `tmpl-cli` executable: `main`, then `tmpl_cli::run`.
- `tmpl-cli mcp`: the MCP server on stdin and stdout.
- For tests, the library exports `run`, `every_command` (the command tree)
  and `mcp_server` (the server over a runtime).
