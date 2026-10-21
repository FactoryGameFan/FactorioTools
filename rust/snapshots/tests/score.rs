//! `Score.HasExpectedScore`: the planner-quality scoreboard over the small list.

use snapshots::{
    SCORE_SNAPSHOT, UPDATE_ENV, first_difference, read_snapshot, score_text, update_hint,
    update_mode, write_snapshot,
};

/// 732 plans with every strategy. Run in release:
/// `cargo test --release -p snapshots --test score -- --ignored`. With `UPDATE_SNAPSHOTS=1`
/// it writes the scoreboard instead.
#[test]
#[ignore]
fn score_matches_the_scoreboard() {
    let actual = score_text();
    if update_mode() {
        let written = write_snapshot(SCORE_SNAPSHOT, &actual)
            .unwrap_or_else(|e| panic!("could not write {SCORE_SNAPSHOT}: {e}"));
        println!("{UPDATE_ENV}=1: {SCORE_SNAPSHOT} {written:?}");
        return;
    }

    let expected = read_snapshot(SCORE_SNAPSHOT)
        .unwrap_or_else(|e| panic!("could not read {SCORE_SNAPSHOT}: {e}\n{}", update_hint()));
    assert!(
        expected == actual,
        "{SCORE_SNAPSHOT}: mismatch\n{}{}",
        first_difference(&expected, &actual),
        update_hint()
    );
}
