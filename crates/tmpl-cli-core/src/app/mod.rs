//! The apps: one per subject a user names on the command line.
//!
//! An app's public surface is its `Command`, `Outcome` and `Error` and one
//! `exec`. Under it, `run` is `exec` without the runtime start; tests call
//! it. A flow loads from the store, decides with the app's pure `rule`, and
//! commits in one store write. There is no `app/common`, `app/shared` or
//! `app/util`: code two apps need goes to the lower app, or to the runtime
//! when it is about the context, the audit guard or errors.

pub mod note;
