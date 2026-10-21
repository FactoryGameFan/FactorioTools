//! The blueprint data model, from `src/FactorioTools/Data/*.cs`.
//!
//! Plain types only. Reading and writing blueprint JSON comes with the serialization stage
//! (#138, stage 12), as it lives in `FactorioTools.Serialization` on the C# side.

use std::fmt;

/// `Direction` (`Data/Direction.cs`). The integer values are Factorio's 16-way directions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Direction {
    #[default]
    Up = 0,
    Right = 2,
    Down = 4,
    Left = 6,
}

impl Direction {
    /// The C# enum's integer value.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// `(Direction)value`, for the four values the enum names. The C# cast accepts any
    /// integer; the planner only ever holds these four.
    pub const fn from_value(value: i32) -> Option<Direction> {
        match value {
            0 => Some(Direction::Up),
            2 => Some(Direction::Right),
            4 => Some(Direction::Down),
            6 => Some(Direction::Left),
            _ => None,
        }
    }
}

/// `Enum.ToString()`: the member name.
impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Direction::Up => "Up",
            Direction::Right => "Right",
            Direction::Down => "Down",
            Direction::Left => "Left",
        })
    }
}

/// `Position` (`Data/Position.cs`). Factorio positions are `float`, not `double`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// `SignalID` (`Data/SignalID.cs`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SignalId {
    pub name: String,
    pub signal_type: String,
}

/// `Icon` (`Data/Icon.cs`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Icon {
    pub signal: SignalId,
    pub index: i32,
}

/// `ModuleInsertPlan` (`FactorioTools.Serialization/Data/EntityItemsConverter.cs`): one
/// item type to insert into an entity inventory, written as a Factorio 2.0 `items` array.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleInsertPlan {
    pub name: String,
    pub inventory: i32,
    pub start_stack: i32,
    pub count: i32,
    pub quality: Option<String>,
}

/// `Entity.Items`, typed `object?` in the C#. It holds one of two shapes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntityItems {
    /// Factorio 1.1: a `Dictionary<string, int>` of item name to count, in insertion order.
    Counts(Vec<(String, i32)>),
    /// Factorio 2.0: a `List<ModuleInsertPlan>`.
    InsertPlans(Vec<ModuleInsertPlan>),
}

/// `Entity` (`Data/Entity.cs`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entity {
    pub entity_number: i32,
    pub name: String,
    pub position: Position,
    pub direction: Option<Direction>,
    /// Factorio 2.1.7 pumpjack flipping. Parsed but never read or written by the planner.
    pub mirror: Option<bool>,
    pub items: Option<EntityItems>,
    pub quality: Option<String>,
    pub neighbours: Option<Vec<i32>>,
}

/// `Blueprint` (`Data/Blueprint.cs`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Blueprint {
    pub icons: Vec<Icon>,
    pub entities: Vec<Entity>,
    pub item: String,
    pub version: u64,
}

/// `BlueprintPage` (`Data/BlueprintPage.cs`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlueprintPage {
    pub blueprint: Blueprint,
    pub index: i32,
}

/// `BlueprintBook` (`Data/BlueprintBook.cs`). Note the version is `long` here and
/// `ulong` on `Blueprint`, as in the C#.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlueprintBook {
    pub blueprints: Vec<BlueprintPage>,
    pub item: String,
    pub label: String,
    pub active_index: i32,
    pub version: i64,
}

/// `BlueprintRoot` (`Data/BlueprintRoot.cs`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlueprintRoot {
    pub blueprint: Option<Blueprint>,
    pub blueprint_book: Option<BlueprintBook>,
}

/// `EntityNames` (`Data/EntityNames.cs`).
pub mod entity_names {
    pub mod vanilla {
        pub const PUMPJACK: &str = "pumpjack";
        pub const PIPE: &str = "pipe";
        pub const PIPE_TO_GROUND: &str = "pipe-to-ground";
        pub const SMALL_ELECTRIC_POLE: &str = "small-electric-pole";
        pub const MEDIUM_ELECTRIC_POLE: &str = "medium-electric-pole";
        pub const BIG_ELECTRIC_POLE: &str = "big-electric-pole";
        pub const SUBSTATION: &str = "substation";
        pub const BEACON: &str = "beacon";
        pub const HEAT_PIPE: &str = "heat-pipe";
        pub const WALL: &str = "stone-wall";

        /// Every constant above, for the tests that check each name against Factorio. The
        /// C# gets this list by reflection. `all_arrays_match_their_constants` fails if a
        /// constant is added here and not to this array.
        pub const ALL: [&str; 10] = [
            PUMPJACK,
            PIPE,
            PIPE_TO_GROUND,
            SMALL_ELECTRIC_POLE,
            MEDIUM_ELECTRIC_POLE,
            BIG_ELECTRIC_POLE,
            SUBSTATION,
            BEACON,
            HEAT_PIPE,
            WALL,
        ];
    }

    pub mod aai_industry {
        pub const SMALL_IRON_ELECTRIC_POLE: &str = "small-iron-electric-pole";
    }
}

/// `ItemNames` (`Data/ItemNames.cs`).
pub mod item_names {
    pub mod vanilla {
        /// Renamed in Factorio 2.0 from "effectivity-module-3", which the game now rejects.
        pub const EFFICIENCY_MODULE_3: &str = "efficiency-module-3";
        pub const PRODUCTIVITY_MODULE_3: &str = "productivity-module-3";
        pub const SPEED_MODULE_3: &str = "speed-module-3";
        pub const BLUEPRINT: &str = "blueprint";

        /// Every constant above, as `entity_names::vanilla::ALL` is.
        pub const ALL: [&str; 4] = [
            EFFICIENCY_MODULE_3,
            PRODUCTIVITY_MODULE_3,
            SPEED_MODULE_3,
            BLUEPRINT,
        ];
    }
}

/// `SignalTypes` (`Data/SignalTypes.cs`).
pub mod signal_types {
    pub mod vanilla {
        pub const ITEM: &str = "item";
        pub const FLUID: &str = "fluid";
        pub const VIRTUAL: &str = "virtual";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_values_and_names() {
        assert_eq!(Direction::Left.value(), 6);
        assert_eq!(Direction::from_value(4), Some(Direction::Down));
        assert_eq!(Direction::from_value(1), None);
        assert_eq!(Direction::default(), Direction::Up);
        assert_eq!(Direction::Right.to_string(), "Right");
    }

    /// The source text inside the `pub mod vanilla {` block that follows `parent` in this
    /// file. These blocks hold no nested braces, so the first `}` closes them.
    fn vanilla_source(parent: &str) -> &'static str {
        let source = include_str!("data.rs");
        let open = "pub mod vanilla {";
        let start = source.find(parent).expect("parent module");
        let start = start + source[start..].find(open).expect("vanilla module") + open.len();
        let end = start + source[start..].find('}').expect("closing brace");
        &source[start..end]
    }

    /// Rust has no reflection, so the `ALL` arrays are kept by hand. This counts the
    /// constants declared in each block's source and checks that every array value is
    /// declared there exactly once. A constant left out of `ALL`, or one listed twice, fails.
    #[test]
    fn all_arrays_match_their_constants() {
        let cases: [(&str, &[&str]); 2] = [
            ("pub mod entity_names {", &entity_names::vanilla::ALL),
            ("pub mod item_names {", &item_names::vanilla::ALL),
        ];
        for (parent, all) in cases {
            let block = vanilla_source(parent);
            // Every `pub const` except `ALL` itself.
            let declared = block.matches("pub const ").count() - 1;
            assert_eq!(
                declared,
                all.len(),
                "{parent}: a constant is missing from ALL"
            );
            for name in all {
                let literal = format!("\"{name}\"");
                assert_eq!(
                    block.matches(&literal).count(),
                    1,
                    "{parent}: {name} is not declared exactly once"
                );
            }
        }
    }
}
