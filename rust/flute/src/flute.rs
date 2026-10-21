//! The FLUTE algorithm, from `FLUTE.cs` in FluteSharp (a port of FLUTE 3.1's C).
//!
//! This follows the C# line for line, including its index arithmetic, so the trees come
//! out identical. `flutes_MD` scores breaking points with `f32` (`float` in the C#), and
//! the operations here keep the C#'s order and per-operation `f32` rounding.

use crate::lut::{LookUpTable, NUMGRP};
use crate::{Branch, FluteError, Point, Tree};

/// `FLUTE`.
#[derive(Clone, Debug)]
pub struct Flute {
    lut: LookUpTable,
}

impl Flute {
    /// `new FLUTE(lut)`.
    pub fn new(lut: LookUpTable) -> Self {
        Flute { lut }
    }

    /// `Execute(points)`, with the default accuracy of 3.
    pub fn execute(&self, points: &[Point]) -> Result<Tree, FluteError> {
        self.execute_with_accuracy(points, 3)
    }

    /// `Execute(points, accuracy)`.
    pub fn execute_with_accuracy(
        &self,
        points: &[Point],
        accuracy: i32,
    ) -> Result<Tree, FluteError> {
        if points.len() == 1 {
            return Ok(Tree {
                deg: 1,
                length: 0,
                branch: vec![Branch {
                    x: points[0].x,
                    y: points[0].y,
                    n: 0,
                }],
            });
        } else if points.len() == 2 {
            return Ok(Tree {
                deg: 2,
                length: (points[0].x - points[1].x).abs() + (points[0].y - points[1].y).abs(),
                branch: vec![
                    Branch {
                        x: points[0].x,
                        y: points[0].y,
                        n: 1,
                    },
                    Branch {
                        x: points[1].x,
                        y: points[1].y,
                        n: 1,
                    },
                ],
            });
        } else if points.is_empty() {
            // The C# reaches `new Branch[2 * d - 2]` with d = 0 and throws
            // OverflowException.
            return Err(FluteError::NoPoints);
        }

        // Both sorts break ties by index, so the order is total and any correct sort gives
        // the same result as List.Sort.
        let mut points_by_x: Vec<(Point, usize)> = points.iter().copied().zip(0..).collect();
        points_by_x.sort_by(|a, b| a.0.x.cmp(&b.0.x).then(a.1.cmp(&b.1)));

        let mut points_by_y: Vec<(Point, usize)> = points_by_x
            .iter()
            .enumerate()
            .map(|(i, &(p, _))| (p, i))
            .collect();
        points_by_y.sort_by(|a, b| a.0.y.cmp(&b.0.y).then(a.1.cmp(&b.1)));

        let xs: Vec<i32> = points_by_x.iter().map(|p| p.0.x).collect();
        let ys: Vec<i32> = points_by_y.iter().map(|p| p.0.y).collect();
        let s: Vec<i32> = points_by_y.iter().map(|p| p.1 as i32).collect();

        Ok(self.flutes(points.len(), &xs, &ys, &s, accuracy))
    }

    fn flutes(&self, d: usize, xs: &[i32], ys: &[i32], s: &[i32], acc: i32) -> Tree {
        if d <= self.lut.d {
            self.flutes_ld(d, xs, ys, s)
        } else {
            self.flutes_md(d, xs, ys, s, acc)
        }
    }

    fn flutes_ld(&self, d: usize, xs: &[i32], ys: &[i32], s: &[i32]) -> Tree {
        let lut_d = self.lut.d;
        let mut dd = vec![0i32; 2 * lut_d - 2]; // 0..D-2 for v, D-1..2*D-3 for h
        let mut l = vec![0i32; self.lut.mpowv + 1];
        let minl;
        let mut t = Tree {
            deg: d as i32,
            length: 0,
            branch: vec![Branch::default(); 2 * d - 2],
        };
        let su = |i: usize| s[i] as usize;

        if d == 2 {
            minl = xs[1] - xs[0] + ys[1] - ys[0];
            t.branch[0] = Branch {
                x: xs[su(0)],
                y: ys[0],
                n: 1,
            };
            t.branch[1] = Branch {
                x: xs[su(1)],
                y: ys[1],
                n: 1,
            };
        } else if d == 3 {
            minl = xs[2] - xs[0] + ys[2] - ys[0];
            t.branch[0] = Branch {
                x: xs[su(0)],
                y: ys[0],
                n: 3,
            };
            t.branch[1] = Branch {
                x: xs[su(1)],
                y: ys[1],
                n: 3,
            };
            t.branch[2] = Branch {
                x: xs[su(2)],
                y: ys[2],
                n: 3,
            };
            t.branch[3] = Branch {
                x: xs[1],
                y: ys[1],
                n: 3,
            };
        } else {
            let mut k: i32 = 0;
            if s[0] < s[2] {
                k += 1;
            }
            if s[1] < s[2] {
                k += 1;
            }

            for i in 3..d {
                // p0 = 0 always, skip i = 1 for symmetry
                let mut pi = s[i];
                for j in (i + 1..d).rev() {
                    if s[j] < s[i] {
                        pi -= 1;
                    }
                }
                k = pi + (i as i32 + 1) * k;
            }

            let numgrp = NUMGRP[d] as i32;
            let hflip;
            if k < numgrp {
                // no horizontal flip
                hflip = false;
                for i in 1..=d - 3 {
                    dd[i] = ys[i + 1] - ys[i];
                    dd[d - 1 + i] = xs[i + 1] - xs[i];
                }
            } else {
                hflip = true;
                k = 2 * numgrp - 1 - k;
                for i in 1..=d - 3 {
                    dd[i] = ys[i + 1] - ys[i];
                    dd[d - 1 + i] = xs[d - 1 - i] - xs[d - 2 - i];
                }
            }

            let k = k as usize;
            let mut best = xs[d - 1] - xs[0] + ys[d - 1] - ys[0];
            l[0] = best;
            let rlist = self.lut.group(d, k);
            let mut i = 0;
            while rlist[0].seg[i] > 0 {
                best += dd[rlist[0].seg[i] as usize];
                i += 1;
            }
            let mut bestrlist = &rlist[0];
            l[1] = best;
            let mut j = 2;
            let mut rlisti = 0;
            while j <= self.lut.numsoln(d, k) {
                rlisti += 1;
                let candidate = &rlist[rlisti];
                let mut sum = l[candidate.parent as usize];
                let mut i = 0;
                while candidate.seg[i] > 0 {
                    sum += dd[candidate.seg[i] as usize];
                    i += 1;
                }
                let mut i = 10;
                while candidate.seg[i] > 0 {
                    sum -= dd[candidate.seg[i] as usize];
                    i -= 1;
                }
                if sum < best {
                    best = sum;
                    bestrlist = candidate;
                }
                l[j] = sum;
                j += 1;
            }
            minl = best;

            let nb = |i: usize| i32::from(bestrlist.neighbor[i]);
            t.branch[0].x = xs[su(0)];
            t.branch[0].y = ys[0];
            t.branch[1].x = xs[su(1)];
            t.branch[1].y = ys[1];
            for i in 2..d - 2 {
                t.branch[i].x = xs[su(i)];
                t.branch[i].y = ys[i];
                t.branch[i].n = nb(i);
            }
            t.branch[d - 2].x = xs[su(d - 2)];
            t.branch[d - 2].y = ys[d - 2];
            t.branch[d - 1].x = xs[su(d - 1)];
            t.branch[d - 1].y = ys[d - 1];
            if hflip {
                if s[1] < s[0] {
                    t.branch[0].n = nb(1);
                    t.branch[1].n = nb(0);
                } else {
                    t.branch[0].n = nb(0);
                    t.branch[1].n = nb(1);
                }
                if s[d - 1] < s[d - 2] {
                    t.branch[d - 2].n = nb(d - 1);
                    t.branch[d - 1].n = nb(d - 2);
                } else {
                    t.branch[d - 2].n = nb(d - 2);
                    t.branch[d - 1].n = nb(d - 1);
                }
                for i in d..2 * d - 2 {
                    let rc = usize::from(bestrlist.rowcol[i - d]);
                    t.branch[i].x = xs[d - 1 - rc % 16];
                    t.branch[i].y = ys[rc / 16];
                    t.branch[i].n = nb(i);
                }
            } else {
                if s[0] < s[1] {
                    t.branch[0].n = nb(1);
                    t.branch[1].n = nb(0);
                } else {
                    t.branch[0].n = nb(0);
                    t.branch[1].n = nb(1);
                }
                if s[d - 2] < s[d - 1] {
                    t.branch[d - 2].n = nb(d - 1);
                    t.branch[d - 1].n = nb(d - 2);
                } else {
                    t.branch[d - 2].n = nb(d - 2);
                    t.branch[d - 1].n = nb(d - 1);
                }
                for i in d..2 * d - 2 {
                    let rc = usize::from(bestrlist.rowcol[i - d]);
                    t.branch[i].x = xs[rc % 16];
                    t.branch[i].y = ys[rc / 16];
                    t.branch[i].n = nb(i);
                }
            }
        }
        t.length = minl;
        t
    }

    fn flutes_md(&self, d: usize, xs: &[i32], ys: &[i32], s: &[i32], mut acc: i32) -> Tree {
        let mut x1 = vec![0i32; d];
        let mut x2 = vec![0i32; d];
        let mut y1 = vec![0i32; d];
        let mut y2 = vec![0i32; d];
        let mut si = vec![0usize; d];
        let mut s1 = vec![0i32; d];
        let mut s2 = vec![0i32; d];
        let mut score = vec![0f32; 2 * d];
        let mut penalty = vec![0f32; d];
        let mut distx = vec![0i32; d];
        let mut disty = vec![0i32; d];
        let su = |i: usize| s[i] as usize;
        let di = d as i32;

        if s[0] < s[d - 1] {
            let mut ms = s[0].max(s[1]);
            let mut i = 2;
            while i <= ms as usize {
                ms = ms.max(s[i]);
                i += 1;
            }
            if ms <= di - 3 {
                let ms = ms as usize;
                x1[..=ms].copy_from_slice(&xs[..=ms]);
                y1[..=ms].copy_from_slice(&ys[..=ms]);
                s1[..=ms].copy_from_slice(&s[..=ms]);
                x1[ms + 1] = xs[ms];
                y1[ms + 1] = ys[ms];
                s1[ms + 1] = ms as i32 + 1;

                s2[0] = 0;
                for i in 1..=d - 1 - ms {
                    s2[i] = s[i + ms] - ms as i32;
                }

                let t1 = self.flutes(ms + 2, &x1, &y1, &s1, acc);
                let t2 = self.flutes(d - ms, &xs[ms..], &ys[ms..], &s2, acc);
                return dmergetree(&t1, &t2);
            }
        } else {
            // (s[0] > s[d-1])
            let mut ms = s[0].min(s[1]);
            let mut i = 2;
            while i as i32 <= di - 1 - ms {
                ms = ms.min(s[i]);
                i += 1;
            }
            if ms >= 2 {
                let ms = ms as usize;
                x1[0] = xs[ms];
                y1[0] = ys[0];
                s1[0] = s[0] - ms as i32 + 1;
                for i in 1..=d - 1 - ms {
                    x1[i] = xs[i + ms - 1];
                    y1[i] = ys[i];
                    s1[i] = s[i] - ms as i32 + 1;
                }
                x1[d - ms] = xs[d - 1];
                y1[d - ms] = ys[d - 1 - ms];
                s1[d - ms] = 0;

                s2[0] = ms as i32;
                for i in 1..=ms {
                    s2[i] = s[i + d - 1 - ms];
                }

                let t1 = self.flutes(d + 1 - ms, &x1, &y1, &s1, acc);
                let t2 = self.flutes(ms + 1, xs, &ys[d - 1 - ms..], &s2, acc);
                return dmergetree(&t1, &t2);
            }
        }

        // Find inverse si[] of s[]
        for r in 0..d {
            si[su(r)] = r;
        }

        // Determine breaking directions and positions dp[]
        let mut lb = (di - 2 * acc + 2) / 4;
        if lb < 2 {
            lb = 2;
        }
        let ub = di - 1 - lb;
        let lb = lb as usize;
        let ub = ub as usize;

        // Compute scores
        const AA: f32 = 0.6; // 2.0*BB
        const BB: f32 = 0.3;
        let cc: f32 = 7.4f32 / ((di as f32 + 10.0f32) * (di as f32 - 3.0f32));
        let ddc: f32 = 4.8f32 / ((di - 1) as f32);

        // Compute penalty[]
        let dx: f32 = cc * ((xs[d - 2] - xs[1]) as f32);
        let dy: f32 = cc * ((ys[d - 2] - ys[1]) as f32);
        let mut pnlty: f32 = 0.0;
        let mut r = d / 2;
        while r >= 2 {
            penalty[r] = pnlty;
            penalty[d - 1 - r] = pnlty;
            r -= 1;
            pnlty += dx;
        }
        penalty[1] = pnlty;
        penalty[d - 2] = pnlty;
        penalty[0] = pnlty;
        penalty[d - 1] = pnlty;

        let mut pnlty: f32 = dy;
        let mut r = d / 2 - 1;
        while r >= 2 {
            penalty[su(r)] += pnlty;
            penalty[su(d - 1 - r)] += pnlty;
            r -= 1;
            pnlty += dy;
        }
        penalty[su(1)] += pnlty;
        penalty[su(d - 2)] += pnlty;
        penalty[su(0)] += pnlty;
        penalty[su(d - 1)] += pnlty;

        // Compute distx[], disty[]
        let xydiff = (xs[d - 1] - xs[0]) - (ys[d - 1] - ys[0]);
        let (mut mins, mut maxs) = if s[0] < s[1] {
            (su(0), su(1))
        } else {
            (su(1), su(0))
        };
        let (mut minsi, mut maxsi) = if si[0] < si[1] {
            (si[0], si[1])
        } else {
            (si[1], si[0])
        };
        for r in 2..=ub {
            if su(r) < mins {
                mins = su(r);
            } else if su(r) > maxs {
                maxs = su(r);
            }
            distx[r] = xs[maxs] - xs[mins];
            if si[r] < minsi {
                minsi = si[r];
            } else if si[r] > maxsi {
                maxsi = si[r];
            }
            disty[r] = ys[maxsi] - ys[minsi] + xydiff;
        }

        let (mut mins, mut maxs) = if s[d - 2] < s[d - 1] {
            (su(d - 2), su(d - 1))
        } else {
            (su(d - 1), su(d - 2))
        };
        let (mut minsi, mut maxsi) = if si[d - 2] < si[d - 1] {
            (si[d - 2], si[d - 1])
        } else {
            (si[d - 1], si[d - 2])
        };
        let mut r = d - 3;
        while r >= lb {
            if su(r) < mins {
                mins = su(r);
            } else if su(r) > maxs {
                maxs = su(r);
            }
            distx[r] += xs[maxs] - xs[mins];
            if si[r] < minsi {
                minsi = si[r];
            } else if si[r] > maxsi {
                maxsi = si[r];
            }
            disty[r] += ys[maxsi] - ys[minsi];
            r -= 1;
        }

        let mut nbp = 0;
        for r in lb..=ub {
            // In the C#, `(int) - float - float*int - float*int` evaluates left to right,
            // converting each int to float where it meets a float.
            score[nbp] = if si[r] <= 1 {
                ((xs[r + 1] - xs[r - 1]) as f32)
                    - penalty[r]
                    - AA * ((ys[2] - ys[1]) as f32)
                    - ddc * (disty[r] as f32)
            } else if si[r] >= d - 2 {
                ((xs[r + 1] - xs[r - 1]) as f32)
                    - penalty[r]
                    - AA * ((ys[d - 2] - ys[d - 3]) as f32)
                    - ddc * (disty[r] as f32)
            } else {
                ((xs[r + 1] - xs[r - 1]) as f32)
                    - penalty[r]
                    - BB * ((ys[si[r] + 1] - ys[si[r] - 1]) as f32)
                    - ddc * (disty[r] as f32)
            };
            nbp += 1;

            score[nbp] = if su(r) <= 1 {
                ((ys[r + 1] - ys[r - 1]) as f32)
                    - penalty[su(r)]
                    - AA * ((xs[2] - xs[1]) as f32)
                    - ddc * (distx[r] as f32)
            } else if su(r) >= d - 2 {
                ((ys[r + 1] - ys[r - 1]) as f32)
                    - penalty[su(r)]
                    - AA * ((xs[d - 2] - xs[d - 3]) as f32)
                    - ddc * (distx[r] as f32)
            } else {
                ((ys[r + 1] - ys[r - 1]) as f32)
                    - penalty[su(r)]
                    - BB * ((xs[su(r) + 1] - xs[su(r) - 1]) as f32)
                    - ddc * (distx[r] as f32)
            };
            nbp += 1;
        }

        let newacc;
        if acc <= 3 {
            newacc = 1;
        } else {
            newacc = acc / 2;
            if acc >= nbp as i32 {
                acc = nbp as i32 - 1;
            }
        }

        let mut minl = i32::MAX;
        let mut bestt1 = Tree::default();
        let mut bestt2 = Tree::default();
        let mut bestbp = 0;
        for _ in 0..acc {
            let mut maxbp = 0;
            for bp in 1..nbp {
                if score[maxbp] < score[bp] {
                    maxbp = bp;
                }
            }
            score[maxbp] = -9e9f32;

            let p = break_pt(maxbp, lb);
            let t1;
            let t2;
            let mut ll;
            // Breaking in p
            if break_in_x(maxbp) {
                // break in x
                let mut n1 = 0;
                let mut n2 = 0;
                let mut nn1 = 0;
                let mut nn2 = 0;
                for r in 0..d {
                    if su(r) < p {
                        s1[n1] = s[r];
                        y1[n1] = ys[r];
                        n1 += 1;
                    } else if su(r) > p {
                        s2[n2] = s[r] - p as i32;
                        y2[n2] = ys[r];
                        n2 += 1;
                    } else {
                        // if (s[r] == p)  i.e.,  r = si[p]
                        s1[n1] = p as i32;
                        s2[n2] = 0;
                        y1[n1] = ys[r];
                        y2[n2] = ys[r];
                        nn1 = n1;
                        nn2 = n2;
                        n1 += 1;
                        n2 += 1;
                    }
                }

                t1 = self.flutes(p + 1, xs, &y1, &s1, newacc);
                t2 = self.flutes(d - p, &xs[p..], &y2, &s2, newacc);
                ll = t1.length + t2.length;
                let coord1 = t1.branch[t1.branch[nn1].n as usize].y;
                let coord2 = t2.branch[t2.branch[nn2].n as usize].y;
                if t2.branch[nn2].y > coord1.max(coord2) {
                    ll -= t2.branch[nn2].y - coord1.max(coord2);
                } else if t2.branch[nn2].y < coord1.min(coord2) {
                    ll -= coord1.min(coord2) - t2.branch[nn2].y;
                }
            } else {
                // if (!BreakInX(maxbp))
                let mut n1 = 0;
                let mut n2 = 0;
                for r in 0..d {
                    if si[r] < p {
                        s1[si[r]] = n1;
                        x1[n1 as usize] = xs[r];
                        n1 += 1;
                    } else if si[r] > p {
                        s2[si[r] - p] = n2;
                        x2[n2 as usize] = xs[r];
                        n2 += 1;
                    } else {
                        // if (si[r] == p)  i.e.,  r = s[p]
                        s1[p] = n1;
                        s2[0] = n2;
                        x1[n1 as usize] = xs[r];
                        x2[n2 as usize] = xs[r];
                        n1 += 1;
                        n2 += 1;
                    }
                }

                t1 = self.flutes(p + 1, &x1, ys, &s1, newacc);
                t2 = self.flutes(d - p, &x2, &ys[p..], &s2, newacc);
                ll = t1.length + t2.length;
                let coord1 = t1.branch[t1.branch[p].n as usize].x;
                let coord2 = t2.branch[t2.branch[0].n as usize].x;
                if t2.branch[0].x > coord1.max(coord2) {
                    ll -= t2.branch[0].x - coord1.max(coord2);
                } else if t2.branch[0].x < coord1.min(coord2) {
                    ll -= coord1.min(coord2) - t2.branch[0].x;
                }
            }
            if minl > ll {
                minl = ll;
                bestt1 = t1;
                bestt2 = t2;
                bestbp = maxbp;
            }
        }

        if break_in_x(bestbp) {
            hmergetree(&bestt1, &bestt2, s)
        } else {
            vmergetree(&bestt1, &bestt2)
        }
    }
}

/// `BreakPt`.
fn break_pt(bp: usize, lb: usize) -> usize {
    bp / 2 + lb
}

/// `BreakInX`.
fn break_in_x(bp: usize) -> bool {
    bp.is_multiple_of(2)
}

/// `dmergetree`.
fn dmergetree(t1: &Tree, t2: &Tree) -> Tree {
    let d = t1.deg + t2.deg - 2;
    let mut t = Tree {
        deg: d,
        length: t1.length + t2.length,
        branch: vec![Branch::default(); (2 * d - 2) as usize],
    };
    let offset1 = t2.deg - 2;
    let offset2 = 2 * t1.deg - 4;
    let du = d as usize;
    let t1deg = t1.deg as usize;

    for i in 0..=t1deg - 2 {
        t.branch[i] = shifted(t1.branch[i], offset1);
    }
    for i in t1deg - 1..=du - 1 {
        t.branch[i] = shifted(t2.branch[i + 2 - t1deg], offset2);
    }
    for i in du..=du + t1deg - 3 {
        t.branch[i] = shifted(t1.branch[(i as i32 - offset1) as usize], offset1);
    }
    for i in du + t1deg - 2..=2 * du - 3 {
        t.branch[i] = shifted(t2.branch[(i as i32 - offset2) as usize], offset2);
    }

    let mut prev = t2.branch[0].n + offset2;
    let mut curr = t1.branch[t1deg - 1].n + offset1;
    let mut next = t.branch[curr as usize].n;
    while curr != next {
        t.branch[curr as usize].n = prev;
        prev = curr;
        curr = next;
        next = t.branch[curr as usize].n;
    }
    t.branch[curr as usize].n = prev;

    t
}

/// `hmergetree`.
fn hmergetree(t1: &Tree, t2: &Tree, s: &[i32]) -> Tree {
    let deg = t1.deg + t2.deg - 1;
    let mut t = Tree {
        deg,
        length: t1.length + t2.length,
        branch: vec![Branch::default(); (2 * deg - 2) as usize],
    };
    let offset1 = t2.deg - 1;
    let offset2 = 2 * t1.deg - 3;
    let degu = deg as usize;
    let t1deg = t1.deg as usize;

    let p = t1.deg - 1;
    let mut n1 = 0;
    let mut n2 = 0;
    let mut nn1 = 0;
    let mut nn2 = 0;
    let mut ii = 0;
    for (i, &si) in s.iter().enumerate().take(degu) {
        if si < p {
            t.branch[i] = shifted(t1.branch[n1], offset1);
            n1 += 1;
        } else if si > p {
            t.branch[i] = shifted(t2.branch[n2], offset2);
            n2 += 1;
        } else {
            t.branch[i] = shifted(t2.branch[n2], offset2);
            nn1 = n1;
            nn2 = n2;
            ii = i;
            n1 += 1;
            n2 += 1;
        }
    }
    for i in degu..=degu + t1deg - 3 {
        t.branch[i] = shifted(t1.branch[(i as i32 - offset1) as usize], offset1);
    }
    for i in degu + t1deg - 2..=2 * degu - 4 {
        t.branch[i] = shifted(t2.branch[(i as i32 - offset2) as usize], offset2);
    }
    let extra = 2 * degu - 3;
    let coord1 = t1.branch[t1.branch[nn1].n as usize].y;
    let coord2 = t2.branch[t2.branch[nn2].n as usize].y;
    if t2.branch[nn2].y > coord1.max(coord2) {
        t.branch[extra].y = coord1.max(coord2);
        t.length -= t2.branch[nn2].y - t.branch[extra].y;
    } else if t2.branch[nn2].y < coord1.min(coord2) {
        t.branch[extra].y = coord1.min(coord2);
        t.length -= t.branch[extra].y - t2.branch[nn2].y;
    } else {
        t.branch[extra].y = t2.branch[nn2].y;
    }
    t.branch[extra].x = t2.branch[nn2].x;
    t.branch[extra].n = t.branch[ii].n;
    t.branch[ii].n = extra as i32;

    let mut prev = extra as i32;
    let mut curr = t1.branch[nn1].n + offset1;
    let mut next = t.branch[curr as usize].n;
    while curr != next {
        t.branch[curr as usize].n = prev;
        prev = curr;
        curr = next;
        next = t.branch[curr as usize].n;
    }
    t.branch[curr as usize].n = prev;

    t
}

/// `vmergetree`.
fn vmergetree(t1: &Tree, t2: &Tree) -> Tree {
    let deg = t1.deg + t2.deg - 1;
    let mut t = Tree {
        deg,
        length: t1.length + t2.length,
        branch: vec![Branch::default(); (2 * deg - 2) as usize],
    };
    let offset1 = t2.deg - 1;
    let offset2 = 2 * t1.deg - 3;
    let degu = deg as usize;
    let t1deg = t1.deg as usize;

    for i in 0..=t1deg - 2 {
        t.branch[i] = shifted(t1.branch[i], offset1);
    }
    for i in t1deg - 1..=degu - 1 {
        t.branch[i] = shifted(t2.branch[i + 1 - t1deg], offset2);
    }
    for i in degu..=degu + t1deg - 3 {
        t.branch[i] = shifted(t1.branch[(i as i32 - offset1) as usize], offset1);
    }
    for i in degu + t1deg - 2..=2 * degu - 4 {
        t.branch[i] = shifted(t2.branch[(i as i32 - offset2) as usize], offset2);
    }
    let extra = 2 * degu - 3;
    let coord1 = t1.branch[t1.branch[t1deg - 1].n as usize].x;
    let coord2 = t2.branch[t2.branch[0].n as usize].x;
    if t2.branch[0].x > coord1.max(coord2) {
        t.branch[extra].x = coord1.max(coord2);
        t.length -= t2.branch[0].x - t.branch[extra].x;
    } else if t2.branch[0].x < coord1.min(coord2) {
        t.branch[extra].x = coord1.min(coord2);
        t.length -= t.branch[extra].x - t2.branch[0].x;
    } else {
        t.branch[extra].x = t2.branch[0].x;
    }
    t.branch[extra].y = t2.branch[0].y;
    t.branch[extra].n = t.branch[t1deg - 1].n;
    t.branch[t1deg - 1].n = extra as i32;

    let mut prev = extra as i32;
    let mut curr = t1.branch[t1deg - 1].n + offset1;
    let mut next = t.branch[curr as usize].n;
    while curr != next {
        t.branch[curr as usize].n = prev;
        prev = curr;
        curr = next;
        next = t.branch[curr as usize].n;
    }
    t.branch[curr as usize].n = prev;

    t
}

/// A branch with its neighbor index moved by `offset`, as the merges copy them.
fn shifted(branch: Branch, offset: i32) -> Branch {
    Branch {
        x: branch.x,
        y: branch.y,
        n: branch.n + offset,
    }
}
