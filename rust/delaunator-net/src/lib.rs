//! A line-for-line port of DelaunatorSharp's `DelaunatorSharp/Delaunator.cs` (a submodule
//! here until stage 13b), limited to what the oil-field planner uses: the constructor,
//! `Triangles`, `Halfedges` and `Hull`.
//!
//! Keep the arithmetic exactly as the C# writes it. The planner feeds in grid points, so
//! many lie on one circle and many distances tie. Which triangles come out then depends
//! on the seed choice, the custom quicksort below and the non-robust `orient` and
//! `in_circle` tests, all with plain `f64` math in the C#'s order. Rust never fuses a
//! multiply and an add on its own, so no `mul_add` here, ever.

use std::fmt;

/// Why a triangulation failed. Each variant is a C# exception the constructor throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelaunatorError {
    /// `ArgumentOutOfRangeException`: fewer than 3 points.
    TooFewPoints,
    /// `Exception("No Delaunay triangulation exists for this input.")`: every point is
    /// collinear or identical.
    NoTriangulation,
}

impl fmt::Display for DelaunatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DelaunatorError::TooFewPoints => f.write_str("Need at least 3 points"),
            DelaunatorError::NoTriangulation => {
                f.write_str("No Delaunay triangulation exists for this input.")
            }
        }
    }
}

impl std::error::Error for DelaunatorError {}

/// `Math.Pow(2, -52)`.
const EPSILON: f64 = f64::EPSILON;

/// The C#'s fixed-size `EDGE_STACK`. Pushes past it are dropped, as in the C#.
const EDGE_STACK_LEN: usize = 512;

/// The result of a triangulation.
#[derive(Debug, Clone)]
pub struct Delaunator {
    /// One value per half-edge: the index of the point the half-edge starts at.
    pub triangles: Vec<i32>,
    /// One value per half-edge: the opposite half-edge in the adjacent triangle, or -1.
    pub halfedges: Vec<i32>,
    /// The point indices around the convex hull.
    pub hull: Vec<i32>,
}

/// The working state the C# keeps in fields while the constructor runs.
struct Builder<'a> {
    coords: &'a [f64],
    triangles: Vec<i32>,
    halfedges: Vec<i32>,
    triangles_len: usize,
    hash_size: i32,
    hull_prev: Vec<i32>,
    hull_next: Vec<i32>,
    hull_tri: Vec<i32>,
    hull_hash: Vec<i32>,
    hull_start: i32,
    cx: f64,
    cy: f64,
    edge_stack: [i32; EDGE_STACK_LEN],
}

impl Delaunator {
    /// `new Delaunator(IPoint[] points)`.
    pub fn new(points: &[(f64, f64)]) -> Result<Delaunator, DelaunatorError> {
        if points.len() < 3 {
            return Err(DelaunatorError::TooFewPoints);
        }

        let mut coords = vec![0.0; points.len() * 2];
        for (i, &(x, y)) in points.iter().enumerate() {
            coords[2 * i] = x;
            coords[2 * i + 1] = y;
        }

        let n = points.len();
        let max_triangles = 2 * n - 5;

        // C# arrays start zeroed, and the hull arrays and hash are read before every slot
        // is written (the hash lookup accepts 0 as a point index), so zero them too.
        let mut b = Builder {
            coords: &coords,
            triangles: vec![0; max_triangles * 3],
            halfedges: vec![0; max_triangles * 3],
            triangles_len: 0,
            hash_size: (n as f64).sqrt().ceil() as i32,
            hull_prev: vec![0; n],
            hull_next: vec![0; n],
            hull_tri: vec![0; n],
            hull_hash: Vec::new(),
            hull_start: 0,
            cx: 0.0,
            cy: 0.0,
            edge_stack: [0; EDGE_STACK_LEN],
        };
        b.hull_hash = vec![0; b.hash_size as usize];

        let mut ids: Vec<i32> = vec![0; n];

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for (i, id) in ids.iter_mut().enumerate() {
            let x = coords[2 * i];
            let y = coords[2 * i + 1];
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
            *id = i as i32;
        }

        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;

        let mut min_dist = f64::INFINITY;
        let mut i0 = 0;
        let mut i1 = 0;
        let mut i2 = 0;

        // pick a seed point close to the center
        for i in 0..n {
            let d = dist(cx, cy, coords[2 * i], coords[2 * i + 1]);
            if d < min_dist {
                i0 = i;
                min_dist = d;
            }
        }
        let i0x = coords[2 * i0];
        let i0y = coords[2 * i0 + 1];

        min_dist = f64::INFINITY;

        // find the point closest to the seed
        for i in 0..n {
            if i == i0 {
                continue;
            }
            let d = dist(i0x, i0y, coords[2 * i], coords[2 * i + 1]);
            if d < min_dist && d > 0.0 {
                i1 = i;
                min_dist = d;
            }
        }

        let mut i1x = coords[2 * i1];
        let mut i1y = coords[2 * i1 + 1];

        let mut min_radius = f64::INFINITY;

        // find the third point which forms the smallest circumcircle with the first two
        for i in 0..n {
            if i == i0 || i == i1 {
                continue;
            }
            let r = circumradius(i0x, i0y, i1x, i1y, coords[2 * i], coords[2 * i + 1]);
            if r < min_radius {
                i2 = i;
                min_radius = r;
            }
        }
        let mut i2x = coords[2 * i2];
        let mut i2y = coords[2 * i2 + 1];

        if min_radius == f64::INFINITY {
            return Err(DelaunatorError::NoTriangulation);
        }

        if orient(i0x, i0y, i1x, i1y, i2x, i2y) {
            std::mem::swap(&mut i1, &mut i2);
            std::mem::swap(&mut i1x, &mut i2x);
            std::mem::swap(&mut i1y, &mut i2y);
        }

        let (center_x, center_y) = circumcenter(i0x, i0y, i1x, i1y, i2x, i2y);
        b.cx = center_x;
        b.cy = center_y;

        let mut dists = vec![0.0; n];
        for (i, d) in dists.iter_mut().enumerate() {
            *d = dist(coords[2 * i], coords[2 * i + 1], center_x, center_y);
        }

        // sort the points by distance from the seed triangle circumcenter
        quicksort(&mut ids, &dists, 0, n as i32 - 1);

        let (i0, i1, i2) = (i0 as i32, i1 as i32, i2 as i32);

        // set up the seed triangle as the starting hull
        b.hull_start = i0;
        let mut hull_size: i32 = 3;

        b.hull_prev[i2 as usize] = i1;
        b.hull_next[i0 as usize] = i1;
        b.hull_prev[i0 as usize] = i2;
        b.hull_next[i1 as usize] = i2;
        b.hull_prev[i1 as usize] = i0;
        b.hull_next[i2 as usize] = i0;

        b.hull_tri[i0 as usize] = 0;
        b.hull_tri[i1 as usize] = 1;
        b.hull_tri[i2 as usize] = 2;

        let k = b.hash_key(i0x, i0y);
        b.hull_hash[k] = i0;
        let k = b.hash_key(i1x, i1y);
        b.hull_hash[k] = i1;
        let k = b.hash_key(i2x, i2y);
        b.hull_hash[k] = i2;

        b.add_triangle(i0, i1, i2, -1, -1, -1);

        let mut xp = 0.0;
        let mut yp = 0.0;

        for (k, &i) in ids.iter().enumerate() {
            let x = coords[2 * i as usize];
            let y = coords[2 * i as usize + 1];

            // skip near-duplicate points
            if k > 0 && (x - xp).abs() <= EPSILON && (y - yp).abs() <= EPSILON {
                continue;
            }
            xp = x;
            yp = y;

            // skip seed triangle points
            if i == i0 || i == i1 || i == i2 {
                continue;
            }

            // find a visible edge on the convex hull using edge hash
            let mut start = 0;
            for j in 0..b.hash_size {
                let key = b.hash_key(x, y) as i32;
                start = b.hull_hash[((key + j) % b.hash_size) as usize];
                if start != -1 && start != b.hull_next[start as usize] {
                    break;
                }
            }

            start = b.hull_prev[start as usize];
            let mut e = start;
            let mut q = b.hull_next[e as usize];

            while !orient(x, y, b.x(e), b.y(e), b.x(q), b.y(q)) {
                e = q;
                if e == start {
                    e = i32::MAX;
                    break;
                }

                q = b.hull_next[e as usize];
            }

            if e == i32::MAX {
                continue; // likely a near-duplicate point; skip it
            }

            // add the first triangle from the point
            let mut t = b.add_triangle(
                e,
                i,
                b.hull_next[e as usize],
                -1,
                -1,
                b.hull_tri[e as usize],
            );

            // recursively flip triangles from the point until they satisfy the Delaunay condition
            b.hull_tri[i as usize] = b.legalize(t + 2);
            b.hull_tri[e as usize] = t; // keep track of boundary triangles on the hull
            hull_size += 1;

            // walk forward through the hull, adding more triangles and flipping recursively
            let mut next = b.hull_next[e as usize];
            q = b.hull_next[next as usize];

            while orient(x, y, b.x(next), b.y(next), b.x(q), b.y(q)) {
                t = b.add_triangle(
                    next,
                    i,
                    q,
                    b.hull_tri[i as usize],
                    -1,
                    b.hull_tri[next as usize],
                );
                b.hull_tri[i as usize] = b.legalize(t + 2);
                b.hull_next[next as usize] = next; // mark as removed
                hull_size -= 1;
                next = q;

                q = b.hull_next[next as usize];
            }

            // walk backward from the other side, adding more triangles and flipping
            if e == start {
                q = b.hull_prev[e as usize];

                while orient(x, y, b.x(q), b.y(q), b.x(e), b.y(e)) {
                    t = b.add_triangle(q, i, e, -1, b.hull_tri[e as usize], b.hull_tri[q as usize]);
                    b.legalize(t + 2);
                    b.hull_tri[q as usize] = t;
                    b.hull_next[e as usize] = e; // mark as removed
                    hull_size -= 1;
                    e = q;

                    q = b.hull_prev[e as usize];
                }
            }

            // update the hull indices
            b.hull_prev[i as usize] = e;
            b.hull_start = e;
            b.hull_prev[next as usize] = i;
            b.hull_next[e as usize] = i;
            b.hull_next[i as usize] = next;

            // save the two new edges in the hash table
            let k = b.hash_key(x, y);
            b.hull_hash[k] = i;
            let k = b.hash_key(b.x(e), b.y(e));
            b.hull_hash[k] = e;
        }

        let mut hull = vec![0; hull_size as usize];
        let mut s = b.hull_start;
        for h in hull.iter_mut() {
            *h = s;
            s = b.hull_next[s as usize];
        }

        // trim typed triangle mesh arrays
        let len = b.triangles_len;
        b.triangles.truncate(len);
        b.halfedges.truncate(len);

        Ok(Delaunator {
            triangles: b.triangles,
            halfedges: b.halfedges,
            hull,
        })
    }
}

impl Builder<'_> {
    fn x(&self, i: i32) -> f64 {
        self.coords[2 * i as usize]
    }

    fn y(&self, i: i32) -> f64 {
        self.coords[2 * i as usize + 1]
    }

    /// `Legalize`, with the recursion replaced by the fixed-size edge stack.
    fn legalize(&mut self, mut a: i32) -> i32 {
        let mut i: usize = 0;
        let mut ar;

        loop {
            let b = self.halfedges[a as usize];

            // if the pair of triangles doesn't satisfy the Delaunay condition (p1 is inside
            // the circumcircle of [p0, pl, pr]), flip them, then do the same check/flip
            // recursively for the new pair of triangles
            let a0 = a - a % 3;
            ar = a0 + (a + 2) % 3;

            if b == -1 {
                // convex hull edge
                if i == 0 {
                    break;
                }
                i -= 1;
                a = self.edge_stack[i];
                continue;
            }

            let b0 = b - b % 3;
            let al = a0 + (a + 1) % 3;
            let bl = b0 + (b + 2) % 3;

            let p0 = self.triangles[ar as usize];
            let pr = self.triangles[a as usize];
            let pl = self.triangles[al as usize];
            let p1 = self.triangles[bl as usize];

            let illegal = in_circle(
                self.x(p0),
                self.y(p0),
                self.x(pr),
                self.y(pr),
                self.x(pl),
                self.y(pl),
                self.x(p1),
                self.y(p1),
            );

            if illegal {
                self.triangles[a as usize] = p1;
                self.triangles[b as usize] = p0;

                let hbl = self.halfedges[bl as usize];

                // edge swapped on the other side of the hull (rare); fix the halfedge reference
                if hbl == -1 {
                    let mut e = self.hull_start;
                    loop {
                        if self.hull_tri[e as usize] == bl {
                            self.hull_tri[e as usize] = a;
                            break;
                        }
                        e = self.hull_prev[e as usize];
                        if e == self.hull_start {
                            break;
                        }
                    }
                }
                self.link(a, hbl);
                self.link(b, self.halfedges[ar as usize]);
                self.link(ar, bl);

                let br = b0 + (b + 1) % 3;

                // don't worry about hitting the cap: it can only happen on extremely degenerate input
                if i < EDGE_STACK_LEN {
                    self.edge_stack[i] = br;
                    i += 1;
                }
            } else {
                if i == 0 {
                    break;
                }
                i -= 1;
                a = self.edge_stack[i];
            }
        }

        ar
    }

    fn add_triangle(&mut self, i0: i32, i1: i32, i2: i32, a: i32, b: i32, c: i32) -> i32 {
        let t = self.triangles_len as i32;

        self.triangles[t as usize] = i0;
        self.triangles[t as usize + 1] = i1;
        self.triangles[t as usize + 2] = i2;

        self.link(t, a);
        self.link(t + 1, b);
        self.link(t + 2, c);

        self.triangles_len += 3;
        t
    }

    fn link(&mut self, a: i32, b: i32) {
        self.halfedges[a as usize] = b;
        if b != -1 {
            self.halfedges[b as usize] = a;
        }
    }

    /// `HashKey`: `(int)(Math.Floor(PseudoAngle(x - cx, y - cy) * hashSize) % hashSize)`.
    ///
    /// A point on the circumcenter gives `0 / 0`, so NaN. .NET 9 and later convert NaN to
    /// 0 when casting to `int`, and so does Rust's `as`, so both land on slot 0.
    fn hash_key(&self, x: f64, y: f64) -> usize {
        let size = f64::from(self.hash_size);
        let key = ((pseudo_angle(x - self.cx, y - self.cy) * size).floor() % size) as i32;
        key as usize
    }
}

fn pseudo_angle(dx: f64, dy: f64) -> f64 {
    let p = dx / (dx.abs() + dy.abs());
    (if dy > 0.0 { 3.0 - p } else { 1.0 + p }) / 4.0 // [0..1]
}

/// The C#'s own quicksort of point ids by distance: insertion sort at 20 or fewer, else a
/// median-of-three partition. It is not stable, and ties are common, so it must stay
/// exactly as written.
fn quicksort(ids: &mut [i32], dists: &[f64], left: i32, right: i32) {
    let d = |ids: &[i32], k: i32| dists[ids[k as usize] as usize];
    if right - left <= 20 {
        for i in left + 1..=right {
            let temp = ids[i as usize];
            let temp_dist = dists[temp as usize];
            let mut j = i - 1;
            while j >= left && d(ids, j) > temp_dist {
                ids[(j + 1) as usize] = ids[j as usize];
                j -= 1;
            }
            ids[(j + 1) as usize] = temp;
        }
    } else {
        let median = (left + right) >> 1;
        let mut i = left + 1;
        let mut j = right;
        ids.swap(median as usize, i as usize);
        if d(ids, left) > d(ids, right) {
            ids.swap(left as usize, right as usize);
        }
        if d(ids, i) > d(ids, right) {
            ids.swap(i as usize, right as usize);
        }
        if d(ids, left) > d(ids, i) {
            ids.swap(left as usize, i as usize);
        }

        let temp = ids[i as usize];
        let temp_dist = dists[temp as usize];
        loop {
            loop {
                i += 1;
                if d(ids, i) >= temp_dist {
                    break;
                }
            }
            loop {
                j -= 1;
                if d(ids, j) <= temp_dist {
                    break;
                }
            }
            if j < i {
                break;
            }
            ids.swap(i as usize, j as usize);
        }
        ids[(left + 1) as usize] = ids[j as usize];
        ids[j as usize] = temp;

        if right - i + 1 >= j - left {
            quicksort(ids, dists, i, right);
            quicksort(ids, dists, left, j - 1);
        } else {
            quicksort(ids, dists, left, j - 1);
            quicksort(ids, dists, i, right);
        }
    }
}

fn orient(px: f64, py: f64, qx: f64, qy: f64, rx: f64, ry: f64) -> bool {
    (qy - py) * (rx - qx) - (qx - px) * (ry - qy) < 0.0
}

#[allow(clippy::too_many_arguments)]
fn in_circle(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, px: f64, py: f64) -> bool {
    let dx = ax - px;
    let dy = ay - py;
    let ex = bx - px;
    let ey = by - py;
    let fx = cx - px;
    let fy = cy - py;

    let ap = dx * dx + dy * dy;
    let bp = ex * ex + ey * ey;
    let cp = fx * fx + fy * fy;

    dx * (ey * cp - bp * fy) - dy * (ex * cp - bp * fx) + ap * (ex * fy - ey * fx) < 0.0
}

fn circumradius(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let ex = cx - ax;
    let ey = cy - ay;
    let bl = dx * dx + dy * dy;
    let cl = ex * ex + ey * ey;
    let d = 0.5 / (dx * ey - dy * ex);
    let x = (ey * bl - dy * cl) * d;
    let y = (dx * cl - ex * bl) * d;
    x * x + y * y
}

fn circumcenter(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> (f64, f64) {
    let dx = bx - ax;
    let dy = by - ay;
    let ex = cx - ax;
    let ey = cy - ay;
    let bl = dx * dx + dy * dy;
    let cl = ex * ex + ey * ey;
    let d = 0.5 / (dx * ey - dy * ex);
    let x = ax + (ey * bl - dy * cl) * d;
    let y = ay + (dx * cl - ex * bl) * d;
    (x, y)
}

fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_fewer_than_three_points() {
        assert_eq!(
            Delaunator::new(&[(0.0, 0.0), (1.0, 0.0)]).unwrap_err(),
            DelaunatorError::TooFewPoints
        );
    }

    #[test]
    fn rejects_collinear_points() {
        assert_eq!(
            Delaunator::new(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]).unwrap_err(),
            DelaunatorError::NoTriangulation
        );
    }

    #[test]
    fn triangulates_a_square() {
        let d = Delaunator::new(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]).unwrap();
        assert_eq!(d.triangles.len(), 6);
        assert_eq!(d.hull.len(), 4);
    }
}
