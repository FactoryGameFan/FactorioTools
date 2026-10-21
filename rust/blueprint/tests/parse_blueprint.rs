//! `test/FactorioTools.Test/OilField/ParseBlueprintTest.cs`.

use blueprint::{parse_blueprint, parse_raw, to_internal_direction};
use planner::data::{Direction, entity_names};

/// Three pumpjacks from a probe mod in Factorio 2.1.14, the second and third mirrored.
const MIRRORED_PUMPJACKS: &str = "0eNqN0MkKgzAQBuB3mfNQNC5UX6WU4jKUaU0MSSwVybs36qFQK3gaZvm/w0xQdwNpw8pBOQE3vbJQXiawfFdVN89UJQlK0IPUj6p5gkdg1dIbythfEUg5dkxramnGmxpkTSYc4CaNoHsbAr2a7YBEpwxhXGqQWzbUrNvU4wYUB8B4X0SQbEwfIGcG+uMnB3yx659//PAddiSD9n0ywouMXc6zXBRZKtIiifI4Trz/ADGoghU=";

const VERSION_2_1_14: u64 = 562_954_249_306_113;
const VERSION_1_1_0: u64 = 281_479_271_677_952;

/// One east pumpjack (direction 4) and two rails at 2.x diagonals (2 and 14).
const PUMPJACK_WITH_DIAGONAL_RAILS: &str = "0eNqN0N0KwiAUB/B3OdcW6j4gXyUi3HZY1nSiLhrDd29zEI110dVRz/F34D9B1Q1onTIBxASq7o0HcZ5PpsEXCEbAq9bIbukaqREE2EHbu6wfQCCMdnlRATXEeCGAJqigcDXSZbyaQVfokrUXbO/nD71Z/HkfPRYExlQjgUY5rNduHskO5B/QBydVewuHuXR7ldGEMro1+Q8z+9vkq8m3JsuXGFIg4itbAk90Pk0UJT8VOc9PGS0Zy2J8A6LMfy4=";

/// From Factorio 2.1.14: four pumpjacks facing north, east, south, west.
const FOUR_CARDINAL_PUMPJACKS: &str = "0eNqN0csKwyAQBdB/mbWU+Eho/JVSSh5Dsa1G1JSG4L/XJIsWkkBWg3rvWTgj1K8erVMmgBxBNZ3xIC8jeHU31Wu6M5VGkGB7bR9V84RIQJkWPyBpvBJAE1RQuLTmw3Azva7RpQBZtQnYzqdCZyY7IdkpJzDMM0ayItgBgv4bBFrlsFmexYbID4hsVzxviOKAyHdFyqZvVAF1An7bIPBG5+dEXrAyF0yUPCso5TF+Abr9jgg=";

#[test]
fn keeps_the_mirror_flag_on_a_flipped_entity() {
    let blueprint = parse_blueprint(MIRRORED_PUMPJACKS).unwrap();
    let mirrors: Vec<_> = blueprint.entities.iter().map(|e| e.mirror).collect();
    assert_eq!(mirrors, [None, Some(true), Some(true)]);
}

#[test]
fn converts_blueprint_directions_to_internal_ones() {
    for (raw, version, expected) in [
        // Factorio 2.x blueprints: north omitted, east 4, south 8, west 12.
        (0, VERSION_2_1_14, Direction::Up),
        (4, VERSION_2_1_14, Direction::Right),
        (8, VERSION_2_1_14, Direction::Down),
        (12, VERSION_2_1_14, Direction::Left),
        // An unstamped blueprint is assumed to be modern.
        (4, 0, Direction::Right),
        // Genuine 1.1 blueprints still parse, when they say so.
        (2, VERSION_1_1_0, Direction::Right),
        (6, VERSION_1_1_0, Direction::Left),
    ] {
        assert_eq!(
            to_internal_direction(raw, version).unwrap(),
            expected,
            "{raw} {version}"
        );
    }
}

#[test]
fn leaves_the_direction_of_entities_that_are_not_pumpjacks_alone() {
    // The planner's `Direction` holds only the four pumpjack directions, so the raw values
    // of other entities are checked on the raw JSON model.
    let raw = parse_raw(PUMPJACK_WITH_DIAGONAL_RAILS).unwrap();
    let entities = raw.blueprint.unwrap().entities.unwrap();
    let summary: Vec<_> = entities
        .iter()
        .map(|e| (e.name.clone().unwrap(), e.direction))
        .collect();
    assert_eq!(
        summary,
        [
            ("pumpjack".to_string(), Some(4)),
            ("straight-rail".to_string(), Some(2)),
            ("straight-rail".to_string(), Some(14)),
        ]
    );

    // And parsing does not reject the rails' diagonals: only the pumpjack is converted.
    let blueprint = parse_blueprint(PUMPJACK_WITH_DIAGONAL_RAILS).unwrap();
    assert_eq!(blueprint.entities[0].name, entity_names::vanilla::PUMPJACK);
    assert_eq!(blueprint.entities[0].direction, Some(Direction::Right));
}

#[test]
fn rejects_directions_a_pumpjack_cannot_have() {
    for raw in [2, 6, 10, 14, 1, 5, 9, 13] {
        let error = to_internal_direction(raw, VERSION_2_1_14).unwrap_err();
        assert!(
            error.message.contains(&raw.to_string()),
            "{}",
            error.message
        );
        assert!(error.bad_input);
    }
}

#[test]
fn reads_every_cardinal_from_a_real_factorio_2_1_blueprint() {
    let blueprint = parse_blueprint(FOUR_CARDINAL_PUMPJACKS).unwrap();
    let directions: Vec<_> = blueprint.entities.iter().map(|e| e.direction).collect();
    // North is written as an omitted field, so the first direction is None.
    assert_eq!(
        directions,
        [
            None,
            Some(Direction::Right),
            Some(Direction::Down),
            Some(Direction::Left)
        ]
    );
}

#[test]
fn reports_the_csharp_errors() {
    assert_eq!(
        parse_blueprint("").unwrap_err().message,
        "Input blueprint string is empty."
    );
    assert_eq!(
        parse_blueprint("1abc").unwrap_err().message,
        "Input blueprint does not have the expected version byte of '0' at the beginning."
    );
    let truncated = &MIRRORED_PUMPJACKS[..MIRRORED_PUMPJACKS.len() / 2];
    assert_eq!(
        parse_blueprint(truncated).unwrap_err().message,
        "Input blueprint string could not be fully decoded. Are you sure you copied the whole blueprint?"
    );
}

#[test]
fn reads_blueprint_files_like_read_all_lines() {
    let lines = blueprint::read_blueprint_lines("\u{feff}# comment\r\n  0abc  \n\n0def\r0ghi\n");
    assert_eq!(lines, ["0abc", "0def", "0ghi"]);
}

/// Every corpus blueprint parses.
#[test]
fn parses_the_whole_corpus() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/oil-field");
    for list in ["small-list.txt", "big-list.txt"] {
        let lines = blueprint::read_blueprint_file(root.join(list)).unwrap();
        assert!(!lines.is_empty());
        for (i, line) in lines.iter().enumerate() {
            parse_blueprint(line).unwrap_or_else(|e| panic!("{list} {i}: {}", e.message));
        }
    }
}
