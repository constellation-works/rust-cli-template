---
type: design
summary: "Spec: output contract — streams, modes, JSON fields, readers, sizes, errors and exit codes"
created_date: 2026-09-26
updated_date: 2026-11-27
last_validated: 2026-11-27
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
| `table` | `--format table`, or auto on a terminal | header row + one line per record (`BODY` added with `--include-body`); `show` is one field per line |
| `plain` | auto off a terminal | one tab-separated line per record, no header: `id priority title tags created_at` (plus `body` with `--include-body`) |
| `json` | `--json`, `--format json`, or `TMPL_CLI_FORMAT=json` | one document: the list envelope for `list` (at most 20 compact notes by default), a note object for `show`/`add` |

Precedence: flag > `TMPL_CLI_FORMAT` > auto. An unrecognized environment
value means auto. Off a terminal there are no escapes and no truncation.

## JSON list envelope

`note list --json` (and the `note_list` MCP tool) is always
`{"notes": [<note>, ...], "total": <integer>, "truncated": <boolean>, "warnings": [<skipped>, ...]}`:
it returns at most 20 matching notes by default (`--limit <N>` or `--all` for
more; `--limit` and `--all` are mutually exclusive). `total` counts the
notes that matched the filters, and `truncated` is true when the default or
explicit limit left matching notes out. `warnings` names each record the list
skipped because it could not be read, as
`{"record": "note <id>" | "row <id>", "reason": <string>}`, in store order;
it is `[]` when none was skipped, never omitted, and stderr warns of each
one too. The list still exits 0. No matches is `{"notes": [], "total": 0,
"truncated": false, "warnings": []}`. When a list is cut, stderr also says
`showing N of M matching notes; pass --limit <N> or --all to see more` in
every mode.

## JSON fields (note)

`note show`, `note add`, and `note list --include-body` emit full note
objects with all six fields. By default, `note list` and `note_list` emit
compact note summaries without `body` (`id`, `title`, `tags`, `priority`,
`created_at`); `body` is returned when `--include-body` (`include_body: true`
in MCP) is requested, or per note via `note show`.

| Field | Type | Notes |
|-------|------|-------|
| `id` | integer | positive |
| `title` | string | one line |
| `body` | string or `null` | `null` when absent, never omitted when included; omitted from default `note list` |
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

## Readers and sizes

STD-08 §R1 requires stating each reader, purpose, steady-state size and
repeated size (illustrated over 24 cycles of an 8-hour mandate) for every
agent-facing result and persisted audit record:

| Surface / record | Readers and what they read it for | Steady-state size (1 call / line) | Repeated size (24 cycles) |
|---|---|---|---|
| `status` (`--json` / `status`) | Agents and scripts checking the resolved data directory and store schema format before reading or writing. | ~40–120 bytes (one path string and nullable integer `format`); O(1). | ~1.0–2.9 KB total across 24 calls; constant per call. |
| `note list --json` / `note_list` (default: limit 20, compact) | Agents and scripts browsing matching note ids, titles, tags, priorities and timestamps without fetching bodies. | ~100–180 bytes per typical note (~350 bytes worst case at 200-char title + tags), so <= ~3.6 KB typical (~7 KB worst case) for 20 notes plus 50-byte envelope; O(1) with store growth. | <= ~86 KB typical (~170 KB worst case) over 24 reads; constant per call regardless of store size. |
| `note list --json` / `note_list` (`--include-body`, `--limit N`, `--all`) | Callers explicitly opting into full bodies or larger slices (STD-08 R5). | Opt-in: `N * (metadata + body)` bytes (`all` grows with matching notes). Agents use `note_show` by id rather than `--include-body` or `--all` in loops. | Grows with requested `N` and body size; not a default context. |
| `warnings` (`skipped` entries on `note_list` / `note_show`) | Agents and operators identifying corrupt or invalid rows isolated during a read. | `[]` (2 bytes) on a healthy store; ~70–160 bytes per skipped record (`{"record":"note <id>","reason":"..."}`) when corrupt rows exist. Writes refuse until repaired, capping bad-row drift. | ~0 KB on a healthy store; proportional only to existing un-repaired bad rows if any. |
| `note show --json` / `note_show` | Agents and humans fetching one note's full `body` and metadata by id. | ~120 bytes metadata + the single note's `body` (plus `warnings` only when bad rows exist); O(1) note. | Sum of on-demand single-note fetches; not repeated across the whole store. |
| `note add --json` / `note_add` (incl. audit-failure `warnings`) | Agents and scripts confirming the assigned `id` and normalized fields of the note just written. | ~120 bytes metadata + caller's `body`, plus ~80–150 bytes in `warnings` only if the audit append failed. | ~3–6 KB typical across 24 writes (plus caller-supplied bodies). |
| JSON error (`--json` stderr / MCP `tool_error`) | Agents and scripts branching on stable `code`, `kind`, `retryable` and `hint` without prose parsing. | ~90–220 bytes (one single-line object); O(1). | ~2.2–5.3 KB if all 24 calls fail; constant per failure. |
| Audit line (`audit.jsonl`, format 3) | Operators and audit tools (`cw_app::AuditLog::read`) verifying who mutated the store, via which surface, target id and outcome. | ~150–220 bytes per mutating command (`format`, `at`, `surface`, `actor`, `command`, `status`, `target`, `error_code`, `duration_ms`; no argument values or error text). Reads write nothing. | ~4.3 KB across 24 mutations (read bounded by `cw_fs::Cap`); 0 bytes across 24 read-only calls. |

## Errors and exit codes

| Exit | Meaning | stderr |
|------|---------|--------|
| 0 | success, including an empty result and a closed stdout | warnings for any isolated bad store records, plus one notice at most: nothing matched, the list was cut, or `added note <id> to <file>` |
| 1 | the command failed | `error: <message>` or `{"error": "...", "code": "...", "hint": null}`; `hint` contains a string when a fix is known |
| 2 | usage error | clap's rendering, or `{"error": "...", "code": "usage_error", "hint": null}` |

Stable codes: `note_not_found`, `io_error`, `encode_error`, `store_corrupt`,
`store_bad_record`, `note_id_exhausted`, `store_too_new`,
`store_locked`, `store_symlink`, `store_foreign_owner`, `store_permissions`,
`store_not_regular`, `store_too_large`, `store_lock_reentrant`,
`store_wal_unavailable`, `no_data_dir`, `output_error`, `usage_error`.

`store_wal_unavailable` (kind `refused`, exit 1, on the command line and as
a tool error alike) is what `status`, `note list` and `note show` fail with
when `store.db` exists but its `store.db-wal` or `store.db-shm` is missing
(or the `-shm` needs recovery): an observation creates and changes no file
(STD-01 R31), so it is refused rather than served from the main file alone.
Its `hint` is `the next write creates the missing files: run a command that
writes to this store, with the same --root, then read again`; `note add`
creates the files, after which the reads succeed. Nothing is created or
changed by the refused read, and `note add` is never refused for it.
