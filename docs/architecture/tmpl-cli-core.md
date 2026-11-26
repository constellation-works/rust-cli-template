---
type: architecture
summary: "The tmpl-cli-core library as it is: the apps, their flows and rules, composition and the environment capture."
created_date: 2026-11-25
updated_date: 2026-11-25
---

# tmpl-cli-core

The library that holds what the program does. Its module tree and rules are
in [ARCHITECTURE.md](../../ARCHITECTURE.md#tmpl-cli-core).
Why it is shaped this way is in the feature design docs listed on the
[feature board](../design/FEATURES.md).
<!-- example:begin -->
For the template's own example, start with
[2_design.md](../design/tmpl-cli/2_design.md).
<!-- example:end -->

## What it owns

- The apps: one per subject a user names on the command line, each a
  `cw_app::App` with its `Command`, `Outcome`, typed `Error`, flows and
  pure rules.
- Composition: the runtime every command runs against (the data
  directory, the store, the clock and the audit log), built from the
  captured environment.
- The one read of the process environment (`capture_env`).
- The re-exports a surface needs, so a surface names only this crate: the
  store's record shapes, the kit's error type, and the `cw-app` types.

## Main modules

- [app/](../../crates/tmpl-cli-core/src/app/): one directory per app;
  `status` reports the data directory and the store format.
<!-- example:begin -->
  `note` is the example domain: one file per command and its rules in
  `rule.rs`.
<!-- example:end -->
- [composition.rs](../../crates/tmpl-cli-core/src/composition.rs): `start`,
  the `Runtime`, and the data-directory resolution.
- [environment.rs](../../crates/tmpl-cli-core/src/environment.rs):
  `capture_env`.

## Dependencies

`tmpl-cli-store`, and the kit's `cw-base` (the one error type) and `cw-app`
(the chokepoint, the environment and the audit log). It parses no argument
and touches no terminal; the full list is the table in
[ARCHITECTURE.md](../../ARCHITECTURE.md#crates).

## Entry points

- `capture_env()`: the process environment, once, for `main`.
- `start(env, root)`: the runtime for a surface.
- `app::<app>::exec(runtime, origin, command)`: each app's one entry,
  forwarding to `cw_app::exec`.
