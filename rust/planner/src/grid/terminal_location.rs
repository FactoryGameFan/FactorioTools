//! `TerminalLocation`, from `OilField/Grid/TerminalLocation.cs`: a pumpjack center, one of
//! its four pipe connection points (a terminal), and the direction that terminal faces.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Location;
use crate::data::Direction;

/// A terminal choice for one pumpjack.
///
/// **Identity.** In C# this is a class with no `Equals` override, so `==` and
/// `List.Remove` compare references: two terminals with the same center, location and
/// direction are still different objects. That matters where code builds a new terminal
/// that equals an existing one (`RotateOptimize`) and then compares it with the
/// originals (`Helpers.EliminateOtherTerminals`, and `AddPipes` after it). So each value
/// made by [`TerminalLocation::new`] gets its own identity, copies keep it (a copy is the
/// same object, as when the C# copies a list of terminals), and [`same_object`] compares
/// identities. Use `same_object` wherever the C# compares terminals.
///
/// `==` still compares the values, which is what the replay tests need when they check
/// recorded terminals against the port's.
///
/// [`same_object`]: TerminalLocation::same_object
#[derive(Clone, Copy, Debug)]
pub struct TerminalLocation {
    /// `Center`, which is settable in the C#. Nothing in the planner sets it.
    pub center: Location,
    pub terminal: Location,
    pub direction: Direction,
    identity: u64,
}

static NEXT_IDENTITY: AtomicU64 = AtomicU64::new(1);

impl TerminalLocation {
    /// `new TerminalLocation(center, terminal, direction)`: a new object.
    pub fn new(center: Location, terminal: Location, direction: Direction) -> Self {
        TerminalLocation {
            center,
            terminal,
            direction,
            identity: NEXT_IDENTITY.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// C#'s `==` on two terminals: whether they are the same object.
    pub fn same_object(&self, other: &TerminalLocation) -> bool {
        self.identity == other.identity
    }
}

/// Value equality: the same center, terminal and direction. Not what C#'s `==` does;
/// see [`TerminalLocation::same_object`].
impl PartialEq for TerminalLocation {
    fn eq(&self, other: &Self) -> bool {
        self.center == other.center
            && self.terminal == other.terminal
            && self.direction == other.direction
    }
}

impl Eq for TerminalLocation {}

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

    #[test]
    fn copies_are_the_same_object_and_new_values_are_not() {
        let a = TerminalLocation::new(Location::new(5, 6), Location::new(6, 4), Direction::Up);
        let copy = a;
        let twin = TerminalLocation::new(Location::new(5, 6), Location::new(6, 4), Direction::Up);
        assert!(a.same_object(&copy));
        assert!(!a.same_object(&twin));
        assert_eq!(a, twin);
    }
}
