use std::str::FromStr;

use anyhow::ensure;
use but_core::{ChangeId, DiffSpec, ref_metadata::StackId, sync::RepoExclusive};
use but_ctx::Context;
use but_db::HunkAssignmentsHandleMut;
use but_hunk_assignment::HunkAssignment;
use but_rebase::graph_rebase::Editor;
use itertools::Itertools;

use crate::{Filter, StackTarget};

pub fn process_workspace_rules(
    ctx: &mut Context,
    assignments: &[HunkAssignment],
    perm: &mut RepoExclusive,
) -> anyhow::Result<usize> {
    let mut updates = 0;
    if assignments.is_empty() {
        // Don't create stacks if there are no changes to assign anywhere
        return Ok(updates);
    }
    let rules = super::list_rules(ctx)?
        .into_iter()
        .filter(|r| r.enabled)
        .filter(|r| matches!(r.trigger, super::Trigger::FileSytemChange))
        .filter(|r| {
            matches!(
                &r.action,
                super::Action::Explicit(super::Operation::Assign { .. })
            ) || matches!(
                &r.action,
                super::Action::Explicit(super::Operation::Amend { .. })
            )
        })
        .collect_vec();

    if rules.is_empty() {
        return Ok(updates);
    }

    let context_lines = ctx.settings.context_lines;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;

    let stack_ids: Vec<_> = ws.stacks.iter().filter_map(|s| s.id).collect();
    let mut new_ws = None;

    for rule in rules {
        match rule.action {
            super::Action::Explicit(super::Operation::Assign { target }) => {
                if let Some((stack_id, maybe_new_ws)) =
                    get_or_create_stack_id(&repo, &ws, &mut meta, target, &stack_ids, perm)
                {
                    if let Some(ws) = maybe_new_ws {
                        ensure!(
                            new_ws.is_none(),
                            "BUG: new stacks are only created once if there are no stacks"
                        );
                        new_ws = Some(ws);
                    }
                    let assignments = matching(assignments, rule.filters.clone())
                        .into_iter()
                        .filter(|e| e.stack_id != Some(stack_id))
                        .map(|mut e| {
                            e.stack_id = Some(stack_id);
                            e.branch_ref_bytes = None;
                            e
                        })
                        .collect_vec();
                    updates += handle_assign(
                        db.hunk_assignments_mut()?,
                        &repo,
                        new_ws.as_ref().unwrap_or(&ws),
                        assignments,
                        context_lines,
                    )
                    .unwrap_or_default();
                }
            }
            super::Action::Explicit(super::Operation::Amend { change_id }) => {
                let assignments = matching(assignments, rule.filters.clone());
                let ws = if let Some(new_ws) = new_ws.as_mut() {
                    new_ws
                } else {
                    &mut ws
                };
                handle_amend(&repo, ws, &mut meta, assignments, &change_id, context_lines)
                    .unwrap_or_default();
            }
            _ => continue,
        };
    }

    if let Some(new_ws) = new_ws {
        *ws = new_ws;
    }

    Ok(updates)
}

fn handle_amend(
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    meta: &mut impl but_core::RefMetadata,
    assignments: Vec<HunkAssignment>,
    change_id: &ChangeId,
    context_lines: u32,
) -> anyhow::Result<()> {
    let changes: Vec<DiffSpec> =
        but_workspace::flatten_diff_specs(assignments.into_iter().map(DiffSpec::from));
    let mut commit_id: Option<gix::ObjectId> = None;
    'outer: for commit in ws.commits() {
        let commit_change_id = commit.attach(repo)?.headers().and_then(|hdr| hdr.change_id);
        if commit_change_id.is_some_and(|cid| cid == *change_id) {
            commit_id = Some(commit.id);
            break 'outer;
        }
    }

    let commit_id = commit_id.ok_or_else(|| {
        anyhow::anyhow!("No commit with Change-Id {change_id} found in the current workspace")
    })?;

    let editor = Editor::create(ws, meta, repo)?;
    let outcome = but_workspace::commit::commit_amend(editor, commit_id, changes, context_lines)?;
    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(
            ?outcome.rejected_specs,
            "Failed to commit at least one hunk"
        );
    }
    outcome.rebase.materialize()?;
    Ok(())
}

fn get_or_create_stack_id(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    meta: &mut impl but_core::RefMetadata,
    target: StackTarget,
    stack_ids_in_ws: &[StackId],
    perm: &mut RepoExclusive,
) -> Option<(StackId, Option<but_graph::Workspace>)> {
    match target {
        StackTarget::StackId(stack_id) => {
            if let Ok(stack_id) = StackId::from_str(&stack_id) {
                if stack_ids_in_ws.iter().any(|e| e == &stack_id) {
                    Some((stack_id, None))
                } else {
                    None
                }
            } else {
                None
            }
        }
        StackTarget::Leftmost => {
            if stack_ids_in_ws.is_empty() {
                create_stack(repo, ws, meta, perm)
                    .ok()
                    .map(|(id, ws)| (id, Some(ws)))
            } else {
                stack_ids_in_ws.first().copied().map(|id| (id, None))
            }
        }
        StackTarget::Rightmost => {
            if stack_ids_in_ws.is_empty() {
                create_stack(repo, ws, meta, perm)
                    .ok()
                    .map(|(id, ws)| (id, Some(ws)))
            } else {
                stack_ids_in_ws.last().copied().map(|id| (id, None))
            }
        }
    }
}

fn create_stack(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    meta: &mut impl but_core::RefMetadata,
    _perm: &mut RepoExclusive,
) -> anyhow::Result<(StackId, but_graph::Workspace)> {
    use anyhow::Context;
    let branch_name = but_core::branch::unique_canned_refname(repo)?;
    let new_ws = but_workspace::branch::create_reference(
        branch_name.as_ref(),
        None,
        repo,
        ws,
        meta,
        |_| StackId::generate(),
        None,
    )?;
    let (stack, _) = new_ws
        .find_segment_and_stack_by_refname(branch_name.as_ref())
        .context("BUG: need to find stack that was just created")?;
    stack
        .id
        .context("BUG: newly created stacks always have an ID")
        .map(|id| (id, new_ws.into_owned()))
}

fn handle_assign(
    db: HunkAssignmentsHandleMut,
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    assignments: Vec<HunkAssignment>,
    context_lines: u32,
) -> anyhow::Result<usize> {
    let len = assignments.len();
    but_hunk_assignment::assign(
        db,
        repo,
        workspace,
        but_hunk_assignment::assignments_to_requests(assignments),
        context_lines,
    )
    .map(|()| len)
    .or_else(|_| Ok(0))
}

fn matching(wt_assignments: &[HunkAssignment], filters: Vec<Filter>) -> Vec<HunkAssignment> {
    if filters.is_empty() {
        return wt_assignments.to_vec();
    }
    let mut assignments = Vec::new();
    for filter in filters {
        match filter {
            Filter::PathMatchesRegex(regex) => {
                for change in wt_assignments.iter() {
                    if regex.is_match(&change.path) {
                        assignments.push(change.clone());
                    }
                }
            }
            Filter::ContentMatchesRegex(regex) => {
                for change in wt_assignments.iter() {
                    if let Some(diff) = change.diff.clone() {
                        let diff = diff.to_string();
                        let matching_lines: Vec<&str> =
                            diff.lines().filter(|line| line.starts_with('+')).collect();
                        if matching_lines.iter().any(|line| regex.is_match(line)) {
                            assignments.push(change.clone());
                        }
                    }
                }
            }
            Filter::FileChangeType(_) => continue,
            Filter::SemanticType(_) => continue,
            Filter::ClaudeCodeSessionId(_) => continue,
        }
    }
    assignments
}
