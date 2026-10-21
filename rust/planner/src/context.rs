//! `Context`, from `src/FactorioTools/OilField/Models/Context.cs`: the state one plan carries
//! from step to step.
//!
//! `SharedInstances` is left out (see `containers/mod.rs`). The set and dictionary factories
//! copy the default C# build (`USE_HASHSETS`): an enumerable set is a `LocationHashSet`, a
//! non-enumerable one a `LocationBitSet`, and a dictionary a `LocationHashDictionary`.

use crate::Location;
use crate::containers::{LocationDictionary, LocationSet};
use crate::data::{Blueprint, Direction};
use crate::grid::{SquareGrid, TerminalLocation};
use crate::options::OilFieldOptions;

#[derive(Debug)]
pub struct Context {
    pub options: OilFieldOptions,
    pub input_blueprint: Blueprint,
    /// Added to a blueprint X position to get its grid X. `float`, as in the C#.
    pub delta_x: f32,
    /// Added to a blueprint Y position to get its grid Y.
    pub delta_y: f32,
    pub grid: SquareGrid,
    /// Pumpjack centers, sorted by Y then X.
    pub centers: Vec<Location>,
    pub center_to_terminals: LocationDictionary<Vec<TerminalLocation>>,
    pub center_to_original_direction: LocationDictionary<Direction>,
    pub location_to_terminals: LocationDictionary<Vec<TerminalLocation>>,
    /// For each cell (`y * width + x`), how many pumpjack side cells border it.
    pub location_to_adjacent_count: Vec<i32>,
    /// The pole supply width after quality: base + 2 * quality level.
    pub electric_pole_supply_width_with_quality: i32,
    /// The pole supply height after quality.
    pub electric_pole_supply_height_with_quality: i32,
    /// The pole wire reach after quality.
    pub electric_pole_wire_reach_with_quality: f64,
    /// The square of `electric_pole_wire_reach_with_quality`.
    pub electric_pole_wire_reach_squared_with_quality: f64,
    /// The heat pipe network, once `AddHeatPipes` has routed it.
    pub heat_pipes: Option<LocationSet>,
    /// Pumpjacks dropped so the rest of the field can be fully heated.
    pub heat_dropped_pumpjacks: i32,
}

impl Context {
    /// `GetLocationDictionary<T>()`.
    pub fn get_location_dictionary<T>(&self) -> LocationDictionary<T> {
        LocationDictionary::new()
    }

    /// `GetLocationDictionary<T>(capacity)`.
    pub fn get_location_dictionary_with_capacity<T>(
        &self,
        capacity: usize,
    ) -> LocationDictionary<T> {
        LocationDictionary::with_capacity(capacity)
    }

    /// `GetLocationSet(ILocationSet other)`: a copy with the same implementation.
    pub fn get_location_set_copy(&self, other: &LocationSet) -> LocationSet {
        other.copy()
    }

    /// `GetLocationSet()`: a non-enumerable bit set.
    pub fn get_location_set(&self) -> LocationSet {
        self.get_location_set_enumerable(false)
    }

    /// `GetLocationSet(allowEnumerate)`.
    pub fn get_location_set_enumerable(&self, allow_enumerate: bool) -> LocationSet {
        if allow_enumerate {
            LocationSet::new_hash()
        } else {
            LocationSet::new_bit(self.grid.width(), self.grid.height())
        }
    }

    /// `GetLocationSet(location)` and `GetLocationSet(location, allowEnumerate)`.
    pub fn get_location_set_with(&self, location: Location, allow_enumerate: bool) -> LocationSet {
        let mut set = self.get_location_set_enumerable(allow_enumerate);
        set.add(location);
        set
    }

    /// `GetLocationSet(capacity, allowEnumerate)`. A bit set ignores the capacity.
    pub fn get_location_set_with_capacity(
        &self,
        capacity: usize,
        allow_enumerate: bool,
    ) -> LocationSet {
        if allow_enumerate {
            LocationSet::new_hash_with_capacity(capacity)
        } else {
            LocationSet::new_bit(self.grid.width(), self.grid.height())
        }
    }

    /// `GetLocationSet(location, capacity, allowEnumerate)`.
    pub fn get_location_set_with_capacity_and(
        &self,
        location: Location,
        capacity: usize,
        allow_enumerate: bool,
    ) -> LocationSet {
        let mut set = self.get_location_set_with_capacity(capacity, allow_enumerate);
        set.add(location);
        set
    }

    /// `GetLocationSet(IReadOnlyCollection<Location> locations, allowEnumerate)`.
    pub fn get_location_set_from(
        &self,
        locations: impl IntoIterator<Item = Location>,
        allow_enumerate: bool,
    ) -> LocationSet {
        let mut set = self.get_location_set_enumerable(allow_enumerate);
        for location in locations {
            set.add(location);
        }
        set
    }

    /// `GetReadOnlyLocationSet(locations, allowEnumerate)`: empty and single-item inputs get
    /// the light `EmptyLocationSet` and `SingleLocationSet`, anything bigger a normal set.
    pub fn get_read_only_location_set(
        &self,
        locations: impl IntoIterator<Item = Location>,
        allow_enumerate: bool,
    ) -> LocationSet {
        let mut first = Location::INVALID;
        let mut count = 0usize;
        let mut set: Option<LocationSet> = None;
        for location in locations {
            match count {
                0 => first = location,
                1 => {
                    let mut new_set = self.get_location_set_enumerable(allow_enumerate);
                    new_set.add(first);
                    new_set.add(location);
                    set = Some(new_set);
                }
                _ => {
                    set.as_mut()
                        .expect("set exists after two items")
                        .add(location);
                }
            }
            count += 1;
        }

        match set {
            Some(set) => set,
            None if count == 0 => LocationSet::Empty,
            None => LocationSet::new_single(first),
        }
    }

    /// `GetSingleLocationSet(location)`.
    pub fn get_single_location_set(&self, location: Location) -> LocationSet {
        LocationSet::new_single(location)
    }
}
