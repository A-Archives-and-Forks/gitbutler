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

use anyhow::{Context, Result};

use crate::{Graph, projection::commit::is_managed_workspace_by_message};

/// A head with the commits that are considered upstream of it.
pub struct HeadStatus {
    /// The particular head we're looking at
    pub head: gix::ObjectId,
    /// The commits that are considered upstream of it.
    pub upstream_commits: Vec<gix::ObjectId>,
}

boolean_enums::gen_boolean_enum!(pub FirstParentTraversal);

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
        first_parent: FirstParentTraversal,
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
            let mut walk = repo.rev_walk([target_ref_id]).with_hidden([head]);
            if first_parent == FirstParentTraversal::Yes {
                walk = walk.first_parent_only();
            };

            out.push(HeadStatus {
                head: head.detach(),
                upstream_commits: walk.all()?.map(|c| Ok(c?.id)).collect::<Result<_>>()?,
            })
        }

        Ok(out)
    }
}
