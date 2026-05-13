//! Debugging utilities exposed as a dedicated CLI.
#![forbid(unsafe_code)]

use std::ffi::OsString;

use anyhow::Result;
use clap::Parser;

pub mod args;
pub(crate) mod command;
mod metadata;
mod setup;
mod trace;

use args::{Args, Subcommands};

/// Parse CLI arguments and dispatch the requested subcommand.
pub fn handle_args(args: impl Iterator<Item = OsString>) -> Result<()> {
    let args = Args::parse_from(args);
    trace::init(args.trace)?;

    let _span = tracing::info_span!("run").entered();
    match &args.cmd {
        Subcommands::Graph(graph_args) => command::graph::run(&args, graph_args),
        Subcommands::Revision(revision_args) => command::revision::run(&args, revision_args),
    }
}
