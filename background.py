#!/usr/bin/env python3
"""Animated SVG background builder — trihexagonal blueprint.

A hexagon lattice with sparse equilateral triangles, plus a center icon.
Light theme only. The four axes below are independent and combine freely:

  --bg static         no background animation
  --bg scan           hexagon (and triangle) opacities sweep diagonally
  --bg lights         hexagons occasionally light their border, then fill, then rest
  --bg closeopen      every hexagon is a window, mostly shut; a few shrink open
                      onto the image at a time, then close again
  --fg rotate         the hexatri icon's triangle rings counter-rotate
  --fg static         still icon
  --icon hexatri      nested hexagon/triangle glyph
  --icon ship         simple delta spaceship (always static -- see below)
  --bg-image none     plain lattice
  --bg-image space    a few hexagons become windows onto a procedural starfield
  --overlay none      nothing above the lattice
  --overlay matrix    columns of characters at --matrix-angle in --matrix-color.
                      The characters never move: a lit head walks down each
                      column into the next fixed glyph while the ones behind it
                      fade out in place

Animation is pure CSS (no SMIL, no JS) and honours prefers-reduced-motion
(which falls back to the clean static look). Stdlib only, no external assets:
the starfield is drawn, not embedded, so output stays a small self-contained
.svg that is crisp at any resolution. Sizes scale with min(w,h) so pattern
density is constant across resolutions.

Triangle logic: every hexagon is EITHER a "holder" (one of its edges is a
triangle's base) OR an "intersector" (a triangle's tip pokes into it), never
both. Triangles never overlap and never sit behind the center icon.

Space cells sit fully outside the icon's clear zone, and under --bg lights
they pulse their border only -- a pale fill flash would wash the stars out.

Three cross-axis rules, all rejected rather than silently ignored: --fg rotate
applies to --icon hexatri only (the ship is static by design, so --icon ship
resolves --fg to static), --bg closeopen needs a --bg-image to open onto, and
--matrix-angle / --matrix-color need --overlay matrix to steer.

Everything except the animations depends only on --seed, so a given seed
yields the same layout across every bg / fg / icon / bg-image / overlay
combination -- the rain draws from its own stream and never moves a hexagon.

Examples:
  bgsvg --bg lights --fg rotate --resolution 4k --out a.svg
  bgsvg --bg closeopen --bg-image space --resolution 2560x1440 --out b.svg
  bgsvg --overlay matrix --matrix-angle 250 --matrix-color '#395e53cc'
  bgsvg --bg static --resolution mobile,1080p            # -> ./out/*.svg
"""
import argparse
import collections
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

BG = ("static", "scan", "lights", "closeopen")
FG = ("rotate", "static")
ICON = ("hexatri", "ship")
BG_IMAGE = ("none", "space")
OVERLAY = ("none", "matrix")

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


def hex_rgba(s):
    """'#rrggbb' or '#rrggbbaa' -> ('#rrggbb', alpha). Split rather than passed
    through as an 8-digit hex so the trail can scale the alpha per glyph."""
    m = re.fullmatch(r"#([0-9a-fA-F]{6})([0-9a-fA-F]{2})?", s.strip())
    if not m:
        raise ValueError(f"bad colour '{s}': use #rrggbb or #rrggbbaa")
    return "#" + m.group(1).lower(), int(m.group(2) or "ff", 16) / 255


# Light-theme palette: a = hexagons, b = triangles.
PAL = dict(a=darken("#6fb7d1", 0.58), b=darken("#77c9a6", 0.58),
           bg=("#eef3f6", "#d9e3ea"), ink=darken("#6fb7d1", 0.70))

STROKE_O = 0.27   # hexagon border baseline opacity (also the reduced-motion value)
FILL_O = 0.38     # triangle fill baseline opacity

# --bg-image space. VOID stays in the blue-slate family rather than going black:
# a true #000 reads as a hole punched in from a different design.
VOID = darken("#6fb7d1", 0.90)
SPACE_FRAC = 0.08       # share of eligible hexagons that become windows
SPACE_STARS = 24        # scattered per cell bbox; ~75% survive the hexagon clip
SPACE_STROKE_O = 0.34   # a space cell's border sits a touch brighter than the field
# --bg closeopen. Every eligible hexagon is a window, so the *duty cycle* is what
# keeps the field sparse: a cell shows something for ~14% of its period and is
# fully open for ~4%, which leaves a handful open at a time out of ~77. The period
# is long because lowering the ratio alone would turn each opening into a blink.
BLIND_S = (60, 90)      # per-cell shutter period, so cells never sync
# Shutter keyframes as percentages of one cycle: the blind leaves scale(1) at [0],
# is fully open over [1]..[2], and is shut again from [3]. The window behind it
# derives its own on/off keyframes from these, so the two cannot drift apart.
BLIND_KF = (44, 49, 53, 58)
# --overlay matrix. Rule 5 still applies, so a head takes 18-34 s to walk a
# column rather than the second or two the film uses, and only a third of the
# column slots carry one. ASCII only: no font can be embedded without breaking
# the self-contained rule, so a katakana set would be tofu wherever no CJK font
# is installed. The set also excludes < > & " ' so glyphs never need escaping.
MATRIX_GLYPHS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ*+-=/\\|:;.#$%@?!"
MATRIX_COLOR = "#395e53b3"   # PAL['b'] at 0.70: the default stays in-palette
MATRIX_FRAC = 0.34           # share of column slots carrying a stream
MATRIX_S = (18, 34)          # seconds for a head to walk one column
# One glyph's life, as percentages of that walk: it flares at 0, is down to trail
# level by [0], and is out by [1]. The trail is therefore 26% of a column -- ~7
# lit glyphs behind the head -- and [0] is about one cell-time, so the head holds
# its own value for exactly its own turn.
MATRIX_KF = (3, 29)
# On a high-key canvas "brighter" means *more opaque*: the head is the darkest
# glyph and the trail dissolves into the page. Without the step the head reads
# as just another trail glyph, since the falloff alone is ~4% over one cell.
MATRIX_HEAD_STEP = 0.62      # the drop from head to the glyph just behind it


# ---- the shared grid ------------------------------------------------------
Lattice = collections.namedtuple("Lattice", "u s clear_r hexes center cx0 cy0")


def lattice(w, h):
    """The hexagon grid every stage works from -- renderer and asserts alike, so
    they cannot drift apart. Density is constant across resolutions because the
    cell size is tied to min(w,h)."""
    u = min(w, h)
    s = u / 9.0
    D = 2 * s
    rowh = D * SQRT3 / 2
    R, C = int(h / rowh) + 2, int(w / D) + 2
    return Lattice(u, s, u * 0.28,
                   [(r, c) for r in range(-1, R) for c in range(-1, C)],
                   lambda r, c: (c * D + (r % 2) * D / 2, r * rowh),
                   w / 2, h / 2)


# ---- space cells ----------------------------------------------------------
def cell_rng(tag, seed, r, c):
    """Per-cell stream. Keyed by coordinates rather than draw order so a cell's
    stars never shift because some *other* cell consumed a different number of
    values -- that is what keeps the layout identical across bg/fg/icon."""
    return random.Random(f"{tag}:{seed}:{r}:{c}")


def space_cells(lat, seed, every=False):
    """Which hexagons become windows onto the void. A cell qualifies only if the
    whole hexagon clears the icon zone (centre distance >= clear_r + s), the same
    exclusion the triangles obey.

    every=True (--bg closeopen) takes the entire eligible field instead of a
    SPACE_FRAC sample: the blinds hold all but a handful shut at any instant, so
    sparseness moves from space to time and a window can open anywhere rather
    than always in the same seven places."""
    out = set()
    for (r, c) in lat.hexes:
        cx, cy = lat.center(r, c)
        if math.hypot(cx - lat.cx0, cy - lat.cy0) < lat.clear_r + lat.s:
            continue
        if every or cell_rng("pick", seed, r, c).random() < SPACE_FRAC:
            out.add((r, c))
    return out


def space_cell(seed, poly, cx, cy, s, r, c, phase=None):
    """One hexagon of procedural deep space: clipped void ground, a faint nebula,
    then seeded stars. Drawn, never embedded -- no assets, crisp at any size.

    phase (--bg closeopen) is its blind's timing, which makes the cell switch
    itself off while that blind covers it. SVG does no occlusion culling, so
    without this the stars are repainted every frame under a shut blind."""
    g = cell_rng("star", seed, r, c)
    cid = f"sp{r}_{c}"                      # cell coords, so ids are stable
    win = f' class="win" style="{phase}"' if phase else ""
    star = PAL["bg"][0]
    nx, ny = cx + (g.random() - .5) * s, cy + (g.random() - .5) * s
    nrx, nry = s * (.55 + g.random() * .5), s * (.30 + g.random() * .3)
    neb = "neba" if g.random() < .6 else "nebb"
    ang = g.random() * 180
    p = [f'<clipPath id="{cid}"><polygon points="{poly}"/></clipPath>',
         f'<g{win} clip-path="url(#{cid})">',
         f'<polygon points="{poly}" fill="{VOID}"/>',
         f'<ellipse cx="{fmt(nx)}" cy="{fmt(ny)}" rx="{fmt(nrx)}" ry="{fmt(nry)}" '
         f'fill="url(#{neb})" transform="rotate({fmt(ang)} {fmt(nx)} {fmt(ny)})"/>']
    for i in range(SPACE_STARS):
        x = cx + (g.random() - .5) * 2 * s
        y = cy + (g.random() - .5) * SQRT3 * s
        rad = s * (.008 + g.random() ** 2 * .022)
        o = .35 + g.random() * .6
        if i < 2:                                   # two anchor stars get a soft bloom
            p.append(f'<circle cx="{fmt(x)}" cy="{fmt(y)}" r="{fmt(rad * 3.5)}" '
                     f'fill="{star}" fill-opacity="0.12"/>')
            rad *= 1.4
        p.append(f'<circle cx="{fmt(x)}" cy="{fmt(y)}" r="{fmt(rad)}" fill="{star}" '
                 f'fill-opacity="{fmt(o)}"/>')
    p.append("</g>")
    return "".join(p)


# ---- background pattern ---------------------------------------------------
def blind_phase(seed, r, c):
    """--bg closeopen: the timing of one cell's shutter, drawn from the cell's own
    stream rather than draw order. The blind and the window it covers are given
    this same string, so the window can switch itself off exactly while it is
    hidden -- one value, two users, no way for them to fall out of step."""
    g = cell_rng("blind", seed, r, c)
    d = BLIND_S[0] + g.random() * (BLIND_S[1] - BLIND_S[0])
    return f"animation-delay:-{fmt(g.random()*d)}s;animation-duration:{fmt(d)}s"


def pat_trihex(w, h, lat, bg, bg_image="none", seed=0):
    """Hexagon lattice (spacing 2s) + sparse triangles under the holder/
    intersector rule, styled per the chosen background animation."""
    a, b, ink = PAL["a"], PAL["b"], PAL["ink"]
    s, clear_r, hexes, center = lat.s, lat.clear_r, lat.hexes, lat.center
    cx0, cy0 = lat.cx0, lat.cy0
    sw = fmt(lat.u * 0.0013)
    hexset = set(hexes)

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

    space = space_cells(lat, seed, bg == "closeopen") if bg_image == "space" else set()

    voids, out = [], []
    for (r, c) in hexes:
        cx, cy = center(r, c)
        poly = pts(regular_poly(cx, cy, s, 6, math.pi / 6))
        void = (r, c) in space
        if void:
            phase = blind_phase(seed, r, c) if bg == "closeopen" else None
            voids.append(space_cell(seed, poly, cx, cy, s, r, c, phase))
            if phase:
                # canvas-coloured, so a shut blind is indistinguishable from any
                # other lattice cell -- see the userSpaceOnUse note on #bg
                voids.append(f'<polygon class="blind" style="{phase}" '
                             f'points="{poly}" fill="url(#bg)"/>')
        # A window's border sits brighter than the field to mark the few portals --
        # but under closeopen *every* cell is a portal, so that would just raise the
        # whole lattice (rule 1). There the blind opening is the only marker.
        so = SPACE_STROKE_O if void and bg != "closeopen" else STROKE_O
        if bg == "scan":
            out.append(f'<polygon class="scan" style="animation-delay:{scan_delay(cx,cy)}s" '
                       f'points="{poly}" fill="none" stroke="{ink}" stroke-opacity="{so}" stroke-width="{sw}"/>')
        elif bg == "lights" and void:
            # border-only pulse: the usual pale fill flash would wash the stars out
            out.append(f'<polygon class="lightb" style="animation-delay:-'
                       f'{fmt(cell_rng("delay", seed, r, c).random()*9)}s" points="{poly}" fill="none" '
                       f'stroke="{ink}" stroke-opacity="{so}" stroke-width="{sw}"/>')
        elif bg == "lights":
            out.append(f'<polygon class="light" style="animation-delay:-{fmt(random.random()*9)}s;'
                       f'animation-duration:{fmt(7+random.random()*5)}s" points="{poly}" fill="{a}" '
                       f'fill-opacity="0" stroke="{ink}" stroke-opacity="{so}" stroke-width="{sw}"/>')
        else:
            out.append(f'<polygon points="{poly}" fill="none" stroke="{ink}" '
                       f'stroke-opacity="{so}" stroke-width="{sw}"/>')

    fills = []
    for (v1, v2, apex, col) in tris:
        if bg == "scan":
            tx, ty = (v1[0] + v2[0] + apex[0]) / 3, (v1[1] + v2[1] + apex[1]) / 3
            fills.append(f'<polygon class="wavef" style="animation-delay:{scan_delay(tx,ty)}s" '
                         f'points="{pts([v1,v2,apex])}" fill="{col}" fill-opacity="{FILL_O}"/>')
        else:
            fills.append(f'<polygon points="{pts([v1,v2,apex])}" fill="{col}" fill-opacity="{FILL_O}"/>')
    # voids sit under the triangles, so a translucent triangle crossing a window
    # reads as a shard catching light; borders go on top of everything.
    return voids + fills + out


# ---- overlay --------------------------------------------------------------
def pat_matrix(w, h, lat, seed, angle, color):
    """--overlay matrix: columns of characters that stay put while a lit head
    walks down them at `angle` degrees (0 = downward, increasing clockwise).

    NOTHING MOVES. Every cell of a column holds one character, chosen once and
    fixed for good; what travels is the lighting. Each glyph runs the same
    keyframes -- flare to --o as the head arrives, step down to --t, fade out
    over the trail -- offset by one cell-time per cell, so the head keeps
    advancing into a fresh character while the ones behind it dim in place.
    Translating the glyphs instead would slide the characters across the canvas,
    which is the one thing this effect must not do.

    The band is the canvas seen from the travel direction -- P across, T along --
    so no cell is ever placed somewhere the canvas cannot reach. Cells are placed
    by rotating their band coordinates here rather than by wrapping them in a
    rotated group: glyphs have to stay upright at every angle, so a rotated group
    would need a counter-rotation on every single one of them. Rotating in Python
    leaves the output with no transform at all (~45 bytes a glyph, and at a steep
    angle there are ~500 of them).

    Each glyph's fill-opacity attribute holds the value its keyframes give it at
    t=0, so prefers-reduced-motion freezes the true opening frame rather than
    some other state: a still field of columns, each lit for the length of its
    trail. The attribute loses to the animation while one is running.

    Drawn under the halo, so the halo subtracts the rain around the icon the way
    it subtracts the lattice -- the focal point stays uncontested for free.

    Columns draw from cell_rng, off the global stream the lattice uses, so
    switching the overlay on cannot move a hexagon. Keyed by slot rather than
    draw order for the reason cell_rng exists: the slot count follows the angle,
    so a shared stream would reshuffle every glyph when the rain is merely
    re-aimed."""
    rgb, alpha = hex_rgba(color)
    hold, out_at = MATRIX_KF
    rad = math.radians(angle)
    co, si = math.cos(rad), math.sin(rad)
    P, T = w * abs(co) + h * abs(si), w * abs(si) + h * abs(co)   # the band, across x along
    fs = lat.u / 28.0
    pitch, step = fs * 1.6, fs * 1.05
    cx0, cy0 = lat.cx0, lat.cy0
    N = int(T / step) + 2                        # cells spanning the band, head to tail
    out = [f'<g class="matrix" font-family="monospace" font-size="{fmt(fs)}" '
           f'text-anchor="middle" fill="{rgb}" '
           f'style="--o:{fmt(alpha)};--t:{fmt(alpha * MATRIX_HEAD_STEP)}">']
    for i in range(int(P / pitch) + 1):
        g = cell_rng("rain", seed, 0, i)
        if g.random() > MATRIX_FRAC:
            continue
        dx = i * pitch - P / 2
        dur = g.uniform(*MATRIX_S)
        head = g.randrange(N)                    # the cell lit at t=0
        # --d is inherited by every glyph below, so a column sets its speed once
        # instead of repeating animation-duration on all N of them
        out.append(f'<g style="--d:{fmt(dur)}s">')
        for j in range(N):
            dy = j * step - T / 2
            pct = 100.0 * ((head - j) % N) / N    # how far into its cycle this cell is
            if pct <= hold:                       # ...evaluated against the keyframes,
                o = alpha * (1 - (1 - MATRIX_HEAD_STEP) * pct / hold)
            elif pct <= out_at:
                o = alpha * MATRIX_HEAD_STEP * (1 - (pct - hold) / (out_at - hold))
            else:
                o = 0.0
            out.append(f'<text class="rain" x="{fmt(cx0 + dx * co - dy * si)}" '
                       f'y="{fmt(cy0 + dx * si + dy * co)}" '
                       f'style="animation-delay:-{fmt(dur * (head + N - j) / N)}s" '
                       f'fill-opacity="{fmt(o)}">{g.choice(MATRIX_GLYPHS)}</text>')
        out.append("</g>")
    out.append("</g>")
    return "".join(out)


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


def ico_ship():
    """Simple delta spaceship on the same 200-unit grid as ico_hexatri, in the
    same thin-outline language: swept hull, narrow fuselage, the hexagonal
    cockpit carried over from the other glyph, and a two-dash exhaust. No rotate
    variant -- a spinning ship reads as a crash, not as ambience."""
    a, b = PAL["a"], PAL["b"]
    # A narrow fuselage read against a wide swept delta -- two *different* shapes.
    # Concentric copies of one delta just read as a chevron.
    hull = [(0, -80), (68, 46), (0, 20), (-68, 46)]
    fuselage = [(0, -64), (17, 34), (-17, 34)]
    parts = [f'<polygon points="{pts(hull)}" fill="none" stroke="{a}" stroke-width="3.6"/>',
             f'<polygon points="{pts(fuselage)}" fill="none" stroke="{b}" stroke-width="2.4" '
             f'stroke-opacity="0.8"/>',
             f'<polygon points="{pts(regular_poly(0,-22,8,6,math.pi/6))}" fill="{a}" '
             f'fill-opacity="0.28" stroke="{a}" stroke-width="2"/>']
    for i, (hw, y) in enumerate(((14, 42), (8, 54))):   # exhaust, aft of the wing roots
        parts.append(f'<line x1="{-hw}" y1="{y}" x2="{hw}" y2="{y}" stroke="{b}" '
                     f'stroke-width="2.4" stroke-opacity="{fmt(0.6 - i * 0.28)}"/>')
    return "".join(parts)


# ---- assembly -------------------------------------------------------------
def css():
    k0, k1, k2, k3 = BLIND_KF
    f0, f1 = MATRIX_KF
    return ("<style>"
            "@keyframes spin{to{transform:rotate(360deg)}}"
            "@keyframes rspin{to{transform:rotate(-360deg)}}"
            "@keyframes scan{0%,100%{stroke-opacity:.16}50%{stroke-opacity:.62}}"
            "@keyframes wavef{0%,100%{fill-opacity:.12}50%{fill-opacity:.6}}"
            "@keyframes light{0%{stroke-opacity:.27;fill-opacity:0}7%{stroke-opacity:.85;fill-opacity:0}"
            "16%{stroke-opacity:.85;fill-opacity:.42}26%{stroke-opacity:.27;fill-opacity:0}"
            "100%{stroke-opacity:.27;fill-opacity:0}}"
            "@keyframes lightb{0%{stroke-opacity:.34}8%{stroke-opacity:.9}"
            "24%{stroke-opacity:.34}100%{stroke-opacity:.34}}"
            # closing is opening played backwards, so one symmetric cycle covers both.
            # Mostly shut: 86% closed, ~5% shrinking, 4% open, ~5% growing back.
            f"@keyframes blind{{0%,{k0}%{{transform:scale(1)}}"
            f"{k1}%,{k2}%{{transform:scale(0)}}{k3}%,100%{{transform:scale(1)}}}}"
            # ...and the window switches off whenever its blind covers it, with a
            # 1% margin either side so the stars are already there before the blind
            # starts to move. Both spans come from BLIND_KF: they cannot drift.
            f"@keyframes winvis{{0%,{k0-2}%{{display:none}}"
            f"{k0-1}%,{k3+1}%{{display:inline}}{k3+2}%,100%{{display:none}}}}"
            ".spin{animation:spin 24s linear infinite;transform-box:fill-box;transform-origin:center}"
            ".rspin{animation:rspin 24s linear infinite;transform-box:fill-box;transform-origin:center}"
            ".scan{animation:scan 5s ease-in-out infinite}"
            ".wavef{animation:wavef 5s ease-in-out infinite}"
            ".light{animation:light 9s ease-in-out infinite}"
            ".lightb{animation:lightb 9s ease-in-out infinite}"
            # both rest in the *open* state: prefers-reduced-motion kills the
            # animations below, and a blind stuck shut (or a window stuck off)
            # would hide the starfield entirely
            ".blind{animation:blind 75s ease-in-out infinite;transform-box:fill-box;"
            "transform-origin:center;transform:scale(0)}"
            ".win{animation:winvis 75s ease-in-out infinite;display:inline}"
            # One glyph's life, and the only thing --overlay matrix animates: the
            # characters never move, the lighting does. --o/--t (the colour's alpha
            # and its trail step) and --d (the column's speed) are all inherited
            # from ancestors, so a glyph itself only has to carry its delay.
            f"@keyframes rain{{0%{{fill-opacity:var(--o)}}{f0}%{{fill-opacity:var(--t)}}"
            f"{f1}%,100%{{fill-opacity:0}}}}"
            ".rain{animation:rain var(--d) linear infinite}"
            "@media (prefers-reduced-motion:reduce){*{animation:none!important}}"
            "</style>")


def build_svg(w, h, bg="static", fg="rotate", icon="hexatri", bg_image="none", seed=0,
              overlay="none", matrix_angle=0.0, matrix_color=MATRIX_COLOR):
    random.seed(f"trihex:{seed}")   # layout depends only on seed, not bg/fg/icon/image
    lat = lattice(w, h)
    u, clear_r = lat.u, lat.clear_r
    defs = [
        # userSpaceOnUse so any shape can paint canvas: the default objectBoundingBox
        # would squeeze the whole ramp into a single hexagon, and a closed blind
        # (--bg closeopen) would read as a patch instead of vanishing into the page.
        f'<linearGradient id="bg" gradientUnits="userSpaceOnUse" x1="0" y1="0" '
        f'x2="{fmt(w * 0.4)}" y2="{fmt(h)}"><stop offset="0%" stop-color="{PAL["bg"][0]}"/>'
        f'<stop offset="100%" stop-color="{PAL["bg"][1]}"/></linearGradient>',
        '<radialGradient id="vig"><stop offset="55%" stop-color="#8fa3b8" stop-opacity="0"/>'
        '<stop offset="100%" stop-color="#8fa3b8" stop-opacity="0.16"/></radialGradient>',
        f'<radialGradient id="halo"><stop offset="0%" stop-color="{PAL["bg"][0]}" stop-opacity="0.92"/>'
        f'<stop offset="55%" stop-color="{PAL["bg"][0]}" stop-opacity="0.65"/>'
        f'<stop offset="100%" stop-color="{PAL["bg"][0]}" stop-opacity="0"/></radialGradient>',
        '<filter id="ink" x="-30%" y="-30%" width="160%" height="160%">'
        '<feDropShadow dx="0" dy="2" stdDeviation="3" flood-color="#1e293b" flood-opacity="0.25"/></filter>',
    ]
    if bg_image == "space":
        defs += [f'<radialGradient id="{gid}"><stop offset="0%" stop-color="{col}" stop-opacity="0.3"/>'
                 f'<stop offset="100%" stop-color="{col}" stop-opacity="0"/></radialGradient>'
                 for gid, col in (("neba", "#6fb7d1"), ("nebb", "#77c9a6"))]

    bg_svg = pat_trihex(w, h, lat, bg, bg_image, seed)
    rain_svg = pat_matrix(w, h, lat, seed, matrix_angle, matrix_color) if overlay == "matrix" else ""
    k = u * 0.34 / 200
    glyph = ico_ship() if icon == "ship" else ico_hexatri(fg)
    icon_svg = (f'<g transform="translate({fmt(w/2)},{fmt(h/2)}) scale({fmt(k)})" filter="url(#ink)">'
                f'{glyph}</g>')

    label = ("trihexagonal background with a spaceship icon" if icon == "ship"
             else "trihexagonal background")
    if bg_image == "space":
        label += (", some hexagons opening and closing onto a starfield"
                  if bg == "closeopen" else ", some hexagons showing a starfield")
    if overlay == "matrix":
        label += ", with streams of characters drifting across it"
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {fmt(w)} {fmt(h)}" width="{fmt(w)}" height="{fmt(h)}" '
        f'preserveAspectRatio="xMidYMid slice" role="img" aria-label="{label}"><title>{label}</title>'
        f'<defs>{"".join(defs)}</defs>{css()}'
        f'<rect width="{fmt(w)}" height="{fmt(h)}" fill="url(#bg)"/>'
        f'<g>{"".join(bg_svg)}</g>{rain_svg}'
        f'<circle cx="{fmt(w/2)}" cy="{fmt(h/2)}" r="{fmt(clear_r)}" fill="url(#halo)"/>'
        f'<rect width="{fmt(w)}" height="{fmt(h)}" fill="url(#vig)"/>{icon_svg}</svg>\n')


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
    ap.add_argument("--fg", default=None, choices=FG,
                    help="center-icon animation (default: rotate for hexatri, static for ship)")
    ap.add_argument("--icon", default="hexatri", choices=ICON, help="center glyph")
    ap.add_argument("--bg-image", dest="bg_image", default="none", choices=BG_IMAGE,
                    help="imagery inside some hexagons (required by --bg closeopen)")
    ap.add_argument("--overlay", default="none", choices=OVERLAY, help="layer above the lattice")
    ap.add_argument("--matrix-angle", dest="matrix_angle", type=float, default=None,
                    help="--overlay matrix travel direction, 0-360 (0 = down, clockwise; default 0)")
    ap.add_argument("--matrix-color", dest="matrix_color", default=None,
                    help=f"--overlay matrix colour, #rrggbb or #rrggbbaa (default {MATRIX_COLOR})")
    ap.add_argument("-r", "--resolution", default="1080p", help="comma list of presets or WIDTHxHEIGHT")
    ap.add_argument("-o", "--out", default=None, help="output .svg file, '-' for stdout, or a directory")
    ap.add_argument("-s", "--seed", type=int, default=0)
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args(argv)

    if args.list:
        print("bg:       " + ", ".join(BG) + "   (closeopen: --bg-image space only)")
        print("fg:       " + ", ".join(FG) + "   (rotate: --icon hexatri only)")
        print("icon:     " + ", ".join(ICON))
        print("bg-image: " + ", ".join(BG_IMAGE))
        print("overlay:  " + ", ".join(OVERLAY)
              + f"   (matrix: --matrix-angle 0-360, --matrix-color #rrggbbaa, default {MATRIX_COLOR})")
        print("presets:  " + ", ".join(f"{k} ({w}x{h})" for k, (w, h) in PRESETS.items()))
        return 0
    if args.selftest:
        return selftest()

    if args.icon == "ship" and args.fg == "rotate":
        ap.error("--fg rotate is not available for --icon ship (the ship is static by "
                 "design); drop --fg, or use --icon hexatri")
    if args.bg == "closeopen" and args.bg_image == "none":
        ap.error("--bg closeopen has nothing to reveal with --bg-image none (its "
                 "hexagons open onto a background image); add --bg-image space, or "
                 "use --bg lights")
    if args.overlay != "matrix":
        for flag, val in (("--matrix-angle", args.matrix_angle),
                          ("--matrix-color", args.matrix_color)):
            if val is not None:
                ap.error(f"{flag} has no rain to steer with --overlay {args.overlay}; "
                         f"add --overlay matrix, or drop {flag}")
    angle = args.matrix_angle if args.matrix_angle is not None else 0.0
    if not 0 <= angle <= 360:
        ap.error(f"--matrix-angle {fmt(angle)} is outside 0-360 (0 = falling down, "
                 "increasing clockwise)")
    color = args.matrix_color or MATRIX_COLOR
    try:
        hex_rgba(color)
    except ValueError as e:
        ap.error(f"--matrix-color: {e}")
    fg = args.fg or ("static" if args.icon == "ship" else "rotate")

    resolutions = [parse_res(r) for r in args.resolution.split(",")]

    def render(w, h):
        return build_svg(w, h, args.bg, fg, args.icon, args.bg_image, args.seed,
                         args.overlay, angle, color)

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
    for w, h in resolutions:   # every axis, always, in flag order -- no optional parts
        path = os.path.join(
            outdir,
            f"trihex-{args.bg}-{fg}-{args.icon}-{args.bg_image}-{args.overlay}-{w}x{h}.svg")
        with open(path, "w") as f:
            f.write(render(w, h))
        print(path)
    return 0


def selftest():
    from xml.dom.minidom import parseString
    n = 0
    for bg in BG:
        for fg in FG:
            for icon in ICON:
                for img in BG_IMAGE:
                    for ov in OVERLAY:
                        svg = build_svg(640, 360, bg=bg, fg=fg, icon=icon, bg_image=img,
                                        seed=1, overlay=ov)
                        parseString(svg)
                        assert svg.startswith("<svg") and "prefers-reduced-motion" in svg
                        assert ("<line x1=" in svg) == (icon == "ship"), "icon dispatch is wrong"
                        assert ("<clipPath" in svg) == (img == "space"), "bg-image dispatch is wrong"
                        assert ('class="blind"' in svg) == (bg == "closeopen" and img == "space"), \
                            "blinds must exist exactly when closeopen has windows to cover"
                        assert ('class="rain"' in svg) == (ov == "matrix"), "overlay dispatch is wrong"
                        n += 1
    _assert_constraints(1920, 1080)
    _assert_constraints(1080, 1920)
    _assert_space(1920, 1080)
    _assert_space(1080, 1920)
    _assert_matrix(1920, 1080)
    _assert_matrix(1080, 1920)

    _assert_rejected(["--icon", "ship", "--fg", "rotate", "-o", "-"],
                     "--icon ship --fg rotate must be rejected")
    _assert_rejected(["--bg", "closeopen", "-o", "-"],
                     "--bg closeopen --bg-image none must be rejected")
    _assert_rejected(["--matrix-angle", "90", "-o", "-"],
                     "--matrix-angle without --overlay matrix must be rejected")
    _assert_rejected(["--matrix-color", "#112233", "-o", "-"],
                     "--matrix-color without --overlay matrix must be rejected")
    _assert_rejected(["--overlay", "matrix", "--matrix-angle", "400", "-o", "-"],
                     "an out-of-range --matrix-angle must be rejected")
    _assert_rejected(["--overlay", "matrix", "--matrix-color", "395e53", "-o", "-"],
                     "a malformed --matrix-color must be rejected")

    print(f"selftest ok: {n} bg x fg x icon x bg-image x overlay combos valid; "
          "holder/intersector, clear-center, space-cell clearance/lights-opt-out/blind "
          "layering hold; rain stays upright, fades from its head and never moves the "
          "lattice; ship rejects --fg rotate, closeopen rejects --bg-image none, the "
          "--matrix-* flags reject a missing overlay, a bad angle and a bad colour")
    return 0


def _assert_rejected(argv, why):
    import contextlib
    import io
    with contextlib.redirect_stderr(io.StringIO()):      # argparse prints usage
        try:
            main(argv)
        except SystemExit as e:
            assert e.code == 2, f"expected argparse exit 2, got {e.code}"
        else:
            raise AssertionError(why)


def _assert_space(w, h):
    """Space cells must clear the icon zone entirely, stay sparse, never take the
    fill-flashing .light class (which would wash the starfield out), and under
    --bg closeopen carry exactly one blind apiece, layered under the triangles."""
    lat = lattice(w, h)
    space = space_cells(lat, 0)
    for (r, c) in space:
        cx, cy = lat.center(r, c)
        d = math.hypot(cx - lat.cx0, cy - lat.cy0)
        assert d - lat.s >= lat.clear_r, "a space cell overlaps the icon zone"
    assert space, "no space cells were placed"
    assert len(space) <= len(lat.hexes) * SPACE_FRAC * 2, "space cells are not sparse"

    svg = build_svg(w, h, bg="lights", bg_image="space", seed=0)
    assert svg.count('<clipPath id="sp') == len(space), "rendered space cells != selected"
    assert svg.count('class="lightb"') == len(space), "a space cell is missing its border pulse"
    assert svg.count('class="light"') == len(lat.hexes) - len(space), "a space cell got the fill flash"

    every = space_cells(lat, 0, every=True)
    assert space < every, "closeopen must widen the window pool, not reuse the sparse one"
    for (r, c) in every:                      # the icon zone stays clear even so
        cx, cy = lat.center(r, c)
        assert math.hypot(cx - lat.cx0, cy - lat.cy0) - lat.s >= lat.clear_r, \
            "a closeopen window overlaps the icon zone"

    svg = build_svg(w, h, bg="closeopen", bg_image="space", seed=0)
    assert svg.count('class="blind"') == len(every), "every eligible hexagon must be a window"
    # fill *and* opacity: a lone fill-opacity="0.38" also matches a star, and with
    # every cell a window there are now enough stars to hit that value by chance.
    tris = [i for i in (svg.find(f'fill="{PAL[k]}" fill-opacity="{FILL_O}"') for k in "ab") if i >= 0]
    assert tris, "no triangle to check blind layering against"
    assert svg.rindex('class="blind"') < min(tris), \
        "a blind is painted over the triangles instead of under them"
    assert f"transform-origin:center;transform:scale(0)}}" in svg, \
        "blinds must rest open, so prefers-reduced-motion still shows the starfield"
    assert ".win{animation:winvis 75s ease-in-out infinite;display:inline}" in svg, \
        "windows must rest rendered, for the same reason"
    assert f'stroke-opacity="{SPACE_STROKE_O}"' not in svg, \
        "closeopen windows must not raise the whole lattice to the window border opacity"

    # Every window carries its own blind's timing, so it hides exactly while
    # covered. Desync here shows up as a starfield popping in over a shut blind.
    wins = re.findall(r'class="win" style="([^"]+)"', svg)
    blinds = re.findall(r'class="blind" style="([^"]+)"', svg)
    assert wins == blinds != [], "a window is out of phase with its own blind"
    assert 'class="win"' not in build_svg(w, h, bg="lights", bg_image="space", seed=0), \
        "a bg without blinds has nothing covering its windows, so they must never switch off"


def _assert_matrix(w, h):
    """The characters must stay put while the lighting walks the column, stay
    upright at any angle, and leave the lattice byte-identical: the overlay is a
    layer, never a layout input."""
    from xml.dom.minidom import parseString

    def polys(s):                       # the lattice and its triangles, verbatim
        return re.findall(r"<polygon[^>]*>", s)

    def columns(s):
        return re.findall(r'<g style="--d:[^"]*">(.*?)</g>', s)

    plain = build_svg(w, h, bg="lights", bg_image="space", seed=3)
    for angle in (0, 90, 181, 359.5):
        svg = build_svg(w, h, bg="lights", bg_image="space", seed=3,
                        overlay="matrix", matrix_angle=angle)
        parseString(svg)
        assert polys(svg) == polys(plain), f"the overlay moved the lattice at {fmt(angle)} deg"

        # Cells are rotated into place in Python, so glyphs are upright at every
        # angle for free. A transform creeping back in means either tilted
        # characters or ~45 wasted bytes on each of ~500 of them.
        layer = svg[svg.index('class="matrix"'):svg.index("<circle")]
        assert "transform" not in layer, \
            f"a rain glyph carries a transform at {fmt(angle)} deg"
        glyphs = re.findall(r"<text[^>]*>(.)</text>", svg)
        assert glyphs and set(glyphs) <= set(MATRIX_GLYPHS), "a glyph is off the set"

        cols = columns(svg)
        assert cols, "no rain columns were placed"
        assert len({col.count("<text") for col in cols}) == 1, \
            "every column must span the whole band, or a head would stop mid-canvas"
        for col in cols:
            o = [float(v) for v in re.findall(r'fill-opacity="([\d.]+)"', col)]
            lit = [v for v in o if v]
            assert 0 < len(lit) < len(o), \
                "a column must be part lit and part dark -- that gap is the trail"
            assert o.count(max(o)) == 1, "a column must have exactly one head"
            # Walking back from the head must give one contiguous run that only
            # ever dims. Anything else means the stagger is not advancing the
            # lighting a single cell at a time.
            head = o.index(max(o))
            run = [o[(head - k) % len(o)] for k in range(len(lit))]
            assert all(run) and run == sorted(run, reverse=True), \
                "the trail must be one contiguous run fading back from the head"

        # The stagger IS the travel: consecutive cells are one cell-time apart, so
        # the lighting advances by exactly one character per step. Collapse these
        # to a single delay and the whole column flashes at once instead.
        d = [float(v) for v in re.findall(r"animation-delay:-([\d.]+)s", cols[0])]
        gaps = [d[k] - d[k + 1] for k in range(len(d) - 1)]
        assert min(gaps) > 0 and max(gaps) - min(gaps) <= 0.02, \
            "glyph delays must step by one constant cell-time down the column"

    # The characters are anchored by x/y and only their opacity is animated. An
    # animated transform here would slide them across the canvas, which is the
    # one thing this effect must not do.
    assert "translate" not in re.search(r"@keyframes rain\{[^}]*\}[^}]*\}", svg).group(), \
        "the rain keyframes must animate opacity only -- characters never move"
    assert ".rain{animation:rain var(--d) linear infinite}" in svg, \
        "columns must take their speed from the inherited --d"

    # The colour is split into fill + alpha (the trail scales it) rather than
    # passed through as an 8-digit hex, and the alpha reaches the glyphs as --o.
    svg = build_svg(w, h, overlay="matrix", matrix_color="#8899aa80")
    assert 'fill="#8899aa"' in svg and "#8899aa80" not in svg, "--matrix-color is not applied"
    assert f"--o:{fmt(0x80 / 255)};" in svg, "the colour's alpha never reaches the glyphs"


def _assert_constraints(w, h):
    """Re-derive roles and assert the invariants the builder promises."""
    random.seed("trihex:0")
    lat = lattice(w, h)
    s, clear_r, hexes, center = lat.s, lat.clear_r, lat.hexes, lat.center
    cx0, cy0 = lat.cx0, lat.cy0
    hexset = set(hexes)

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
