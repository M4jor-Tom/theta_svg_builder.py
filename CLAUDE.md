# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`svg_builder` generates a **self-contained, CSS-animated SVG** wallpaper (a hexagon lattice with sparse triangles + a center icon). Everything is one Python file plus a generated protobuf module, no external assets — even the "space" starfield is drawn as SVG primitives, so output stays small and crisp at any resolution.

## Commands

```sh
nix run .#bgsvg                       # renders ./parameters.json
nix run .#bgsvg -- path/to/config.json
python3 background.py --selftest      # invariants — run after ANY change
python3 test/golden.py                # the picture did not change (--regen when it should have)
nix build                             # default package = bgsvg
nix develop -c protoc --python_out=. parameters.proto   # regenerate _pb2
```
Without Nix, `background.py` runs on any Python 3 with `protobuf >= 7.35.1` installed
(the generated `parameters_pb2.py` refuses to import against an older runtime).

Two tests, and a change is unfinished until both pass. `--selftest` builds all 42 valid `background.motion` × `background.image` × `icon` × `overlay` combinations, parses each as XML, and asserts the invariants below (`selftest`/`_assert_*` in `background.py`) — it says a render is *well-formed*. `test/golden.py` says it is *unchanged*: the same 42 configs live in `test/golden/<sha512 of the SVG>/`, each beside the `<sha512 of the SVG>_background.svg` it renders. One rule covers both files — each is named by the sha512 of its own bytes, exactly as written — so `sha512sum` reproduces every name unaided. The SVG is kept, not just its hash, so a failure reports the first differing byte instead of only a moved hash. Both enumerate from `valid_configs()` rather than each carrying its own loop, so a new axis cannot reach one surface and miss the other — add an enum value and both sweeps grow. It fails on any byte that moves, so run `--regen` when the picture was **meant** to move, and read the diff first — a golden change you did not intend is the regression.

## Architecture

`background.py` is the whole program; `flake.nix` just wraps it as the `bgsvg` app. Read `docs/mood/` (README + `matrix.png` + `samples/`) before touching anything visual — it is the graphic-mood contract, and the point is to extend the look without breaking it.

**One config, one render** — `parameters.json` is the whole input and
`parameters.proto` is its schema; `--selftest` and the config path are the
only CLI surface left. **Conditional rules are structural where the model
allows it, and rejected where it does not.** A motion belongs to the icon that
declares it (`Hexatri.Motion`; `Ship` declares none, and nothing assumes the
next glyph rotates), and the matrix angle and colour live inside `Matrix` — so
both are unwritable rather than rejected. `Background` keeps `motion` and
`image` as orthogonal enums, matching the parameters `pat_trihex` takes, which
leaves `CLOSEOPEN` + `NONE` expressible; `validate()` rejects it, along with
the angle range, the colour format and the resolution format. When adding a
rule, try moving a field before adding a check.

**Determinism** — geometry depends ONLY on `seed`; the animation/icon/image/overlay choices never move a hexagon. Same seed ⇒ same layout across every combination. `pat_matrix` gets its own `random.Random` for exactly this reason, and `_assert_matrix` compares every `<polygon>` overlay-on vs overlay-off. Keep new features on this rule so seeds stay stable.

**Pure CSS animation, reduced-motion-safe** — animation is `@keyframes` embedded in the SVG (no SMIL, no JS), and every animated element MUST have a resting state that `prefers-reduced-motion` falls back to (the clean static look). `css()` centralizes this.

**Build pipeline** (`build_svg`): `lattice()` (the one shared hex grid) → `pat_trihex()` (triangles + optional `space` windows + optional `closeopen` blinds) → `pat_matrix()` (optional character rain) → `ico_hexatri()`/`ico_ship()` (center glyph) → `css()` → assemble. Sizes scale with `min(w, h)`, so pattern density is resolution-independent.

**`ship` is a solid, not linework.** The hull is tiled by four translucent facets whose *fill steps* carry the relief; the ship's own gradients live in the glyph, not in `build_svg`'s `defs`, because they have exactly one consumer. `_assert_ship` checks the facets still tile the hull *by area*, so the cloak cannot move the silhouette, and it asserts against `ico_ship()` alone — matched against a whole page, the lattice supplies polygons that satisfy those patterns by chance. Why the facets are lit and valued the way they are is argued in `ico_ship`'s docstring; read it there.

**`matrix` rain — nothing moves.** The characters are anchored by `x`/`y` and never translate; the *lighting* travels. Every cell of a column holds one glyph fixed at generation, and all of them run the same `fill-opacity` keyframes offset by one cell-time, so the head advances into a fresh character while those behind dim in place. Animating a transform instead would slide the characters across the canvas — the one thing this effect must not do, and what `_assert_matrix` guards. The layer sits between the lattice and the halo circle, so the halo *subtracts* it around the icon exactly as it subtracts the lattice; there is no icon-exclusion code and there should not be. The remaining design (one rotated group, upright counter-rotated glyphs, `--o`/`--t`/`--d` inheritance, the t=0 resting attribute) is argued in `pat_matrix`'s docstring — read it there rather than restating it here.

**Triangle invariant** — every hexagon is either a *holder* (owns one edge as a triangle base) or an *intersector* (a triangle tip pokes in), never both, pierced at most once. This is what keeps triangles few, non-overlapping, and out of the icon zone. `_assert_constraints` checks it.

**`closeopen` occlusion trick** — SVG does no occlusion culling, so each window `display:none`s itself for exactly the span its opaque blind covers it (measured large frame-rate/memory wins). The window's off-span and its blind's shut-span are both derived from ONE constant (`blind_phase`); do not split them or a window will desync from its blind.
