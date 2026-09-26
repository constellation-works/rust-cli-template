---
type: design
summary: "Spec: output contract — streams, modes, JSON fields, errors and exit codes"
last_validated: 2026-09-26
---

# Spec: Output contract

Every data command renders one typed payload in the mode the sink resolved;
stdout carries only that payload; errors and notices go to stderr; the exit
code says whether it worked. The JSON form is a public contract.

## Why This Exists

Three readers — a person at a terminal, a script in a pipe, an agent through
a shell tool — need different bytes from the same result, and scripts and
agents break silently when the machine form drifts.

## Modes

| Mode | Chosen by | Shape |
|------|-----------|-------|
| `table` | `--format table`, or auto on a terminal | header row + one line per record; `show` is one field per line |
| `plain` | auto off a terminal | one tab-separated line per record, no header: `id priority title tags created_at` |
| `json` | `--json`, `--format json`, or `TMPL_CLI_FORMAT=json` | one document: the list envelope for `list`, a note object for `show`/`add` |

Precedence: flag > `TMPL_CLI_FORMAT` > auto. An unrecognized environment
value means auto. Off a terminal there are no escapes and no truncation.

## JSON list envelope

`note list --json` is always
`{"notes": [<note>, ...], "total": <integer>, "truncated": <boolean>}`:
`total` counts the notes that matched the filters, and `truncated` is true
when `--limit` returned fewer. No matches is `{"notes": [], "total": 0,
"truncated": false}`. When a list is cut, stderr also says `showing N of M
matching notes` in every mode.

## JSON fields (note)

| Field | Type | Notes |
|-------|------|-------|
| `id` | integer | positive |
| `title` | string | one line |
| `body` | string or `null` | `null` when absent, never omitted |
| `tags` | array of strings | sorted; `[]` when none |
| `priority` | string | `low`, `normal` or `high` |
| `created_at` | string | RFC 3339 with offset |

Adding a field is compatible. Renaming, removing or retyping one is a
breaking change and shows up as a golden diff.

## Errors and exit codes

| Exit | Meaning | stderr |
|------|---------|--------|
| 0 | success, including an empty result and a closed stdout | one notice at most: nothing matched, the list was cut, or `added note <id> to <file>` |
| 1 | the command failed | `error: <message>` or `{"error": "...", "code": "..."}` |
| 2 | usage error | clap's rendering, or `{"error": "...", "code": "usage_error"}` |

Stable codes: `note_not_found`, `io_error`, `store_corrupt`, `store_too_new`,
`store_locked`, `store_symlink`, `store_foreign_owner`, `store_permissions`,
`no_data_dir`, `output_error`, `usage_error`.
