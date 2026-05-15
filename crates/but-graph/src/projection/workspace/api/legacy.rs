use anyhow::{Context, Result};

use crate::{FirstParent, projection::Workspace};

/// A head with the commits that are considered upstream of it.
pub struct HeadStatus {
    /// The particular head we're looking at
    pub head: gix::ObjectId,
    /// The commits that are considered upstream of it.
    pub upstream_commits: Vec<gix::ObjectId>,
}

/// Support for `gitbutler-` crates.
impl Workspace {
    /// Lists the commits in the set `target_ref ^[branch in workspace]`.
    ///
    /// Returns one entry for each stack tip in the workspace.
    /// If the workspace has no stack tips, returns one entry for the traversal
    /// entrypoint if it has a commit.
    ///
    /// Could return zero head statuses if the workspace has no stack tips and
    /// the traversal entrypoint has no commit (which would be unexpected, too).
    ///
    /// If a stack head or the HEAD itself in the plain-git-mode scenario has no common
    /// history with the `target_ref`, as a logical extension of the specified
    /// revspec, all the commits reachable from the `target_ref` will be
    /// returned - the tip that could hide some of the target isn't present.
    ///
    /// When looking at the new understanding of stacks used in the new
    /// `integrate_upstream` function, if you have a stack with N >1 heads,
    /// there will be a N entries in here which correspond with each of the
    /// stack heads. The results for each head in the same stack should be
    /// the same.
    ///
    /// ## Before Promoting this to non-legacy
    ///
    /// What I don't like is the semantics of "empty workspace == one HeadStatus".
    /// This isn't to say it's wrong, but it certainly doesn't appear the most logical.
    /// Could be an issue with terminology, what is `Head` anyway? Is it a branch, Stack,
    /// branch in a stack?
    /// Terms used here are Tip for the commit-id of the start of something.
    ///
    /// Also, we can easily write what's needed to serve the needs of new code, so it's
    /// also fine if this goes away.
    /// 
    /// If only the commits that aren't in the workspace were needed (i.e. not per stack),
    /// then one can do a mere pruned traversal from `target_tip ^InWorkspace`.
    pub fn upstream_commits(&self, first_parent: FirstParent) -> Result<Vec<HeadStatus>> {
        let graph = &self.graph;
        let target_segment_id = self
            .target_ref
            .as_ref()
            .context("Upstream commits can only be calculated on a workspace with a target ref")?
            .segment_index;
        let mut heads = self
            .stacks
            .iter()
            .filter_map(|stack| stack.tip_skip_empty())
            .collect::<Vec<_>>();
        if heads.is_empty()
            && let Some(entrypoint) = graph.entrypoint_commit()
        {
            heads.push(entrypoint.id);
        }

        let mut out = vec![];

        for head in heads {
            let head_segment_id = graph.commit_id_to_segment_id(head)?;
            out.push(HeadStatus {
                head,
                upstream_commits: graph
                    .find_segments_reachable_from_a_not_b(
                        target_segment_id,
                        head_segment_id,
                        first_parent,
                    )
                    .into_iter()
                    .map(|commit| commit.id)
                    .collect(),
            })
        }

        Ok(out)
    }
    /// Return the target reference name if this workspace has a branch-backed target.
    ///
    /// ## Before Promoting this to non-legacy
    ///
    /// To me this looks like an 'unsure' way of getting the target-ref name.
    /// I'd trust that `but-graph` knows how to 'see' the target ref during traversal
    /// so the `target_ref` field is populated. I would *not* read it from `self.metadata`,
    /// which means it might also not exist at all.
    /// If promoted as is, these exact semantics should be documented, along with its intended use.
    pub fn target_ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.target_ref
            .as_ref()
            .map(|target| target.ref_name.as_ref())
            .or_else(|| {
                self.metadata
                    .as_ref()
                    .and_then(|metadata| metadata.target_ref.as_ref().map(|name| name.as_ref()))
            })
    }

    /// Return the remembered target commit id that anchors this workspace to its target.
    ///
    /// This is the projection equivalent of the legacy `Target::sha` field. It intentionally
    /// differs from [`Self::target_ref_tip_commit_id()`], which returns the current tip of the target
    /// branch.
    ///
    /// ## Before Promoting this to non-legacy
    ///
    /// I'd expect this to not be useful unless maybe for display purposes.
    /// What I don't like about this function is that it resorts prefers `metadata` over
    /// the resolved and validated `target_commit` on this instance, without making clear why
    /// in the docs.
    /// I think it's important to nail this semantically, and if in doubt, I'd rather make `metadata`
    /// inaccessible to provide only a single-source of truth and remove ambiguity.
    pub fn target_base_commit_id(&self) -> Option<gix::ObjectId> {
        self.metadata
            .as_ref()
            .and_then(|metadata| metadata.target_commit_id)
            .or_else(|| self.target_commit.as_ref().map(|target| target.commit_id))
    }

    /// Return the current tip commit id of the target reference if it is available in the graph.
    pub fn target_ref_tip_commit_id(&self) -> Option<gix::ObjectId> {
        self.target_ref
            .as_ref()
            .and_then(|target| self.graph.tip_skip_empty(target.segment_index))
            .map(|commit| commit.id)
    }
}
