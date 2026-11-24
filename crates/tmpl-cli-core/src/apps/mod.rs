//! The apps: one per subject a user names on the command line.
//!
//! An app's public surface is its `Command`, `Outcome` and `Error` and one
//! `exec`. Under it, `run` is `exec` without the runtime start; tests call
//! it. A flow loads from the store, decides with the app's pure `rules`, and
//! commits in one store write. There is no `apps/common`, `apps/shared` or
//! `apps/util`: code two apps need goes to the lower app, or to the runtime
//! when it is about the context, the audit guard or errors.

pub mod note;
