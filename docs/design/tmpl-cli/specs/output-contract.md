---
type: design
summary: "Spec: output contract — streams, modes, JSON fields, errors and exit codes"
created_date: 2026-09-26
updated_date: 2026-11-25
last_validated: 2026-11-25
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
`{"notes": [<note>, ...], "total": <integer>, "truncated": <boolean>, "warnings": [<skipped>, ...]}`:
`total` counts the notes that matched the filters, and `truncated` is true
when `--limit` returned fewer. `warnings` names each record the list
skipped because it could not be read, as
`{"record": "note <id>" | "row <id>", "reason": <string>}`, in store order;
it is `[]` when none was skipped, never omitted, and stderr warns of each
one too. The list still exits 0. No matches is `{"notes": [], "total": 0,
"truncated": false, "warnings": []}`. When a list is cut, stderr also says `showing N of M
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

## MCP tool results

Each MCP tool returns the same JSON object as `--json`. Because an MCP
client has no stderr channel on a tool call, warnings from a successful call
are carried in an additive `warnings` array on the tool result:

- `note_list` always carries `"warnings": [<skipped>, ...]` from the list
  envelope above.
- `note_show` adds `"warnings": [<skipped>, ...]` with the same
  `{"record": "note <id>" | "row <id>", "reason": <string>}` entries when
  other rows in the store were skipped, and omits `warnings` when none was
  skipped.
- A mutating tool whose audit line could not be written (`note_add`) adds
  `"warnings": ["the audit line for `<command>` was not written: <reason>"]`
  and omits `warnings` when the audit line was written.

## Errors and exit codes

| Exit | Meaning | stderr |
|------|---------|--------|
| 0 | success, including an empty result and a closed stdout | warnings for any isolated bad store records, plus one notice at most: nothing matched, the list was cut, or `added note <id> to <file>` |
| 1 | the command failed | `error: <message>` or `{"error": "...", "code": "...", "hint": null}`; `hint` contains a string when a fix is known |
| 2 | usage error | clap's rendering, or `{"error": "...", "code": "usage_error", "hint": null}` |

Stable codes: `note_not_found`, `io_error`, `encode_error`, `store_corrupt`,
`store_bad_record`, `note_id_exhausted`, `store_too_new`,
`store_locked`, `store_symlink`, `store_foreign_owner`, `store_permissions`,
`store_not_regular`, `store_too_large`, `store_lock_reentrant`, `no_data_dir`, `output_error`, `usage_error`.
