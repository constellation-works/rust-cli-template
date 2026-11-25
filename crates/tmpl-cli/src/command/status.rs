//! The `status` command: where the data directory is and which store format
//! it holds. The handler maps the arguments to the `status` app's `Command`,
//! calls `app::status::exec`, and turns the `Outcome` into a payload
//! (STD-02 R2).

use crate::output::{Output, Payload, StatusPayload};
use clap::Args;
use tmpl_cli_core::app::status::{self, Outcome};
use tmpl_cli_core::{Env, Error};

pub(super) const STATUS_EXAMPLES: &str = "\
Examples:
  tmpl-cli status
  tmpl-cli status --json
  tmpl-cli status | cut -f1

A store with no database yet has no format: '-', or null with --json.";

/// Arguments to `status`: none of its own; the global flags apply.
#[derive(Debug, Args)]
pub(crate) struct StatusArgs {}

/// Run `status` through the `status` app and pass the payload to `finish`.
pub(super) fn run<R>(
    _args: StatusArgs,
    env: &Env,
    finish: impl FnOnce(Result<Output, Error>) -> R,
) -> R {
    status::exec(env, status::Command::Show, |outcome| {
        finish(outcome.map(output))
    })
}

/// The app's outcome as the payload every output mode renders.
fn output(outcome: Outcome) -> Output {
    match outcome {
        Outcome::Shown { root, format } => Output::new(Payload::Status(StatusPayload {
            root: root.display().to_string(),
            format,
        })),
    }
}
