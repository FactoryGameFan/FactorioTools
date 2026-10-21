//! FLUTE, a rectilinear Steiner minimal tree algorithm, as the oil-field planner uses it.
//!
//! **FLUTE is by Dr. Chris C. N. Chu, Iowa State University,
//! <http://home.engineering.iastate.edu/~cnchu/>.** It is used under his Attribution
//! Assurance License (`LICENSE-FLUTE`), which asks for that attribution to be shown
//! prominently each time a program built from it launches. The planner's site shows it in
//! its footer. Anyone planning commercial use must tell the author first.
//!
//! This crate is a Rust port of [FluteSharp](https://github.com/joelverhagen/FluteSharp) by
//! Joel Verhagen (MIT, `LICENSE-FluteSharp`), which ports FLUTE 3.1's C. FluteSharp's lookup
//! table reader comes from Stefan Mücke's Java port, `Flute.java`.
//!
//! The port matches FluteSharp exactly, tree for tree, on the inputs recorded in
//! `fixtures/` (see `FIXTURES.md`). Like the planner, it supports the degree 6 lookup
//! table that FluteSharp's source generator embeds, and the default accuracy of 3.
//!
//! ```
//! use flute::{Flute, LookUpTable, Point};
//!
//! let flute = Flute::new(LookUpTable::degree6());
//! let tree = flute.execute(&[Point::new(0, 0), Point::new(4, 2), Point::new(1, 5)]).unwrap();
//! assert_eq!(tree.length, 9);
//! ```

mod flute;
mod lut;

use std::fmt;

pub use flute::Flute;
pub use lut::LookUpTable;

/// `Point`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }
}

/// `Branch`: a tree node and the index of the node it connects to. A node whose `n` is
/// its own index is the root.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Branch {
    pub x: i32,
    pub y: i32,
    pub n: i32,
}

/// `Tree`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tree {
    pub deg: i32,
    pub length: i32,
    pub branch: Vec<Branch>,
}

/// Where FluteSharp throws, this returns an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FluteError {
    /// `Execute` with no points. FluteSharp throws `OverflowException` from
    /// `new Branch[2 * d - 2]` with d = 0.
    NoPoints,
    /// `new LookUpTable(d, ...)` with d outside 4 to 9.
    DegreeOutOfRange(usize),
    /// The lookup table files did not parse.
    InvalidData(String),
}

impl fmt::Display for FluteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FluteError::NoPoints => write!(f, "FLUTE needs at least one point."),
            FluteError::DegreeOutOfRange(d) => write!(
                f,
                "The degree for the look-up table must be between 4 and 9, inclusive, not {d}."
            ),
            FluteError::InvalidData(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for FluteError {}
