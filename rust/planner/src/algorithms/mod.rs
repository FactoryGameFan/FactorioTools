//! Graph and line algorithms, from `src/FactorioTools/OilField/Algorithms`.
//!
//! The C# versions take a `Context`, but only for scratch buffers (`SharedInstances`, which
//! are cleared after each call) and for `GetLocationSet`/`GetLocationDictionary`. The ports
//! take what they need instead: the grid to walk, and the grid size where the C# builds a
//! `LocationBitSet`.
//!
//! Scratch lookups that the C# only ever queries (never enumerates), such as A*'s
//! `costSoFar` and Dijkstra's `inQueue`, use [`LocationMap`] and [`LocationLookup`]: plain
//! hash containers, since their order cannot be seen. Anything whose order the result
//! depends on uses the .NET-ordered containers.

mod a_star;
mod breadth_first_finder;
mod bresenhams_line;
mod dijkstras;
mod prims;

pub use a_star::{AStarResult, get_shortest_path as a_star, get_shortest_path_into as a_star_into};
pub use breadth_first_finder::get_shortest_path as breadth_first_shortest_path;
pub use bresenhams_line::get_path as bresenhams_line;
pub use dijkstras::{DijkstrasResult, PreviousSet, get_shortest_paths as dijkstras};
pub use prims::get_minimum_spanning_tree;

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

use crate::Location;

/// A fast hasher for `Location` keys in lookup-only scratch containers. It packs the two
/// coordinates into one word and mixes it with a multiply, which is enough for grid cells.
#[derive(Default)]
pub struct LocationHasher(u64);

impl Hasher for LocationHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(8) ^ u64::from(b)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }

    fn write_i32(&mut self, value: i32) {
        self.0 =
            (self.0.rotate_left(32) ^ u64::from(value as u32)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

/// A lookup-only map keyed by location. Never enumerate it where order matters.
pub type LocationMap<V> = HashMap<Location, V, BuildHasherDefault<LocationHasher>>;

/// A lookup-only set of locations. Never enumerate it where order matters.
pub type LocationLookup = HashSet<Location, BuildHasherDefault<LocationHasher>>;
