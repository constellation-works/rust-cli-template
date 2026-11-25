---
type: guide
summary: How to run tmpl-cli: its commands, output modes, data directory, environment variables and exit status.
created_date: 2026-11-22
updated_date: 2026-11-25
---

# Using tmpl-cli

Run `tmpl-cli --help` for command groups and global flags, or add `--help`
after a command for its arguments and examples.

```sh
tmpl-cli status
tmpl-cli status --json
```

`status` shows the data directory and the store's format (`-` in a pipe and
`null` with `--json` until a command has created the database). It reads
only and creates nothing.

The command line's behaviour is the cw-kit's (`cw-cli`), shared by every
constellation CLI. A command displays a table on a terminal and
tab-separated lines in a pipe, and writes only its result to stdout;
notices, warnings and errors go to stderr. A failure is an `error:` line,
followed by a `hint:` line when there is a known fix. With `--json`, a
result is one JSON document on one line of stdout, and a failure is one
object on stderr: `{"error": ..., "code": ..., "hint": ...}`, with `hint`
`null` when there is none. An ESC or other terminal control character in
stored text is shown as an escape (`\u{1b}`), never sent to the terminal
raw.
<!-- example:begin -->

## Notes

Run `tmpl-cli note --help` for the verbs.

```sh
tmpl-cli note add "Call the plumber" --tag home
tmpl-cli note list --tag home
tmpl-cli note list --json | jq '.notes[].title'
tmpl-cli note show <id>
```

Use an ID from `note list` with `note show`. With `--json`, a list is
`{"notes": [...], "total": N, "truncated": false}` and one note is an
object. Each `note add` appends one event to `audit.jsonl` with the note ID
and outcome.
<!-- example:end -->

## MCP server

`tmpl-cli mcp` serves every command as a Model Context Protocol tool on
stdin and stdout, for an agent's MCP client, until the client closes its
input:

```sh
tmpl-cli --root <DIR> mcp
```

Each tool is named after its command, with `_` between the words
(`status`), takes the command's arguments as JSON fields, and returns what
the command prints with `--json`. A failure is a tool error carrying the
same `code` the command line reports. A change made through a tool is
audited with the surface `mcp` and the client's name as the actor.
<!-- example:begin -->
The note tools are `note_add`, `note_list` and `note_show`.
<!-- example:end -->

## Configuration

Every variable the CLI reads, generated from their declarations (the
kit's, then the CLI's own); `make goldens UPDATE=1` rewrites it:

<!-- generated: environment table -->
| Variable | Meaning | Default | Flag |
|---|---|---|---|
| `TMPL_CLI_ROOT` | The data directory. | `$HOME/.<app>` | `--root` |
| `TMPL_CLI_FORMAT` | The output format. | — | — |
| `TMPL_CLI_LOG` | The log filter. | — | — |
| `NO_COLOR` | Any non-empty value turns color off. | — | — |
| `TERM` | The terminal type, such as `dumb`. | — | — |
| `HOME` | The home directory; the default data directory is under it. | — | — |
| `USER` | The user the environment claims, recorded as the actor; a claim, not an identity. | — | — |
| `USERNAME` | The claimed user when the one above is unset, empty or not UTF-8, on every platform. | — | — |
| `CLICOLOR_FORCE` | Any non-empty value turns color on at a terminal, even for `TERM=dumb`. | — | — |
| `COLUMNS` | The terminal width to fit tables to. | — | — |
<!-- end generated -->

Unset, the output format is `auto` (`--format auto|table|json`, `--json`),
logging is at `warn` (try `debug`), color is on for a terminal, tables fit
the terminal's width (`COLUMNS`, else the size the terminal reports), and
the audit log records no actor. A `TMPL_CLI_LOG` that does not parse is
reported once as a warning and logging stays at `warn`.

The environment is read once at start. An empty variable counts as unset
(`TMPL_CLI_LOG=` logs at `warn`).

Records live in a SQLite database, `store.db`, under the data directory,
with SQLite's `store.db-wal` and `store.db-shm` beside it. A mutating
command appends one line with the record ID and outcome to the audit log,
`audit.jsonl`; `.audit.lock` is its lock file. Every file is `0600` and the
directory `0700` on Unix. Reading commands change no record and write no
audit line; in a writable directory a read of an existing database may
rebuild `store.db-shm` (SQLite's index, not your data) and creates a missing
`store.db-wal` (empty) or `store.db-shm`, owner-only; in a directory you
cannot write, a read writes nothing.

## Audit log

`audit.jsonl` holds one JSON line per command that changes a record: when it
started (`at`), the surface (`cli` or `mcp`), the actor (`USER`, else
`USERNAME`, on the command line; the client's name through a tool; written
as `<invalid>` when it is not a plain identifier), the command, its
`status`, the record it acted on (`target`), the error `code` on failure and
`duration_ms`. It never holds an argument value or an error message. Lines are
written in `format` 3, the cw-kit's audit format. A data directory from an
earlier build keeps its format 1 and 2 lines; they stay readable, and new
lines follow them in format 3. Earlier builds wrote `at` to the second;
format 3 keeps fractions of a second. If a line cannot be written, the
command still succeeds or fails on its own and prints
`warning: the audit line for `<command>` was not written: <reason>` on
stderr after its output.

## Troubleshooting

Exit status is 0 on success, 1 when a command fails and 2 on a usage error;
the kit maps each kind of failure to its code.
If a file cannot be found, check the resolved data directory (`tmpl-cli
status`) and use `--root <DIR>` to select a different one. If the CLI
refuses a file's permissions, follow the remedy in the error. The store
creates its own directories and files with owner-only modes on Unix.
