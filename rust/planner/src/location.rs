//! `Location`, from `src/FactorioTools/OilField/Grid/Location.cs`.

use std::cmp::Ordering;
use std::fmt;

/// A grid cell. Copies the C# `Location` with `LOCATION_AS_STRUCT`: compared by value.
///
/// `is_valid` is not stored. The C# sets `IsValid` once in the constructor from the same
/// coordinates, so computing it on demand gives the same answer.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    pub x: i32,
    pub y: i32,
}

impl Location {
    /// `Location.Invalid`.
    pub const INVALID: Location = Location { x: -1, y: -1 };

    pub const fn new(x: i32, y: i32) -> Self {
        Location { x, y }
    }

    /// `IsValid`: both coordinates are zero or more.
    pub const fn is_valid(self) -> bool {
        self.x >= 0 && self.y >= 0
    }

    /// `GetManhattanDistance`.
    pub fn manhattan_distance(self, other: Location) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    /// `GetEuclideanDistance(Location)`. The squares are summed as `int`, then
    /// converted to `double` for `Math.Sqrt`, the same as the C#.
    pub fn euclidean_distance(self, other: Location) -> f64 {
        f64::from(self.euclidean_distance_squared(other)).sqrt()
    }

    /// `GetEuclideanDistanceSquared(Location)`.
    pub fn euclidean_distance_squared(self, other: Location) -> i32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    /// `GetEuclideanDistance(double, double)`.
    pub fn euclidean_distance_to(self, bx: f64, by: f64) -> f64 {
        self.euclidean_distance_squared_to(bx, by).sqrt()
    }

    /// `GetEuclideanDistanceSquared(int, int)`.
    pub fn euclidean_distance_squared_to_int(self, bx: i32, by: i32) -> i32 {
        let dx = self.x - bx;
        let dy = self.y - by;
        dx * dx + dy * dy
    }

    /// `GetEuclideanDistanceSquared(double, double)`.
    pub fn euclidean_distance_squared_to(self, bx: f64, by: f64) -> f64 {
        let dx = f64::from(self.x) - bx;
        let dy = f64::from(self.y) - by;
        dx * dx + dy * dy
    }

    /// `Translate(int, int)`.
    pub const fn translate(self, dx: i32, dy: i32) -> Location {
        Location::new(self.x + dx, self.y + dy)
    }

    /// `Translate(Location)`.
    pub const fn translate_by(self, translation: Location) -> Location {
        self.translate(translation.x, translation.y)
    }

    /// `ToString("M")`: `(X, Y)`.
    pub fn to_string_m(self) -> String {
        format!("({}, {})", self.x, self.y)
    }
}

/// `CompareTo`: by X, then by Y.
impl Ord for Location {
    fn cmp(&self, other: &Self) -> Ordering {
        self.x.cmp(&other.x).then(self.y.cmp(&other.y))
    }
}

impl PartialOrd for Location {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// `ToString()` (format "S"): the coordinates, then the 1-based line and column as
/// VS Code shows them.
impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({}, {}) / (Ln {}, Col {})",
            self.x,
            self.y,
            self.y + 1,
            self.x + 1
        )
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validity_matches_the_constructor() {
        assert!(Location::new(0, 0).is_valid());
        assert!(!Location::INVALID.is_valid());
        assert!(!Location::new(3, -1).is_valid());
    }

    #[test]
    fn compares_by_x_then_y() {
        assert!(Location::new(1, 9) < Location::new(2, 0));
        assert!(Location::new(2, 0) < Location::new(2, 1));
    }

    #[test]
    fn distances() {
        let a = Location::new(1, 2);
        let b = Location::new(4, 6);
        assert_eq!(a.manhattan_distance(b), 7);
        assert_eq!(a.euclidean_distance_squared(b), 25);
        assert_eq!(a.euclidean_distance(b), 5.0);
    }

    #[test]
    fn formats_like_the_csharp() {
        assert_eq!(Location::new(3, 4).to_string(), "(3, 4) / (Ln 5, Col 4)");
        assert_eq!(Location::new(3, 4).to_string_m(), "(3, 4)");
    }
}
