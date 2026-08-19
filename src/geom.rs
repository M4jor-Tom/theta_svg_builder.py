//! Number formatting and the shared hexagon grid.
use std::f64::consts::PI;

/// `math.sqrt(3)`, as a literal because `f64::sqrt` is not const. Pinned by a
/// test against `3f64.sqrt()`.
pub const SQRT3: f64 = 1.732_050_807_568_877_2;

/// Neighbour (dr, dc) per edge k (edge normal at 60*k deg), by row parity.
pub const NB: [[(i32, i32); 6]; 2] = [
    [(0, 1), (1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0)],
    [(0, 1), (1, 1), (1, 0), (0, -1), (-1, 0), (-1, 1)],
];

/// Two decimals, trailing zeros stripped, and never a negative zero: Python
/// reaches the integer branch through `int(round(x, 2))`, which turns `-0.0`
/// into `0`. Formatting once and trimming gives the same answer as Python's
/// round-then-format without a second rounding step to get wrong.
pub fn fmt(x: f64) -> String {
    let s = format!("{x:.2}");
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-0" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

/// A point list as an SVG `points` attribute.
pub fn pts(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{},{}", fmt(*x), fmt(*y)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `n` vertices of a regular polygon, starting at `rot`.
pub fn regular_poly(cx: f64, cy: f64, r: f64, n: usize, rot: f64) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let a = rot + 2.0 * PI * i as f64 / n as f64;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// The hexagon grid every stage works from -- renderer and asserts alike, so
/// they cannot drift apart. Density is constant across resolutions because the
/// cell size is tied to min(w, h).
pub struct Lattice {
    pub u: f64,
    pub s: f64,
    pub clear_r: f64,
    pub hexes: Vec<(i32, i32)>,
    pub cx0: f64,
    pub cy0: f64,
    d: f64,
    rowh: f64,
}

impl Lattice {
    pub fn new(w: u32, h: u32) -> Self {
        let (wf, hf) = (w as f64, h as f64);
        let u = wf.min(hf);
        let s = u / 9.0;
        let d = 2.0 * s;
        let rowh = d * SQRT3 / 2.0;
        let rows = (hf / rowh) as i32 + 2;
        let cols = (wf / d) as i32 + 2;
        let hexes = (-1..rows)
            .flat_map(|r| (-1..cols).map(move |c| (r, c)))
            .collect();
        Self {
            u,
            s,
            clear_r: u * 0.28,
            hexes,
            cx0: wf / 2.0,
            cy0: hf / 2.0,
            d,
            rowh,
        }
    }

    /// Odd rows are offset by half a cell. `r` starts at -1, so the parity has
    /// to be `rem_euclid`: Rust's `%` would offset row -1 the wrong way.
    pub fn center(&self, r: i32, c: i32) -> (f64, f64) {
        (
            c as f64 * self.d + r.rem_euclid(2) as f64 * self.d / 2.0,
            r as f64 * self.rowh,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Python's `fmt`: round to two decimals, then drop a trailing ".0" and any
    /// trailing zeros. Rendering `-0.00` or `-0` would put a byte in the file
    /// that the Python never wrote -- `int(-0.0)` is `0`.
    #[test]
    fn fmt_matches_python() {
        for (x, want) in [
            (1920.0, "1920"),
            (0.0, "0"),
            (-0.0, "0"),
            (-0.001, "0"),
            (0.1, "0.1"),
            (2.5, "2.5"),
            (-1.5, "-1.5"),
            (0.125, "0.12"),
            (0.375, "0.38"),
            (768.0, "768"),
            (1.0 / 3.0, "0.33"),
        ] {
            assert_eq!(fmt(x), want, "fmt({x})");
        }
    }

    #[test]
    fn sqrt3_is_the_same_double_python_computes() {
        assert_eq!(SQRT3, 3f64.sqrt());
    }

    /// `r % 2` in `center` runs with r = -1, where Python returns 1 and Rust's
    /// `%` returns -1. Row -1 must be offset like every other odd row.
    #[test]
    fn odd_row_offset_survives_negative_rows() {
        let lat = Lattice::new(1920, 1080);
        let (x_neg, _) = lat.center(-1, 0);
        let (x_pos, _) = lat.center(1, 0);
        assert_eq!(x_neg, x_pos);
        assert!(x_neg > 0.0, "an odd row is offset by half a cell");
    }
}
