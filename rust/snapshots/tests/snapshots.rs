//! The Rust planner against the C# planner's committed Verify snapshots. With
//! `UPDATE_SNAPSHOTS=1` these tests write the snapshots instead; see
//! [`snapshots::UPDATE_COMMAND`].

use planner::models::PipeStrategy;
use snapshots::{
    SCORE_SNAPSHOT, UPDATE_ENV, all_cases, big_list, check, electric_pole_options,
    planner_test_cases, small_list, update_hint, update_mode,
};

/// The `PlannerTest` and `PlannerIssueTest` snapshots, which run quickly enough in a debug
/// build.
#[test]
fn planner_test_snapshots_match() {
    let (report, failures) = check(&planner_test_cases());
    println!("{report}");
    assert_eq!(failures, 0, "{report}");
}

/// Every planner snapshot, all but the scoreboard. Slow in a debug build, so run it in
/// release: `cargo test --release -p snapshots --test snapshots -- --ignored`.
#[test]
#[ignore]
fn all_snapshots_match() {
    let (report, failures) = check(&all_cases(1));
    println!("{report}");
    assert_eq!(failures, 0, "{report}");
}

/// Every committed `*.verified.txt` under the test directory must be one the Rust harness
/// checks, so a snapshot added on the C# side can't be skipped here without failing, and
/// every one it checks must be committed. With `UPDATE_SNAPSHOTS=1` it deletes the files no
/// case produces any more instead, and leaves creating the missing ones to the tests above.
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

    // `all_cases` includes `planner_test_cases`; the scoreboard is checked by tests/score.rs.
    let cases = all_cases(1);
    let checked: BTreeSet<String> = cases
        .iter()
        .map(|case| case.snapshot.clone())
        .chain([SCORE_SNAPSHOT.to_string()])
        .collect();

    // Update mode deletes by this set, so check its size before trusting it, in place of a
    // pinned file count that every new blueprint would have to edit. Each small-list
    // blueprint makes 24 PlannerWithAllOptions snapshots (4 poles, 3 beacon settings, with
    // and without underground pipes) and one each in AllStrategiesOnSmallList and
    // NonStandardBeacon. Each big-list blueprint makes one in AllStrategiesOnBigList. Then
    // one per pipe strategy for the isolated-area theory, the PlannerTest and
    // PlannerIssueTest methods, and the scoreboard.
    assert_eq!(
        checked.len(),
        cases.len() + 1,
        "two cases write the same snapshot file"
    );
    let expected = small_list().len() * (electric_pole_options().len() * 3 * 2 + 2)
        + big_list().len()
        + PipeStrategy::ALL.len()
        + planner_test_cases().len()
        + 1;
    assert_eq!(
        checked.len(),
        expected,
        "all_cases no longer makes the snapshots this test expects from the two lists; \
         if a theory was added or removed on purpose, update this count's formula"
    );

    let root = snapshots::oil_field_test_dir();
    let mut on_disk = BTreeSet::new();
    walk(&root, &root, &mut on_disk);
    let unchecked: Vec<_> = on_disk.difference(&checked).collect();

    if update_mode() {
        for stale in &unchecked {
            let path = root.join(stale);
            std::fs::remove_file(&path)
                .unwrap_or_else(|e| panic!("could not delete {}: {e}", path.display()));
            println!("{UPDATE_ENV}=1: deleted {stale}");
        }
        println!(
            "{UPDATE_ENV}=1: deleted {} stale snapshots",
            unchecked.len()
        );
        return;
    }

    let missing: Vec<_> = checked.difference(&on_disk).collect();
    assert!(
        unchecked.is_empty() && missing.is_empty(),
        "snapshots on disk but not checked: {unchecked:?}; checked but not on disk: \
         {missing:?}\n{}",
        update_hint()
    );
}
