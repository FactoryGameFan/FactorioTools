//! `TerminalLocation`, from `OilField/Grid/TerminalLocation.cs`: a pumpjack center, one of
//! its four pipe connection points (a terminal), and the direction that terminal faces.

use std::fmt;

use crate::Location;
use crate::data::Direction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalLocation {
    /// `Center`, which is settable in the C#.
    pub center: Location,
    pub terminal: Location,
    pub direction: Direction,
}

impl TerminalLocation {
    pub fn new(center: Location, terminal: Location, direction: Direction) -> Self {
        TerminalLocation {
            center,
            terminal,
            direction,
        }
    }
}

/// `ToString()`: `Pump (X, Y) Direction terminal (X, Y)`.
impl fmt::Display for TerminalLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Pump {} {} terminal ({})",
            self.center.to_string_m(),
            self.direction,
            self.terminal.to_string_m()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_the_csharp() {
        let t = TerminalLocation::new(Location::new(5, 6), Location::new(6, 4), Direction::Up);
        assert_eq!(t.to_string(), "Pump (5, 6) Up terminal ((6, 4))");
    }
}
