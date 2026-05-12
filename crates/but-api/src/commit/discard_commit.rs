use crate::WorkspaceState;
use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_rebase::graph_rebase::Editor;
use tracing::instrument;

use crate::commit::types::CommitDiscardResult;

/// Discard `subject_commit_id`, removing it from the branch history.
///
/// Unlike [`super::uncommit::commit_uncommit()`], the commit's changes are **not**
/// reassigned to the workspace — they are permanently removed from the branch.
///
/// When `dry_run` is enabled, the returned workspace previews the discard
/// without materializing the rebase.
/// See [`commit_discard_only_with_perm()`] for details.
#[but_api(try_from = crate::commit::json::CommitDiscardResult)]
pub fn commit_discard_only(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    dry_run: DryRun,
) -> anyhow::Result<CommitDiscardResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_discard_only_with_perm(ctx, subject_commit_id, dry_run, guard.write_permission())
}

/// Discard `subject_commit_id` under caller-held exclusive repository access.
///
/// The commit is removed from branch history and its changes are **lost**
/// (not reassigned to the workspace). This variant does not create an oplog
/// entry. When `dry_run` is enabled, it returns the projected workspace state
/// without materializing the rebase.
pub fn commit_discard_only_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitDiscardResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let rebase = but_workspace::commit::discard_commits(editor, [subject_commit_id])?;

    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;

    Ok(CommitDiscardResult {
        discarded_commit: subject_commit_id,
        workspace,
    })
}

/// Discard `subject_commit_id`, removing it from the branch history.
///
/// Unlike [`super::uncommit::commit_uncommit()`], the commit's changes are **not**
/// reassigned to the workspace — they are permanently removed from the branch.
///
/// When `dry_run` is enabled, the returned workspace previews the discard and
/// no oplog entry is persisted. See [`commit_discard_with_perm()`] for details.
#[but_api(napi, try_from = crate::commit::json::CommitDiscardResult)]
#[instrument(err(Debug))]
pub fn commit_discard(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    dry_run: DryRun,
) -> anyhow::Result<CommitDiscardResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_discard_with_perm(ctx, subject_commit_id, dry_run, guard.write_permission())
}

/// Discard `subject_commit_id` under caller-held exclusive repository access
/// and record an oplog snapshot on success.
///
/// The commit is removed from branch history and its changes are **lost**
/// (not reassigned to the workspace). An oplog snapshot is recorded so the
/// operation can be reverted from the timeline. When `dry_run` is enabled,
/// it returns the projected workspace state and skips oplog persistence.
pub fn commit_discard_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitDiscardResult> {
    let details = SnapshotDetails::new(OperationKind::DiscardCommit)
        .with_trailers([Trailer::Sha(subject_commit_id)]);
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        details,
        perm.read_permission(),
        dry_run,
    );

    let res = commit_discard_only_with_perm(ctx, subject_commit_id, dry_run, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}
