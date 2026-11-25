//! The `status` command: where the data directory is and which store format
//! it holds. The handler maps the arguments to the `status` app's `Command`,
//! calls `app::status::exec`, and turns the `Outcome` into a payload
//! (STD-02 R2).

use crate::output::{Output, Payload, StatusPayload};
use clap::Args;
use cw_mcp::schemars::JsonSchema;
use serde::Deserialize;
use tmpl_cli_core::app::status::{self, Outcome};
use tmpl_cli_core::{AuditWarning, Error, Origin, Runtime};

pub(super) const STATUS_EXAMPLES: &str = "\
Examples:
  tmpl-cli status
  tmpl-cli status --json
  tmpl-cli status | cut -f1

A store with no database yet has no format: '-', or null with --json.";

/// Arguments to `status`, and the `status` tool's input: none of its own;
/// the global flags apply.
#[derive(Debug, Args, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "cw_mcp::schemars")]
pub(crate) struct StatusArgs {}

/// Run `status` through the `status` app against `runtime` for `origin`,
/// pass the payload to `finish`, then settle the (empty) audit record.
pub(super) fn run<R>(
    _args: StatusArgs,
    runtime: &Runtime,
    origin: &Origin,
    finish: impl FnOnce(Result<Output, Error>) -> R,
) -> (R, Option<AuditWarning>) {
    let (outcome, audit) = status::exec(runtime, origin, status::Command::Show).into_parts();
    let rendered = finish(outcome.map(output));
    (rendered, audit.settle())
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
