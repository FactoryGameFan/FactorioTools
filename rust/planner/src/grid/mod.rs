//! The grid and its entities, from `src/FactorioTools/OilField/Grid`.

mod entity;
mod square_grid;
mod terminal_location;

pub use entity::{EntityKind, GridEntity};
pub use square_grid::{ExistingPipeGrid, GridNeighbors, NEIGHBOR_COST, SquareGrid};
pub use terminal_location::TerminalLocation;
