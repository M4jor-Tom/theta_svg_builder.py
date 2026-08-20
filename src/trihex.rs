//! The hexagon lattice and the sparse triangles laid over it.
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use askama::Template;

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

/// One star, already reduced to what `trihex.svg` substitutes verbatim.
/// `bloom` is the halo radius of the two anchor stars and `None` for the rest,
/// so the template's `{% if let %}` is the whole of that decision -- it never
/// counts stars itself. `bloom` and `r` are two separate numbers, not one
/// scaled twice: the halo is measured off the plain radius and the star only
/// grows afterwards, an order `space_cell` keeps and the template cannot see.
struct Star {
    x: String,
    y: String,
    r: String,
    o: String,
    bloom: Option<String>,
}

/// One hexagon of procedural deep space, reduced to substitutable values.
///
/// `phase` is ONE field with TWO consumers. `trihex.svg` hands this same
/// string to the `<g class="win">` holding the stars and to the
/// `<polygon class="blind">` drawn over it, and the blind exists exactly when
/// the phase does -- so the window switches off for precisely the span its
/// own shutter covers it. A separate window phase and blind phase is what
/// would let the two drift apart and pop a starfield in over a closed blind;
/// see `blind_phase`. The blind is painted `url(#bg)`, canvas colour, so a
/// shut one is indistinguishable from any other lattice cell -- which is why
/// that gradient is `userSpaceOnUse` (see `svg.rs`'s `Defs`).
///
/// `rot` is the whole `rotate(...)` transform value rather than the bare
/// angle: a template may only substitute, so it must not be the thing that
/// puts the spaces between an angle and the centre it turns about.
struct VoidCell {
    cid: String,
    poly: String,
    phase: Option<String>,
    nx: String,
    ny: String,
    nrx: String,
    nry: String,
    neb: &'static str,
    rot: String,
    stars: Vec<Star>,
}

/// One hexagon of procedural deep space: clipped void ground, a faint nebula,
/// then seeded stars. Drawn, never embedded -- no assets, crisp at any size.
///
/// `phase` is forwarded straight through to `VoidCell`; see there for why it
/// exists.
// the reference's own signature (background.py:207); the caller already holds
// the formatted polygon, so there is nothing here to bundle away
#[allow(clippy::too_many_arguments)]
fn space_cell(
    seed: u32,
    poly: String,
    cx: f64,
    cy: f64,
    s: f64,
    r: i32,
    c: i32,
    phase: Option<String>,
) -> VoidCell {
    let mut g = cell_rng("star", seed, r, c);
    let cid = format!("sp{r}_{c}"); // cell coords, so ids are stable
    let (nx, ny) = (cx + (g.random() - 0.5) * s, cy + (g.random() - 0.5) * s);
    let (nrx, nry) = (s * (0.55 + g.random() * 0.5), s * (0.30 + g.random() * 0.3));
    let neb = if g.random() < 0.6 { "neba" } else { "nebb" };
    let ang = g.random() * 180.0;
    let (nxf, nyf) = (fmt(nx), fmt(ny));
    let rot = format!("rotate({} {nxf} {nyf})", fmt(ang));
    let stars = (0..SPACE_STARS)
        .map(|i| {
            let x = cx + (g.random() - 0.5) * 2.0 * s;
            let y = cy + (g.random() - 0.5) * SQRT3 * s;
            let mut rad = s * (0.008 + g.random().powi(2) * 0.022);
            let o = 0.35 + g.random() * 0.6;
            let bloom = if i < 2 {
                // two anchor stars get a soft bloom, sized off the plain
                // radius -- the star itself only grows afterwards
                let b = fmt(rad * 3.5);
                rad *= 1.4;
                Some(b)
            } else {
                None
            };
            Star {
                x: fmt(x),
                y: fmt(y),
                r: fmt(rad),
                o: fmt(o),
                bloom,
            }
        })
        .collect();
    VoidCell {
        cid,
        poly,
        phase,
        nx: nxf,
        ny: nyf,
        nrx: fmt(nrx),
        nry: fmt(nry),
        neb,
        rot,
        stars,
    }
}

/// One triangle. `style` is the complete `animation-delay:...s` value under
/// background.motion SCAN and `None` otherwise. The `wavef` class it pairs
/// with is a literal in the template rather than a second field, so there is
/// nothing here for this `Option` to fall out of agreement with.
struct TriFill {
    points: String,
    col: String,
    style: Option<String>,
}

/// A hexagon border's class and its whole `style` value, or neither -- one
/// field, not two, for the reason `icon.rs`'s `RingMotion` spells out: two
/// independent `Option`s can be set apart from each other, and the template
/// would only discover it at render time.
struct HexAnim {
    cls: &'static str,
    style: String,
}

/// One hexagon border. `fill` and `fill_opacity` do stay two fields, because
/// only the `light` arm paints a (transparent) fill for its keyframes to
/// flash up, and an absent `fill_opacity` simply renders nothing -- unlike a
/// split class/style pair there is no unwrap for them to disagree through.
struct HexBorder {
    poly: String,
    anim: Option<HexAnim>,
    fill: &'static str,
    fill_opacity: Option<&'static str>,
    so: String,
}

/// The template context for `trihex.svg`. `templates/trihex.svg` walks
/// `voids`, then `tris`, then `hexes` as three separate loops in that order,
/// and THAT -- not the order these fields are declared here -- is what fixes
/// the layering: voids sit under the triangles, so a translucent triangle
/// crossing a window reads as a shard catching light, and the borders go on
/// top of everything. Reordering these fields would change nothing;
/// reordering the template's loops would silently change the picture.
///
/// `void`, `star`, `fill_o`, `ink` and `sw` are whole-render constants held
/// once here instead of copied onto every cell, star and hexagon -- at 1080p
/// that is roughly two thousand hexagons and two thousand stars. There is no
/// palette `a` beside them: the one element that paints it is the `light`
/// border, and it carries the colour on `HexBorder::fill` precisely so the
/// template never has to pick between `a` and `none` itself.
///
/// Four of `trihex.svg`'s conditionals wrap an optional *attribute group*
/// inside an open tag, and each one is written `{% if ... +%} attr="..."
/// {% endif +%}`. Those `+` markers are load-bearing: `whitespace =
/// "suppress"` (`askama.toml`) trims the whitespace touching a `{% %}` block
/// whether or not the rest of that text node is itself blank. On the opening
/// `{% if ... +%}` the text that follows is ` class="win" style="` -- not
/// whitespace-only -- yet its leading space would still be stripped, running
/// `<g` into `class`; on `{% endif +%}` it would run `<polygon` straight into
/// `points` and `fill="none"` straight into `stroke`. Each `+` keeps its one
/// separating space, and the closing one stays on `{% endif %}` so it is
/// emitted whether or not the branch was taken -- exactly what
/// `<polygon points=...>` needs when there is no class to announce.
/// `templates/root.svg` hits the same rule between two `{{ }}` expressions;
/// see `svg.rs`'s `Root`.
#[derive(askama::Template)]
#[template(path = "trihex.svg")]
struct Trihex {
    voids: Vec<VoidCell>,
    tris: Vec<TriFill>,
    hexes: Vec<HexBorder>,
    void: &'static str,
    star: &'static str,
    fill_o: String,
    ink: &'static str,
    sw: String,
}

/// Hexagon lattice (spacing 2s) + sparse triangles under the holder/
/// intersector rule, styled per the chosen background animation.
pub fn pat_trihex(w: u32, h: u32, lat: &Lattice, scene: &Scene, rng: &mut PyRandom) -> String {
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
    // eager, and it has to stay that way: this loop's `lights` non-void branch
    // draws two values per hex from the global stream, and those draws must
    // land in `lat.hexes` order immediately after `assign()`'s -- not because
    // of `tris` or `voids`, which never touch the global stream, but because
    // any other order here desyncs every hexagon drawn after the first one.
    let mut hexes = Vec::with_capacity(lat.hexes.len());
    for (r, c) in &lat.hexes {
        let (cx, cy) = lat.center(*r, *c);
        let poly = pts(&regular_poly(cx, cy, lat.s, 6, PI / 6.0));
        let void = space.contains(&(*r, *c));
        if void {
            voids.push(space_cell(
                scene.seed,
                poly.clone(),
                cx,
                cy,
                lat.s,
                *r,
                *c,
                closeopen.then(|| blind_phase(scene.seed, *r, *c)),
            ));
        }
        // A window's border sits brighter than the field to mark the few portals --
        // but under closeopen *every* cell is a portal, so that would just raise the
        // whole lattice (rule 1). There the blind opening is the only marker.
        let so = if void && !closeopen {
            SPACE_STROKE_O
        } else {
            STROKE_O
        };
        let (anim, fill, fill_opacity) = match scene.motion {
            background::Motion::Scan => (
                Some(HexAnim {
                    cls: "scan",
                    style: format!("animation-delay:{}s", scan_delay(cx, cy)),
                }),
                "none",
                None,
            ),
            // border-only pulse: the usual pale fill flash would wash the stars out
            background::Motion::Lights if void => {
                let delay = fmt(cell_rng("delay", scene.seed, *r, *c).random() * 9.0);
                (
                    Some(HexAnim {
                        cls: "lightb",
                        style: format!("animation-delay:-{delay}s"),
                    }),
                    "none",
                    None,
                )
            }
            background::Motion::Lights => {
                // two draws off the global stream, delay before duration
                let delay = fmt(rng.random() * 9.0);
                let dur = fmt(7.0 + rng.random() * 5.0);
                (
                    Some(HexAnim {
                        cls: "light",
                        style: format!("animation-delay:-{delay}s;animation-duration:{dur}s"),
                    }),
                    PAL.a.as_str(),
                    Some("0"),
                )
            }
            _ => (None, "none", None),
        };
        hexes.push(HexBorder {
            poly,
            anim,
            fill,
            fill_opacity,
            so: so.to_string(),
        });
    }

    let tris = plan
        .tris
        .iter()
        .map(|(v1, v2, apex, col)| TriFill {
            points: pts(&[*v1, *v2, *apex]),
            col: col.clone(),
            style: (scene.motion == background::Motion::Scan).then(|| {
                let (tx, ty) = ((v1.0 + v2.0 + apex.0) / 3.0, (v1.1 + v2.1 + apex.1) / 3.0);
                format!("animation-delay:{}s", scan_delay(tx, ty))
            }),
        })
        .collect();

    Trihex {
        voids,
        tris,
        hexes,
        void: VOID.as_str(),
        star: PAL.bg.0.as_str(),
        fill_o: FILL_O.to_string(),
        ink: PAL.ink.as_str(),
        sw,
    }
    .render()
    .unwrap()
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
