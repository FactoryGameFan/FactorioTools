//! Grid entities, from `OilField/Grid`: `GridEntity.cs` and its subclasses
//! (`PumpjackCenter`, `PumpjackSide`, `BeaconCenter`, `BeaconSide`, `ElectricPoleCenter`,
//! `ElectricPoleSide`, `Pipe`, `Terminal`, `UndergroundPipe`, `HeatPipe`,
//! `TemporaryEntity`, `AvoidEntity`).
//!
//! The C# class hierarchy becomes one enum. The one inheritance the planner tests for is
//! `Terminal` and `UndergroundPipe` both being a `Pipe` (`is Pipe`, `IsEntityType<Pipe>`),
//! which [`EntityKind::is_pipe`] keeps.
//!
//! A side entity holds a reference to its center in the C#. Here it holds the center's id,
//! which `SquareGrid::get_entity` resolves while the center is on the grid. That is the
//! only way the C# reaches a center through a side, too (`grid.EntityIdToLocation[side.Center.Id]`).

use dotnet_compat::NetHashSet;

use crate::data::Direction;

/// One entity on the grid: its id and what it is.
#[derive(Debug)]
pub struct GridEntity {
    /// `GridEntity.Id`, from `SquareGrid.GetId()`.
    pub id: i32,
    pub kind: EntityKind,
}

/// The concrete C# entity class, with that class's own state.
#[derive(Debug)]
pub enum EntityKind {
    /// `PumpjackCenter`, whose `Direction` the planner sets at the end.
    PumpjackCenter {
        direction: Direction,
    },
    /// `PumpjackSide`, with its center's id.
    PumpjackSide {
        center: i32,
    },
    BeaconCenter,
    /// `BeaconSide`, with its center's id.
    BeaconSide {
        center: i32,
    },
    /// `ElectricPoleCenter`. `Neighbors` is a `HashSet<int>` of pole ids, and the planner
    /// walks it in .NET's order.
    ElectricPoleCenter {
        neighbors: NetHashSet<i32>,
    },
    /// `ElectricPoleSide`, with its center's id.
    ElectricPoleSide {
        center: i32,
    },
    Pipe,
    /// `Terminal`, a `Pipe` subclass.
    Terminal,
    /// `UndergroundPipe`, a `Pipe` subclass.
    UndergroundPipe {
        direction: Direction,
    },
    HeatPipe,
    TemporaryEntity,
    AvoidEntity,
}

impl GridEntity {
    pub fn new(id: i32, kind: EntityKind) -> Self {
        GridEntity { id, kind }
    }

    /// `new PumpjackCenter(id)`: the direction starts as `default(Direction)`, which is `Up`.
    pub fn pumpjack_center(id: i32) -> Self {
        Self::new(
            id,
            EntityKind::PumpjackCenter {
                direction: Direction::Up,
            },
        )
    }

    /// `new ElectricPoleCenter(id)`, with no neighbors.
    pub fn electric_pole_center(id: i32) -> Self {
        Self::new(
            id,
            EntityKind::ElectricPoleCenter {
                neighbors: NetHashSet::new(),
            },
        )
    }

    /// `Label`, which `SquareGrid.ToString` draws.
    pub fn label(&self) -> &'static str {
        self.kind.label()
    }

    /// A copy for a cloned grid.
    ///
    /// The C# clones a grid by copying its array of entity references, so the clone and the
    /// original share the same entity objects, and a change to a pole's neighbors or a
    /// pumpjack's direction through one shows in the other. The port copies the entities
    /// instead. That gives the same result as long as nothing changes an entity through a
    /// clone and then reads it through the original. Pole neighbor sets are copied with
    /// `HashSet<int>`'s copy constructor, which can differ from the shared original in
    /// where later adds land if the set has removed entries.
    pub fn copy(&self) -> GridEntity {
        let kind = match &self.kind {
            EntityKind::PumpjackCenter { direction } => EntityKind::PumpjackCenter {
                direction: *direction,
            },
            EntityKind::PumpjackSide { center } => EntityKind::PumpjackSide { center: *center },
            EntityKind::BeaconCenter => EntityKind::BeaconCenter,
            EntityKind::BeaconSide { center } => EntityKind::BeaconSide { center: *center },
            EntityKind::ElectricPoleCenter { neighbors } => EntityKind::ElectricPoleCenter {
                neighbors: NetHashSet::copy_from(neighbors),
            },
            EntityKind::ElectricPoleSide { center } => {
                EntityKind::ElectricPoleSide { center: *center }
            }
            EntityKind::Pipe => EntityKind::Pipe,
            EntityKind::Terminal => EntityKind::Terminal,
            EntityKind::UndergroundPipe { direction } => EntityKind::UndergroundPipe {
                direction: *direction,
            },
            EntityKind::HeatPipe => EntityKind::HeatPipe,
            EntityKind::TemporaryEntity => EntityKind::TemporaryEntity,
            EntityKind::AvoidEntity => EntityKind::AvoidEntity,
        };
        GridEntity { id: self.id, kind }
    }
}

impl EntityKind {
    /// `Label`.
    pub fn label(&self) -> &'static str {
        match self {
            EntityKind::PumpjackCenter { .. } => "J",
            EntityKind::PumpjackSide { .. } => "j",
            EntityKind::BeaconCenter => "B",
            EntityKind::BeaconSide { .. } => "b",
            EntityKind::ElectricPoleCenter { .. } => "E",
            EntityKind::ElectricPoleSide { .. } => "e",
            EntityKind::Pipe => "o",
            EntityKind::Terminal => "+",
            EntityKind::UndergroundPipe { direction } => match direction {
                Direction::Up => "^",
                Direction::Right => ">",
                Direction::Down => "v",
                Direction::Left => "<",
            },
            EntityKind::HeatPipe => "h",
            EntityKind::TemporaryEntity => "?",
            EntityKind::AvoidEntity => "X",
        }
    }

    /// `is Pipe`: a plain pipe, a terminal or an underground pipe.
    pub fn is_pipe(&self) -> bool {
        matches!(
            self,
            EntityKind::Pipe | EntityKind::Terminal | EntityKind::UndergroundPipe { .. }
        )
    }

    /// The center id of a side entity (`PumpjackSide`, `BeaconSide`, `ElectricPoleSide`).
    pub fn center(&self) -> Option<i32> {
        match self {
            EntityKind::PumpjackSide { center }
            | EntityKind::BeaconSide { center }
            | EntityKind::ElectricPoleSide { center } => Some(*center),
            _ => None,
        }
    }
}
