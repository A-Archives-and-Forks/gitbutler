use but_graph::{Graph, SegmentIndex, SegmentRelation};
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
    assert_eq!(graph.find_git_merge_base(merged, main), Some(main));

    // For (merged, A), both A and main are common in ancestry, but A is the nearest one.
    assert_eq!(graph.find_git_merge_base(merged, a), Some(a));
    assert_ne!(graph.find_git_merge_base(merged, a), Some(main));

    // Independent branches under the same merge should converge at main.
    assert_eq!(graph.find_git_merge_base(a, c), Some(main));
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

fn segment_id_by_ref_name(graph: &Graph, name: &str) -> anyhow::Result<SegmentIndex> {
    let full_name: gix::refs::FullName = name.try_into()?;
    graph
        .named_segment_by_ref_name(full_name.as_ref())
        .map(|s| s.id)
        .ok_or_else(|| anyhow::anyhow!("missing segment for {name}"))
}
