---
type: architecture
summary: "The tmpl-cli-store library as it is: the data directory, the SQLite database and its schema steps, one repository per record, and the record shapes."
created_date: 2026-11-25
updated_date: 2026-11-25
---

# tmpl-cli-store

The library that keeps the data. Its module tree and rules are in
[ARCHITECTURE.md](../../ARCHITECTURE.md#tmpl-cli-store). Why it is shaped
this way is in the feature design docs listed on the
[feature board](../design/FEATURES.md).
<!-- example:begin -->
For the template's own example, the store's contract is the
[store spec](../design/tmpl-cli/specs/store.md).
<!-- example:end -->

## What it owns

- The data directory's database file and its owner-only check.
- The schema: one module per table and the one ordered list of steps.
- One repository per record: its SQL and its row mapping.
- The record shapes (`contract`) and the invariants each type keeps on
  itself.
- The store's stable code for each of the kit's file and database errors.

## Main modules

- [store.rs](../../crates/tmpl-cli-store/src/store.rs): `Store::open`,
  `read` (a snapshot) and `write` (one transaction).
- [driver/sqlite/](../../crates/tmpl-cli-store/src/driver/sqlite/): the
  database file, `open`, and `schema/` with its `MIGRATIONS`.
- [repository/](../../crates/tmpl-cli-store/src/repository/): `Snapshot`,
  `Tx`, and one module per record.
- [contract/](../../crates/tmpl-cli-store/src/contract/): the record
  shapes.
- [error.rs](../../crates/tmpl-cli-store/src/error.rs): the store's error
  codes and bad-record reports.

## Dependencies

The kit's `cw-base`, `cw-fs` (every file access) and `cw-sqlite` (the
database, and `rusqlite` only through its re-export). It holds no rule of
any app; the full list is the table in
[ARCHITECTURE.md](../../ARCHITECTURE.md#crates).

## Entry points

- `Store::open(root)`, then `read()` and `write(|tx| …)`.
- `snapshot.<record>()` and `tx.<record>()`: the only way to a record.
- The `test-support` feature: raw SQL for other crates' tests.
