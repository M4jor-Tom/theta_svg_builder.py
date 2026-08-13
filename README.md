# bgsvg — trihexagonal animated SVG background builder

Generates a self-contained `.svg`: a **hexagon lattice with sparse triangles**
plus a **nested hexagon/triangle center icon**, light theme. Background and
foreground animations are chosen independently and combine freely. Animation is
pure CSS (no SMIL, no JS) and honours `prefers-reduced-motion` (which falls back
to the clean static look). Python stdlib only. Sizes scale with
`min(width, height)`, so pattern density is constant across resolutions.

## Animations

- `--bg static` — no background animation.
- `--bg scan` — hexagon (and triangle) opacities sweep diagonally across the canvas.
- `--bg lights` — hexagons occasionally light their **border**, then their **fill**, then fade back; random per-hexagon timing, so only a few glow at once.
- `--fg rotate` — the icon's two triangle rings counter-rotate inside the static hex frame.
- `--fg static` — still icon.

`--bg` and `--fg` are independent; any pair works.

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
nix run .#bgsvg -- --bg scan --fg static --resolution 2560x1440 --out b.svg
nix run .#bgsvg -- --bg static --resolution mobile,1080p          # -> ./out/*.svg
nix run .#bgsvg -- --bg lights --resolution 2560x1440 --out -     # stdout
nix run .#bgsvg -- --list                                        # bg / fg / presets
```

Without Nix: `python3 background.py --bg lights --out t.svg` (needs Python 3).

## Options

| flag | default | meaning |
|------|---------|---------|
| `--bg` | `static` | background animation: `static` `scan` `lights` |
| `--fg` | `rotate` | icon animation: `rotate` `static` |
| `-r, --resolution` | `1080p` | comma list of presets or `WIDTHxHEIGHT` |
| `-o, --out` | `./out/` | `.svg` file (single), `-` for stdout, or a directory |
| `-s, --seed` | `0` | random seed (layout depends only on seed, not on bg/fg) |

**Presets:** `720p 1080p 1440p 4k mobile tablet square ultrawide`.

## Check

```sh
python3 background.py --selftest   # valid SVG + asserts the triangle invariants
```
