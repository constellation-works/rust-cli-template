//! The one chokepoint (STD-02 R24): every app's `exec` forwards to [`exec`].
//!
//! Starting the runtime, wrapping a mutating command in the audit guard,
//! telling the guard the outcome and handing the result to the surface are
//! written here once. An app supplies only what differs, through [`App`]:
//! whether a command is audited, its dispatch, and the record its outcome
//! names for the audit line. A new app cannot run a command outside the
//! guard, because it has no other way to start the runtime.
//!
//! Every app's failure leaves here as the workspace's one [`Error`]: the
//! app's typed error crosses through its own translator (STD-02 R12), and so
//! does a runtime that could not start.

use super::{AuditGuard, AuditMeta, Ctx, Runtime};
use crate::composition::Env;
use tmpl_cli_common::Error;

/// What an app supplies to run through the chokepoint.
pub(crate) trait App {
    /// What the app can be asked to do.
    type Command;
    /// What a command produced.
    type Outcome;
    /// Why a command failed, as the app's own typed error; its translator
    /// turns it into the workspace's [`Error`].
    type Error: Into<Error>;

    /// What the audit log records for `command`, or `None` when it is not
    /// audited (a read-only command; STD-01 R31).
    fn audit(command: &Self::Command) -> Option<AuditMeta>;

    /// Run `command`: dispatch only, without the runtime start or the guard.
    fn run(ctx: &Ctx<'_>, command: Self::Command) -> Result<Self::Outcome, Self::Error>;

    /// The id of the record `outcome` acted on, if it names one.
    fn target(outcome: &Self::Outcome) -> Option<String>;
}

/// Run one command of app `A` against the environment `env` and pass its
/// result to `finish`.
///
/// A mutating command runs inside the audit guard, created before the flow so
/// an early return or panic is still logged as a failure, and dropped after
/// `finish` so a fail-open audit warning reaches stderr after the command's
/// own payload or error object. A runtime that cannot start is reported to
/// `finish` and audits nothing, since there is no store to audit to.
pub(crate) fn exec<A: App, R>(
    env: &Env,
    command: A::Command,
    finish: impl FnOnce(Result<A::Outcome, Error>) -> R,
) -> R {
    match Runtime::start(env) {
        Ok(runtime) => run_with::<A, R>(&runtime.ctx(), command, finish),
        Err(err) => finish(Err(err.into())),
    }
}

/// [`exec`] against an already started runtime.
pub(crate) fn run_with<A: App, R>(
    ctx: &Ctx<'_>,
    command: A::Command,
    finish: impl FnOnce(Result<A::Outcome, Error>) -> R,
) -> R {
    let mut audit = A::audit(&command).map(|meta| {
        AuditGuard::start(ctx.store(), meta, ctx.now(), ctx.actor().map(str::to_owned))
    });
    let result = A::run(ctx, command).map_err(Into::into);
    if let Some(audit) = audit.as_mut() {
        audit.mark_result(result.as_ref().map(A::target).map_err(Error::code));
    }
    finish(result)
}
