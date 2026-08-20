//! The hexagon lattice and the sparse triangles laid over it.
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use crate::geom::{Lattice, NB, SQRT3, fmt, pts, regular_poly};
use crate::params::{Scene, background};
use crate::rng::{PyRandom, cell_rng};
use crate::style::{BLIND_S, FILL_O, PAL, SPACE_FRAC, SPACE_STARS, SPACE_STROKE_O, STROKE_O, VOID};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Holder,
    Inter,
}

/// One triangle: the two vertices of the edge it sits on, its apex, and its
/// fill colour.
type Tri = ((f64, f64), (f64, f64), (f64, f64), String);

/// Which hexagons hold a triangle, which are pierced by one, and the triangles
/// themselves. Geometry only -- it never looks at the motion, image, icon or
/// overlay, which is what makes one seed give one layout everywhere.
struct Plan {
    role: HashMap<(i32, i32), Role>,
    poked: HashSet<(i32, i32)>,
    tris: Vec<Tri>,
}

/// Assign roles + triangles (geometry is independent of bg/fg).
fn assign(lat: &Lattice, rng: &mut PyRandom) -> Plan {
    let hexset: HashSet<(i32, i32)> = lat.hexes.iter().copied().collect();
    let mut plan = Plan {
        role: HashMap::new(),
        poked: HashSet::new(),
        tris: Vec::new(),
    };
    let mut order = lat.hexes.clone();
    rng.shuffle(&mut order);
    for (r, c) in order {
        // skip -> fewer triangles. Python's `or` short-circuits, so a hexagon
        // that already has a role draws no value at all; consuming one here
        // would desynchronise every hexagon after it.
        if plan.role.contains_key(&(r, c)) || rng.random() > 0.5 {
            continue;
        }
        let (cx, cy) = lat.center(r, c);
        let v = regular_poly(cx, cy, lat.s, 6, PI / 6.0);
        for k in rng.sample(6, 6) {
            let (dr, dc) = NB[r.rem_euclid(2) as usize][k];
            let n = (r + dr, c + dc);
            if plan.role.get(&n) == Some(&Role::Holder) || plan.poked.contains(&n) {
                continue;
            }
            // V[(k - 1) % 6]: at k = 0 Python wraps to V[5], so `+ 5` here.
            let (v1, v2) = (v[(k + 5) % 6], v[k]);
            let mid = ((v1.0 + v2.0) / 2.0, (v1.1 + v2.1) / 2.0);
            let apex = (2.0 * mid.0 - cx, 2.0 * mid.1 - cy);
            let d = [v1, v2, apex]
                .iter()
                .map(|(x, y)| (x - lat.cx0).hypot(y - lat.cy0))
                .fold(f64::INFINITY, f64::min);
            if d < lat.clear_r {
                continue; // keep the icon zone empty
            }
            let col = if rng.random() < 0.6 { &PAL.a } else { &PAL.b };
            plan.tris.push((v1, v2, apex, col.clone()));
            plan.role.insert((r, c), Role::Holder);
            if hexset.contains(&n) {
                plan.role.insert(n, Role::Inter);
            }
            plan.poked.insert(n);
            break;
        }
    }
    plan
}

/// Which hexagons become windows onto the void. A cell qualifies only if the
/// whole hexagon clears the icon zone (centre distance >= clear_r + s), the same
/// exclusion the triangles obey.
///
/// `every` (background.motion CLOSEOPEN) takes the entire eligible field instead
/// of a SPACE_FRAC sample: the blinds hold all but a handful shut at any
/// instant, so sparseness moves from space to time and a window can open
/// anywhere rather than always in the same seven places.
pub fn space_cells(lat: &Lattice, seed: u32, every: bool) -> HashSet<(i32, i32)> {
    let mut out = HashSet::new();
    for (r, c) in &lat.hexes {
        let (cx, cy) = lat.center(*r, *c);
        if (cx - lat.cx0).hypot(cy - lat.cy0) < lat.clear_r + lat.s {
            continue;
        }
        // Python's `or` short-circuits: under `every` no cell touches its own
        // stream at all, and an ineligible cell never reaches the draw either.
        if every || cell_rng("pick", seed, *r, *c).random() < SPACE_FRAC {
            out.insert((*r, *c));
        }
    }
    out
}

/// One hexagon of procedural deep space: clipped void ground, a faint nebula,
/// then seeded stars. Drawn, never embedded -- no assets, crisp at any size.
///
/// `phase` (background.motion CLOSEOPEN) is its blind's timing, which makes the
/// cell switch itself off while that blind covers it. SVG does no occlusion
/// culling, so without this the stars are repainted every frame under a shut
/// blind.
// the reference's own signature (background.py:207); the caller already holds
// the formatted polygon, so there is nothing here to bundle away
#[allow(clippy::too_many_arguments)]
fn space_cell(
    seed: u32,
    poly: &str,
    cx: f64,
    cy: f64,
    s: f64,
    r: i32,
    c: i32,
    phase: Option<&str>,
) -> String {
    let mut g = cell_rng("star", seed, r, c);
    let cid = format!("sp{r}_{c}"); // cell coords, so ids are stable
    let win = phase.map_or(String::new(), |p| format!(" class=\"win\" style=\"{p}\""));
    let star = &PAL.bg.0;
    let (nx, ny) = (cx + (g.random() - 0.5) * s, cy + (g.random() - 0.5) * s);
    let (nrx, nry) = (s * (0.55 + g.random() * 0.5), s * (0.30 + g.random() * 0.3));
    let neb = if g.random() < 0.6 { "neba" } else { "nebb" };
    let ang = g.random() * 180.0;
    let void = &*VOID;
    let (nxf, nyf) = (fmt(nx), fmt(ny));
    let mut p = format!(
        "<clipPath id=\"{cid}\"><polygon points=\"{poly}\"/></clipPath>\
         <g{win} clip-path=\"url(#{cid})\">\
         <polygon points=\"{poly}\" fill=\"{void}\"/>\
         <ellipse cx=\"{nxf}\" cy=\"{nyf}\" rx=\"{}\" ry=\"{}\" \
         fill=\"url(#{neb})\" transform=\"rotate({} {nxf} {nyf})\"/>",
        fmt(nrx),
        fmt(nry),
        fmt(ang)
    );
    for i in 0..SPACE_STARS {
        let x = cx + (g.random() - 0.5) * 2.0 * s;
        let y = cy + (g.random() - 0.5) * SQRT3 * s;
        let mut rad = s * (0.008 + g.random().powi(2) * 0.022);
        let o = 0.35 + g.random() * 0.6;
        if i < 2 {
            // two anchor stars get a soft bloom
            p.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{star}\" fill-opacity=\"0.12\"/>",
                fmt(x),
                fmt(y),
                fmt(rad * 3.5)
            ));
            rad *= 1.4;
        }
        p.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{star}\" fill-opacity=\"{}\"/>",
            fmt(x),
            fmt(y),
            fmt(rad),
            fmt(o)
        ));
    }
    p.push_str("</g>");
    p
}

/// background.motion CLOSEOPEN: the timing of one cell's shutter, drawn from the
/// cell's own stream rather than draw order. The blind and the window it covers
/// are given this same string, so the window can switch itself off exactly while
/// it is hidden -- one value, two users, no way for them to fall out of step.
fn blind_phase(seed: u32, r: i32, c: i32) -> String {
    let mut g = cell_rng("blind", seed, r, c);
    let d = BLIND_S.0 + g.random() * (BLIND_S.1 - BLIND_S.0);
    // the delay is drawn first, so it is bound first
    let delay = fmt(g.random() * d);
    format!("animation-delay:-{delay}s;animation-duration:{}s", fmt(d))
}

/// Hexagon lattice (spacing 2s) + sparse triangles under the holder/
/// intersector rule, styled per the chosen background animation.
pub fn pat_trihex(w: u32, h: u32, lat: &Lattice, scene: &Scene, rng: &mut PyRandom) -> String {
    let (a, ink) = (&PAL.a, &PAL.ink);
    let sw = fmt(lat.u * 0.0013);
    let plan = assign(lat, rng);

    // --- render ---
    let scan_delay = |x: f64, y: f64| fmt(-((x + y) / (w as f64 + h as f64)) * 5.0);
    let closeopen = scene.motion == background::Motion::Closeopen;
    let space = if scene.image == background::Image::Starfield {
        space_cells(lat, scene.seed, closeopen)
    } else {
        HashSet::new()
    };

    let mut voids = Vec::new();
    // eager, and it has to stay that way: the `lights` non-void branch draws
    // from the global stream, so deferring this loop past `fills` would reorder
    // the draws and move every hexagon.
    let mut out = Vec::with_capacity(lat.hexes.len());
    for (r, c) in &lat.hexes {
        let (cx, cy) = lat.center(*r, *c);
        let poly = pts(&regular_poly(cx, cy, lat.s, 6, PI / 6.0));
        let void = space.contains(&(*r, *c));
        if void {
            let phase = closeopen.then(|| blind_phase(scene.seed, *r, *c));
            voids.push(space_cell(
                scene.seed,
                &poly,
                cx,
                cy,
                lat.s,
                *r,
                *c,
                phase.as_deref(),
            ));
            if let Some(phase) = &phase {
                // canvas-coloured, so a shut blind is indistinguishable from any
                // other lattice cell -- see the userSpaceOnUse note on #bg
                voids.push(format!(
                    "<polygon class=\"blind\" style=\"{phase}\" \
                     points=\"{poly}\" fill=\"url(#bg)\"/>"
                ));
            }
        }
        // A window's border sits brighter than the field to mark the few portals --
        // but under closeopen *every* cell is a portal, so that would just raise the
        // whole lattice (rule 1). There the blind opening is the only marker.
        let so = if void && !closeopen {
            SPACE_STROKE_O
        } else {
            STROKE_O
        };
        out.push(match scene.motion {
            background::Motion::Scan => format!(
                "<polygon class=\"scan\" style=\"animation-delay:{}s\" \
                 points=\"{poly}\" fill=\"none\" stroke=\"{ink}\" \
                 stroke-opacity=\"{so}\" stroke-width=\"{sw}\"/>",
                scan_delay(cx, cy)
            ),
            // border-only pulse: the usual pale fill flash would wash the stars out
            background::Motion::Lights if void => {
                let delay = fmt(cell_rng("delay", scene.seed, *r, *c).random() * 9.0);
                format!(
                    "<polygon class=\"lightb\" style=\"animation-delay:-{delay}s\" \
                     points=\"{poly}\" fill=\"none\" stroke=\"{ink}\" \
                     stroke-opacity=\"{so}\" stroke-width=\"{sw}\"/>"
                )
            }
            background::Motion::Lights => {
                // two draws off the global stream, delay before duration
                let delay = fmt(rng.random() * 9.0);
                let dur = fmt(7.0 + rng.random() * 5.0);
                format!(
                    "<polygon class=\"light\" style=\"animation-delay:-{delay}s;\
                     animation-duration:{dur}s\" points=\"{poly}\" fill=\"{a}\" \
                     fill-opacity=\"0\" stroke=\"{ink}\" stroke-opacity=\"{so}\" \
                     stroke-width=\"{sw}\"/>"
                )
            }
            _ => format!(
                "<polygon points=\"{poly}\" fill=\"none\" stroke=\"{ink}\" \
                 stroke-opacity=\"{so}\" stroke-width=\"{sw}\"/>"
            ),
        });
    }

    let fills = plan.tris.iter().map(|(v1, v2, apex, col)| {
        let points = pts(&[*v1, *v2, *apex]);
        if scene.motion == background::Motion::Scan {
            let (tx, ty) = ((v1.0 + v2.0 + apex.0) / 3.0, (v1.1 + v2.1 + apex.1) / 3.0);
            format!(
                "<polygon class=\"wavef\" style=\"animation-delay:{}s\" \
                 points=\"{points}\" fill=\"{col}\" fill-opacity=\"{FILL_O}\"/>",
                scan_delay(tx, ty)
            )
        } else {
            format!("<polygon points=\"{points}\" fill=\"{col}\" fill-opacity=\"{FILL_O}\"/>")
        }
    });
    // voids sit under the triangles, so a translucent triangle crossing a window
    // reads as a shard catching light; borders go on top of everything.
    voids.into_iter().chain(fills).chain(out).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Glyph;
    use crate::svg::build_svg;

    /// A starfield render with nothing else turned on, which is what
    /// `background.py`'s `build_svg(w, h, bg=..., bg_image="space")` defaults to.
    fn starfield(motion: background::Motion) -> Scene {
        Scene {
            seed: 0,
            motion,
            image: background::Image::Starfield,
            glyph: Glyph::Hexatri { rotate: true },
            overlay: None,
        }
    }

    /// The style strings of every `class="<class>"` element, in document order --
    /// the `re.findall(r'class="win" style="([^"]+)"', svg)` of the Python.
    fn styles<'a>(svg: &'a str, class: &str) -> Vec<&'a str> {
        let key = format!("class=\"{class}\" style=\"");
        svg.match_indices(&key)
            .map(|(i, _)| {
                let rest = &svg[i + key.len()..];
                &rest[..rest.find('"').expect("an unterminated style attribute")]
            })
            .collect()
    }

    /// Space cells must clear the icon zone entirely, stay sparse, never take
    /// the fill-flashing .light class (which would wash the starfield out), and
    /// under background.motion CLOSEOPEN carry exactly one blind apiece,
    /// layered under the triangles.
    #[test]
    fn space_cells_clear_the_icon_zone_and_keep_their_blinds() {
        for (w, h) in [(1920u32, 1080u32), (1080, 1920)] {
            let lat = Lattice::new(w, h);
            let space = space_cells(&lat, 0, false);
            for (r, c) in &space {
                let (cx, cy) = lat.center(*r, *c);
                let d = (cx - lat.cx0).hypot(cy - lat.cy0);
                assert!(
                    d - lat.s >= lat.clear_r,
                    "a space cell overlaps the icon zone"
                );
            }
            assert!(!space.is_empty(), "no space cells were placed");
            assert!(
                space.len() as f64 <= lat.hexes.len() as f64 * SPACE_FRAC * 2.0,
                "space cells are not sparse"
            );

            let svg = build_svg(w, h, &starfield(background::Motion::Lights));
            assert_eq!(
                svg.matches("<clipPath id=\"sp").count(),
                space.len(),
                "rendered space cells != selected"
            );
            assert_eq!(
                svg.matches("class=\"lightb\"").count(),
                space.len(),
                "a space cell is missing its border pulse"
            );
            assert_eq!(
                svg.matches("class=\"light\"").count(),
                lat.hexes.len() - space.len(),
                "a space cell got the fill flash"
            );

            let every = space_cells(&lat, 0, true);
            assert!(
                space.is_subset(&every) && space.len() < every.len(),
                "closeopen must widen the window pool, not reuse the sparse one"
            );
            for (r, c) in &every {
                // the icon zone stays clear even so
                let (cx, cy) = lat.center(*r, *c);
                assert!(
                    (cx - lat.cx0).hypot(cy - lat.cy0) - lat.s >= lat.clear_r,
                    "a closeopen window overlaps the icon zone"
                );
            }

            let svg = build_svg(w, h, &starfield(background::Motion::Closeopen));
            assert_eq!(
                svg.matches("class=\"blind\"").count(),
                every.len(),
                "every eligible hexagon must be a window"
            );
            // fill *and* opacity: a lone fill-opacity="0.38" also matches a star,
            // and with every cell a window there are now enough stars to hit that
            // value by chance.
            let tris = [&PAL.a, &PAL.b]
                .iter()
                .filter_map(|col| svg.find(&format!("fill=\"{col}\" fill-opacity=\"{FILL_O}\"")))
                .min()
                .expect("no triangle to check blind layering against");
            assert!(
                svg.rfind("class=\"blind\"").expect("a blind was rendered") < tris,
                "a blind is painted over the triangles instead of under them"
            );
            assert!(
                svg.contains("transform-origin:center;transform:scale(0)}"),
                "blinds must rest open, so prefers-reduced-motion still shows the starfield"
            );
            assert!(
                svg.contains(".win{animation:winvis 75s ease-in-out infinite;display:inline}"),
                "windows must rest rendered, for the same reason"
            );
            assert!(
                !svg.contains(&format!("stroke-opacity=\"{SPACE_STROKE_O}\"")),
                "closeopen windows must not raise the whole lattice to the window border opacity"
            );

            // Every window carries its own blind's timing, so it hides exactly
            // while covered. Desync here shows up as a starfield popping in over
            // a shut blind.
            let wins = styles(&svg, "win");
            let blinds = styles(&svg, "blind");
            assert!(
                wins == blinds && !wins.is_empty(),
                "a window is out of phase with its own blind"
            );
            assert!(
                !build_svg(w, h, &starfield(background::Motion::Lights)).contains("class=\"win\""),
                "a bg without blinds has nothing covering its windows, so they must never switch off"
            );
        }
    }

    /// Every hexagon is EITHER a holder (one of its edges is a triangle's base)
    /// OR an intersector (a triangle's tip pokes into it), never both, and no
    /// triangle sits inside the icon zone. This is what keeps triangles few,
    /// non-overlapping and out from behind the glyph.
    #[test]
    fn roles_are_exclusive_and_the_centre_is_clear() {
        for (w, h) in [(1920u32, 1080u32), (1080, 1920)] {
            let lat = Lattice::new(w, h);
            let mut rng = PyRandom::new("trihex:0");
            let plan = assign(&lat, &mut rng);
            let holders: HashSet<_> = plan
                .role
                .iter()
                .filter(|(_, v)| **v == Role::Holder)
                .map(|(k, _)| *k)
                .collect();
            let inters: HashSet<_> = plan
                .role
                .iter()
                .filter(|(_, v)| **v == Role::Inter)
                .map(|(k, _)| *k)
                .collect();
            assert!(
                holders.is_disjoint(&inters),
                "a hexagon is both holder and intersector"
            );
            assert_eq!(
                plan.poked.len(),
                holders.len(),
                "poked count != triangle count"
            );
            for (v1, v2, apex, _) in &plan.tris {
                let d = [v1, v2, apex]
                    .iter()
                    .map(|(x, y)| (x - lat.cx0).hypot(y - lat.cy0))
                    .fold(f64::INFINITY, f64::min);
                assert!(d >= lat.clear_r, "a triangle sits inside the icon zone");
            }
        }
    }
}
