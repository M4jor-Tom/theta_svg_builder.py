//! Document assembly: the `<svg>` header, `<defs>`, the stylesheet, and the
//! three content slots (background pattern, rain overlay, centre glyph).
use askama::Template;

use crate::geom::{Lattice, fmt};
use crate::icon::{ico_hexatri, ico_ship};
use crate::matrix::pat_matrix;
use crate::params::{Glyph, Scene, background};
use crate::rng::PyRandom;
use crate::style::{PAL, css};
use crate::trihex::pat_trihex;

/// The template context for `defs.svg`: the gradients and filter every render
/// needs, plus the starfield nebula pair. `w04`/`h`/`bg0`/`bg1` feed the `bg`
/// gradient, which is `userSpaceOnUse` so any shape can paint canvas -- the
/// default `objectBoundingBox` would squeeze the whole ramp into a single
/// hexagon, and a closed blind (`background.motion` CLOSEOPEN) would read as
/// a patch instead of vanishing into the page. `nebulae` is empty unless
/// `scene.image` is `Starfield`, and that alone decides whether the template
/// emits them -- there is no separate `starfield: bool` next to it, because a
/// bool that must agree with the vec's length is a second way to say the same
/// thing, and the two could drift out of sync.
#[derive(askama::Template)]
#[template(path = "defs.svg")]
struct Defs {
    w04: String,
    h: String,
    bg0: &'static str,
    bg1: &'static str,
    nebulae: Vec<(&'static str, &'static str)>,
}

/// The template context for `root.svg`: the document shell wrapped around the
/// three content slots (`bg`, `rain`, `glyph`) that the pattern, overlay and
/// icon builders already reduced to complete markup. Every field is a
/// `String` (or `&'static str`) Rust finished assembling before the struct
/// existed -- the template only places them, it never formats a number,
/// derives a unit or picks a sign.
///
/// `templates/root.svg` ends in two newlines, not one: askama always trims
/// exactly one trailing newline off a rendered template, so a file ending in
/// a single `\n` would render with none. The original `format!` ended its
/// output in `</svg>\n`, and the golden corpus is byte-exact against that, so
/// the second newline is load-bearing -- collapsing it back to one silently
/// drops the render's last byte.
///
/// Its `viewBox="0 0 {{+ ws +}} {{+ hs }}"` needs those `+` markers because
/// `whitespace = "suppress"` trims whitespace-only text next to `{{ }}`
/// expressions the same as it does around `{% %}` blocks -- without them the
/// literal spaces between "0 0", `ws` and `hs` vanish and the attribute reads
/// as `viewBox="0 019201080"`. Every other interpolation in this file and in
/// `defs.svg` sits directly against a quote, bracket or another tag, so this
/// is the one place that needs them.
#[derive(askama::Template)]
#[template(path = "root.svg")]
struct Root {
    ws: String,
    hs: String,
    cx: String,
    cy: String,
    clear_r: String,
    k: String,
    label: String,
    defs: String,
    css: String,
    bg: String,
    rain: String,
    glyph: String,
}

/// Port of `background.py:548-594`. Layout depends only on `scene.seed`, never
/// on motion/image/glyph/overlay -- the RNG is seeded first for exactly that
/// reason.
pub fn build_svg(w: u32, h: u32, scene: &Scene) -> String {
    let mut rng = PyRandom::new(&format!("trihex:{}", scene.seed));
    let lat = Lattice::new(w, h);
    let (u, clear_r) = (lat.u, lat.clear_r);

    let nebulae = if scene.image == background::Image::Starfield {
        vec![("neba", "#6fb7d1"), ("nebb", "#77c9a6")]
    } else {
        Vec::new()
    };
    let defs = Defs {
        w04: fmt(w as f64 * 0.4),
        h: fmt(h as f64),
        bg0: PAL.bg.0.as_str(),
        bg1: PAL.bg.1.as_str(),
        nebulae,
    }
    .render()
    .unwrap();

    let bg_svg = pat_trihex(w, h, &lat, scene, &mut rng);
    // between the pattern and the halo, so the halo subtracts the rain around
    // the icon exactly as it subtracts the lattice
    let rain_svg = scene.overlay.as_ref().map_or(String::new(), |rain| {
        pat_matrix(w, h, &lat, scene.seed, rain.angle, &rain.color)
    });
    let k = u * 0.34 / 200.0;
    let glyph = match scene.glyph {
        Glyph::Ship => ico_ship(),
        Glyph::Hexatri { rotate } => ico_hexatri(rotate),
    };
    let (ws, hs) = (fmt(w as f64), fmt(h as f64));
    let (cx, cy) = (fmt(w as f64 / 2.0), fmt(h as f64 / 2.0));

    let ship = matches!(scene.glyph, Glyph::Ship);
    let mut label = if ship {
        "trihexagonal background with a spaceship icon"
    } else {
        "trihexagonal background"
    }
    .to_string();
    if scene.image == background::Image::Starfield {
        label.push_str(if scene.motion == background::Motion::Closeopen {
            ", some hexagons opening and closing onto a starfield"
        } else {
            ", some hexagons showing a starfield"
        });
    }
    if scene.overlay.is_some() {
        label.push_str(", with streams of characters drifting across it");
    }

    Root {
        ws,
        hs,
        cx,
        cy,
        clear_r: fmt(clear_r),
        k: fmt(k),
        label,
        defs,
        css: css(),
        bg: bg_svg,
        rain: rain_svg,
        glyph,
    }
    .render()
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{parse, resolve};

    /// Everything before the first <rect> is the document shell: viewBox,
    /// label, defs and stylesheet. It is identical in every render of the same
    /// size, so a golden's prefix is the oracle for it.
    #[test]
    fn the_document_shell_matches_the_corpus() {
        let want = include_str!("../tests/data/shell.txt");
        let got = build_svg(1920, 1080, &resolve(&parse("{}").unwrap()));
        assert!(
            got.starts_with(want),
            "shell differs:\n{}",
            &got[..want.len().min(got.len())]
        );
    }
}
