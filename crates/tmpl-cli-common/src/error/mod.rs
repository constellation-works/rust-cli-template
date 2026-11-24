//! The one error type of the workspace (STD-02 R11).
//!
//! Every unit keeps its own typed error enum for its own matching
//! (STD-02 R10) and translates it into [`Error`] once, in a `From` impl
//! beside that enum (STD-02 R12). Everything above the unit carries this one
//! type: an app's `exec` returns it, the audit line reads its code, and a
//! surface renders it.
//!
//! An [`Error`] is the unit's typed error, kept whole, with the two things a
//! caller needs without knowing that unit: how the failure is classified
//! ([`Kind`]) and its stable code. Its `Display` and its source chain are the
//! typed error's own, so a message is written once, where the failure is
//! known. A caller that does know the unit gets the typed error back with
//! [`Error::downcast_ref`].

mod kind;

pub use kind::Kind;

use std::error::Error as StdError;
use std::fmt;

/// A failure of any unit: the unit's typed error, its [`Kind`] and its
/// stable code.
///
/// `#[non_exhaustive]`: this is the workspace's public error boundary
/// (STD-02 R11); it is built only through [`Error::new`].
#[derive(Debug)]
#[non_exhaustive]
pub struct Error {
    kind: Kind,
    code: &'static str,
    cause: Box<dyn StdError + Send + Sync + 'static>,
}

impl Error {
    /// Wrap `cause`, a unit's typed error, with its classification and its
    /// stable `snake_case` code (STD-01 R19). Called from the unit's one
    /// translator, never ad hoc at a call site (STD-02 R12).
    #[must_use]
    pub fn new(
        kind: Kind,
        code: &'static str,
        cause: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            code,
            cause: Box::new(cause),
        }
    }

    /// How the failure is classified: what a caller can do about it.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The stable `snake_case` code, for machine output and the audit log
    /// (STD-01 R19). Adding a code is fine; changing one is a breaking
    /// change.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// The unit's typed error, when it is a `T`: for a caller that knows the
    /// unit and needs the variant.
    #[must_use]
    pub fn downcast_ref<T: StdError + 'static>(&self) -> Option<&T> {
        self.cause.downcast_ref::<T>()
    }
}

/// The typed error's own sentence.
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.cause, f)
    }
}

/// The typed error's own source chain, so a surface prints each cause once.
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.cause.source()
    }
}
