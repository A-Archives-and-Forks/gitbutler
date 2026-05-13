//! Implementation of the `revision` debug commands.

use std::io::Write;

use anyhow::{Context as _, Result, bail, ensure};
use but_core::ref_metadata::{
    StackId, Workspace, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
};
use gix::{
    bstr::ByteVec, odb::store::RefreshMode, reference::Category, refs::Target,
    revision::plumbing::Spec,
};

use crate::{
    args::{Args, LogArgs, MergeBaseArgs, RevisionArgs, RevisionGraphArgs, RevisionSubcommands},
    metadata::EmptyRefMetadata,
    setup,
};

/// Execute the `revision` subcommand.
pub(crate) fn run(args: &Args, revision_args: &RevisionArgs) -> Result<()> {
    let mut repo = setup::repo_from_args(args)?;
    repo.objects.refresh = RefreshMode::Never;
    let meta = EmptyRefMetadata;

    match &revision_args.cmd {
        RevisionSubcommands::Log(log_args) => log(&repo, &meta, log_args),
        RevisionSubcommands::MergeBase(merge_base_args) => {
            merge_base(&repo, &meta, merge_base_args)
        }
    }
}

fn log(repo: &gix::Repository, meta: &EmptyRefMetadata, log_args: &LogArgs) -> Result<()> {
    let parsed = repo
        .rev_parse(log_args.rev_spec.as_str())
        .with_context(|| format!("Failed to parse rev-spec '{}'", log_args.rev_spec))?
        .detach();

    let (included, excluded) = match parsed {
        Spec::Include(commit_id) => (commit_id, None),
        Spec::Range { from, to } => (to, Some(from)),
        other => bail!("Unsupported rev-spec for revision log: {other}"),
    };
    let mut graph_commits = vec![included];
    graph_commits.extend(excluded);

    let graph_args = resolve_graph_args(repo, &log_args.graph)?;
    let graph = {
        let _span =
            tracing::info_span!("build graph", commit_count = graph_commits.len()).entered();
        graph_for_revisions(repo, meta, &graph_commits, graph_args)?
    };

    let _span = tracing::info_span!("traverse graph").entered();
    let commits = if let Some(excluded) = excluded {
        graph.find_commit_ids_reachable_from_a_not_b(included, excluded, log_args.first_parent)?
    } else {
        bail!("Need to specify a rev-spec of form `a..b` to indicate an exclusion for now.")
    };

    let mut out = Vec::with_capacity(512);
    for commit_id in commits {
        commit_id.write_hex_to(&mut out)?;
        out.push_byte(b'\n');
    }

    std::io::stdout().write_all(&out)?;
    Ok(())
}

fn merge_base(
    repo: &gix::Repository,
    meta: &EmptyRefMetadata,
    merge_base_args: &MergeBaseArgs,
) -> Result<()> {
    let commits = {
        let _span = tracing::info_span!(
            "resolve revisions",
            revision_count = merge_base_args.revisions.len()
        )
        .entered();
        merge_base_args
            .revisions
            .iter()
            .map(|rev| {
                repo.rev_parse_single(rev.as_str())
                    .map(|id| id.detach())
                    .with_context(|| format!("Failed to resolve revision '{rev}'"))
            })
            .collect::<Result<Vec<_>>>()?
    };

    let graph_args = resolve_graph_args(repo, &merge_base_args.graph)?;
    let graph = {
        let _span = tracing::info_span!("build graph", commit_count = commits.len()).entered();
        graph_for_revisions(repo, meta, &commits, graph_args)?
    };

    let segments = {
        let _span = tracing::info_span!("map commit ids to segments", commit_count = commits.len())
            .entered();
        commits
            .iter()
            .copied()
            .map(|commit_id| graph.commit_id_to_segment_id(commit_id))
            .collect::<Result<Vec<_>>>()
            .context("Failed to map commit ids to graph segments")?
    };

    let merge_base = {
        let _span = tracing::info_span!("compute octopus merge-base", commit_count = commits.len())
            .entered();
        graph
            .find_merge_base_octopus(segments)
            .map(|segment_id| {
                graph
                    .tip_skip_empty(segment_id)
                    .map(|commit| commit.id)
                    .with_context(|| {
                        format!(
                            "BUG: Segment {segment_id:?} does not contain a reachable tip commit"
                        )
                    })
            })
            .transpose()
            .context("Failed to compute octopus merge-base from graph")?
    };

    let Some(merge_base) = merge_base else {
        bail!(
            "No merge-base found for revisions: {}",
            merge_base_args.revisions.join(", ")
        );
    };
    println!("{merge_base}");

    Ok(())
}

struct GraphArgs {
    target_ref: Option<gix::refs::FullName>,
    extra_target: Option<gix::ObjectId>,
}

fn resolve_graph_args(repo: &gix::Repository, graph_args: &RevisionGraphArgs) -> Result<GraphArgs> {
    let target_ref = graph_args
        .target_ref
        .as_deref()
        .map(|target_ref| {
            let reference = repo
                .find_reference(target_ref)
                .with_context(|| format!("Failed to find target ref '{target_ref}'"))?;
            let name = reference.name().to_owned();
            ensure!(
                name.category() == Some(Category::RemoteBranch),
                "Target ref '{name}' resolved from '{target_ref}' is not a remote-tracking branch; use --extra-target for arbitrary revisions"
            );
            Ok(name)
        })
        .transpose()?;

    let extra_target = graph_args
        .extra_target
        .as_deref()
        .map(|rev| {
            repo.rev_parse_single(rev)
                .map(|id| id.detach())
                .with_context(|| format!("Failed to resolve extra target '{rev}'"))
        })
        .transpose()?;

    Ok(GraphArgs {
        target_ref,
        extra_target,
    })
}

fn graph_for_revisions(
    repo: &gix::Repository,
    meta: &EmptyRefMetadata,
    commits: &[gix::ObjectId],
    graph_args: GraphArgs,
) -> Result<but_graph::Graph> {
    let first = *commits
        .first()
        .context("BUG: revision graph requires at least one commit")?;
    let options = but_graph::init::Options {
        collect_tags: false,
        commits_limit_hint: None,
        extra_target_commit_id: graph_args.extra_target,
        ..Default::default()
    };
    let mut graph = but_graph::Graph::default();
    graph.options = options;

    let workspace_ref_name = synthetic_ref_name("workspace")?;
    let input_ref_names = (0..commits.len())
        .map(|idx| synthetic_ref_name(&format!("input-{idx}")))
        .collect::<Result<Vec<_>>>()?;

    let refs = std::iter::once(gix::refs::Reference {
        name: workspace_ref_name.clone(),
        target: Target::Object(first),
        peeled: Some(first),
    })
    .chain(
        input_ref_names
            .iter()
            .cloned()
            .zip(commits.iter().copied())
            .map(|(name, id)| gix::refs::Reference {
                name,
                target: Target::Object(id),
                peeled: Some(id),
            }),
    );
    let workspace = Workspace {
        stacks: input_ref_names
            .iter()
            .enumerate()
            .map(|(idx, ref_name)| WorkspaceStack {
                id: StackId::from_number_for_testing(1000 + idx as u128),
                branches: vec![WorkspaceStackBranch {
                    ref_name: ref_name.clone(),
                    archived: false,
                }],
                workspacecommit_relation: WorkspaceCommitRelation::Merged,
            })
            .collect(),
        target_ref: graph_args.target_ref,
        ..Default::default()
    };
    let overlay = but_graph::init::Overlay::default()
        .with_entrypoint(first, Some(workspace_ref_name.clone()))
        .with_references(refs)
        .with_workspace_metadata_override(Some((workspace_ref_name, workspace)));

    graph.redo_traversal_with_overlay(repo, meta, overlay)
}

fn synthetic_ref_name(suffix: &str) -> Result<gix::refs::FullName> {
    format!("refs/heads/but-debug/revision/{suffix}")
        .try_into()
        .with_context(|| format!("BUG: invalid synthetic ref suffix '{suffix}'"))
}
