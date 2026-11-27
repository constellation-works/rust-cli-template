//! The terminal decision and the sink built from it.

use cw_cli::{Format, Probe, Sink, Terminal};
use tmpl_cli_core::{APP, Env};

/// The terminal decision for this invocation, made from the environment
/// `main` captured, one `probe` of the streams and the format the command
/// line asked for (`None` defers to `TMPL_CLI_FORMAT`, then `auto`).
pub(crate) fn decide(env: &Env, probe: Probe, requested: Option<Format>) -> Terminal {
    Terminal::decide(env, APP, probe, requested)
}

/// A sink on the process's stdout and stderr, rendering as `terminal`
/// decided.
pub(crate) fn sink(terminal: Terminal) -> Sink {
    Sink::stdio(terminal)
}
