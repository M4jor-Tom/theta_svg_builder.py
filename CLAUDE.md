# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`svg_builder` generates a **self-contained, CSS-animated SVG** wallpaper (a hexagon lattice with sparse triangles + a center icon). Everything is one Python file, **stdlib only**, no external assets — even the "space" starfield is drawn as SVG primitives, so output stays small and crisp at any resolution.

## Commands

```sh
nix run .#bgsvg -- --bg lights --fg rotate --resolution 4k --out wall.svg
nix run .#bgsvg -- --list            # every axis + presets
python3 background.py --selftest     # the test suite — run after ANY change
nix build                            # default package = bgsvg
```
Without Nix, `background.py` runs on plain Python 3 (`python3 background.py …`).

`--selftest` is the only test: it builds all 32 `bg × fg × icon × bg-image` combinations, parses each as XML, and asserts the invariants below (`selftest`/`_assert_*` in `background.py`). Treat a change as unfinished until it passes.

## Architecture

`background.py` is the whole program; `flake.nix` just wraps it as the `bgsvg` app. Read `docs/mood/` (README + `matrix.png` + `samples/`) before touching anything visual — it is the graphic-mood contract, and the point is to extend the look without breaking it.

**Four independent axes, combined freely** — `--bg` (`static`/`scan`/`lights`/`closeopen`), `--fg` (`rotate`/`static`), `--icon` (`hexatri`/`ship`), `--bg-image` (`none`/`space`). Exactly two combinations are **rejected with an error, not silently ignored**: `--icon ship --fg rotate` (the ship is static by design) and `--bg closeopen --bg-image none` (closeopen has nothing to reveal). Preserve that "reject, don't ignore" stance for new incompatibilities.

**Determinism** — geometry depends ONLY on `--seed`; the animation/icon/image choices never move a hexagon. Same seed ⇒ same layout across every combination. Keep new features on this rule so seeds stay stable.

**Pure CSS animation, reduced-motion-safe** — animation is `@keyframes` embedded in the SVG (no SMIL, no JS), and every animated element MUST have a resting state that `prefers-reduced-motion` falls back to (the clean static look). `css()` centralizes this.

**Build pipeline** (`build_svg`): `lattice()` (the one shared hex grid) → `pat_trihex()` (triangles + optional `space` windows + optional `closeopen` blinds) → `ico_hexatri()`/`ico_ship()` (center glyph) → `css()` → assemble. Sizes scale with `min(w, h)`, so pattern density is resolution-independent.

**Triangle invariant** — every hexagon is either a *holder* (owns one edge as a triangle base) or an *intersector* (a triangle tip pokes in), never both, pierced at most once. This is what keeps triangles few, non-overlapping, and out of the icon zone. `_assert_constraints` checks it.

**`closeopen` occlusion trick** — SVG does no occlusion culling, so each window `display:none`s itself for exactly the span its opaque blind covers it (measured large frame-rate/memory wins). The window's off-span and its blind's shut-span are both derived from ONE constant (`blind_phase`); do not split them or a window will desync from its blind.
