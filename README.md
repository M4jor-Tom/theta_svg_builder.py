# bgsvg — trihexagonal animated SVG background builder

Generates a self-contained `.svg`: a **hexagon lattice with sparse triangles**
plus a **center icon**, light theme. Animation is pure CSS (no SMIL, no JS) and
honours `prefers-reduced-motion` (which falls back to the clean static look).
Python stdlib only, no external assets. Sizes scale with `min(width, height)`,
so pattern density is constant across resolutions.

The graphic mood — and the rules for extending it without breaking it — is
documented in [`docs/mood/`](docs/mood/).

## Animations

- `--bg static` — no background animation.
- `--bg scan` — hexagon (and triangle) opacities sweep diagonally across the canvas.
- `--bg lights` — hexagons occasionally light their **border**, then their **fill**, then fade back; random per-hexagon timing, so only a few glow at once.
- `--bg closeopen` — **every** hexagon becomes a window, each covered by a **blind** that shrinks about its own centre, so the picture behind it appears as a ring widening from the border inward, holds open, then grows shut again. Closing is opening played backwards. Blinds stay shut 86% of a 60–90 s cycle with a random phase per cell, so at 1080p roughly 11 of the 76 windows show anything at a given moment and only ~3 are fully open — the field stays sparse in **time** rather than in space, and a window can open anywhere. Needs a `--bg-image` to open onto.
- `--fg rotate` — the icon's two triangle rings counter-rotate inside the static hex frame.
- `--fg static` — still icon.

## Icons

- `--icon hexatri` — the nested hexagon/triangle glyph. Supports `--fg rotate`.
- `--icon ship` — a simple delta spaceship: narrow fuselage, wide swept wings, hexagonal cockpit.

The ship is **static by design**, so `--icon ship` resolves `--fg` to `static`.
Passing `--fg rotate` with it is rejected rather than silently ignored:

```sh
$ bgsvg --icon ship --fg rotate
error: --fg rotate is not available for --icon ship (the ship is static by design); ...
```

## Background images

- `--bg-image none` — plain lattice.
- `--bg-image space` — ~8% of hexagons become windows onto a **procedurally drawn starfield**: void ground, a faint nebula, and seeded stars. Drawn rather than embedded, so there are no assets and output stays small and crisp at any resolution (a 4k frame is ~48 KB).

Space cells always clear the icon zone, and under `--bg lights` they pulse their
border only — the usual fill flash would wash the stars out.

`--bg closeopen` is the exception to the ~8%: it needs a window everywhere, so it
draws a starfield in **every** eligible hexagon and lets the blinds do the
rationing. That is ~76 starfields instead of ~7, which costs file size — a 4k
frame goes from ~48 KB to ~222 KB (~35 KB gzipped). Their borders also stay at
the normal lattice opacity rather than the brighter window value, since marking
every cell would just raise the whole field.

Because SVG does no occlusion culling, a window would otherwise be repainted every
frame underneath the shut, fully opaque blind hiding it. Each window therefore
switches itself off (`display:none`) for exactly the span its own blind covers it,
which measured **+22% frame rate, 5–10× fewer dropped frames and ~110 MiB less
renderer memory** at 1080p. Both keyframe spans are derived from one constant, so
a window cannot fall out of step with its blind.

`--bg closeopen` is the animation built for this axis, and it needs something to
reveal, so it is rejected with `--bg-image none` rather than rendering nothing:

```sh
$ bgsvg --bg closeopen
error: --bg closeopen has nothing to reveal with --bg-image none ...
```

All four axes are independent and combine freely, apart from those two
documented exceptions — `--icon ship --fg rotate` and `--bg closeopen
--bg-image none` — both rejected rather than silently ignored. Everything except
the animations depends only on `--seed`, so one seed gives the same layout across
every combination.

## Triangle logic

Every hexagon plays exactly one role, never both:

- **holder** — one of its edges is the base of a triangle, or
- **intersector** — a triangle's tip pokes into it.

A holder claims a single edge and marks the poked neighbour as an intersector
(locked out of holding, pierced at most once) → **few triangles, no overlaps**,
and a **clear center** (no triangle in the icon zone, plus a soft halo behind
the icon).

## Run

```sh
nix run .#bgsvg -- --bg lights --fg rotate --resolution 4k --out wall.svg
nix run .#bgsvg -- --bg closeopen --bg-image space --resolution 2560x1440 --out b.svg
nix run .#bgsvg -- --bg static --resolution mobile,1080p          # -> ./out/*.svg
nix run .#bgsvg -- --bg lights --resolution 2560x1440 --out -     # stdout
nix run .#bgsvg -- --list                                         # every axis + presets
```

Without Nix: `python3 background.py --bg lights --out t.svg` (needs Python 3).

## Options

| flag | default | meaning |
|------|---------|---------|
| `--bg` | `static` | background animation: `static` `scan` `lights` `closeopen` (`closeopen` needs `--bg-image space`) |
| `--fg` | per icon | icon animation: `rotate` `static` (default `rotate` for `hexatri`, `static` for `ship`) |
| `--icon` | `hexatri` | center glyph: `hexatri` `ship` |
| `--bg-image` | `none` | imagery inside some hexagons: `none` `space` |
| `-r, --resolution` | `1080p` | comma list of presets or `WIDTHxHEIGHT` |
| `-o, --out` | `./out/` | `.svg` file (single), `-` for stdout, or a directory |
| `-s, --seed` | `0` | random seed (layout depends only on seed, not on bg/fg/icon/bg-image) |

Writing to a directory names each file for every axis in flag order:
`trihex-<bg>-<fg>-<icon>-<bg-image>-<W>x<H>.svg`.

**Presets:** `720p 1080p 1440p 4k mobile tablet square ultrawide`.

## Check

```sh
python3 background.py --selftest
```

Parses all 32 `bg` × `fg` × `icon` × `bg-image` combinations as XML and asserts
the invariants: holder/intersector roles, the clear center, space-cell clearance
and their opt-out of the fill flash, one blind per window layered *under* the
triangles, every window sharing its own blind's phase, blinds and windows resting
open so `prefers-reduced-motion` still shows the starfield, and that both
`--icon ship --fg rotate` and `--bg closeopen --bg-image none` are rejected.
