//! `OilFieldOptions`, from `src/FactorioTools/OilField/OilFieldOptions.cs`.
//!
//! The C# presets are static properties that build a fresh object on every read, which
//! callers then change. The Rust presets are functions returning a fresh value for the same
//! reason.

use crate::data::{entity_names, item_names};
use crate::models::{BeaconStrategy, PipeStrategy};
use crate::quality::Quality;

/// `OilFieldOptions.AllPipeStrategies`.
pub const ALL_PIPE_STRATEGIES: [PipeStrategy; 5] = [
    PipeStrategy::FbeOriginal,
    PipeStrategy::Fbe,
    PipeStrategy::ConnectedCentersDelaunay,
    PipeStrategy::ConnectedCentersDelaunayMst,
    PipeStrategy::ConnectedCentersFlute,
];

/// `OilFieldOptions.DefaultPipeStrategies`.
pub const DEFAULT_PIPE_STRATEGIES: [PipeStrategy; 4] = [
    PipeStrategy::Fbe,
    PipeStrategy::ConnectedCentersDelaunay,
    PipeStrategy::ConnectedCentersDelaunayMst,
    PipeStrategy::ConnectedCentersFlute,
];

/// `OilFieldOptions.AllBeaconStrategies`.
pub const ALL_BEACON_STRATEGIES: [BeaconStrategy; 3] = [
    BeaconStrategy::FbeOriginal,
    BeaconStrategy::Fbe,
    BeaconStrategy::Snug,
];

/// `OilFieldOptions.DefaultBeaconStrategies`.
pub const DEFAULT_BEACON_STRATEGIES: [BeaconStrategy; 2] =
    [BeaconStrategy::Fbe, BeaconStrategy::Snug];

/// `OilFieldOptions`. Field defaults are in [`Default`]; each field's doc is the C#'s.
#[derive(Clone, Debug, PartialEq)]
pub struct OilFieldOptions {
    /// Whether to use underground pipes (pipe-to-ground).
    pub use_underground_pipes: bool,
    /// Whether to add beacons around the pumpjacks.
    pub add_beacons: bool,
    /// Whether to run the pipe optimizer after each pipe strategy. The better of the
    /// optimized and unoptimized plans is still the one used.
    pub optimize_pipes: bool,
    /// Whether beacon effects may overlap. Mods like Space Exploration forbid it.
    pub overlap_beacons: bool,
    /// Whether to add electric poles around the pumpjacks and beacons.
    pub add_electric_poles: bool,
    /// Whether to route a heat pipe network next to every pumpjack and pipe (Aquilo).
    pub add_heat_pipes: bool,
    /// The heat pipe's internal entity name.
    pub heat_pipe_entity_name: String,
    /// The pipe planning strategies to try.
    pub pipe_strategies: Vec<PipeStrategy>,
    /// The beacon planning strategies to try. Ignored when `add_beacons` is false.
    pub beacon_strategies: Vec<BeaconStrategy>,
    /// The electric pole's internal entity name.
    pub electric_pole_entity_name: String,
    /// The width of the area an electric pole powers.
    pub electric_pole_supply_width: i32,
    /// The height of the area an electric pole powers.
    pub electric_pole_supply_height: i32,
    /// How far apart two electric poles can be and still connect.
    pub electric_pole_wire_reach: f64,
    /// The electric pole entity's width.
    pub electric_pole_width: i32,
    /// The electric pole entity's height.
    pub electric_pole_height: i32,
    /// The beacon's internal entity name.
    pub beacon_entity_name: String,
    /// The width of the area a beacon affects.
    pub beacon_supply_width: i32,
    /// The height of the area a beacon affects.
    pub beacon_supply_height: i32,
    /// The beacon entity's width.
    pub beacon_width: i32,
    /// The beacon entity's height.
    pub beacon_height: i32,
    /// Whether to run extra validations on the result.
    pub validate_solution: bool,
    /// Modules for each pumpjack: item name and count, in the C# `Dictionary`'s order.
    pub pumpjack_modules: Vec<(String, i32)>,
    /// Modules for each beacon: item name and count, in the C# `Dictionary`'s order.
    pub beacon_modules: Vec<(String, i32)>,
    /// Pumpjack quality in the output blueprint. Output only.
    pub pumpjack_quality: Quality,
    /// Beacon quality in the output blueprint. Output only.
    pub beacon_quality: Quality,
    /// Electric pole quality. It enlarges supply area and wire reach, so it affects planning.
    pub electric_pole_quality: Quality,
    /// Quality of the pumpjack modules. Output only.
    pub pumpjack_module_quality: Quality,
    /// Quality of the beacon modules. Output only.
    pub beacon_module_quality: Quality,
}

/// `new OilFieldOptions()`: every property initializer in the C#.
impl Default for OilFieldOptions {
    fn default() -> Self {
        OilFieldOptions {
            use_underground_pipes: true,
            add_beacons: true,
            optimize_pipes: true,
            overlap_beacons: true,
            add_electric_poles: true,
            add_heat_pipes: false,
            heat_pipe_entity_name: entity_names::vanilla::HEAT_PIPE.to_string(),
            pipe_strategies: DEFAULT_PIPE_STRATEGIES.to_vec(),
            beacon_strategies: DEFAULT_BEACON_STRATEGIES.to_vec(),
            electric_pole_entity_name: entity_names::vanilla::MEDIUM_ELECTRIC_POLE.to_string(),
            electric_pole_supply_width: 7,
            electric_pole_supply_height: 7,
            electric_pole_wire_reach: 9.0,
            electric_pole_width: 1,
            electric_pole_height: 1,
            beacon_entity_name: entity_names::vanilla::BEACON.to_string(),
            beacon_supply_width: 9,
            beacon_supply_height: 9,
            beacon_width: 3,
            beacon_height: 3,
            validate_solution: false,
            pumpjack_modules: vec![(item_names::vanilla::PRODUCTIVITY_MODULE_3.to_string(), 2)],
            beacon_modules: vec![(item_names::vanilla::SPEED_MODULE_3.to_string(), 2)],
            pumpjack_quality: Quality::NORMAL,
            beacon_quality: Quality::NORMAL,
            electric_pole_quality: Quality::NORMAL,
            pumpjack_module_quality: Quality::NORMAL,
            beacon_module_quality: Quality::NORMAL,
        }
    }
}

impl OilFieldOptions {
    fn with_pole(
        name: &str,
        supply_width: i32,
        supply_height: i32,
        wire_reach: f64,
        width: i32,
        height: i32,
    ) -> Self {
        OilFieldOptions {
            electric_pole_entity_name: name.to_string(),
            electric_pole_supply_width: supply_width,
            electric_pole_supply_height: supply_height,
            electric_pole_wire_reach: wire_reach,
            electric_pole_width: width,
            electric_pole_height: height,
            ..OilFieldOptions::default()
        }
    }

    /// `ForSmallIronElectricPole` (AAI Industry).
    pub fn for_small_iron_electric_pole() -> Self {
        Self::with_pole(
            entity_names::aai_industry::SMALL_IRON_ELECTRIC_POLE,
            5,
            5,
            7.5,
            1,
            1,
        )
    }

    /// `ForSmallElectricPole`.
    pub fn for_small_electric_pole() -> Self {
        Self::with_pole(entity_names::vanilla::SMALL_ELECTRIC_POLE, 5, 5, 7.5, 1, 1)
    }

    /// `ForMediumElectricPole`.
    pub fn for_medium_electric_pole() -> Self {
        Self::with_pole(entity_names::vanilla::MEDIUM_ELECTRIC_POLE, 7, 7, 9.0, 1, 1)
    }

    /// `ForBigElectricPole`.
    pub fn for_big_electric_pole() -> Self {
        Self::with_pole(entity_names::vanilla::BIG_ELECTRIC_POLE, 4, 4, 32.0, 2, 2)
    }

    /// `ForSubstation`.
    pub fn for_substation() -> Self {
        Self::with_pole(entity_names::vanilla::SUBSTATION, 18, 18, 18.0, 2, 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // OilFieldOptionsTest.AllPipeStrategies.
    #[test]
    fn all_pipe_strategies() {
        assert_eq!(ALL_PIPE_STRATEGIES, PipeStrategy::ALL);
    }

    // OilFieldOptionsTest.DefaultPipeStrategies: every value but FbeOriginal, in order.
    #[test]
    fn default_pipe_strategies() {
        let expected: Vec<_> = PipeStrategy::ALL
            .into_iter()
            .filter(|s| *s != PipeStrategy::FbeOriginal)
            .collect();
        assert_eq!(DEFAULT_PIPE_STRATEGIES.to_vec(), expected);
    }

    // OilFieldOptionsTest.AllBeaconStrategies.
    #[test]
    fn all_beacon_strategies() {
        assert_eq!(ALL_BEACON_STRATEGIES, BeaconStrategy::ALL);
    }

    // OilFieldOptionsTest.DefaultBeaconStrategies.
    #[test]
    fn default_beacon_strategies() {
        let expected: Vec<_> = BeaconStrategy::ALL
            .into_iter()
            .filter(|s| *s != BeaconStrategy::FbeOriginal)
            .collect();
        assert_eq!(DEFAULT_BEACON_STRATEGIES.to_vec(), expected);
    }

    // OilFieldOptionsTest.QualityDefaultsAreNormal.
    #[test]
    fn quality_defaults_are_normal() {
        let options = OilFieldOptions::default();
        assert_eq!(options.pumpjack_quality, Quality::NORMAL);
        assert_eq!(options.beacon_quality, Quality::NORMAL);
        assert_eq!(options.electric_pole_quality, Quality::NORMAL);
        assert_eq!(options.pumpjack_module_quality, Quality::NORMAL);
        assert_eq!(options.beacon_module_quality, Quality::NORMAL);
    }

    #[test]
    fn presets_change_only_the_pole() {
        let medium = OilFieldOptions::for_medium_electric_pole();
        assert_eq!(medium, OilFieldOptions::default());
        let big = OilFieldOptions::for_big_electric_pole();
        assert_eq!(big.electric_pole_width, 2);
        assert_eq!(big.electric_pole_wire_reach, 32.0);
        assert_eq!(big.beacon_supply_width, 9);
    }
}
