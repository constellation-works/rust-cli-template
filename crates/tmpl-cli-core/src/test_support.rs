//! Test support, behind the `test-support` feature: the runtime context and
//! each app's `run`, so a test can pin the clock or shorten the lock wait
//! instead of going through [`crate::apps::note::exec`] and the process
//! environment.

pub use crate::runtime::Ctx;

use crate::apps::note::{Command, Error, Outcome};

/// [`crate::apps::note::exec`] without the runtime start or the audit guard.
pub fn note_run(ctx: &Ctx<'_>, command: Command) -> Result<Outcome, Error> {
    crate::apps::note::run(ctx, command)
}
