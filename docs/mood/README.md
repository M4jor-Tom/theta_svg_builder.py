# Graphic mood

Reference for anyone — human or agent — extending `background.py`. It records
*why* the output looks the way it does, so new features join the system instead
of arriving from a different one.

**The mood in one line:** a clinical high-altitude blueprint. Cold, high-key,
almost empty, with one thing worth looking at.

**Touchstones:** the Sky Tower in *Oblivion* (white, glazed, isolated, sunlit
from a long way up) and *Antichamber* (white void, black linework, geometry as
the subject). Not neon cyberpunk, not HUD sci-fi, not dark mode.

![matrix](matrix.png)

![details](details.png)

## The five rules

Everything below follows from these. If a change breaks one, it will look wrong
no matter how good it is on its own.

**1. Contrast is a hierarchy tool, not a texture tool.**
The lattice lives at 0.27 stroke opacity — deliberately near-invisible. Dark
values are rationed and spent almost entirely on the center icon, which is why
one focal point wins absolutely. Raising the field's contrast to make it "more
visible" destroys the whole effect.

**2. Everything is high-key except what is deliberately not.**
The canvas sits in the top 15% of the value range (`#eef3f6` → `#d9e3ea`). Only
two things are allowed to be dark: the icon strokes, and space cells. A third
dark element would flatten the hierarchy.

**3. Two desaturated hues, no warmth.**
Steel-blue and sea-green, both pulled 58% toward near-black slate. Nothing
saturated, nothing warm, no third hue. The restraint is the style.

**4. The halo subtracts, it does not glow.**
The radial gradient behind the icon paints *canvas colour* over the lattice —
it erases pattern rather than adding light. Clean room, not spotlight. Any new
"emphasis" effect should remove competing detail rather than add brightness.

**5. Motion is ambient and slow.**
24s rotations, 5s sweeps, 9s light cycles with randomised per-cell phase so only
a few cells breathe at once. Nothing pulses fast, nothing loops visibly, nothing
asks for attention. If a viewer notices the animation as an event, it is wrong.

## Palette

`background.py` is the source of truth (`PAL`, `VOID`); these are its computed
values, listed so you can match them by eye without running anything.

| role | value | notes |
|------|-------|-------|
| canvas gradient | `#eef3f6` → `#d9e3ea` | top-left to bottom-right |
| `a` hexagons, ship hull | `#365665` | steel-blue, `#6fb7d1` darkened 58% |
| `b` triangles, fuselage | `#395e53` | sea-green, `#77c9a6` darkened 58% |
| `ink` lattice strokes | `#2a424f` | at 0.27 opacity |
| `VOID` space ground | `#16212a` | deep blue-slate, **never `#000`** |
| stars | `#eef3f6` | the canvas highlight, reused |
| nebula | `#6fb7d1` / `#77c9a6` | 0.3 → 0 radial, the *undarkened* hues |

`VOID` is not black on purpose. A true black reads as a hole punched in from a
different design; keeping it in the blue-slate family makes it a window.

## Geometry

- Hexagon lattice at spacing `2s`, `s = min(w,h)/9`, so density is constant at
  every resolution.
- Triangles are sparse and follow the holder/intersector rule (see the main
  README) — few, never overlapping, never inside the icon zone.
- The icon glyphs are drawn on a 200-unit grid and scaled by `min(w,h)*0.34/200`.
  Keep new glyphs inside a radius of ~88 units so they occupy the same footprint.
- Sacred-geometry nesting is the icon language: shapes contain shapes, and
  tangency is deliberate. `hexatri` interleaves 6-fold and 3-fold symmetry.
- **Nested copies of one shape read as a chevron, not an object.** The `ship`
  glyph works because the narrow fuselage and the wide swept delta are different
  shapes; an earlier version using two concentric deltas just looked like an "A".

## Space cells (`--bg-image space`)

Windows onto the void, at ~8% of eligible hexagons. Sparse is the point: a few
portals, not a checkerboard. Each is a clipped void ground, one faint nebula
ellipse, and ~18 seeded stars with two bloomed anchors — drawn, never embedded,
so there are no assets and it stays crisp at 4k.

The ~8% applies to `--bg static`, `scan` and `lights`. Under `--bg closeopen`
every eligible hexagon is a window and the blinds keep all but a few shut — same
sparseness, enforced in time instead (see below).

Two constraints exist for mood reasons, not technical ones:

- A cell must clear the icon zone **entirely** (centre distance ≥ `clear_r + s`),
  the same exclusion triangles obey.
- Under `--bg lights` a cell pulses its **border only** (`.lightb`). The normal
  pale fill flash would wash the starfield out.

Triangles are allowed to cross a space cell. They are translucent, so they read
as shards catching light — this is intentional, not an oversight.

## Blinds (`--bg closeopen`)

A canvas-filled hexagon over each window, scaling about its own centre: closed at
`scale(1)`, open at `scale(0)`. It reveals from the border inward, which keeps
the static lattice edge as the constant and lets the picture grow inside it —
the opposite (shrinking the starfield) would peel the cell away from its own
outline. Rule 4 applies: the blind **subtracts** the window rather than adding a
glow to announce it.

**Sparseness moves from space to time.** Here *every* eligible hexagon is a
window — ~76 rather than ~7 — and the duty cycle does the rationing: a blind is
shut for 86% of its cycle, so about 11 windows show anything and ~3 are fully
open at once. Fewer black cells than the sparse mode, but a window can open
anywhere instead of always in the same seven places. Two consequences follow:

- The period is **60–90 s**, not the 9–24 s used elsewhere. Lowering the open
  ratio without stretching the period turns each opening into a blink, and rule 5
  forbids anything that registers as an event. Fully open lasts ~3 s; the shrink
  takes ~4 s.
- Window borders drop back to `STROKE_O`. The brighter `SPACE_STROKE_O` exists to
  mark a handful of portals; applied to every cell it is just the lattice opacity
  raised across the board, which rule 1 forbids outright.

The cost is file size: ~76 starfields instead of ~7 takes a 4k frame from ~48 KB
to ~222 KB (~35 KB gzipped). Acceptable for a wallpaper. If it ever stops being
acceptable, share a handful of starfields through `<defs>` and `<use>` before
touching the star count — the density of stars is doing real work, and halving it
was measured to buy nothing (see below).

### Windows switch themselves off while covered

SVG performs no occlusion culling: a shut, fully opaque blind does **not** stop
the starfield beneath it from being repainted every frame. So each window also
carries `class="win"`, whose `winvis` keyframes set `display:none` for exactly
the span its blind covers it. Measured on this lattice at 1080p, back to back in
software rasterisation:

| | frame rate | dropped frames | renderer RSS |
|---|---|---|---|
| always painted | 46–49 fps | 99–127 | 1142 MiB |
| off while covered | **57–59 fps** | **13–25** | **1031 MiB** |

Roughly +22% frame rate, 5–10× fewer dropped frames, ~110 MiB less resident
memory, for +6 KB of output. The cost is confined to the paint area, not the star
count — a control with half the stars ran no faster than the baseline, which is
why cutting star density is the wrong lever.

Two caveats worth keeping: this was measured under software rasterisation with no
GPU available, and on a real GPU every variant likely sits at 60 fps and the win
shrinks toward nothing; and absolute frame rates moved a lot with machine load,
so only same-batch comparisons mean anything.

**`BLIND_KF` is the single source of both keyframe sets.** The window's on/off
span is derived from the blind's, one percent wider on each side, so the stars are
already present before the blind starts to move. Hand-writing the two spans
separately is how you get a starfield popping in over a shut blind; if you retime
the blind, retime it there and both follow.

Three more things this depends on, all easy to break:

- The blind is filled with `url(#bg)`, and `#bg` is `gradientUnits="userSpaceOnUse"`
  for exactly that reason. Revert it to the default `objectBoundingBox` and each
  blind squeezes the whole canvas ramp into one hexagon, so a *closed* cell shows
  as a faint patch instead of disappearing.
- The blind sits **above its window, below the triangles**. A triangle crossing a
  window must stay put while the blind moves under it.
- `.blind` rests at `scale(0)` and `.win` at `display:inline` — both **open** — and
  the keyframes override them while running. Rest either one closed and
  `prefers-reduced-motion` hides the starfield completely: motion off should cost
  the motion, never the picture.


## Extending

**Do:** reuse `PAL`; keep new strokes thin and uniform; make new motion slower
than you think it should be; add darkness only by *removing* light; put new
detail near the center or not at all.

**Don't:** add a third hue; add warmth; raise the lattice opacity; add fast or
looping-visible animation; embed raster assets; use pure black or pure white;
add a second focal point.

## Regenerating

```sh
python3 background.py --bg static    --icon hexatri                  -r 1280x720 -o docs/mood/samples/static-hexatri.svg
python3 background.py --bg static    --icon ship                     -r 1280x720 -o docs/mood/samples/static-ship.svg
python3 background.py --bg lights    --icon hexatri --bg-image space -r 1280x720 -o docs/mood/samples/lights-hexatri-space.svg
python3 background.py --bg scan      --icon ship    --bg-image space -r 1280x720 -o docs/mood/samples/scan-ship-space.svg
python3 background.py --bg closeopen --icon hexatri --bg-image space -r 1280x720 -o docs/mood/samples/closeopen-hexatri-space.svg
```

The `.svg` files in `samples/` are the live artifacts — open them in a browser to
see the animation, which the PNGs cannot show. The PNGs exist so the mood is
inspectable without running anything.
