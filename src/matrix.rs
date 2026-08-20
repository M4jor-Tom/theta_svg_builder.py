//! `overlay.matrix`: the character rain.
use std::fmt::Write as _;

use crate::geom::{Lattice, fmt};
use crate::rng::cell_rng;
use crate::style::{MATRIX_FRAC, MATRIX_GLYPHS, MATRIX_HEAD_STEP, MATRIX_KF, MATRIX_S, hex_rgba};

/// overlay.matrix: columns of characters that stay put while a lit head walks
/// down them at `angle` degrees (0 = downward, increasing clockwise).
///
/// NOTHING MOVES. Every cell of a column holds one character, chosen once and
/// fixed for good; what travels is the lighting. Each glyph runs the same
/// keyframes -- flare to --o as the head arrives, step down to --t, fade out
/// over the trail -- offset by one cell-time per cell, so the head keeps
/// advancing into a fresh character while the ones behind it dim in place.
/// Translating the glyphs instead would slide the characters across the canvas,
/// which is the one thing this effect must not do.
///
/// The band is the canvas seen from the travel direction -- P across, T along --
/// so no cell is ever placed somewhere the canvas cannot reach. Cells are placed
/// by rotating their band coordinates here rather than by wrapping them in a
/// rotated group: glyphs have to stay upright at every angle, so a rotated group
/// would need a counter-rotation on every single one of them. Rotating in Rust
/// leaves the output with no transform at all (~45 bytes a glyph, and at a steep
/// angle there are ~500 of them).
///
/// Each glyph's fill-opacity attribute holds the value its keyframes give it at
/// t=0, so prefers-reduced-motion freezes the true opening frame rather than
/// some other state: a still field of columns, each lit for the length of its
/// trail. The attribute loses to the animation while one is running.
///
/// Drawn under the halo, so the halo subtracts the rain around the icon the way
/// it subtracts the lattice -- the focal point stays uncontested for free.
///
/// Columns draw from cell_rng, off the global stream the lattice uses, so
/// switching the overlay on cannot move a hexagon. Keyed by slot rather than
/// draw order for the reason cell_rng exists: the slot count follows the angle,
/// so a shared stream would reshuffle every glyph when the rain is merely
/// re-aimed.
pub fn pat_matrix(w: u32, h: u32, lat: &Lattice, seed: u32, angle: f64, color: &str) -> String {
    let (rgb, alpha) = hex_rgba(color).expect("validate() already accepted the colour");
    let (hold, out_at) = MATRIX_KF;
    let rad = angle.to_radians();
    let (co, si) = (rad.cos(), rad.sin());
    // the band -- P across x, T along
    let (band_p, band_t) = (
        w as f64 * co.abs() + h as f64 * si.abs(),
        w as f64 * si.abs() + h as f64 * co.abs(),
    );
    let fs = lat.u / 28.0;
    let (pitch, step) = (fs * 1.6, fs * 1.05);
    let (cx0, cy0) = (lat.cx0, lat.cy0);
    let n = (band_t / step) as i64 + 2; // cells spanning the band, head to tail
    let mut out = format!(
        "<g class=\"matrix\" font-family=\"monospace\" font-size=\"{}\" \
         text-anchor=\"middle\" fill=\"{rgb}\" style=\"--o:{};--t:{}\">",
        fmt(fs),
        fmt(alpha),
        fmt(alpha * MATRIX_HEAD_STEP)
    );
    for i in 0..(band_p / pitch) as i32 + 1 {
        let mut g = cell_rng("rain", seed, 0, i);
        if g.random() > MATRIX_FRAC {
            continue;
        }
        let dx = i as f64 * pitch - band_p / 2.0;
        let dur = g.uniform(MATRIX_S.0, MATRIX_S.1);
        let head = g.randrange(n as u32) as i64; // the cell lit at t=0
        // --d is inherited by every glyph below, so a column sets its speed once
        // instead of repeating animation-duration on all N of them
        write!(out, "<g style=\"--d:{}s\">", fmt(dur)).expect("writing to a String");
        for j in 0..n {
            let dy = j as f64 * step - band_t / 2.0;
            // how far into its cycle this cell is -- `head - j` is negative for
            // most cells, and Python's `%` is never negative, so rem_euclid
            let pct = 100.0 * (head - j).rem_euclid(n) as f64 / n as f64;
            // ...evaluated against the keyframes,
            let o = if pct <= hold {
                alpha * (1.0 - (1.0 - MATRIX_HEAD_STEP) * pct / hold)
            } else if pct <= out_at {
                alpha * MATRIX_HEAD_STEP * (1.0 - (pct - hold) / (out_at - hold))
            } else {
                0.0
            };
            write!(
                out,
                "<text class=\"rain\" x=\"{}\" y=\"{}\" \
                 style=\"animation-delay:-{}s\" fill-opacity=\"{}\">{}</text>",
                fmt(cx0 + dx * co - dy * si),
                fmt(cy0 + dx * si + dy * co),
                fmt(dur * (head + n - j) as f64 / n as f64),
                fmt(o),
                g.choice(MATRIX_GLYPHS) as char
            )
            .expect("writing to a String");
        }
        out.push_str("</g>");
    }
    out.push_str("</g>");
    out
}

#[cfg(test)]
mod tests {
    use crate::geom::fmt;
    use crate::params::{Glyph, Rain, Scene, background};
    use crate::style::{MATRIX_COLOR, MATRIX_GLYPHS};
    use crate::svg::build_svg;

    /// `background.py`'s `build_svg(w, h, bg="lights", bg_image="space",
    /// seed=3, ...)`, with the overlay switched on or off.
    fn scene(overlay: Option<Rain>) -> Scene {
        Scene {
            seed: 3,
            motion: background::Motion::Lights,
            image: background::Image::Starfield,
            glyph: Glyph::Hexatri { rotate: true },
            overlay,
        }
    }

    /// Every `<polygon ...>` tag, as its raw slice: the lattice and its
    /// triangles, verbatim.
    fn polys(s: &str) -> Vec<&str> {
        s.match_indices("<polygon")
            .map(|(i, _)| &s[i..=i + s[i..].find('>').expect("a closing bracket")])
            .collect()
    }

    /// The inner source of every rain column -- the Rust of
    /// `re.findall(r'<g style="--d:[^"]*">(.*?)</g>', s)`.
    fn columns(s: &str) -> Vec<&str> {
        s.match_indices("<g style=\"--d:")
            .map(|(i, _)| {
                let rest = &s[i..];
                let open = rest.find("\">").expect("an open tag") + 2;
                let close = rest.find("</g>").expect("a closing tag");
                &rest[open..close]
            })
            .collect()
    }

    /// The single character each `<text>` carries.
    fn glyphs(s: &str) -> Vec<u8> {
        s.match_indices("</text>")
            .map(|(i, _)| s.as_bytes()[i - 1])
            .collect()
    }

    /// Every number introduced by `key` and terminated by `end`, in document
    /// order.
    fn nums(s: &str, key: &str, end: char) -> Vec<f64> {
        s.match_indices(key)
            .map(|(i, _)| {
                let rest = &s[i + key.len()..];
                rest[..rest.find(end).expect("a terminator")]
                    .parse()
                    .expect("a number")
            })
            .collect()
    }

    /// The characters must stay put while the lighting walks the column, stay
    /// upright at any angle, and leave the lattice byte-identical: the overlay
    /// is a layer, never a layout input.
    #[test]
    fn the_rain_lights_a_column_without_moving_anything() {
        for (w, h) in [(1920u32, 1080u32), (1080, 1920)] {
            let plain = build_svg(w, h, &scene(None));
            for angle in [0.0, 90.0, 181.0, 359.5] {
                let svg = build_svg(
                    w,
                    h,
                    &scene(Some(Rain {
                        angle,
                        color: MATRIX_COLOR.to_string(),
                    })),
                );
                assert_eq!(
                    polys(&svg),
                    polys(&plain),
                    "the overlay moved the lattice at {} deg",
                    fmt(angle)
                );

                // Cells are rotated into place in Rust, so glyphs are upright
                // at every angle for free. A transform creeping back in means
                // either tilted characters or ~45 wasted bytes on each of ~500
                // of them.
                // ...up to the halo circle, which is the first `<circle` AFTER
                // the layer: a starfield's stars are circles too, and Python's
                // `svg.index("<circle")` finds one of those and slices
                // backwards to an empty -- so vacuously clean -- string.
                let start = svg.find("class=\"matrix\"").expect("the rain layer");
                let rest = &svg[start..];
                let layer = &rest[..rest.find("<circle").expect("the halo")];
                assert!(
                    !layer.contains("transform"),
                    "a rain glyph carries a transform at {} deg",
                    fmt(angle)
                );
                let glyphs = glyphs(&svg);
                assert!(
                    !glyphs.is_empty() && glyphs.iter().all(|c| MATRIX_GLYPHS.contains(c)),
                    "a glyph is off the set"
                );

                let cols = columns(&svg);
                assert!(!cols.is_empty(), "no rain columns were placed");
                let cells = cols[0].matches("<text").count();
                assert!(
                    cols.iter().all(|col| col.matches("<text").count() == cells),
                    "every column must span the whole band, or a head would stop mid-canvas"
                );
                for col in &cols {
                    let o = nums(col, "fill-opacity=\"", '"');
                    let lit = o.iter().filter(|v| **v != 0.0).count();
                    assert!(
                        0 < lit && lit < o.len(),
                        "a column must be part lit and part dark -- that gap is the trail"
                    );
                    let top = o.iter().copied().fold(f64::MIN, f64::max);
                    assert_eq!(
                        o.iter().filter(|v| **v == top).count(),
                        1,
                        "a column must have exactly one head"
                    );
                    // Walking back from the head must give one contiguous run
                    // that only ever dims. Anything else means the stagger is
                    // not advancing the lighting a single cell at a time.
                    let head = o.iter().position(|v| *v == top).expect("the head");
                    let n = o.len() as i64;
                    let run: Vec<f64> = (0..lit as i64)
                        .map(|k| o[(head as i64 - k).rem_euclid(n) as usize])
                        .collect();
                    assert!(
                        run.iter().all(|v| *v != 0.0) && run.windows(2).all(|w| w[0] >= w[1]),
                        "the trail must be one contiguous run fading back from the head"
                    );
                }

                // The stagger IS the travel: consecutive cells are one
                // cell-time apart, so the lighting advances by exactly one
                // character per step. Collapse these to a single delay and the
                // whole column flashes at once instead.
                let d = nums(cols[0], "animation-delay:-", 's');
                let gaps: Vec<f64> = d.windows(2).map(|w| w[0] - w[1]).collect();
                let lo = gaps.iter().copied().fold(f64::MAX, f64::min);
                let hi = gaps.iter().copied().fold(f64::MIN, f64::max);
                assert!(
                    lo > 0.0 && hi - lo <= 0.02,
                    "glyph delays must step by one constant cell-time down the column"
                );

                // The characters are anchored by x/y and only their opacity is
                // animated. An animated transform here would slide them across
                // the canvas, which is the one thing this effect must not do.
                let kf = &svg[svg.find("@keyframes rain{").expect("the rain keyframes")..];
                assert!(
                    !kf[..kf.find("}}").expect("the end of the rule")].contains("translate"),
                    "the rain keyframes must animate opacity only -- characters never move"
                );
                assert!(
                    svg.contains(".rain{animation:rain var(--d) linear infinite}"),
                    "columns must take their speed from the inherited --d"
                );
            }

            // The colour is split into fill + alpha (the trail scales it)
            // rather than passed through as an 8-digit hex, and the alpha
            // reaches the glyphs as --o.
            let svg = build_svg(
                w,
                h,
                &Scene {
                    seed: 0,
                    motion: background::Motion::Static,
                    image: background::Image::None,
                    glyph: Glyph::Hexatri { rotate: true },
                    overlay: Some(Rain {
                        angle: 0.0,
                        color: "#8899aa80".to_string(),
                    }),
                },
            );
            assert!(
                svg.contains("fill=\"#8899aa\"") && !svg.contains("#8899aa80"),
                "overlay.matrix.color is not applied"
            );
            assert!(
                svg.contains(&format!("--o:{};", fmt(0x80 as f64 / 255.0))),
                "the colour's alpha never reaches the glyphs"
            );
        }
    }
}
