//! Document assembly: the `<svg>` header, `<defs>`, the stylesheet, and the
//! three content slots (background pattern, rain overlay, centre glyph).
use crate::geom::{Lattice, fmt};
use crate::params::{Glyph, Scene, background};
use crate::rng::PyRandom;
use crate::style::{PAL, css};
use crate::trihex::pat_trihex;

/// Port of `background.py:548-594`. Layout depends only on `scene.seed`, never
/// on motion/image/glyph/overlay -- the RNG is seeded first for exactly that
/// reason.
pub fn build_svg(w: u32, h: u32, scene: &Scene) -> String {
    let mut rng = PyRandom::new(&format!("trihex:{}", scene.seed));
    let lat = Lattice::new(w, h);
    let (u, clear_r) = (lat.u, lat.clear_r);

    let mut defs = vec![
        // userSpaceOnUse so any shape can paint canvas: the default objectBoundingBox
        // would squeeze the whole ramp into a single hexagon, and a closed blind
        // (background.motion CLOSEOPEN) would read as a patch instead of vanishing into the page.
        format!(
            "<linearGradient id=\"bg\" gradientUnits=\"userSpaceOnUse\" x1=\"0\" y1=\"0\" \
             x2=\"{}\" y2=\"{}\"><stop offset=\"0%\" stop-color=\"{}\"/>\
             <stop offset=\"100%\" stop-color=\"{}\"/></linearGradient>",
            fmt(w as f64 * 0.4),
            fmt(h as f64),
            PAL.bg.0,
            PAL.bg.1
        ),
        "<radialGradient id=\"vig\"><stop offset=\"55%\" stop-color=\"#8fa3b8\" stop-opacity=\"0\"/>\
         <stop offset=\"100%\" stop-color=\"#8fa3b8\" stop-opacity=\"0.16\"/></radialGradient>"
            .to_string(),
        format!(
            "<radialGradient id=\"halo\"><stop offset=\"0%\" stop-color=\"{0}\" stop-opacity=\"0.92\"/>\
             <stop offset=\"55%\" stop-color=\"{0}\" stop-opacity=\"0.65\"/>\
             <stop offset=\"100%\" stop-color=\"{0}\" stop-opacity=\"0\"/></radialGradient>",
            PAL.bg.0
        ),
        "<filter id=\"ink\" x=\"-30%\" y=\"-30%\" width=\"160%\" height=\"160%\">\
         <feDropShadow dx=\"0\" dy=\"2\" stdDeviation=\"3\" flood-color=\"#1e293b\" flood-opacity=\"0.25\"/></filter>"
            .to_string(),
    ];
    if scene.image == background::Image::Starfield {
        for (gid, col) in [("neba", "#6fb7d1"), ("nebb", "#77c9a6")] {
            defs.push(format!(
                "<radialGradient id=\"{gid}\"><stop offset=\"0%\" stop-color=\"{col}\" stop-opacity=\"0.3\"/>\
                 <stop offset=\"100%\" stop-color=\"{col}\" stop-opacity=\"0\"/></radialGradient>"
            ));
        }
    }

    let bg_svg = pat_trihex(&lat, &mut rng);
    let rain_svg = String::new();
    let k = u * 0.34 / 200.0;
    let glyph = String::new();
    let (ws, hs) = (fmt(w as f64), fmt(h as f64));
    let (cx, cy) = (fmt(w as f64 / 2.0), fmt(h as f64 / 2.0));
    let icon_svg = format!(
        "<g transform=\"translate({cx},{cy}) scale({})\" filter=\"url(#ink)\">{glyph}</g>",
        fmt(k)
    );

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
    let defs = defs.join("");
    let css = css();
    let clear_r = fmt(clear_r);

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {ws} {hs}\" width=\"{ws}\" height=\"{hs}\" \
         preserveAspectRatio=\"xMidYMid slice\" role=\"img\" aria-label=\"{label}\"><title>{label}</title>\
         <defs>{defs}</defs>{css}\
         <rect width=\"{ws}\" height=\"{hs}\" fill=\"url(#bg)\"/>\
         <g>{bg_svg}</g>{rain_svg}\
         <circle cx=\"{cx}\" cy=\"{cy}\" r=\"{clear_r}\" fill=\"url(#halo)\"/>\
         <rect width=\"{ws}\" height=\"{hs}\" fill=\"url(#vig)\"/>{icon_svg}</svg>\n"
    )
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
