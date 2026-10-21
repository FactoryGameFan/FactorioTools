//! The committed blueprint lists are already normalized: `normalize-lists` would change
//! nothing. The contract tests check that each blueprint survives `CleanBlueprint`
//! unchanged; this checks the lists as a whole, so it also catches a repeated blueprint,
//! one in both lists, a line that does not parse, or a line written differently from how
//! the tool writes it.

use std::collections::HashMap;

use snapshots::normalize::{blueprint_keys, file_lines, normalize_lists};
use snapshots::oil_field_test_dir;

fn read(name: &str) -> String {
    std::fs::read_to_string(oil_field_test_dir().join(name)).expect("the list reads")
}

/// The first place two texts differ, as line numbers and the two lines.
fn first_difference(old: &str, new: &str) -> String {
    let (old, new) = (file_lines(old), file_lines(new));
    let at = old
        .iter()
        .zip(&new)
        .position(|(a, b)| a != b)
        .unwrap_or(old.len().min(new.len()));
    let line = |lines: &[&str]| {
        lines.get(at).map_or("(end of file)".to_string(), |l| {
            l.chars().take(60).collect()
        })
    };
    format!(
        "line {}: was {:?}, would be {:?} ({} lines, would be {})",
        at + 1,
        line(&old),
        line(&new),
        old.len(),
        new.len()
    )
}

#[test]
fn committed_lists_are_normalized() {
    let big = read("big-list.txt");
    let small = read("small-list.txt");
    let big_keys = blueprint_keys("big-list.txt", &big).unwrap_or_else(|e| panic!("{e}"));
    let small_keys = blueprint_keys("small-list.txt", &small).unwrap_or_else(|e| panic!("{e}"));

    // Named failures first, since the rewrite check below only says where the text moves.
    let mut problems = Vec::new();
    let mut seen: HashMap<&str, (&str, usize)> = HashMap::new();
    for (name, keys) in [("small-list.txt", &small_keys), ("big-list.txt", &big_keys)] {
        for (line, json) in keys {
            if let Some((first_name, first_line)) = seen.insert(json, (name, *line)) {
                problems.push(format!(
                    "{name} line {line} is the same blueprint as {first_name} line {first_line}"
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));

    let (new_big, new_small) =
        normalize_lists(("big-list.txt", &big), ("small-list.txt", &small), false)
            .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        new_big == big,
        "normalize-lists would rewrite big-list.txt, {}",
        first_difference(&big, &new_big)
    );
    assert!(
        new_small == small,
        "normalize-lists would rewrite small-list.txt, {}",
        first_difference(&small, &new_small)
    );
}
