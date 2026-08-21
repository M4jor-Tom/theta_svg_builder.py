# bgsvg — trihexagonal animated SVG background builder

Generates a self-contained `.svg`: a **hexagon lattice with sparse triangles**
plus a **center icon**, light theme. Animation is pure CSS (no SMIL, no JS) and
honours `prefers-reduced-motion` (which falls back to the clean static look).
One Rust crate, no external assets. Sizes scale with `min(width, height)`,
so pattern density is constant across resolutions.

The graphic mood — and the rules for extending it without breaking it — is
documented in [`docs/mood/`](docs/mood/).

## Animations

- `background.motion STATIC` — no background animation.
- `background.motion SCAN` — hexagon (and triangle) opacities sweep diagonally across the canvas.
- `background.motion LIGHTS` — hexagons occasionally light their **border**, then their **fill**, then fade back; random per-hexagon timing, so only a few glow at once.
- `background.motion CLOSEOPEN` — **every** hexagon becomes a window, each covered by a **blind** that shrinks about its own centre, so the picture behind it appears as a ring widening from the border inward, holds open, then grows shut again. Closing is opening played backwards. Blinds stay shut 86% of a 60–90 s cycle with a random phase per cell, so at 1080p roughly 11 of the 76 windows show anything at a given moment and only ~3 are fully open — the field stays sparse in **time** rather than in space, and a window can open anywhere. Needs `background.image STARFIELD` to open onto.
- `icon.hexatri.motion ROTATE` — the icon's two triangle rings counter-rotate inside the static hex frame.
- `icon.hexatri.motion STATIC` — still icon.

## Icons

- `icon.hexatri` — the nested hexagon/triangle glyph. Supports `icon.hexatri.motion ROTATE`.
- `icon.ship` — a cloaked delta spaceship: a swept hull folded along its spine into
  four translucent facets, lit crest, hexagonal cockpit. Nothing reaches a third
  opacity, so the lattice reads straight through it.

The ship is **static by design**: `Ship` declares no `motion` field at all, so
a rotating ship cannot be resolved to anything — it can only be rejected if
you try to write one:

```sh
$ bgsvg rotate-ship.json
rotate-ship.json: unknown field `motion`, there are no fields at line 1 column 25
```

## Background images

- `background.image NONE` — plain lattice.
- `background.image STARFIELD` — ~8% of hexagons become windows onto a **procedurally drawn starfield**: void ground, a faint nebula, and seeded stars. Drawn rather than embedded, so there are no assets and output stays small and crisp at any resolution (a 4k frame is ~48 KB).

Space cells always clear the icon zone, and under `background.motion LIGHTS` they pulse their
border only — the usual fill flash would wash the stars out.

`background.motion CLOSEOPEN` is the exception to the ~8%: it needs a window everywhere, so it
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

`background.motion CLOSEOPEN` needs something to reveal, so it is rejected with
`background.image NONE` rather than rendering nothing — the one rule
`parameters.proto` cannot express, checked in `validate()`:

```sh
$ bgsvg closeopen-none.json
closeopen-none.json: background motion CLOSEOPEN has nothing to reveal with
image NONE (its hexagons open onto an image); set image STARFIELD, or motion
LIGHTS
```

`background.motion` and `background.image` are independent and combine freely
apart from that one exception. A motion belongs to the icon that declares it
(only `hexatri` has one), so a rotating ship is unwritable rather than a
runtime rejection. Everything except the animations depends only on `seed`, so
one seed gives the same layout across every combination.

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
nix run .#bgsvg                       # the schema's defaults
nix run .#bgsvg -- path/to/config.json
nix run .#bgsvg -- --configs          # dump the 42-config corpus as JSON lines
nix run .#bgsvg -- --descriptor      # parameters.proto as a FileDescriptorSet, for other languages
nix develop -c cargo test             # invariants
nix develop -c python3 test/golden.py # the picture did not change
```

Without Nix: Rust 1.85+ (edition 2024) and `protoc` on `PATH`, then
`cargo build --release`.

## Configuration

One JSON file describes one render. `parameters.proto` is the schema — read it
for the authoritative list; every zero value there is this program's default,
so `{}` is a complete config.

```json
{
  "seed": 0,
  "output": { "directory": { "path": "out", "resolutions": ["1080p", "4k"] } },
  "background": { "motion": "CLOSEOPEN", "image": "STARFIELD" },
  "icon": { "hexatri": { "motion": "ROTATE" } },
  "overlay": { "matrix": { "angle": 250, "color": "#395e53cc" } }
}
```

| field | default | meaning |
|---|---|---|
| `seed` | `0` | layout depends only on this, never on the animation, icon, image or overlay |
| `output.file` | — | `{ path, resolution }` — one `.svg` |
| `output.stdout` | — | `{ resolution }` — write to stdout |
| `output.directory` | `{ "path": "out", "resolutions": ["1080p"] }` | one file per resolution |
| `background.motion` | `STATIC` | `STATIC` `SCAN` `LIGHTS` `CLOSEOPEN` (`CLOSEOPEN` needs an image) |
| `background.image` | `NONE` | `NONE` `STARFIELD` |
| `icon.hexatri.motion` | `ROTATE` | `ROTATE` `STATIC` |
| `icon.ship` | — | no motion: the ship is static by design |
| `overlay.matrix` | absent | `{ angle, color }` — `0`–`360`, `#rrggbb` or `#rrggbbaa` |

A resolution is a preset (`720p` `1080p` `1440p` `4k` `mobile` `tablet`
`square` `ultrawide`) or `WIDTHxHEIGHT`. Empty means `1080p`.

Three things the old flags allowed and this does not. `rotate` exists only
inside `hexatri`, so no icon inherits an animation it has no use for; the
matrix `angle` and `color` exist only inside `matrix`, so they cannot be set
with no rain to steer; and one output stream carries one resolution, so
writing four sizes to a single file is not expressible. `CLOSEOPEN` with
`NONE` is the one rule the schema cannot carry — it is rejected at load.

## Check

```sh
nix develop -c cargo test
```

Builds all 42 valid `background.motion` × `background.image` × `icon` ×
`overlay` combinations and asserts the invariants:
holder/intersector roles, the clear center, space-cell clearance and their
opt-out of the fill flash, one blind per window layered *under* the
triangles, every window sharing its own blind's phase, blinds and windows
resting open so `prefers-reduced-motion` still shows the starfield, and that
an empty config renders the old defaults. It also asserts that the schema or
`validate()` rejects a ship with a motion, a stray matrix knob, two output
sinks at once, a typo'd key, `background.motion CLOSEOPEN` without an image,
an out-of-range matrix angle, a malformed colour, a malformed resolution,
malformed JSON, and a missing config file.

```sh
nix develop -c python3 test/golden.py            # verify
nix develop -c python3 test/golden.py --regen    # rewrite after an intended visual change
```

`cargo test` says the right code ran and the invariants hold; the golden
corpus says the picture is unchanged and well-formed. `test/golden/` holds
those same 42 configs, each kept beside the SVG it renders:

```
test/golden/<sha512 of the SVG>/<sha512 of the JSON>_parameters.json
                               /<sha512 of the SVG>_background.svg
```

One rule covers both files: each is named by the sha512 of its own bytes,
exactly as written. So `sha512sum` reproduces every name in the corpus, and
nothing has to trust this program to check its own work:

```sh
sha512sum test/golden/<D>/*                             # -> <F>, <D>
nix run .#bgsvg -- test/golden/<D>/<F>_parameters.json
sha512sum out/trihex-*.svg                              # -> D
```

Keeping the SVG rather than only its hash is what lets a failure say *what*
moved — it reports the first differing byte with the text either side, since a
line diff says nothing about a one-line document. The corpus is 4.2 MB, about
0.6 MB compressed.

The goldens fix `seed` at `0` and carry no `output`, because geometry depends
only on the seed and the sink picks a destination, not pixels; `test/golden.py`
renders them at 1080p. A directory sink with several resolutions is one config
with several SVGs, which this layout cannot name, so the corpus holds
single-render configs only. Two configs in one directory would mean they render
byte-identical SVGs — an axis that stopped changing the picture — and `scan()`
reports it.
