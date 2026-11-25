# Using tmpl-cli

Run `tmpl-cli --help` for command groups and global flags. Run
`tmpl-cli note --help` for the verbs, or add `--help` after a verb for its
arguments and examples.

```sh
tmpl-cli note list --tag home
tmpl-cli note list --json | jq '.notes[].title'
tmpl-cli note show <id>
```

`note list` displays a table on a terminal and tab-separated lines in a pipe.
Use an ID from the list with `note show`. With `--json`, a list is
`{"notes": [...], "total": N, "truncated": false}`, one note is an object,
and errors are a single object with `error` and `code` on stderr.

## Configuration

| Setting | Flag | Environment | Default |
|---------|------|-------------|---------|
| Data directory | `--root <DIR>` | `TMPL_CLI_ROOT` | `~/.tmpl-cli` |
| Output format | `--format auto\|table\|json`, `--json` | `TMPL_CLI_FORMAT` | `auto` |
| Color | — | `NO_COLOR`, `CLICOLOR_FORCE`, `TERM` | on for a terminal |
| Log level (stderr) | — | `TMPL_CLI_LOG` (e.g. `debug`) | `warn` |

Notes live in a SQLite database, `notes.db`, under the data directory, with
SQLite's `notes.db-wal` and `notes.db-shm` beside it. Each `note add` appends
one event to `audit.jsonl` with the note ID and outcome; `.audit.lock` is its
lock file. Every file is `0600` and the directory `0700` on Unix. Reading
commands change no note and write no audit line; in a writable directory a
read may rebuild `notes.db-shm` (SQLite's index, not your notes) and
creates a missing `notes.db-wal` (empty) or `notes.db-shm`, owner-only; in a
directory you cannot write, a read writes nothing.

## Troubleshooting

Exit status is 0 on success, 1 when a command fails and 2 on a usage error.
If a file cannot be found, check the resolved data directory and use
`--root <DIR>` to select a different one. If the CLI refuses a file's
permissions, follow the remedy in the error. The store creates its own
directories and files with owner-only modes on Unix.
