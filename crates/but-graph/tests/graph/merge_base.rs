use but_graph::{Graph, Segment, SegmentIndex, SegmentRelation};
use but_testsupport::graph_tree;

use crate::init::{read_only_in_memory_scenario, standard_options};

#[test]
fn find_git_merge_base_handles_duplicate_queue_entries_and_redundant_bases() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let merged = segment_id_by_ref_name(&graph, "refs/heads/merged")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    let c = segment_id_by_ref_name(&graph, "refs/heads/C")?;
    let main = segment_id_by_ref_name(&graph, "refs/heads/main")?;

    // merged -> (A,C) -> ... -> main causes the walk from merged to queue shared ancestors repeatedly.
    assert_eq!(graph.find_merge_base(merged, main), Some(main));

    // For (merged, A), both A and main are common in ancestry, but A is the nearest one.
    assert_eq!(graph.find_merge_base(merged, a), Some(a));
    assert_ne!(graph.find_merge_base(merged, a), Some(main));

    // Independent branches under the same merge should converge at main.
    assert_eq!(graph.find_merge_base(a, c), Some(main));
    assert_eq!(graph.find_merge_base_octopus([a, c, merged]), Some(main));

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:7[3]:main
            │       │           └── 🏁·965998b (⌂|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:5[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:7: (main)
                    └── ►:6[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:7: (main)
    ");

    Ok(())
}

#[test]
fn relation_between_matches_merge_base_in_redundant_ancestor_case() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let merged = segment_id_by_ref_name(&graph, "refs/heads/merged")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    let c = segment_id_by_ref_name(&graph, "refs/heads/C")?;

    assert_eq!(graph.relation_between(a, merged), SegmentRelation::Ancestor);
    assert_eq!(
        graph.relation_between(merged, a),
        SegmentRelation::Descendant
    );
    assert_eq!(graph.relation_between(a, c), SegmentRelation::Diverged);
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:7[3]:main
            │       │           └── 🏁·965998b (⌂|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:5[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:7: (main)
                    └── ►:6[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:7: (main)
    ");

    Ok(())
}

#[test]
fn relation_between_handles_identity_and_disjoint_segments() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let mut graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let main = segment_id_by_ref_name(&graph, "refs/heads/main")?;
    assert_eq!(
        graph.relation_between(main, main),
        SegmentRelation::Identity
    );

    let orphan = graph.insert_segment(Segment {
        id: SegmentIndex::new(usize::MAX),
        generation: 0,
        ref_info: None,
        remote_tracking_ref_name: None,
        sibling_segment_id: None,
        remote_tracking_branch_segment_id: None,
        commits: Vec::new(),
        metadata: None,
    });
    assert_eq!(
        graph.relation_between(main, orphan),
        SegmentRelation::Disjoint
    );

    Ok(())
}

#[test]
fn merge_base_apis_can_resolve_segments_by_first_commit_id() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let merged = segment_id_by_ref_name(&graph, "refs/heads/merged")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    let c = segment_id_by_ref_name(&graph, "refs/heads/C")?;
    let main = segment_id_by_ref_name(&graph, "refs/heads/main")?;

    let merged_id = graph[merged].tip().expect("commit");
    let a_id = graph[a].tip().expect("commit");
    let c_id = graph[c].tip().expect("commit");
    let main_id = graph[main].tip().expect("commit");

    assert_eq!(
        graph.relation_between_by_commit_id(a_id, merged_id)?,
        SegmentRelation::Ancestor
    );
    assert_eq!(
        graph.find_merge_base_by_commit_id(merged_id, a_id)?,
        Some(a_id)
    );
    assert_eq!(
        graph.find_merge_base_octopus_by_commit_id([a_id, c_id, merged_id])?,
        Some(main_id)
    );

    assert!(
        graph
            .find_merge_base_by_commit_id(repo.object_hash().null(), main_id)
            .is_err()
    );

    Ok(())
}

fn segment_id_by_ref_name(graph: &Graph, name: &str) -> anyhow::Result<SegmentIndex> {
    let full_name: gix::refs::FullName = name.try_into()?;
    graph
        .named_segment_by_ref_name(full_name.as_ref())
        .map(|s| s.id)
        .ok_or_else(|| anyhow::anyhow!("missing segment for {name}"))
}
