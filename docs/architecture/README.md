---
type: architecture
summary: How one tmpl-cli invocation flows end to end through the three crates, with a page per crate.
created_date: 2026-11-25
updated_date: 2026-11-25
---

# Architecture overview

This folder describes the code as it is. The layering rules and the
dependency table, which a script enforces, live in one place only: the root
[ARCHITECTURE.md](../../ARCHITECTURE.md). Why the code is shaped this way is
in the feature design docs on the [feature board](../design/FEATURES.md).
<!-- example:begin -->
For the template's own example, start with its
[design](../design/tmpl-cli/2_design.md).
<!-- example:end -->

## Components

- [tmpl-cli](tmpl-cli.md): the binary, the command line and the MCP server.
- [tmpl-cli-core](tmpl-cli-core.md): the apps and composition.
- [tmpl-cli-store](tmpl-cli-store.md): the data directory and the database.

## Data flow

One command line, from the arguments to the bytes on stdout:

1. **Entry.** `main` in [tmpl-cli](tmpl-cli.md) reads the arguments and
   asks [tmpl-cli-core](tmpl-cli-core.md) to capture the environment once
   (`capture_env`). It hands both to `tmpl_cli::run`.
2. **Parse and decide.** `run` probes the streams, parses the arguments
   with clap, and lets the kit's `cw-cli` decide the output mode, colour
   and width from the probe and the captured environment, then opens the
   kit's output sink.
3. **Compose.** `tmpl_cli_core::start` resolves the data directory from
   `--root` and the environment, opens the store in
   [tmpl-cli-store](tmpl-cli-store.md), and builds the runtime: the store,
   the clock and the audit log.
4. **Execute.** The command's handler maps the arguments to its app's
   `Command` and calls the app's `exec`, which forwards to the kit's
   chokepoint, `cw_app::exec`. The chokepoint starts the audit record of a
   mutating command, runs the app's flow, and returns the outcome with the
   pending record.
5. **Store.** A flow reads through one `Store::read` snapshot, decides with
   its app's pure rules, and writes in one `Store::write` transaction; the
   record repositories hold the SQL.
6. **Render and settle.** The handler turns the outcome into a payload,
   renders it through the sink (a table, tab-separated lines or JSON), then
   settles the audit record. A failure goes to stderr through the kit's
   error report, with the exit code its kind calls for.

`tmpl-cli mcp` takes the same path from step 3: each MCP tool's input is
its command's arguments, and the tool calls the same handler, so it reaches
the same `exec` and returns what `--json` prints.
