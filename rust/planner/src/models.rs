//! The small model types from `src/FactorioTools/OilField/Models` and
//! `OilField/ICandidateFactory.cs`. `Context` has its own module (`context.rs`), and
//! `NoPathBetweenTerminalsException` lives in `error.rs`. `SharedInstances` is not ported
//! (see `containers/mod.rs`).

use std::fmt;

use crate::Location;
use crate::containers::CountedBitArray;

/// `PipeStrategy` (`Models/PipeStrategy.cs`). The discriminants are the C# enum values,
/// which the planner sorts plans by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PipeStrategy {
    /// The original Factorio Blueprint Editor (FBE) pipe planning strategy.
    FbeOriginal = 0,
    /// FBE's pipe planning strategy, with minor modifications.
    Fbe = 1,
    /// Connected centers, using a Delaunay triangulation to pick pumpjacks to connect.
    ConnectedCentersDelaunay = 2,
    /// Connected centers, using a Delaunay triangulation then Prim's minimum spanning tree.
    ConnectedCentersDelaunayMst = 3,
    /// Connected centers, using Dr. Chris C. N. Chu's FLUTE algorithm.
    ConnectedCentersFlute = 4,
}

impl PipeStrategy {
    /// `Enum.GetValues<PipeStrategy>()`, in declaration order.
    pub const ALL: [PipeStrategy; 5] = [
        PipeStrategy::FbeOriginal,
        PipeStrategy::Fbe,
        PipeStrategy::ConnectedCentersDelaunay,
        PipeStrategy::ConnectedCentersDelaunayMst,
        PipeStrategy::ConnectedCentersFlute,
    ];

    /// The C# enum's integer value.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// The C# member name (`Enum.ToString()`).
    pub const fn name(self) -> &'static str {
        match self {
            PipeStrategy::FbeOriginal => "FbeOriginal",
            PipeStrategy::Fbe => "Fbe",
            PipeStrategy::ConnectedCentersDelaunay => "ConnectedCentersDelaunay",
            PipeStrategy::ConnectedCentersDelaunayMst => "ConnectedCentersDelaunayMst",
            PipeStrategy::ConnectedCentersFlute => "ConnectedCentersFlute",
        }
    }
}

/// `BeaconStrategy` (`Models/BeaconStrategy.cs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BeaconStrategy {
    /// The original FBE beacon planning strategy.
    FbeOriginal = 0,
    /// A modified FBE beacon planning strategy.
    Fbe = 1,
    /// Places beacons as close ("snug") to each other as possible.
    Snug = 2,
}

impl BeaconStrategy {
    /// `Enum.GetValues<BeaconStrategy>()`, in declaration order.
    pub const ALL: [BeaconStrategy; 3] = [
        BeaconStrategy::FbeOriginal,
        BeaconStrategy::Fbe,
        BeaconStrategy::Snug,
    ];

    /// The C# enum's integer value.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// The C# member name (`Enum.ToString()`).
    pub const fn name(self) -> &'static str {
        match self {
            BeaconStrategy::FbeOriginal => "FbeOriginal",
            BeaconStrategy::Fbe => "Fbe",
            BeaconStrategy::Snug => "Snug",
        }
    }
}

/// `AvoidLocation` (`Models/AvoidLocation.cs`): a blueprint position, in `float`, that the
/// planner must leave empty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvoidLocation {
    pub x: f32,
    pub y: f32,
}

impl AvoidLocation {
    pub const fn new(x: f32, y: f32) -> Self {
        AvoidLocation { x, y }
    }
}

/// `BeaconSolution` (`Models/BeaconSolution.cs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeaconSolution {
    pub strategy: BeaconStrategy,
    pub beacons: Vec<Location>,
    pub effects: i32,
    pub effects_given_counts: Vec<i32>,
}

/// `Endpoints` (`Models/Endpoints.cs`): the two ends of a line between pumpjacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoints {
    pub a: Location,
    pub b: Location,
}

impl Endpoints {
    pub const fn new(a: Location, b: Location) -> Self {
        Endpoints { a, b }
    }
}

/// `ProviderRecipient` (`Models/ProviderRecipient.cs`): an entity, such as a pumpjack, that
/// receives the effect of a provider such as an electric pole or a beacon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderRecipient {
    pub center: Location,
    pub width: i32,
    pub height: i32,
}

impl ProviderRecipient {
    pub const fn new(center: Location, width: i32, height: i32) -> Self {
        ProviderRecipient {
            center,
            width,
            height,
        }
    }
}

/// `CandidateInfo` (`Models/CandidateInfo.cs`): what a provider candidate (a beacon or pole
/// spot) would cover. Both fields are public and mutable in the C#.
#[derive(Clone, Debug)]
pub struct CandidateInfo {
    pub covered: CountedBitArray,
    pub entity_distance: f64,
}

impl CandidateInfo {
    pub fn new(covered: CountedBitArray) -> Self {
        CandidateInfo {
            covered,
            entity_distance: 0.0,
        }
    }
}

/// `ElectricPoleCandidateInfo` (`Models/ElectricPoleCandidateInfo.cs`), a `CandidateInfo`
/// subclass in the C#. The base fields live in `base`.
#[derive(Clone, Debug)]
pub struct ElectricPoleCandidateInfo {
    pub base: CandidateInfo,
    pub priority_powered: i32,
    pub others_connected: i32,
    pub pole_distance: i32,
    pub middle_distance: i32,
}

impl ElectricPoleCandidateInfo {
    pub fn new(covered: CountedBitArray) -> Self {
        ElectricPoleCandidateInfo {
            base: CandidateInfo::new(covered),
            priority_powered: 0,
            others_connected: 0,
            pole_distance: 0,
            middle_distance: 0,
        }
    }
}

/// The C# constraint `where TInfo : CandidateInfo`: access to the base fields of either
/// candidate info type.
pub trait AsCandidateInfo {
    fn info(&self) -> &CandidateInfo;
    fn info_mut(&mut self) -> &mut CandidateInfo;
}

impl AsCandidateInfo for CandidateInfo {
    fn info(&self) -> &CandidateInfo {
        self
    }

    fn info_mut(&mut self) -> &mut CandidateInfo {
        self
    }
}

impl AsCandidateInfo for ElectricPoleCandidateInfo {
    fn info(&self) -> &CandidateInfo {
        &self.base
    }

    fn info_mut(&mut self) -> &mut CandidateInfo {
        &mut self.base
    }
}

/// `ICandidateFactory<TInfo>` (`OilField/ICandidateFactory.cs`).
pub trait CandidateFactory<TInfo: AsCandidateInfo> {
    fn create(&self, covered: CountedBitArray) -> TInfo;
}

/// `OilFieldPlan` (`Models/OilFieldPlan.cs`): one attempted plan and its counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OilFieldPlan {
    /// The pipe strategy that made the plan.
    pub pipe_strategy: PipeStrategy,
    /// Whether the pipe optimizer ran.
    pub optimize_pipes: bool,
    /// The beacon strategy, if beacons were planned.
    pub beacon_strategy: Option<BeaconStrategy>,
    /// Beacon effects given to pumpjacks. Higher is better.
    pub beacon_effect_count: i32,
    /// Beacons placed. For equal effects, lower is better.
    pub beacon_count: i32,
    /// Pipes placed, counting only the ends of underground stretches. For equal effects and
    /// beacons, lower is better.
    pub pipe_count: i32,
    /// Pipes before beacons or underground pipes were placed.
    pub pipe_count_without_underground: i32,
}

impl OilFieldPlan {
    /// `IsEquivalent`: the same effects, beacons and pipes.
    pub fn is_equivalent(&self, other: &OilFieldPlan) -> bool {
        self.beacon_effect_count == other.beacon_effect_count
            && self.beacon_count == other.beacon_count
            && self.pipe_count == other.pipe_count
    }

    /// `ToString(includeCounts)`, the plan line in every Verify planner snapshot.
    pub fn to_string_with_counts(&self, include_counts: bool) -> String {
        let mut output = String::from(match self.pipe_strategy {
            PipeStrategy::FbeOriginal => "FBE",
            PipeStrategy::Fbe => "FBE*",
            PipeStrategy::ConnectedCentersDelaunay => "CC-DT",
            PipeStrategy::ConnectedCentersDelaunayMst => "CC-DT-MST",
            PipeStrategy::ConnectedCentersFlute => "CC-FLUTE",
        });

        if self.optimize_pipes {
            output.push_str(" -> optimize");
        }

        if let Some(beacon_strategy) = self.beacon_strategy {
            output.push_str(match beacon_strategy {
                BeaconStrategy::FbeOriginal => " -> FBE",
                BeaconStrategy::Fbe => " -> FBE*",
                BeaconStrategy::Snug => " -> snug",
            });
        }

        if include_counts {
            if self.beacon_strategy.is_some() {
                output.push_str(&format!(
                    " (effects: {}, beacons: {}, pipes: {})",
                    self.beacon_effect_count, self.beacon_count, self.pipe_count
                ));
            } else {
                output.push_str(&format!(" (pipes: {})", self.pipe_count));
            }
        }

        output
    }
}

/// `ToString()`: `ToString(includeCounts: false)`.
impl fmt::Display for OilFieldPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_with_counts(false))
    }
}

/// `OilFieldPlanSummary` (`Models/OilFieldPlanSummary.cs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OilFieldPlanSummary {
    /// Pumpjacks removed to make room for electric poles. Must be zero.
    pub missing_pumpjacks: i32,
    /// Pumpjacks rotated from their original direction.
    pub rotated_pumpjacks: i32,
    /// Pumpjacks dropped so the rest could be fully heated on Aquilo.
    pub heat_dropped_pumpjacks: i32,
    /// Pumpjacks still without an adjacent heat pipe in the output.
    pub unheated_pumpjacks: i32,
    /// Pipe tiles still without an adjacent heat pipe in the output.
    pub unheated_pipes: i32,
    /// The best plans, which are exactly the same.
    pub selected_plans: Vec<OilFieldPlan>,
    /// Plans that rank the same as the selected ones but are not exactly the same.
    pub alternate_plans: Vec<OilFieldPlan>,
    /// The plans that were not the best.
    pub unused_plans: Vec<OilFieldPlan>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(beacon_strategy: Option<BeaconStrategy>) -> OilFieldPlan {
        OilFieldPlan {
            pipe_strategy: PipeStrategy::Fbe,
            optimize_pipes: true,
            beacon_strategy,
            beacon_effect_count: 47,
            beacon_count: 43,
            pipe_count: 13,
            pipe_count_without_underground: 20,
        }
    }

    #[test]
    fn plan_lines_match_the_snapshot_format() {
        assert_eq!(
            plan(Some(BeaconStrategy::Fbe)).to_string_with_counts(true),
            "FBE* -> optimize -> FBE* (effects: 47, beacons: 43, pipes: 13)"
        );
        assert_eq!(
            plan(None).to_string_with_counts(true),
            "FBE* -> optimize (pipes: 13)"
        );
        assert_eq!(
            plan(Some(BeaconStrategy::Snug)).to_string(),
            "FBE* -> optimize -> snug"
        );
    }

    #[test]
    fn strategy_values_are_the_csharp_enum_values() {
        assert_eq!(PipeStrategy::ConnectedCentersFlute.value(), 4);
        assert_eq!(BeaconStrategy::Snug.value(), 2);
    }
}
