//! How a failure is classified.

/// What kind of failure an [`super::Error`] is, named for what the caller
/// can do about it. The unit that knows the failure decides its kind, in its
/// translator; a surface decides from the kind alone (an exit code, a retry)
/// and never from a message (STD-02 R10).
///
/// Not `#[non_exhaustive]`: every surface in this workspace must decide what
/// a new kind means, so adding one is meant to break their matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The record the request names does not exist: name one that does.
    NotFound,
    /// The request is not allowed as it stands, by a rule of an app or a
    /// safety check: change the request or the state it names.
    Refused,
    /// Another process holds what this one needs: retry when it finishes.
    Busy,
    /// Stored state is not valid: repair the data.
    Corrupt,
    /// Stored state was written by a newer build: upgrade this tool.
    Incompatible,
    /// The environment and the flags do not say how to run: fix them.
    Config,
    /// The operating system failed an operation: see the cause.
    Io,
    /// A state the program should not reach: a bug to report.
    Internal,
}
