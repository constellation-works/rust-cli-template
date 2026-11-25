//! Test support, behind the `test-support` feature: the runtime context and
//! each app's `run`, so a test can pin the clock or shorten the lock wait
//! instead of going through an app's `exec` and the process environment.

pub use crate::runtime::Ctx;

use crate::app::status;

/// [`crate::app::status::exec`] without the runtime start.
pub fn status_run(
    ctx: &Ctx<'_>,
    command: status::Command,
) -> Result<status::Outcome, status::Error> {
    status::run(ctx, command)
}
// example:begin

/// [`crate::app::note::exec`] without the runtime start or the audit guard.
pub fn note_run(
    ctx: &Ctx<'_>,
    command: crate::app::note::Command,
) -> Result<crate::app::note::Outcome, crate::app::note::Error> {
    crate::app::note::run(ctx, command)
}
// example:end
