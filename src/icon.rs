//! The centre glyph: the hexagon/triangle badge and the ship.
use std::f64::consts::PI;

use crate::geom::{fmt, pts, regular_poly};
use crate::style::PAL;

/// One ring of `ico_hexatri`'s badge: radius, sides, start angle, colour,
/// stroke width, stroke opacity, rotate-origin-y%.
type Ring = (f64, usize, f64, &'static str, f64, f64, Option<f64>);

/// Nested hexagon<->triangle glyph. `rotate` -> the two triangle rings
/// counter-spin inside the static hex frame (rotation centres pinned to the
/// icon centre via transform-origin, so no wobble).
pub fn ico_hexatri(rotate: bool) -> String {
    let (a, b) = (PAL.a.as_str(), PAL.b.as_str());
    let rings: [Ring; 4] = [
        (88.0, 6, PI / 6.0, a, 3.6, 1.0, None),
        (80.0, 3, -PI / 2.0, b, 3.0, 1.0, Some(66.7)), // up triangle
        (48.0, 6, PI / 6.0, a, 2.6, 0.8, None),
        (42.0, 3, PI / 2.0, b, 2.4, 0.8, Some(33.3)), // down triangle
    ];
    let mut parts = String::new();
    for (idx, (r, n, rot, col, swd, o, oy)) in rings.into_iter().enumerate() {
        let mut attr = String::new();
        if let Some(oy) = oy.filter(|_| rotate) {
            let cls = if idx == 1 { "rspin" } else { "spin" };
            let dur = 24 - idx * 3;
            attr = format!(
                " class=\"{cls}\" style=\"animation-duration:{dur}s;transform-origin:50% {oy}%\""
            );
        }
        parts.push_str(&format!(
            "<polygon{attr} points=\"{}\" fill=\"none\" stroke=\"{col}\" stroke-width=\"{}\" \
             stroke-opacity=\"{}\"/>",
            pts(&regular_poly(0.0, 0.0, r, n, rot)),
            fmt(swd),
            fmt(o)
        ));
    }
    parts.push_str(&format!(
        "<polygon points=\"{}\" fill=\"{a}\" fill-opacity=\"0.28\" stroke=\"{a}\" stroke-width=\"2\"/>",
        pts(&regular_poly(0.0, 0.0, 16.0, 6, PI / 6.0))
    ));
    parts
}

/// Cloaked delta spaceship on the same 200-unit grid as `ico_hexatri`: a sheet
/// folded along its spine, read as four facets rather than as an outline. Same
/// silhouette as ever -- the facets tile the hull quad exactly, so nothing
/// about the footprint moved. No rotate variant: a spinning ship reads as a
/// crash.
///
/// RELIEF IS VALUE, NOT LINEWORK. The four facets meet along the spine and
/// carry nothing but a fill: lit wing, lit ridge face, shadowed ridge face,
/// shadowed wing, stepped 0.04 -> 0.28. Stroking the interior folds as well
/// would read as wireframe -- an object seen through, not a solid seen lit --
/// so the folds are left to the value step alone, and only the silhouette
/// keeps its outline.
///
/// ONE LIGHT, NOT FOUR. Every facet draws the same ramp, transparent at the
/// upper left and opaque at the lower right, i.e. lit from where the canvas
/// gradient is already brightest. That takes gradientUnits="userSpaceOnUse"
/// for the same reason #bg does: the default objectBoundingBox restarts the
/// ramp inside every triangle, which gives four separately-lit shards instead
/// of one solid under one light, and the fold stops reading.
///
/// THE CLOAK IS THE TRANSLUCENCY. Nothing here reaches 0.3, so the halo and
/// the lattice come through the hull. That also keeps the glyph inside the
/// contrast budget: filled, but still spending less ink than the old double
/// outline.
///
/// The crest highlight is the one bright element, canvas colour fading aft.
/// On a high-key page a highlight cannot out-light the paper, so it reads
/// only where it lies over the shadowed ridge face -- light added by removing
/// tint, the same inversion the rain's head uses. The ridge stays 35% of the
/// half-span wide, so the narrow-shape-against-wide-shape rule still holds --
/// the narrow shape is now a fold instead of a second outline. Any narrower
/// and its two faces are slivers rather than planes, and the fold stops
/// reading as one.
pub fn ico_ship() -> String {
    let (a, b, lit) = (PAL.a.as_str(), PAL.b.as_str(), PAL.bg.0.as_str());
    let (n, r, l, t) = ((0.0, -80.0), (68.0, 46.0), (-68.0, 46.0), (0.0, 20.0));
    let (kl, kr) = ((-23.8, 29.1), (23.8, 29.1)); // ridge feet, 35% along the trailing edges
    // One source for the crest, so its fade cannot stop landing on its own
    // ends. It starts short of the nose: the hull stroke overdraws the apex,
    // so the highlight emerges from under the point instead of fighting it
    // for the pixel.
    let (cy0, cy1) = (n.1 + 6.0, t.1);
    // The ramp is deliberately shallow (0.5 -> 1, not 0 -> 1): it is the
    // shading *within* a plane, and a wide one drowns the step *between*
    // planes, which is the thing actually doing the relief. Four gently-lit
    // shards read as one flat smear; four flat-ish planes at four values read
    // as a fold.
    let ramp: String = [("a", a), ("b", b)]
        .into_iter()
        .map(|(k, col)| {
            format!(
                "<linearGradient id=\"shp{k}\" gradientUnits=\"userSpaceOnUse\" x1=\"-70\" \
                 y1=\"-80\" x2=\"70\" y2=\"60\"><stop offset=\"0%\" stop-color=\"{col}\" \
                 stop-opacity=\"0.5\"/><stop offset=\"100%\" stop-color=\"{col}\"/></linearGradient>"
            )
        })
        .collect();
    let mut parts = format!(
        "<defs>{ramp}<linearGradient id=\"shpe\" gradientUnits=\"userSpaceOnUse\" x1=\"0\" \
         y1=\"{}\" x2=\"0\" y2=\"{}\"><stop offset=\"0%\" stop-color=\"{lit}\" \
         stop-opacity=\"0.95\"/><stop offset=\"100%\" stop-color=\"{lit}\" \
         stop-opacity=\"0\"/></linearGradient></defs>",
        fmt(cy0),
        fmt(cy1)
    );
    for (facet, k, v) in [
        ([n, kl, l], "a", 0.09),
        ([n, t, kl], "b", 0.04),
        ([n, kr, t], "b", 0.28),
        ([n, r, kr], "a", 0.22),
    ] {
        parts.push_str(&format!(
            "<polygon points=\"{}\" fill=\"url(#shp{k})\" fill-opacity=\"{}\"/>",
            pts(&facet),
            fmt(v)
        ));
    }
    parts.push_str(&format!(
        "<line x1=\"0\" y1=\"{}\" x2=\"0\" y2=\"{}\" stroke=\"url(#shpe)\" stroke-width=\"2.2\"/>",
        fmt(cy0),
        fmt(cy1)
    ));
    parts.push_str(&format!(
        "<polygon points=\"{}\" fill=\"none\" stroke=\"{a}\" stroke-width=\"3.6\"/>",
        pts(&[n, r, t, l])
    ));
    parts.push_str(&format!(
        "<polygon points=\"{}\" fill=\"{a}\" fill-opacity=\"0.28\" stroke=\"{a}\" stroke-width=\"2\"/>",
        pts(&regular_poly(0.0, -22.0, 8.0, 6, PI / 6.0))
    ));
    // exhaust, aft of the wing roots
    for (i, (hw, y)) in [(14, 42), (8, 54)].into_iter().enumerate() {
        let neg_hw = -hw;
        parts.push_str(&format!(
            "<line x1=\"{neg_hw}\" y1=\"{y}\" x2=\"{hw}\" y2=\"{y}\" stroke=\"{b}\" \
             stroke-width=\"2.4\" stroke-opacity=\"{}\"/>",
            fmt(0.6 - i as f64 * 0.28)
        ));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pull one attribute's value out of a `<polygon .../>` (or `<line
    /// .../>`) tag fragment. The glyph's own format is the only grammar these
    /// tests need to know.
    fn attr<'a>(tag: &'a str, name: &str) -> &'a str {
        let key = format!("{name}=\"");
        let start = tag.find(&key).expect("attribute present") + key.len();
        let rest = &tag[start..];
        &rest[..rest.find('"').expect("closing quote")]
    }

    fn parse_points(s: &str) -> Vec<(f64, f64)> {
        s.split_whitespace()
            .map(|p| {
                let (x, y) = p.split_once(',').expect("point is x,y");
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect()
    }

    /// Every `<polygon ...>` tag in the glyph, as its raw source slice.
    fn polygons(glyph: &str) -> Vec<&str> {
        glyph
            .match_indices("<polygon ")
            .map(|(i, _)| {
                let end = glyph[i..].find("/>").expect("self-closing tag") + i;
                &glyph[i..end]
            })
            .collect()
    }

    /// The Rust of `background.py:947-950`: the shoelace formula.
    fn area(poly: &[(f64, f64)]) -> f64 {
        let n = poly.len();
        let sum: f64 = (0..n)
            .map(|i| {
                let (x, y) = poly[i];
                let (xp, yp) = poly[(i + n - 1) % n];
                x * yp - xp * y
            })
            .sum();
        sum.abs() / 2.0
    }

    /// The four shaded facets: the Rust of `background.py:953`.
    fn facet_polys(glyph: &str) -> Vec<(Vec<(f64, f64)>, f64)> {
        polygons(glyph)
            .into_iter()
            .filter(|tag| {
                tag.contains("fill=\"url(#shpa)\"") || tag.contains("fill=\"url(#shpb)\"")
            })
            .map(|tag| {
                (
                    parse_points(attr(tag, "points")),
                    attr(tag, "fill-opacity").parse().expect("a number"),
                )
            })
            .collect()
    }

    /// The one silhouette outline: the Rust of `background.py:958-959`.
    fn hull_poly(glyph: &str) -> Vec<(f64, f64)> {
        let hulls: Vec<&str> = polygons(glyph)
            .into_iter()
            .filter(|tag| tag.contains("fill=\"none\""))
            .collect();
        assert_eq!(
            hulls.len(),
            1,
            "the ship needs exactly one silhouette outline"
        );
        parse_points(attr(hulls[0], "points"))
    }

    /// The ship must read as a folded solid: four facets that tile the hull
    /// exactly (so the cloak never moved the silhouette), no two at the same
    /// value (so every fold has relief), none opaque (so it stays a cloak), and
    /// every ramp spanning the glyph rather than restarting inside a facet.
    ///
    /// Asserted against the glyph alone, never the page: the lattice is
    /// thousands of polygons that would match these patterns by coincidence.
    #[test]
    fn the_ship_is_a_folded_solid() {
        let glyph = ico_ship();
        let facets = facet_polys(&glyph);
        assert_eq!(facets.len(), 4, "the hull needs four facets to fold");
        let vals: Vec<f64> = facets.iter().map(|(_, v)| *v).collect();
        for (i, v) in vals.iter().enumerate() {
            assert!(*v < 0.3, "a facet is too opaque to read as a cloak");
            assert!(
                vals[i + 1..].iter().all(|w| w != v),
                "two facets share a value, so that fold has no relief"
            );
        }
        let hull = hull_poly(&glyph);
        let tiled: f64 = facets.iter().map(|(p, _)| area(p)).sum();
        assert!(
            (tiled - area(&hull)).abs() < 1e-6,
            "the facets no longer tile the hull: the cloak changed the silhouette"
        );
        assert_eq!(
            glyph.matches("<linearGradient").count(),
            glyph.matches(r#"gradientUnits="userSpaceOnUse""#).count(),
            "a facet ramp restarts inside its facet, so that facet is lit on its own"
        );
    }

    /// `background.py:958-959` asserts `len(hull) == 1` before indexing it --
    /// a second silhouette outline must fail loudly, not silently pick the
    /// first one and let a regressed hull area slip through undetected.
    #[test]
    #[should_panic(expected = "the ship needs exactly one silhouette outline")]
    fn hull_poly_rejects_a_second_silhouette() {
        let glyph = r#"<polygon points="0,0 1,0 1,1" fill="none"/><polygon points="2,2 3,2 3,3" fill="none"/>"#;
        hull_poly(glyph);
    }

    /// rotate is hexatri's own animation: the two triangle rings counter-spin
    /// inside a static hex frame, and nothing else in the glyph moves.
    #[test]
    fn hexatri_spins_only_its_triangles() {
        let spinning = ico_hexatri(true);
        assert_eq!(spinning.matches("class=\"spin\"").count(), 1);
        assert_eq!(spinning.matches("class=\"rspin\"").count(), 1);
        assert!(!ico_hexatri(false).contains("class=\"spin\""));
        assert_eq!(
            ico_hexatri(true).matches("<polygon").count(),
            ico_hexatri(false).matches("<polygon").count(),
            "the motion must not change the glyph's geometry"
        );
    }
}
