//! What every app runs against: the context, the chokepoint each app's
//! `exec` forwards to, the audit guard and the errors all apps share.
//!
//! [`Runtime::start`] calls composition once and owns what it resolved; a
//! flow receives the borrowed [`Ctx`]. Only [`exec`] starts the runtime and
//! the audit guard. The binary never sees this module. It holds those four
//! subjects and no others: "two apps use it" is not a reason for code to
//! live here.

mod audit;
mod errors;
mod exec;

pub(crate) use audit::AuditMeta;
pub use errors::Error;
#[cfg(test)]
pub(crate) use exec::run_with;
pub(crate) use exec::{App, exec};

use audit::AuditGuard;

use crate::composition::{self, Env};
use time::OffsetDateTime;
use tmpl_cli_store::Store;

/// What composition resolved for one invocation.
#[derive(Debug)]
pub(crate) struct Runtime {
    store: Store,
    actor: Option<String>,
}

impl Runtime {
    /// Resolve the data directory, open the store and name the actor.
    pub(crate) fn start(env: &Env) -> Result<Self, Error> {
        let composed = composition::compose(env)?;
        Ok(Self {
            store: composed.store,
            actor: composed.actor,
        })
    }

    /// The context a flow runs against, with the real clock.
    pub(crate) fn ctx(&self) -> Ctx<'_> {
        Ctx::new(&self.store, self.actor.as_deref(), &composition::now)
    }
}

/// The borrowed view of the runtime a flow receives.
pub struct Ctx<'a> {
    store: &'a Store,
    actor: Option<&'a str>,
    clock: &'a dyn Fn() -> OffsetDateTime,
}

impl<'a> Ctx<'a> {
    /// A context over `store`, acting as `actor`, reading the time from
    /// `clock`. Tests pass a fixed clock and a store with a short lock wait.
    pub fn new(
        store: &'a Store,
        actor: Option<&'a str>,
        clock: &'a dyn Fn() -> OffsetDateTime,
    ) -> Self {
        Self {
            store,
            actor,
            clock,
        }
    }

    /// The note store.
    pub(crate) fn store(&self) -> &'a Store {
        self.store
    }

    /// The OS user name resolved from `USER`, else `USERNAME`, or `None`.
    pub(crate) fn actor(&self) -> Option<&'a str> {
        self.actor
    }

    /// The current time, for records created by this invocation.
    pub(crate) fn now(&self) -> OffsetDateTime {
        (self.clock)()
    }
}

impl std::fmt::Debug for Ctx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx")
            .field("store", &self.store)
            .field("actor", &self.actor)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
