---
type: policy
summary: How to report a security problem in tmpl-cli privately, what the report may cover, and the limits the tool knowingly has.
created_date: 2026-11-25
updated_date: 2026-11-25
---

# Security

## Reporting a problem

Report a vulnerability privately through GitHub's private vulnerability
reporting on this repository: the **Security** tab, then **Report a
vulnerability**. Do not open a public issue or pull request for it. Say
what you ran, what happened and what you expected; a report is answered
there, and the fix and any advisory are published once a release carries
it.

<!-- template:begin -->
A project made from this template turns private vulnerability reporting on
in its own repository's settings before it publishes this file.
<!-- template:end -->

## Scope

In scope: the code in this repository and what it builds.
<!-- crates:begin -->
That is the `tmpl-cli` binary and its crates, the files it keeps in its
data directory (the database, its lock and the audit log), and the MCP
server it runs on stdin and stdout (`tmpl-cli mcp`).
<!-- crates:end -->

Out of scope, and reported where they live:

- the shared kit crates (`cw-*`), to `constellation-works/satellites`;
- third-party dependencies, to their own maintainers (`cargo deny` in CI
  checks the advisory database);
- the host itself: its accounts, its file permissions outside the data
  directory, and other programs running as the same user.

## Known limits

A change that knowingly leaves a limit lists it here in the same change.
<!-- crates:begin -->

- **One local user.** `tmpl-cli` acts with the authority of the user who
  runs it and trusts that user. It opens no network socket, and its data
  directory is checked to be owner-only on every read and write; it is not
  built for a directory other users can write.
- **No encryption at rest.** The database and the audit log are plain
  files; anyone who can read the data directory can read them.
- **The MCP server trusts its client.** It serves whatever process launched
  it over stdio, with no authentication of its own, and every tool can do
  what the matching command can do, including the mutating ones. Give it
  only to a client you would let run the command line.
- **The audit log fails open.** A mutating command whose audit line cannot
  be written still completes and prints a warning; the log records
  identifiers and error codes, never argument values.
<!-- crates:end -->
