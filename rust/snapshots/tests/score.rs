//! `Score.HasExpectedScore`: the planner-quality scoreboard over the small list.

use snapshots::{first_difference, read_snapshot, score_text};

/// 732 plans with every strategy. Run in release:
/// `cargo test --release -p snapshots --test score -- --ignored`.
#[test]
#[ignore]
fn score_matches_the_scoreboard() {
    let expected = read_snapshot("Score.HasExpectedScore.verified.txt").unwrap();
    let actual = score_text();
    assert!(
        expected == actual,
        "{}",
        first_difference(&expected, &actual)
    );
}
