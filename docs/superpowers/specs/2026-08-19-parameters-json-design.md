# parameters.json — one config file replaces the flag surface

**Date:** 2026-08-19
**Status:** approved, ready for planning

## Problem

`background.py` exposes eleven flags across five visual axes plus output
plumbing. Three cross-flag rules are hand-written rejections in `main()`
(`background.py:641-657`): `--fg rotate` needs `--icon hexatri`, `--bg
closeopen` needs `--bg-image space`, and `--matrix-angle` / `--matrix-color`
need `--overlay matrix`. Every new axis adds another flag and another
hand-written rule, and `--fg` is modelled as a global foreground axis when it
is really one glyph's own animation — the ship has no use for it, and neither
may the next icon.

## Solution

A single `parameters.json` describes a render. `parameters.proto` is its
schema, and `background.py` validates by parsing the JSON through the
generated protobuf message. Conditional flags become *structural* wherever the
model allows it: a rule the schema can express is one nobody has to check,
because the invalid configuration cannot be written down. Two of the three
qualify; the third — `CLOSEOPEN` needing an image — crosses two orthogonal
fields and stays an explicit check.

`protobuf` becomes the project's first runtime dependency. This ends the
repo's stdlib-only and single-file properties — an accepted, deliberate cost.

## Schema

```proto
syntax = "proto3";
package svg_builder;

// One render, fully described. Every zero value is today's CLI default, so an
// empty `{}` renders exactly what bare `bgsvg` renders today.
message Parameters {
  uint32     seed       = 1;  // geometry depends on this and nothing else
  Output     output     = 2;  // unset -> ./out at 1080p
  Background background = 3;  // unset -> plain lattice, no animation
  Icon       icon       = 4;  // unset -> hexatri, rotating
  Overlay    overlay    = 5;  // unset -> nothing above the lattice
}

// ---- output: the sink decides the cardinality ---------------------------
message Output {
  oneof sink {
    File      file      = 1;  // one stream -> one resolution
    Stdout    stdout    = 2;  // one stream -> one resolution
    Directory directory = 3;  // many files -> many resolutions
  }
}
message File      { string path = 1; string resolution = 2; }
message Stdout    { string resolution = 1; }
message Directory { string path = 1; repeated string resolutions = 2; }
// resolution: a preset (720p 1080p 1440p 4k mobile tablet square ultrawide)
// or WIDTHxHEIGHT. Empty = 1080p.

// ---- background ---------------------------------------------------------
// Motion and image are independent, exactly as pat_trihex takes them. The one
// rule that crosses them -- CLOSEOPEN needs windows to open onto -- is the one
// conditional proto3 cannot express; it stays a Python check.
message Background {
  enum Motion { STATIC = 0; SCAN = 1; LIGHTS = 2; CLOSEOPEN = 3; }
  enum Image  { NONE = 0; STARFIELD = 1; }
  Motion motion = 1;
  Image  image  = 2;
}

// ---- icon ---------------------------------------------------------------
// rotate lives inside hexatri and nowhere else: it is that glyph's own
// animation, not a global foreground axis. A future icon declares whatever
// motion vocabulary it actually has; the ship, static by design, declares none.
message Icon {
  oneof glyph {
    Hexatri hexatri = 1;
    Ship    ship    = 2;
  }
}
message Hexatri {
  enum Motion { ROTATE = 0; STATIC = 1; }   // zero = today's default
  Motion motion = 1;
}
message Ship {}

// ---- overlay ------------------------------------------------------------
// angle and colour live inside matrix: with no rain there is nothing to
// steer, so they cannot be written without it.
message Overlay {
  oneof layer { Matrix matrix = 1; }
}
message Matrix {
  double angle = 1;  // 0-360, 0 = falling down, increasing clockwise
  string color = 2;  // #rrggbb or #rrggbbaa; empty = default
}
```

Example:

```json
{
  "seed": 7,
  "output": { "directory": { "path": "out", "resolutions": ["4k", "2560x1440"] } },
  "background": { "motion": "CLOSEOPEN", "image": "STARFIELD" },
  "icon": { "hexatri": { "motion": "ROTATE" } },
  "overlay": { "matrix": { "angle": 250, "color": "#395e53cc" } }
}
```

### Design rules

1. **Conditionals are structural where the model allows.** Two of today's
   three runtime rejections become unrepresentable: `rotate` exists only in
   `Hexatri`, `angle`/`color` only in `Matrix`.
2. **Background motion and image stay orthogonal.** `Motion` carries all four
   values including `CLOSEOPEN`, and `Image` is its own enum, matching the two
   independent parameters `pat_trihex` already takes. Making the image the
   outer discriminator would have bought the third structural rule at the cost
   of distorting the model — motion is motion, image is image. The
   `CLOSEOPEN`-needs-`STARFIELD` rule therefore stays a Python check.
3. **`none` stops being a value where absence says it better.** `overlay:
   none` is an unset `oneof`. `Image.NONE` stays a named value, because an
   image is a property of the background rather than a layer that is present
   or absent.
4. **Every proto zero equals today's CLI default.** Hence `Hexatri.ROTATE = 0`
   rather than a `bool rotate` that would default false and quietly flip
   hexatri to static, and `Motion.STATIC = 0` / `Image.NONE = 0`.
5. **A future icon declares its own motion vocabulary.** Nothing assumes the
   next glyph rotates.

### What the schema cannot express

These stay as Python checks after parsing: `Motion.CLOSEOPEN` requires
`Image.STARFIELD`, angle range 0-360, colour format, resolution string format.
`protovalidate` would cover them at the cost of a second dependency — not
worth it for four checks.

### Structural fix to a live bug

Attaching cardinality to the sink deletes the `endswith(".svg")` string
sniffing in `main()`. Today `-o wall.svg -r 4k,mobile` falls past the
single-file branch at `background.py:674` and silently creates a *directory*
named `wall.svg`. That is unrepresentable in the new schema.

## Interface

```sh
bgsvg [parameters.json]   # positional, defaults to ./parameters.json
bgsvg --selftest
```

`--list` is deleted: the `.proto` is the list, and it cannot drift from the
code. A `parameters.json` at the repo root serves as the default config.

## Changes

### `background.py` — the renderer is untouched

`build_svg`, `lattice`, `pat_trihex`, `pat_matrix`, `ico_hexatri`, `ico_ship`
and `css` keep their exact signatures. Only the CLI boundary moves.

| lines | change |
|---|---|
| 609-627 | `argparse` shrinks to a positional config path + `--selftest` |
| 629-637 | `--list` deleted |
| 641-653 | the three cross-flag `ap.error` blocks deleted |
| 654-662 | angle-range and colour-format checks survive verbatim |
| 663 | `fg = args.fg or (...)` deleted — motion is per-glyph |
| 671-688 | the sink `if/elif` collapses onto the `output` oneof |
| new | `load(path)` → `json.load` + `json_format.ParseDict(..., ignore_unknown_fields=False)` |
| new | `resolve(params)` → the ten values `build_svg` already takes |

`resolve` is deliberate: the message is a boundary format, and
`bg`/`fg`/`icon`/`bg_image`/`overlay` stay the renderer's internal vocabulary
(`Image.STARFIELD` maps to the renderer's existing `bg_image="space"`).
`main()` gets shorter, and the ~900 lines of visual code carry zero risk.

### New files

- `parameters.proto` — the schema above.
- `parameters_pb2.py` — generated, **committed**. Not committing it would
  break the `python3 background.py` path CLAUDE.md promises. Regenerated by
  one `protoc` line, documented in the devShell.
- `parameters.json` — the default config at repo root.
- `docs/mood/samples/*.json` — six configs beside the six sample `.svg`s.

### `flake.nix`

`writeShellApplication` currently interpolates `${./background.py}` as a
single store path, so a sibling module would not be importable. It becomes a
`lib.fileset.toSource` over `background.py` + `parameters_pb2.py`, with
`python3.withPackages (ps: [ ps.protobuf ])` as the interpreter. The devShell
adds `protobuf` for regenerating `_pb2`.

### Docs — four surfaces, ~50 flag references

- `README.md:96-106` — the options table becomes a schema table.
- `background.py:1-52` — the docstring's flag catalogue becomes the config shape.
- `CLAUDE.md` — the "five independent axes" section, the "reject, don't ignore"
  paragraph (now "make it unrepresentable; reject what the schema cannot
  express"), and both stdlib-only claims.
- `docs/mood/README.md` — every visual argument stands; only `--flag`
  spellings change, plus the sample-regeneration block at :257-262, which
  becomes `for f in docs/mood/samples/*.json; do python3 background.py "$f"; done`.
  The sample configs then document the schema for free.

## Testing

`--selftest` remains the only test.

The five nested loops (`background.py:695-710`) become one enumeration of the
*valid* config space — 7 backgrounds (4 motions × 2 images, less the
`CLOSEOPEN`+`NONE` pair the check rejects) × 3 glyphs (2 hexatri + 1 ship) ×
2 overlays = **42 configs, all valid**, against today's 64 that included three
impossible ones. Each runs `load() → resolve() → build_svg`, so the test
exercises the path users actually run. Assertions simplify as a side effect:
`('class="blind"' in svg) == (bg == "closeopen" and img == "space")` loses its
second clause, since the enumeration never yields a blind without windows.

`_assert_rejected` (:741) changes shape and grows from six cases to nine:

| case | rejected by |
|---|---|
| `ship.motion: ROTATE` | schema (`ParseError`) |
| `overlay.angle` outside `matrix` | schema (`ParseError`) |
| `motion: CLOSEOPEN` + `image: NONE` | Python check |
| angle `400` | Python check |
| colour `395e53` | Python check |
| unknown key (`"backgrond"`) | `ignore_unknown_fields=False` |
| two `oneof` members set at once | schema (`ParseError`) |
| malformed JSON | `json.load` |
| missing config file | `open` |

The two schema cases stay precisely as regression tests: they fail loudly if
someone later "simplifies" the schema by flattening a `oneof` back into a
plain field.

The typo case matters most. Today a bad flag is an argparse error; without
`ignore_unknown_fields=False` a typo'd key would silently render the wrong
wallpaper. That is the one new failure mode this design introduces, and it is
closed by default.

One permanent assertion joins them: `{}` must render byte-identical to today's
flagless default. That pins design rule 3 and would catch `Hexatri.ROTATE = 0`
silently regressing.

## Migration

A hard break. No flag shim, no deprecation: the request was to stop using
options, this is a personal repo on `master`, and a compatibility layer would
preserve exactly the surface being deleted. Seeds become non-negative
(`uint32`); nothing depends on that.

Safety net — a one-time equivalence gate:

1. Before any edit, render the six `docs/mood/samples/*.svg` with today's code
   into a scratch directory.
2. After the rewrite, render the six sample JSONs and diff byte-for-byte.

Identical output proves the refactor moved no pixels. Since the samples are
committed, a clean `git diff` on them at the end says the same thing.

Work order is forced by imports:

1. `parameters.proto`
2. generated `_pb2` + `flake.nix`
3. `load` / `resolve` / `main`
4. selftest
5. docs
