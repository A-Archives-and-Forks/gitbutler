//! Command-line argument parsing for `but-debug`.

use std::path::PathBuf;

/// Top-level CLI arguments for `but-debug`.
#[derive(Debug, clap::Parser)]
#[command(
    name = "but-debug",
    about = "Debugging utilities for GitButler repositories",
    version = option_env!("GIX_VERSION")
)]
pub struct Args {
    /// Enable tracing for debug and performance information printed to stderr.
    #[arg(short = 't', long, action = clap::ArgAction::Count)]
    pub trace: u8,
    /// Run as if `but-debug` was started in `PATH` instead of the current working directory.
    #[arg(short = 'C', long, default_value = ".", value_name = "PATH")]
    pub current_dir: PathBuf,
    /// The debugging command to run.
    #[command(subcommand)]
    pub cmd: Subcommands,
}

/// The debugging subcommands supported by `but-debug`.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Return a segmented graph starting from `HEAD`.
    Graph(GraphArgs),
    /// Debug revision graph operations.
    #[clap(visible_alias = "rev")]
    Revision(RevisionArgs),
}

/// Arguments for the `graph` debugging subcommand.
#[derive(Debug, clap::Args)]
pub struct GraphArgs {
    /// Debug-print the whole graph and ignore all other dot-related flags.
    #[arg(long, short = 'd')]
    pub debug: bool,
    /// Print graph statistics first to get a grasp of huge graphs.
    #[arg(long, short = 's')]
    pub stats: bool,
    /// The rev-spec of the extra target to provide for traversal.
    #[arg(long)]
    pub extra_target: Option<String>,
    /// Disable post-processing of the graph, useful if that's failing.
    #[arg(long)]
    pub no_post: bool,
    /// Do not debug-print the workspace.
    ///
    /// If too large, it takes a long time or runs out of memory.
    #[arg(long)]
    pub no_debug_workspace: bool,
    /// Output the dot-file to stdout.
    #[arg(long, conflicts_with = "dot_show")]
    pub dot: bool,
    /// The maximum number of commits to traverse.
    ///
    /// Use only as safety net to prevent runaways.
    #[arg(long)]
    pub hard_limit: Option<usize>,
    /// The hint of the number of commits to traverse.
    ///
    /// Specifying no limit with `--limit` removes all limits.
    #[arg(long, short = 'l', default_value = "300")]
    pub limit: Option<Option<usize>>,
    /// Refill the limit when running over these hashes, provided as short or long hash.
    #[arg(long, short = 'e')]
    pub limit_extension: Vec<String>,
    /// Open the dot-file as SVG instead of writing it to stdout.
    #[arg(long)]
    pub dot_show: bool,
    /// The name of the ref to start the graph traversal at.
    pub ref_name: Option<String>,
}

/// Arguments for the `revision` subcommand.
#[derive(Debug, clap::Args)]
pub struct RevisionArgs {
    /// The revision debugging command to run.
    #[command(subcommand)]
    pub cmd: RevisionSubcommands,
}

/// The debugging subcommands supported by `but-debug revision`.
#[derive(Debug, clap::Subcommand)]
pub enum RevisionSubcommands {
    /// Print commits reachable by a rev-spec.
    Log(LogArgs),
    /// Compute the octopus merge-base for two or more revisions.
    #[command(name = "merge-base")]
    MergeBase(MergeBaseArgs),
}

/// Graph construction options shared by revision debugging subcommands.
#[derive(Debug, clap::Args)]
pub struct RevisionGraphArgs {
    /// The named reference to use as the workspace target during graph traversal.
    #[arg(long)]
    pub target_ref: Option<String>,
    /// The rev-spec of the extra target to provide for graph traversal.
    #[arg(long)]
    pub extra_target: Option<String>,
}

/// Arguments for the `revision log` debugging subcommand.
#[derive(Debug, clap::Args)]
pub struct LogArgs {
    /// Shared graph construction options.
    #[command(flatten)]
    pub graph: RevisionGraphArgs,
    /// The rev-spec to log. Exclusive ranges like `main..branch` are supported.
    pub rev_spec: String,
}

/// Arguments for the `revision merge-base` debugging subcommand.
#[derive(Debug, clap::Args)]
pub struct MergeBaseArgs {
    /// Shared graph construction options.
    #[command(flatten)]
    pub graph: RevisionGraphArgs,
    /// The rev-specs whose octopus merge-base should be computed.
    #[arg(required = true, num_args = 2.., value_name = "REV")]
    pub revisions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory as _, Parser as _};

    use super::{Args, RevisionSubcommands, Subcommands};

    #[test]
    fn clap_configuration_is_valid() {
        Args::command().debug_assert();
    }

    #[test]
    fn merge_base_requires_at_least_two_revisions() {
        assert!(Args::try_parse_from(["but-debug", "revision", "merge-base", "main"]).is_err());

        let args = Args::parse_from(["but-debug", "revision", "merge-base", "main", "feature"]);
        match args.cmd {
            Subcommands::Revision(revision_args) => match revision_args.cmd {
                RevisionSubcommands::MergeBase(args) => {
                    assert_eq!(args.revisions, ["main", "feature"])
                }
                _ => panic!("expected merge-base command"),
            },
            _ => panic!("expected revision command"),
        }
    }

    #[test]
    fn merge_base_accepts_target_options() {
        let args = Args::parse_from([
            "but-debug",
            "revision",
            "merge-base",
            "--target-ref",
            "refs/remotes/origin/main",
            "--extra-target",
            "origin/main~1",
            "main",
            "feature",
        ]);

        match args.cmd {
            Subcommands::Revision(revision_args) => match revision_args.cmd {
                RevisionSubcommands::MergeBase(args) => {
                    assert_eq!(
                        args.graph.target_ref.as_deref(),
                        Some("refs/remotes/origin/main")
                    );
                    assert_eq!(args.graph.extra_target.as_deref(), Some("origin/main~1"));
                    assert_eq!(args.revisions, ["main", "feature"]);
                }
                _ => panic!("expected merge-base command"),
            },
            _ => panic!("expected revision command"),
        }
    }

    #[test]
    fn revision_log_accepts_exclusive_range() {
        let args = Args::parse_from(["but-debug", "revision", "log", "main..feature"]);

        match args.cmd {
            Subcommands::Revision(revision_args) => match revision_args.cmd {
                RevisionSubcommands::Log(args) => assert_eq!(args.rev_spec, "main..feature"),
                _ => panic!("expected log command"),
            },
            _ => panic!("expected revision command"),
        }
    }
}
