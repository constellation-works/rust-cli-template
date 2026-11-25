---
type: design
summary: "Glossary: tmpl-cli"
last_validated: 2026-09-26
---

# Glossary: tmpl-cli

Terms with a specific meaning in this project. General CLI vocabulary (TTY,
stdout, exit code) is excluded.

| Term | Meaning |
|------|---------|
| **Data directory** | Where the store lives: `--root`, else `TMPL_CLI_ROOT`, else `~/.tmpl-cli`. See [2_design.md §2](../2_design.md). |
| **Payload** | The typed record (`NotePayload`) every output mode renders from; its JSON form is the contract. See [specs/output-contract.md](../specs/output-contract.md). |
| **Plain form** | The headerless tab-separated output used when stdout is not a terminal. See [2_design.md §3](../2_design.md). |
| **Sink** | The per-invocation resolution of output mode, color and width. See [2_design.md §2](../2_design.md). |
| **List envelope** | The `--json` shape of every list: `{"notes", "total", "truncated"}`. See [specs/output-contract.md](../specs/output-contract.md). |
| **Audit log** | `<root>/audit.jsonl`: one line per mutating command (what ran, on which note, how it ended), written by the audit middleware. See [2_design.md §5](../2_design.md). |
| **Schema version** | The notes database's `PRAGMA user_version`: the number of schema steps applied from `MIGRATIONS`. A newer one is refused; an older one is migrated by the next write. See [specs/store.md](../specs/store.md). |
