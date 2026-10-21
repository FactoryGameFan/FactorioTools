//! `SquareGrid`, `PipeGrid` and `ExistingPipeGrid`, from `OilField/Grid/SquareGrid.cs`,
//! `PipeGrid.cs` and `ExistingPipeGrid.cs`.
//!
//! Design: the grid owns its entities. Each cell holds `Option<GridEntity>`, and entities
//! refer to each other by id (a side to its center, a pole to its neighbors), resolved
//! through the grid's id-to-location map the way the C# does with `GetEntity<T>(id)`. That
//! avoids `Rc<RefCell<..>>` and keeps every change to an entity going through the grid. The
//! one place this differs from the C#'s shared references is a cloned grid; see
//! [`GridEntity::copy`].
//!
//! `SquareGrid` is abstract in the C#, and its two subclasses differ only in
//! `GetNeighbors`. `PipeGrid` is the one every `Context` holds, so [`SquareGrid`] here is a
//! `PipeGrid`. `ExistingPipeGrid` only ever answers neighbor queries from its pipe set
//! (everything else it shares with the grid it wraps), so it is [`ExistingPipeGrid`], a
//! view over that set. Code that works on either takes a [`GridNeighbors`].

use std::fmt::{self, Write};

use dotnet_compat::NetDictionary;

use crate::Location;
use crate::containers::LocationSet;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::entity::{EntityKind, GridEntity};

/// `SquareGrid.NeighborCost`.
pub const NEIGHBOR_COST: i32 = 1;

/// `SquareGrid.EmptyLabel`.
const EMPTY_LABEL: &str = ".";

/// The four neighbors of a cell, in the C#'s order (east, north, west, south), with
/// `Location.Invalid` for any that may not be entered.
pub trait GridNeighbors {
    fn get_neighbors(&self, location: Location) -> [Location; 4];
}

/// `PipeGrid`, the concrete `SquareGrid`.
#[derive(Debug)]
pub struct SquareGrid {
    width: i32,
    height: i32,
    middle: Location,
    entity_id_to_location: NetDictionary<i32, Location>,
    entity_locations: LocationSet,
    cells: Vec<Option<GridEntity>>,
    next_id: i32,
}

impl SquareGrid {
    /// `new PipeGrid(width, height)`.
    pub fn new(width: i32, height: i32) -> Self {
        SquareGrid {
            width,
            height,
            middle: Location::new(width / 2, height / 2),
            entity_id_to_location: NetDictionary::new(),
            entity_locations: LocationSet::new_hash(),
            cells: (0..width * height).map(|_| None).collect(),
            next_id: 1,
        }
    }

    /// `new PipeGrid(existing)`: `SquareGrid(existing, clone: true)`. The id map goes through
    /// `Dictionary`'s copy constructor, and the location set is rebuilt with the existing
    /// count as its capacity and filled in the existing order, as the C# does.
    pub fn clone_grid(&self) -> SquareGrid {
        let mut entity_locations = LocationSet::new_hash_with_capacity(self.entity_locations.len());
        entity_locations.union_with(&self.entity_locations);
        SquareGrid {
            width: self.width,
            height: self.height,
            middle: self.middle,
            entity_id_to_location: NetDictionary::copy_from(&self.entity_id_to_location),
            entity_locations,
            cells: self
                .cells
                .iter()
                .map(|cell| cell.as_ref().map(GridEntity::copy))
                .collect(),
            next_id: self.next_id,
        }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    /// `Middle`: `(Width / 2, Height / 2)`.
    pub fn middle(&self) -> Location {
        self.middle
    }

    /// `GetIndex`: `Y * Width + X`, with no bounds check, as in the C#. A location off the
    /// left or right edge lands on the neighboring row; one outside the array panics like
    /// the C#'s `IndexOutOfRangeException`.
    fn index(&self, location: Location) -> usize {
        let index = location.y * self.width + location.x;
        if index < 0 || index as usize >= self.cells.len() {
            panic!(
                "IndexOutOfRangeException: {location:?} in a {}x{} grid",
                self.width, self.height
            );
        }
        index as usize
    }

    /// The indexer: the entity at a location, if any.
    pub fn get(&self, location: Location) -> Option<&GridEntity> {
        self.cells[self.index(location)].as_ref()
    }

    /// The indexer, for changing an entity in place (a pumpjack's direction, say).
    pub fn get_mut(&mut self, location: Location) -> Option<&mut GridEntity> {
        let index = self.index(location);
        self.cells[index].as_mut()
    }

    /// `GetId()`: the next entity id.
    pub fn get_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// `EntityIdToLocation`, in .NET's order.
    pub fn entity_id_to_location(&self) -> &NetDictionary<i32, Location> {
        &self.entity_id_to_location
    }

    /// `EntityIdToLocation[id]`.
    pub fn location_of(&self, id: i32) -> Location {
        *self
            .entity_id_to_location
            .get(&id)
            .unwrap_or_else(|| panic!("KeyNotFoundException: entity {id}"))
    }

    /// `EntityLocations`, enumerating in .NET's order.
    pub fn entity_locations(&self) -> &LocationSet {
        &self.entity_locations
    }

    /// `IsEmpty`.
    pub fn is_empty(&self, location: Location) -> bool {
        self.get(location).is_none()
    }

    /// `GetEntity<T>(id)`. The caller checks the kind, where the C# casts.
    pub fn get_entity(&self, id: i32) -> &GridEntity {
        let location = self.location_of(id);
        self.get(location)
            .unwrap_or_else(|| panic!("NullReferenceException: entity {id} at {location:?}"))
    }

    /// `GetEntity<T>(id)`, for changing the entity in place.
    pub fn get_entity_mut(&mut self, id: i32) -> &mut GridEntity {
        let location = self.location_of(id);
        self.get_mut(location)
            .unwrap_or_else(|| panic!("NullReferenceException: entity {id} at {location:?}"))
    }

    /// `AddEntity`.
    pub fn add_entity(&mut self, location: Location, entity: GridEntity) -> FtResult<()> {
        let index = self.index(location);
        if self.cells[index].is_some() {
            return Err(FactorioToolsError::new(format!(
                "There is already an entity at {location}."
            )));
        }
        let id = entity.id;
        self.cells[index] = Some(entity);
        self.entity_locations.add(location);
        self.entity_id_to_location.add(id, location);
        Ok(())
    }

    /// `RemoveEntity`, which also runs the entity's `Unlink`: a pole drops itself from its
    /// neighbors' sets. Returns the removed entity.
    pub fn remove_entity(&mut self, location: Location) -> Option<GridEntity> {
        let index = self.index(location);
        let mut entity = self.cells[index].take()?;
        self.entity_locations.remove(location);
        self.entity_id_to_location.remove(&entity.id);
        if let EntityKind::ElectricPoleCenter { neighbors } = &mut entity.kind {
            // `ElectricPoleCenter.ClearNeighbors(grid)`.
            for neighbor_id in neighbors.iter() {
                self.neighbors_mut(neighbor_id).remove(&entity.id);
            }
            neighbors.clear();
        }
        Some(entity)
    }

    /// `IsEntityType<T>`: whether the entity at the location matches.
    pub fn is_entity_type(
        &self,
        location: Location,
        matches: impl Fn(&EntityKind) -> bool,
    ) -> bool {
        self.get(location)
            .is_some_and(|entity| matches(&entity.kind))
    }

    /// `IsInBounds`.
    pub fn is_in_bounds(&self, location: Location) -> bool {
        location.x >= 0 && location.x < self.width && location.y >= 0 && location.y < self.height
    }

    /// `GetAdjacent`: the four in-bounds neighbors, empty or not.
    pub fn get_adjacent(&self, location: Location) -> [Location; 4] {
        [
            location.translate(1, 0),
            location.translate(0, -1),
            location.translate(-1, 0),
            location.translate(0, 1),
        ]
        .map(|l| {
            if self.is_in_bounds(l) {
                l
            } else {
                Location::INVALID
            }
        })
    }

    /// `ElectricPoleCenter.AddNeighbor`: links two poles both ways.
    pub fn add_neighbor(&mut self, pole_id: i32, neighbor_id: i32) {
        self.neighbors_mut(pole_id).add(neighbor_id);
        self.neighbors_mut(neighbor_id).add(pole_id);
    }

    /// `ElectricPoleCenter.Neighbors`, in .NET's order.
    pub fn neighbors(&self, pole_id: i32) -> &dotnet_compat::NetHashSet<i32> {
        match &self.get_entity(pole_id).kind {
            EntityKind::ElectricPoleCenter { neighbors } => neighbors,
            other => panic!("InvalidCastException: entity {pole_id} is {other:?}"),
        }
    }

    /// `ElectricPoleCenter.ClearNeighbors(grid)`.
    pub fn clear_neighbors(&mut self, pole_id: i32) {
        let neighbor_ids: Vec<i32> = self.neighbors(pole_id).iter().collect();
        for neighbor_id in neighbor_ids {
            self.neighbors_mut(neighbor_id).remove(&pole_id);
        }
        self.neighbors_mut(pole_id).clear();
    }

    fn neighbors_mut(&mut self, pole_id: i32) -> &mut dotnet_compat::NetHashSet<i32> {
        match &mut self.get_entity_mut(pole_id).kind {
            EntityKind::ElectricPoleCenter { neighbors } => neighbors,
            other => panic!("InvalidCastException: entity {pole_id} is {other:?}"),
        }
    }

    /// `ToString(builder, spacing)`: the bounding box of the entities, one padded label per
    /// cell, each row ending with a newline.
    ///
    /// The C# uses its own `Max`/`Min` extensions, which return 0 for an empty grid, so an
    /// empty grid draws the single cell (0, 0). That is copied too.
    pub fn to_string_with_spacing(&self, spacing: usize) -> String {
        let items = self.entity_locations.to_vec();
        let width = items
            .iter()
            .map(|&l| self.get(l).map_or(0, |e| e.label().len()))
            .max()
            .unwrap_or(0)
            + spacing;
        let min_x = items.iter().map(|l| l.x).min().unwrap_or(0);
        let min_y = items.iter().map(|l| l.y).min().unwrap_or(0);
        let max_x = items.iter().map(|l| l.x).max().unwrap_or(0);
        let max_y = items.iter().map(|l| l.y).max().unwrap_or(0);

        let mut out = String::new();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let label = self.cells[(y * self.width + x) as usize]
                    .as_ref()
                    .map_or(EMPTY_LABEL, GridEntity::label);
                // `PadRight` never shortens, and neither does this.
                let _ = write!(out, "{label:<width$}");
            }
            out.push('\n');
        }
        out
    }
}

/// `PipeGrid.GetNeighbors`: in bounds and empty.
impl GridNeighbors for SquareGrid {
    fn get_neighbors(&self, location: Location) -> [Location; 4] {
        [
            location.translate(1, 0),
            location.translate(0, -1),
            location.translate(-1, 0),
            location.translate(0, 1),
        ]
        .map(|l| {
            if self.is_in_bounds(l) && self.is_empty(l) {
                l
            } else {
                Location::INVALID
            }
        })
    }
}

/// `ToString()`: spacing 1.
impl fmt::Display for SquareGrid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_with_spacing(1))
    }
}

/// `ExistingPipeGrid`: a cell's neighbors are the adjacent cells in its pipe set.
///
/// The C# `ExistingPipeGrid(grid, pipes)` shares the wrapped grid's arrays rather than
/// copying them, and its `Pipes` reference is the caller's live set. Holding only the pipe
/// set keeps that: callers use the wrapped grid directly for everything else, and
/// `new PipeGrid(existingPipeGrid)` is `grid.clone_grid()` on the wrapped grid.
pub struct ExistingPipeGrid<'a> {
    pub pipes: &'a LocationSet,
}

impl<'a> ExistingPipeGrid<'a> {
    pub fn new(pipes: &'a LocationSet) -> Self {
        ExistingPipeGrid { pipes }
    }
}

/// `ExistingPipeGrid.GetNeighbors`. There is no bounds check: a pipe set that is a
/// `LocationBitSet` answers for out-of-grid locations by its own index arithmetic.
impl GridNeighbors for ExistingPipeGrid<'_> {
    fn get_neighbors(&self, location: Location) -> [Location; 4] {
        [
            location.translate(1, 0),
            location.translate(0, -1),
            location.translate(-1, 0),
            location.translate(0, 1),
        ]
        .map(|l| {
            if self.pipes.contains(l) {
                l
            } else {
                Location::INVALID
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Direction;

    fn loc(x: i32, y: i32) -> Location {
        Location::new(x, y)
    }

    #[test]
    fn draws_like_the_csharp() {
        let mut grid = SquareGrid::new(6, 5);
        let id = grid.get_id();
        grid.add_entity(loc(1, 1), GridEntity::pumpjack_center(id))
            .unwrap();
        let side = grid.get_id();
        grid.add_entity(
            loc(2, 1),
            GridEntity::new(side, EntityKind::PumpjackSide { center: id }),
        )
        .unwrap();
        let ug = grid.get_id();
        grid.add_entity(
            loc(3, 3),
            GridEntity::new(
                ug,
                EntityKind::UndergroundPipe {
                    direction: Direction::Left,
                },
            ),
        )
        .unwrap();
        assert_eq!(grid.to_string(), "J j . \n. . . \n. . < \n");
        assert_eq!(grid.middle(), loc(3, 2));
    }

    #[test]
    fn empty_grid_draws_cell_zero() {
        // Max over no labels is 0, plus spacing 1: "." padded to 1 is just ".".
        assert_eq!(SquareGrid::new(3, 3).to_string(), ".\n");
    }

    #[test]
    fn rejects_a_second_entity() {
        let mut grid = SquareGrid::new(3, 3);
        let a = grid.get_id();
        grid.add_entity(loc(1, 1), GridEntity::new(a, EntityKind::Pipe))
            .unwrap();
        let b = grid.get_id();
        let err = grid
            .add_entity(loc(1, 1), GridEntity::new(b, EntityKind::Pipe))
            .unwrap_err();
        assert_eq!(
            err.message,
            "There is already an entity at (1, 1) / (Ln 2, Col 2)."
        );
    }

    #[test]
    fn removing_a_pole_unlinks_it() {
        let mut grid = SquareGrid::new(5, 5);
        let ids: Vec<i32> = (0..3).map(|_| grid.get_id()).collect();
        for (i, &id) in ids.iter().enumerate() {
            grid.add_entity(loc(i as i32, 0), GridEntity::electric_pole_center(id))
                .unwrap();
        }
        grid.add_neighbor(ids[0], ids[1]);
        grid.add_neighbor(ids[0], ids[2]);
        grid.add_neighbor(ids[1], ids[2]);
        grid.remove_entity(loc(0, 0));
        assert_eq!(grid.neighbors(ids[1]).to_vec(), vec![ids[2]]);
        assert_eq!(grid.neighbors(ids[2]).to_vec(), vec![ids[1]]);
        assert_eq!(grid.entity_locations().to_vec(), vec![loc(1, 0), loc(2, 0)]);
    }

    #[test]
    fn neighbors_and_clone() {
        let mut grid = SquareGrid::new(3, 3);
        let id = grid.get_id();
        grid.add_entity(loc(2, 1), GridEntity::new(id, EntityKind::Pipe))
            .unwrap();
        assert_eq!(
            grid.get_neighbors(loc(1, 1)),
            [Location::INVALID, loc(1, 0), loc(0, 1), loc(1, 2)]
        );
        assert_eq!(
            grid.get_adjacent(loc(0, 0)),
            [loc(1, 0), Location::INVALID, Location::INVALID, loc(0, 1)]
        );
        let mut clone = grid.clone_grid();
        assert_eq!(clone.get_id(), grid.get_id());
        clone.remove_entity(loc(2, 1));
        assert!(!grid.is_empty(loc(2, 1)));
        assert!(grid.is_entity_type(loc(2, 1), EntityKind::is_pipe));

        let mut pipes = LocationSet::new_hash();
        pipes.add(loc(1, 0));
        assert_eq!(
            ExistingPipeGrid::new(&pipes).get_neighbors(loc(1, 1)),
            [
                Location::INVALID,
                loc(1, 0),
                Location::INVALID,
                Location::INVALID
            ]
        );
    }
}
