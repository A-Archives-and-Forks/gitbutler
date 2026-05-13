//! Describing relationships to the target ref
//!
//! The contents of the workspace is defined as `HEAD ^target.sha`.
//!
//! When it comes to calling `integrate_upstream`, what we really want to do, is
//! take the commits in `HEAD ^target.sha` and make sure they in some way
//! include the `target_ref`.
//!
//! What that practically means, is that if there are commits in the set
//! `target_ref ^[branch in workspace]`, we want to update that branch such
//! that it includes the changes in `target_ref`.
//!
//! Whether or not we update a branch has no relevance to the `target.sha`. The
//! `target.sha` is simply defining what we consider in the workspace.

use std::{backtrace::Backtrace, collections::HashSet};

use anyhow::{Context, Result, bail};
use petgraph::Direction;

use crate::{Graph, projection::commit::is_managed_workspace_by_message};

/// A head with the commits that are considered upstream of it.
pub struct HeadStatus {
    /// The particular head we're looking at
    pub head: gix::ObjectId,
    /// The commits that are considered upstream of it.
    pub upstream_commits: Vec<gix::ObjectId>,
}

impl Graph {
    /// Lists the commits in the set `target_ref ^[branch in workspace]`.
    ///
    /// If `entrypoint_commit` is `is_managed_workspace_by_message`, then we
    /// return an entry for each parent of `entrypoint_commit`, otherwise we
    /// return one entry for `target_ref ^entrypoint_commit`.
    ///
    /// Could return zero head statuses if the workspace commit has no parents.
    ///
    /// If a stack head or the HEAD itself in the PGM scenario has no common
    /// history with the `target_ref`, as a logical extension of the specified
    /// revspec, all the commits reachable from the `target_ref` will be
    /// returned.
    ///
    /// When looking at the new understanding of stacks used in the new
    /// `integrate_upstream` function, if you have a stack with N >1 heads,
    /// there will be a N entries in here which coorespond with each of the
    /// stack heads. The results for each of head in the smae stack should be
    /// the same.
    pub fn upstream_commits(
        &self,
        repo: &gix::Repository,
        target_ref: &gix::refs::FullNameRef,
    ) -> Result<Vec<HeadStatus>> {
        let entrypoint = self.entrypoint_commit().context(
            "Upstream commits can only be calculated on a graph that has some entrypoint commit",
        )?;
        let entrypoint = repo.find_commit(entrypoint.id)?;
        let heads = if is_managed_workspace_by_message(entrypoint.message_raw()?) {
            entrypoint.parent_ids().collect::<Vec<_>>()
        } else {
            vec![entrypoint.id()]
        };

        let target_ref_id = repo.find_reference(target_ref)?.peel_to_commit()?.id;

        let mut out = vec![];

        for head in heads {
            // There are more efficient ways of computing a `A ^B` revset, but
            // with the workspace being reasonably bounded to about 1000 commits
            // of history, I'm not too concerend about performance here.
            let mut negative_commits = HashSet::new();
            self.traverse_commit_and_parents(head.detach(), |id| {
                negative_commits.insert(id);
                false
            })?;

            let mut positive_commits = vec![];
            self.traverse_commit_and_parents(target_ref_id, |id| {
                if negative_commits.contains(&id) {
                    true
                } else {
                    positive_commits.push(id);
                    false
                }
            })?;

            out.push(HeadStatus {
                head: head.detach(),
                upstream_commits: positive_commits,
            })
        }

        Ok(out)
    }
}

impl Graph {
    fn traverse_commit_and_parents(
        &self,
        start: gix::ObjectId,
        mut cb: impl FnMut(gix::ObjectId) -> bool,
    ) -> Result<()> {
        let mut sidx = None;
        for id in self.inner.node_indices() {
            let mut found = false;
            for commit in &self[id].commits {
                if commit.id == start {
                    found = true;
                    sidx = Some(id);
                }

                if found && cb(commit.id) {
                    return Ok(());
                }
            }
            if found {
                break;
            }
        }

        let Some(sidx) = sidx else {
            bail!("Failed to find a segment containing commit {start}");
        };
        self.visit_all_segments_excluding_start_until(sidx, Direction::Outgoing, |segment| {
            for commit in &segment.commits {
                if cb(commit.id) {
                    return true;
                }
            }

            false
        });

        Ok(())
    }
}
