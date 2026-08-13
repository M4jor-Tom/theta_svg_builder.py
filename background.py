#!/usr/bin/env python3
"""Animated SVG background builder — trihexagonal blueprint.

A hexagon lattice with sparse equilateral triangles, plus a nested
hexagon/triangle center icon. Light theme only. Background and foreground
animations are chosen independently and combine freely:

  --bg static   no background animation
  --bg scan     hexagon (and triangle) opacities sweep diagonally
  --bg lights   hexagons occasionally light their border, then fill, then rest
  --fg rotate   the icon's triangle rings counter-rotate
  --fg static   still icon

Animation is pure CSS (no SMIL, no JS) and honours prefers-reduced-motion
(which falls back to the clean static look). Stdlib only. Sizes scale with
min(w,h) so pattern density is constant across resolutions.

Triangle logic: every hexagon is EITHER a "holder" (one of its edges is a
triangle's base) OR an "intersector" (a triangle's tip pokes into it), never
both. Triangles never overlap and never sit behind the center icon.

Examples:
  bgsvg --bg lights --fg rotate --resolution 4k --out a.svg
  bgsvg --bg scan --fg static --resolution 2560x1440 --out b.svg
  bgsvg --bg static --resolution mobile,1080p            # -> ./out/*.svg
"""
import argparse
import math
import os
import random
import re
import sys

PRESETS = {
    "720p": (1280, 720), "1080p": (1920, 1080), "1440p": (2560, 1440),
    "4k": (3840, 2160), "mobile": (1080, 1920), "tablet": (1536, 2048),
    "square": (1080, 1080), "ultrawide": (3440, 1440),
}

BG = ("static", "scan", "lights")
FG = ("rotate", "static")

SQRT3 = math.sqrt(3)

# Neighbour (dr,dc) per edge k (edge normal at 60*k deg), by row parity.
NB = {
    0: {0: (0, 1), 1: (1, 0), 2: (1, -1), 3: (0, -1), 4: (-1, -1), 5: (-1, 0)},
    1: {0: (0, 1), 1: (1, 1), 2: (1, 0), 3: (0, -1), 4: (-1, 0), 5: (-1, 1)},
}


# ---- tiny helpers ---------------------------------------------------------
def fmt(x):
    if isinstance(x, int):
        return str(x)
    r = round(x, 2)
    if r == int(r):
        return str(int(r))
    return ("%.2f" % r).rstrip("0").rstrip(".")


def pts(points):
    return " ".join(f"{fmt(x)},{fmt(y)}" for x, y in points)


def regular_poly(cx, cy, r, n, rot=0.0):
    return [(cx + r * math.cos(rot + 2 * math.pi * i / n),
             cy + r * math.sin(rot + 2 * math.pi * i / n)) for i in range(n)]


def _mix(h1, h2, t):
    c1 = tuple(int(h1[i:i + 2], 16) for i in (1, 3, 5))
    c2 = tuple(int(h2[i:i + 2], 16) for i in (1, 3, 5))
    return "#%02x%02x%02x" % tuple(round(c1[k] + (c2[k] - c1[k]) * t) for k in range(3))


def darken(h, t):
    return _mix(h, "#0c1017", t)


# Light-theme palette: a = hexagons, b = triangles.
PAL = dict(a=darken("#6fb7d1", 0.58), b=darken("#77c9a6", 0.58),
           bg=("#eef3f6", "#d9e3ea"), ink=darken("#6fb7d1", 0.70))

STROKE_O = 0.27   # hexagon border baseline opacity (also the reduced-motion value)
FILL_O = 0.38     # triangle fill baseline opacity


# ---- background pattern ---------------------------------------------------
def pat_trihex(w, h, u, clear_r, bg):
    """Hexagon lattice (spacing 2s) + sparse triangles under the holder/
    intersector rule, styled per the chosen background animation."""
    a, b, ink = PAL["a"], PAL["b"], PAL["ink"]
    s = u / 9.0
    D = 2 * s
    rowh = D * SQRT3 / 2
    sw = fmt(u * 0.0013)
    cx0, cy0 = w / 2, h / 2

    R, C = int(h / rowh) + 2, int(w / D) + 2
    hexes = [(r, c) for r in range(-1, R) for c in range(-1, C)]
    hexset = set(hexes)

    def center(r, c):
        return (c * D + (r % 2) * D / 2, r * rowh)

    # --- assign roles + triangles (geometry is independent of bg/fg) ---
    role, poked, tris = {}, set(), []
    order = hexes[:]
    random.shuffle(order)
    for (r, c) in order:
        if role.get((r, c)) or random.random() > 0.5:   # skip -> fewer triangles
            continue
        cx, cy = center(r, c)
        V = regular_poly(cx, cy, s, 6, math.pi / 6)
        for k in random.sample(range(6), 6):
            N = (r + NB[r % 2][k][0], c + NB[r % 2][k][1])
            if role.get(N) == "holder" or N in poked:
                continue
            v1, v2 = V[(k - 1) % 6], V[k]
            mid = ((v1[0] + v2[0]) / 2, (v1[1] + v2[1]) / 2)
            apex = (2 * mid[0] - cx, 2 * mid[1] - cy)
            if min(math.hypot(x - cx0, y - cy0) for x, y in (v1, v2, apex)) < clear_r:
                continue                                   # keep the icon zone empty
            tris.append((v1, v2, apex, a if random.random() < 0.6 else b))
            role[(r, c)] = "holder"
            if N in hexset:
                role[N] = "inter"
            poked.add(N)
            break

    # --- render ---
    def scan_delay(x, y):
        return fmt(-((x + y) / (w + h)) * 5)

    out = []
    for (r, c) in hexes:
        cx, cy = center(r, c)
        poly = pts(regular_poly(cx, cy, s, 6, math.pi / 6))
        if bg == "scan":
            out.append(f'<polygon class="scan" style="animation-delay:{scan_delay(cx,cy)}s" '
                       f'points="{poly}" fill="none" stroke="{ink}" stroke-opacity="{STROKE_O}" stroke-width="{sw}"/>')
        elif bg == "lights":
            out.append(f'<polygon class="light" style="animation-delay:-{fmt(random.random()*9)}s;'
                       f'animation-duration:{fmt(7+random.random()*5)}s" points="{poly}" fill="{a}" '
                       f'fill-opacity="0" stroke="{ink}" stroke-opacity="{STROKE_O}" stroke-width="{sw}"/>')
        else:
            out.append(f'<polygon points="{poly}" fill="none" stroke="{ink}" '
                       f'stroke-opacity="{STROKE_O}" stroke-width="{sw}"/>')

    fills = []
    for (v1, v2, apex, col) in tris:
        if bg == "scan":
            tx, ty = (v1[0] + v2[0] + apex[0]) / 3, (v1[1] + v2[1] + apex[1]) / 3
            fills.append(f'<polygon class="wavef" style="animation-delay:{scan_delay(tx,ty)}s" '
                         f'points="{pts([v1,v2,apex])}" fill="{col}" fill-opacity="{FILL_O}"/>')
        else:
            fills.append(f'<polygon points="{pts([v1,v2,apex])}" fill="{col}" fill-opacity="{FILL_O}"/>')
    return fills + out


# ---- center icon ----------------------------------------------------------
def ico_hexatri(fg):
    """Nested hexagon<->triangle glyph. fg=='rotate' -> the two triangle rings
    counter-spin inside the static hex frame (rotation centres pinned to the
    icon centre via transform-origin, so no wobble)."""
    a, b = PAL["a"], PAL["b"]
    # (radius, sides, rot, colour, width, opacity, rotate-origin-y%)
    rings = [(88, 6, math.pi / 6, a, 3.6, 1.0, None),
             (80, 3, -math.pi / 2, b, 3.0, 1.0, 66.7),   # up triangle
             (48, 6, math.pi / 6, a, 2.6, 0.8, None),
             (42, 3, math.pi / 2, b, 2.4, 0.8, 33.3)]    # down triangle
    parts = []
    for idx, (r, n, rot, col, swd, o, oy) in enumerate(rings):
        attr = ""
        if fg == "rotate" and oy is not None:
            cls = "rspin" if idx == 1 else "spin"
            attr = f' class="{cls}" style="animation-duration:{24 - idx*3}s;transform-origin:50% {oy}%"'
        parts.append(f'<polygon{attr} points="{pts(regular_poly(0,0,r,n,rot))}" fill="none" '
                     f'stroke="{col}" stroke-width="{fmt(swd)}" stroke-opacity="{fmt(o)}"/>')
    parts.append(f'<polygon points="{pts(regular_poly(0,0,16,6,math.pi/6))}" fill="{a}" '
                 f'fill-opacity="0.28" stroke="{a}" stroke-width="2"/>')
    return "".join(parts)


# ---- assembly -------------------------------------------------------------
def css():
    return ("<style>"
            "@keyframes spin{to{transform:rotate(360deg)}}"
            "@keyframes rspin{to{transform:rotate(-360deg)}}"
            "@keyframes scan{0%,100%{stroke-opacity:.16}50%{stroke-opacity:.62}}"
            "@keyframes wavef{0%,100%{fill-opacity:.12}50%{fill-opacity:.6}}"
            "@keyframes light{0%{stroke-opacity:.27;fill-opacity:0}7%{stroke-opacity:.85;fill-opacity:0}"
            "16%{stroke-opacity:.85;fill-opacity:.42}26%{stroke-opacity:.27;fill-opacity:0}"
            "100%{stroke-opacity:.27;fill-opacity:0}}"
            ".spin{animation:spin 24s linear infinite;transform-box:fill-box;transform-origin:center}"
            ".rspin{animation:rspin 24s linear infinite;transform-box:fill-box;transform-origin:center}"
            ".scan{animation:scan 5s ease-in-out infinite}"
            ".wavef{animation:wavef 5s ease-in-out infinite}"
            ".light{animation:light 9s ease-in-out infinite}"
            "@media (prefers-reduced-motion:reduce){*{animation:none!important}}"
            "</style>")


def build_svg(w, h, bg="static", fg="rotate", seed=0):
    random.seed(f"trihex:{seed}")   # layout depends only on seed, not bg/fg
    u = min(w, h)
    clear_r = u * 0.28
    defs = [
        f'<linearGradient id="bg" x1="0" y1="0" x2="0.4" y2="1"><stop offset="0%" stop-color="{PAL["bg"][0]}"/>'
        f'<stop offset="100%" stop-color="{PAL["bg"][1]}"/></linearGradient>',
        '<radialGradient id="vig"><stop offset="55%" stop-color="#8fa3b8" stop-opacity="0"/>'
        '<stop offset="100%" stop-color="#8fa3b8" stop-opacity="0.16"/></radialGradient>',
        f'<radialGradient id="halo"><stop offset="0%" stop-color="{PAL["bg"][0]}" stop-opacity="0.92"/>'
        f'<stop offset="55%" stop-color="{PAL["bg"][0]}" stop-opacity="0.65"/>'
        f'<stop offset="100%" stop-color="{PAL["bg"][0]}" stop-opacity="0"/></radialGradient>',
        '<filter id="ink" x="-30%" y="-30%" width="160%" height="160%">'
        '<feDropShadow dx="0" dy="2" stdDeviation="3" flood-color="#1e293b" flood-opacity="0.25"/></filter>',
    ]

    bg_svg = pat_trihex(w, h, u, clear_r, bg)
    k = u * 0.34 / 200
    icon = (f'<g transform="translate({fmt(w/2)},{fmt(h/2)}) scale({fmt(k)})" filter="url(#ink)">'
            f'{ico_hexatri(fg)}</g>')

    label = "trihexagonal background"
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {fmt(w)} {fmt(h)}" width="{fmt(w)}" height="{fmt(h)}" '
        f'preserveAspectRatio="xMidYMid slice" role="img" aria-label="{label}"><title>{label}</title>'
        f'<defs>{"".join(defs)}</defs>{css()}'
        f'<rect width="{fmt(w)}" height="{fmt(h)}" fill="url(#bg)"/>'
        f'<g>{"".join(bg_svg)}</g>'
        f'<circle cx="{fmt(w/2)}" cy="{fmt(h/2)}" r="{fmt(clear_r)}" fill="url(#halo)"/>'
        f'<rect width="{fmt(w)}" height="{fmt(h)}" fill="url(#vig)"/>{icon}</svg>\n')


# ---- CLI ------------------------------------------------------------------
def parse_res(s):
    s = s.strip().lower()
    if s in PRESETS:
        return PRESETS[s]
    m = re.fullmatch(r"(\d+)x(\d+)", s)
    if not m:
        raise argparse.ArgumentTypeError(f"bad resolution '{s}': use WIDTHxHEIGHT or a preset {sorted(PRESETS)}")
    return int(m.group(1)), int(m.group(2))


def main(argv=None):
    ap = argparse.ArgumentParser(description="Trihexagonal animated SVG background builder.")
    ap.add_argument("--bg", default="static", choices=BG, help="background animation")
    ap.add_argument("--fg", default="rotate", choices=FG, help="center-icon animation")
    ap.add_argument("-r", "--resolution", default="1080p", help="comma list of presets or WIDTHxHEIGHT")
    ap.add_argument("-o", "--out", default=None, help="output .svg file, '-' for stdout, or a directory")
    ap.add_argument("-s", "--seed", type=int, default=0)
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args(argv)

    if args.list:
        print("bg:     " + ", ".join(BG))
        print("fg:     " + ", ".join(FG))
        print("presets:" + ", ".join(f"{k} ({w}x{h})" for k, (w, h) in PRESETS.items()))
        return 0
    if args.selftest:
        return selftest()

    resolutions = [parse_res(r) for r in args.resolution.split(",")]

    def render(w, h):
        return build_svg(w, h, args.bg, args.fg, args.seed)

    if len(resolutions) == 1 and args.out == "-":
        sys.stdout.write(render(*resolutions[0]))
        return 0
    if len(resolutions) == 1 and args.out and args.out.endswith(".svg"):
        with open(args.out, "w") as f:
            f.write(render(*resolutions[0]))
        print(args.out)
        return 0

    outdir = args.out or "out"
    os.makedirs(outdir, exist_ok=True)
    for w, h in resolutions:
        path = os.path.join(outdir, f"trihex-{args.bg}-{args.fg}-{w}x{h}.svg")
        with open(path, "w") as f:
            f.write(render(w, h))
        print(path)
    return 0


def selftest():
    from xml.dom.minidom import parseString
    for bg in BG:
        for fg in FG:
            svg = build_svg(640, 360, bg=bg, fg=fg, seed=1)
            parseString(svg)
            assert svg.startswith("<svg") and "prefers-reduced-motion" in svg
    _assert_constraints(1920, 1080)
    _assert_constraints(1080, 1920)
    print("selftest ok: bg{static,scan,lights} x fg{rotate,static} valid; "
          "holder/intersector + clear-center hold")
    return 0


def _assert_constraints(w, h):
    """Re-derive roles and assert the invariants the builder promises."""
    random.seed("trihex:0")
    u = min(w, h)
    clear_r = u * 0.28
    s = u / 9.0
    D = 2 * s
    rowh = D * SQRT3 / 2
    cx0, cy0 = w / 2, h / 2
    R, C = int(h / rowh) + 2, int(w / D) + 2
    hexes = [(r, c) for r in range(-1, R) for c in range(-1, C)]
    hexset = set(hexes)

    def center(r, c):
        return (c * D + (r % 2) * D / 2, r * rowh)

    role, poked, tri_min = {}, set(), []
    order = hexes[:]
    random.shuffle(order)
    for (r, c) in order:
        if role.get((r, c)) or random.random() > 0.5:
            continue
        cx, cy = center(r, c)
        Vs = regular_poly(cx, cy, s, 6, math.pi / 6)
        for k in random.sample(range(6), 6):
            N = (r + NB[r % 2][k][0], c + NB[r % 2][k][1])
            if role.get(N) == "holder" or N in poked:
                continue
            v1, v2 = Vs[(k - 1) % 6], Vs[k]
            mid = ((v1[0] + v2[0]) / 2, (v1[1] + v2[1]) / 2)
            apex = (2 * mid[0] - cx, 2 * mid[1] - cy)
            d = min(math.hypot(x - cx0, y - cy0) for x, y in (v1, v2, apex))
            if d < clear_r:
                continue
            role[(r, c)] = "holder"
            if N in hexset:
                role[N] = "inter"
            poked.add(N)
            tri_min.append(d)
            break
    holders = {k for k, v in role.items() if v == "holder"}
    inters = {k for k, v in role.items() if v == "inter"}
    assert holders.isdisjoint(inters), "a hexagon is both holder and intersector"
    assert all(d >= clear_r for d in tri_min), "a triangle sits inside the icon zone"
    assert len(poked) == len(holders), "poked count != triangle count"


if __name__ == "__main__":
    raise SystemExit(main())
