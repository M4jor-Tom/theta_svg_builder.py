//! The hexagon lattice and the sparse triangles laid over it.
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use crate::geom::{Lattice, NB, fmt, pts, regular_poly};
use crate::rng::PyRandom;
use crate::style::{FILL_O, PAL, STROKE_O};

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

/// Hexagon lattice (spacing 2s) + sparse triangles under the holder/
/// intersector rule, styled per the chosen background animation.
pub fn pat_trihex(lat: &Lattice, rng: &mut PyRandom) -> String {
    let ink = &PAL.ink;
    let sw = fmt(lat.u * 0.0013);
    let plan = assign(lat, rng);

    let mut out = Vec::with_capacity(lat.hexes.len());
    for (r, c) in &lat.hexes {
        let (cx, cy) = lat.center(*r, *c);
        let poly = pts(&regular_poly(cx, cy, lat.s, 6, PI / 6.0));
        out.push(format!(
            "<polygon points=\"{poly}\" fill=\"none\" stroke=\"{ink}\" \
             stroke-opacity=\"{STROKE_O}\" stroke-width=\"{sw}\"/>"
        ));
    }

    let fills = plan.tris.iter().map(|(v1, v2, apex, col)| {
        format!(
            "<polygon points=\"{}\" fill=\"{col}\" fill-opacity=\"{FILL_O}\"/>",
            pts(&[*v1, *v2, *apex])
        )
    });
    // borders go on top of everything, so the triangle fills come first.
    fills.chain(out).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
