//! Bresenham's line, from `Algorithms/BresenhamsLine.cs`.

use crate::Location;

/// `BresenhamsLine.GetPath(a, b)`: every cell on the line from `a` to `b`, both included.
pub fn get_path(a: Location, b: Location) -> Vec<Location> {
    let mut line = Vec::new();
    let dx = (b.x - a.x).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let dy = -(b.y - a.y).abs();
    let sy = if a.y < b.y { 1 } else { -1 };
    let mut error = dx + dy;

    let mut x0 = a.x;
    let mut y0 = a.y;

    loop {
        line.push(Location::new(x0, y0));

        if x0 == b.x && y0 == b.y {
            break;
        }

        let e2 = 2 * error;
        if e2 >= dy {
            if x0 == b.x {
                break;
            }
            error += dy;
            x0 += sx;
        }

        if e2 <= dx {
            if y0 == b.y {
                break;
            }

            error += dx;
            y0 += sy;
        }
    }

    line
}

/// Ported from `test/FactorioTools.Test/OilField/Algorithms/BresenhamsLineTest.cs`.
#[cfg(test)]
mod tests {
    use super::*;

    fn l(x: i32, y: i32) -> Location {
        Location::new(x, y)
    }

    #[test]
    fn single_point_returns_that_point() {
        assert_eq!(get_path(l(5, 5), l(5, 5)), vec![l(5, 5)]);
    }

    #[test]
    fn horizontal_line_includes_every_point_left_to_right() {
        assert_eq!(
            get_path(l(0, 0), l(3, 0)),
            vec![l(0, 0), l(1, 0), l(2, 0), l(3, 0)]
        );
    }

    #[test]
    fn vertical_line_includes_every_point_top_to_bottom() {
        assert_eq!(
            get_path(l(0, 0), l(0, 3)),
            vec![l(0, 0), l(0, 1), l(0, 2), l(0, 3)]
        );
    }

    #[test]
    fn perfect_diagonal_steps_one_in_each_axis_per_point() {
        assert_eq!(get_path(l(0, 0), l(2, 2)), vec![l(0, 0), l(1, 1), l(2, 2)]);
    }

    #[test]
    fn shallow_slope_steps_along_the_major_axis() {
        assert_eq!(
            get_path(l(0, 0), l(4, 2)),
            vec![l(0, 0), l(1, 1), l(2, 1), l(3, 2), l(4, 2)]
        );
    }

    #[test]
    fn reversed_endpoints_produces_the_reversed_path() {
        let forward = get_path(l(0, 0), l(3, 0));
        let mut backward = get_path(l(3, 0), l(0, 0));
        backward.reverse();
        assert_eq!(forward, backward);
    }
}
