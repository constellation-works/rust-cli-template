# Using tmpl-cli

Run `tmpl-cli --help` for command groups and global flags, or add `--help`
after a command for its arguments and examples.

```sh
tmpl-cli status
tmpl-cli status --json
```

`status` shows the data directory and the store's format (`-` in a pipe and
`null` with `--json` until a command has created the database). It reads
only and creates nothing. A command displays a table on a terminal and
tab-separated lines in a pipe. With `--json`, a result is one JSON document
on stdout, and errors are a single object with `error` and `code` on stderr.
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

## Configuration

| Setting | Flag | Environment | Default |
|---------|------|-------------|---------|
| Data directory | `--root <DIR>` | `TMPL_CLI_ROOT` | `~/.tmpl-cli` |
| Output format | `--format auto\|table\|json`, `--json` | `TMPL_CLI_FORMAT` | `auto` |
| Color | — | `NO_COLOR`, `CLICOLOR_FORCE`, `TERM` | on for a terminal |
| Log level (stderr) | — | `TMPL_CLI_LOG` (e.g. `debug`) | `warn` |

Records live in a SQLite database, `store.db`, under the data directory,
with SQLite's `store.db-wal` and `store.db-shm` beside it. A mutating
command appends one event to `audit.jsonl` with the record ID and outcome;
`.audit.lock` is its lock file. Every file is `0600` and the directory
`0700` on Unix. Reading commands change no record and write no audit line;
in a writable directory a read of an existing database may rebuild
`store.db-shm` (SQLite's index, not your data) and creates a missing
`store.db-wal` (empty) or `store.db-shm`, owner-only; in a directory you
cannot write, a read writes nothing.

## Troubleshooting

Exit status is 0 on success, 1 when a command fails and 2 on a usage error.
If a file cannot be found, check the resolved data directory (`tmpl-cli
status`) and use `--root <DIR>` to select a different one. If the CLI
refuses a file's permissions, follow the remedy in the error. The store
creates its own directories and files with owner-only modes on Unix.
