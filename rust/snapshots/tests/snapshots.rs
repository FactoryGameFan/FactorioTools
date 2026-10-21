//! The Rust planner against the C# planner's committed Verify snapshots.

use snapshots::{all_cases, check, planner_test_cases};

/// The `PlannerTest` and `PlannerIssueTest` snapshots, which run quickly enough in a debug
/// build.
#[test]
fn planner_test_snapshots_match() {
    let (report, failures) = check(&planner_test_cases());
    println!("{report}");
    assert_eq!(failures, 0, "{report}");
}

/// Every planner snapshot: 2,747 files. Slow in a debug build, so run it in release:
/// `cargo test --release -p snapshots --test snapshots -- --ignored`.
#[test]
#[ignore]
fn all_snapshots_match() {
    let (report, failures) = check(&all_cases(1));
    println!("{report}");
    assert_eq!(failures, 0, "{report}");
}

/// Every committed `*.verified.txt` under the test directory must be one the Rust harness
/// checks, so a snapshot added on the C# side can't be skipped here without failing.
#[test]
fn every_committed_snapshot_is_checked() {
    use std::collections::BTreeSet;
    use std::path::Path;

    fn walk(dir: &Path, root: &Path, found: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).expect("the test directory reads") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                walk(&path, root, found);
            } else if path.to_string_lossy().ends_with(".verified.txt") {
                let relative = path.strip_prefix(root).expect("under the root");
                found.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    let root = snapshots::oil_field_test_dir();
    let mut on_disk = BTreeSet::new();
    walk(&root, &root, &mut on_disk);

    let mut checked: BTreeSet<String> = all_cases(1)
        .into_iter()
        .chain(planner_test_cases())
        .map(|case| case.snapshot)
        .collect();
    // Checked by tests/score.rs.
    checked.insert("Score.HasExpectedScore.verified.txt".to_string());

    let unchecked: Vec<_> = on_disk.difference(&checked).collect();
    let missing: Vec<_> = checked.difference(&on_disk).collect();
    assert!(
        unchecked.is_empty() && missing.is_empty(),
        "snapshots on disk but not checked: {unchecked:?}; checked but not on disk: {missing:?}"
    );
    assert_eq!(
        on_disk.len(),
        2748,
        "the committed snapshot count changed; update this test"
    );
}
