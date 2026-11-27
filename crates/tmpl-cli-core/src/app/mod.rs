//! Defines one app per subject; imports composition.
//!
//! The apps: one per subject a user names on the command line.
//!
//! An app's public surface is its `Command`, `Outcome` and `Error` and one
//! `exec`, which forwards to the kit's chokepoint ([`cw_app::exec`]); the app
//! implements [`cw_app::App`] to say which commands are audited, how to run
//! one and what an outcome acted on. A flow loads from the store, decides
//! with the app's pure `rule`, and commits in one store write. There is no
//! `app/common`, `app/shared` or `app/util`: code two apps need goes to the
//! lower app.

// example:begin
pub mod note;
// example:end

pub mod status;
