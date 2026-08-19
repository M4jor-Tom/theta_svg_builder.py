# Rust port — the renderer becomes a crate, the picture does not move

**Date:** 2026-08-19
**Status:** approved, ready for planning

## Problem

`background.py` is 1075 lines carrying the whole program: schema boundary,
RNG, geometry, four pattern generators, two glyphs, the CSS, the CLI, and its
own test suite. `parameters_pb2.py` is generated Python committed beside it.
The ROADMAP asks for Rust with an object split.

Nothing about the *picture* is wrong. The mood contract in `docs/mood/`, the
42-config golden corpus and the invariants in `--selftest` all describe
behaviour that must survive the port unchanged, byte for byte.

## Solution

A Cargo package `bgsvg`: a thin binary over a library of ten modules, one per
stage `CLAUDE.md` already names. `parameters.proto` stays the schema and stops
generating Python — `build.rs` generates Rust from it instead. The 42 golden
SVGs are the acceptance test: **the port is done when `test/golden.py` passes
against the corpus it already holds**, unmodified.

Four decisions, taken before design:

1. **Byte-identical output.** The port reimplements CPython's `random` rather
   than choosing a Rust RNG. Cost: one module. Payoff: the committed corpus
   becomes proof the refactor moved nothing, instead of a test that passes
   trivially because it was regenerated to match whatever the new code does.
2. **`parameters.proto` stays authoritative**, compiled to Rust at build time.
3. **`--selftest` becomes `cargo test`.** Invariants move next to the code
   they check, where they can reach private items.
4. **`parameters.json` is deleted**; a bare `bgsvg` renders `{}`.

## Crate layout

Standard Cargo layout: library plus binary, flat `snake_case` modules,
integration tests in `tests/`.

| file | responsibility |
|---|---|
| `Cargo.toml` / `Cargo.lock` | package `bgsvg`, edition 2024, lock committed for Nix |
| `build.rs` | `prost-build` → descriptor set → `pbjson-build` |
| `parameters.proto` | unchanged — still the schema |
| `src/main.rs` | collects arguments, calls `run`, returns its exit code — nothing else |
| `src/lib.rs` | crate docs (the module docstring), module wiring, and `run(args) -> i32`: the CLI body lives in the library so `tests/` can drive the whole boundary without spawning a process |
| `src/params.rs` | generated types, `validate`, `resolve` → `Scene`, `valid_configs`, resolution parsing |
| `src/rng.rs` | CPython-compatible Mersenne Twister |
| `src/geom.rs` | `fmt`, `pts`, `regular_poly`, `Lattice` |
| `src/style.rs` | palette, tuning constants, `css()` |
| `src/trihex.rs` | lattice pattern, space cells, blinds |
| `src/matrix.rs` | the rain overlay |
| `src/icon.rs` | hexatri and ship |
| `src/svg.rs` | `build_svg` assembly |
| `tests/reject.rs` | the config boundary, through `run()` |

**Every docstring is ported as rustdoc.** `CLAUDE.md` defers to `ico_ship`'s
and `pat_matrix`'s docstrings as the authority for why the visuals are what
they are ("read it there rather than restating it here"). A port that drops
them deletes the mood contract's reasoning while keeping its output.

Objects where they earn their keep: `Lattice` (methods rather than a
namedtuple), `PyRandom`, and `Scene`. Not a trait in sight — there is one
implementation of everything here, and an `IconRenderer` trait with two impls
would be scaffolding for a third icon that does not exist.

### `Scene` deletes the vocabulary bridge

`resolve()` currently maps schema enums onto lowercase strings (`_BG`, `_IMG`)
because Python has no better boundary type. In Rust the schema enums *are*
types, so the renderer takes them directly:

```rust
pub struct Scene {
    pub seed: u32,
    pub motion: background::Motion,   // STATIC | SCAN | LIGHTS | CLOSEOPEN
    pub image: background::Image,     // NONE | STARFIELD
    pub glyph: Glyph,                 // Hexatri { rotate: bool } | Ship
    pub overlay: Option<Rain>,        // angle + resolved colour
}
```

`_BG` and `_IMG` disappear; `Ship`'s lack of a motion becomes a variant
without a field rather than a `"static"` string; and `overlay: Option<Rain>`
means the matrix angle and colour cannot be read when there is no rain — the
schema's structural rule, restated in the renderer's own types.

The four axes stay independent of geometry, exactly as before: `Scene` is
consumed by rendering, never by `lattice()` or the triangle assignment.

## Schema: proto → prost/pbjson

`build.rs`:

```rust
let out = PathBuf::from(env::var("OUT_DIR")?);
prost_build::Config::new()
    .file_descriptor_set_path(out.join("descriptor.bin"))
    .compile_protos(&["parameters.proto"], &["."])?;
pbjson_build::Builder::new()
    .register_descriptors(&fs::read(out.join("descriptor.bin"))?)?
    .build(&[".svg_builder"])?;
```

`params.rs` includes both generated files. Parsing is
`serde_json::from_str::<Parameters>`.

**Why prost + pbjson and not rust-protobuf.** Three rejections the selftest
asserts must come from the schema, not from hand-written checks. Reading the
generated-code templates:

| case | pbjson | rust-protobuf's `protobuf-json-mapping` |
|---|---|---|
| unknown field | `unknown_field` error | `UnknownFieldName` error |
| unknown enum name | error | `UnknownEnumVariantName` error |
| two members of one `oneof` | `duplicate_field` error | **not detected — last wins** |

`{"output": {"file": {...}, "stdout": {}}}` would parse *successfully* under
rust-protobuf, silently keeping one sink, and `validate()` could not catch it
afterwards because the evidence is gone by then. Closing that would mean a
descriptor-driven pre-pass over the raw JSON. pbjson gets it right for free,
at the price of `protoc` at build time — already in the flake.

`validate()` keeps exactly the four rules the schema cannot state: CLOSEOPEN
needs an image, angle within 0–360, colour format, resolution format.

### `valid_configs` stays schema-driven

```rust
(0..).map_while(|i| background::Motion::try_from(i).ok())
```

walks declaration order and stops at the first gap, so adding an enum value to
the `.proto` still grows both sweeps at once — the property `CLAUDE.md` calls
out ("a new axis cannot reach one surface and miss the other") — with no
hardcoded list and no runtime reflection. These enums are dense by
construction; a deliberately sparse one would truncate the sweep, and that
gets a comment where it matters.

## The RNG

`src/rng.rs` reimplements CPython's `random` module for the calls this program
makes:

- MT19937 with `init_by_array`, seeded the way `random.seed(str)` seeds:
  `n = int.from_bytes(s + sha512(s), "big")`, split into little-endian 32-bit
  words, `keyused = ceil(bits/32)`. Verified against the live interpreter:
  `Random("trihex:0")` and `Random(n)` produce identical streams.
- `random()` = `genrand_res53`, `getrandbits(k)`, `_randbelow` (rejection
  sampling on `bit_length`), `shuffle` (Fisher–Yates downward), `sample`'s
  pool algorithm, `uniform`, `choice`.

Pinned by unit tests against vectors captured from Python 3.14.7:
`Random("trihex:0").random()` → `0.323979587515701, 0.480793333456907,
0.521798912248572`; `getrandbits(32)` → `1391481750, 664579869, 2064991622,
3668470967`; `sample(range(6), 6)` → `[2, 1, 3, 5, 4, 0]`.

`sha2` is a dependency. The string seeding needs SHA-512 once per cell RNG,
and a hand-rolled hash is the wrong place to save eighty lines.

## CLI

```sh
bgsvg [config.json]   # no argument -> {}, the schema's own defaults
bgsvg --configs       # the corpus contract: 42 canonical configs, one per line
```

Two flags do not justify `clap`; `std::env::args` covers it. Errors are a
three-variant enum with `Display`, no `anyhow`/`thiserror`, printed as
`{path}: {msg}` to stderr with exit 2 — unchanged from today.

`--configs` prints exactly the bytes `test/golden.py` hashes today: sorted
keys, `,`/`:` separators, `"angle":250` as an integer. `serde_json`'s compact
output over its `BTreeMap`-backed `Map` matches Python's
`json.dumps(sort_keys=True, separators=(",", ":"))` byte for byte, which is
why the corpus's JSON-side hashes do not move.

## Tests

**`cargo test`** — invariants as unit tests beside their code
(holder/intersector roles, clear centre, space-cell clearance and lights
opt-out, blind layering and phase-sharing, the ship's four facets tiling its
hull, the rain's contiguous fading trail and constant stagger, `{}` resolving
to the old defaults), plus `tests/reject.rs` driving `run()` over the ten
rejection cases. `nix build` runs them in `checkPhase`.

**`test/golden.py`** — same path, same corpus, pure stdlib. It reads the 42
configs from `bgsvg --configs`, renders each by running the binary in a
temporary working directory and reading back the path it prints, and compares
against `test/golden/`. The binary is located via `$BGSVG`, else
`target/release/bgsvg`, else `target/debug/bgsvg`, else `PATH`.

This is the acceptance test for the whole port. `--regen` stays, and must
never be needed: a golden that moves means the port is wrong, not that the
picture changed.

## Port hazards

Found by reading the source. In descending order of how much time each would
cost if missed:

1. **`%` on negatives.** `NB[r % 2]` and `(r % 2) * D/2` run with `r = -1`;
   `(k - 1) % 6` runs with `k = 0`; `(head - j) % N` goes negative in every
   rain column. Python returns a non-negative result, Rust follows the sign of
   the dividend. All of these are `rem_euclid`. Missing one silently
   rearranges the lattice.
2. **`round()` is banker's rounding.** `_mix` rounds channel values with
   Python's `round`, which is half-to-even; `f64::round` is half-away-from-zero.
   Must be `round_ties_even`. Wrong here tints every colour in the file.
3. **Negative zero.** `fmt(-0.001)` prints `0` in Python, via
   `int(-0.0) == 0`. Rust's `{:.2}` prints `-0.00`, and a naive integer branch
   prints `-0`.
4. **RNG call order.** `motion == LIGHTS` draws two values from the *global*
   stream inside the render loop, after triangle assignment. The call
   sequence, not just the seed, is the contract.
5. **`math.hypot`.** CPython 3.14 uses its own correctly-rounded
   implementation; Rust calls libm. It is only ever compared against
   `clear_r`, so a 1-ulp gap could in principle flip one cell's eligibility.
   The goldens are the detector; if it fires, port CPython's algorithm.
6. **Raw float interpolation.** Values like `stroke-opacity="{so}"` bypass
   `fmt()` and use Python's `str(float)`. Rust's `Display` is also
   shortest-round-trip but diverges on exponent form (`1e-05` vs `0.00001`).
   Every raw interpolation site gets audited rather than assumed.
7. **`0x0` resolution.** Today `parse_res` accepts it and `lattice()` raises
   `ZeroDivisionError` — uncaught, exit 1 with a traceback (a known gap). Rust
   would divide by zero in floating point and emit a *garbage SVG*, which is
   worse. `parse_res` therefore rejects a zero dimension. **This is the one
   deliberate behaviour change in the port**: exit 2 with a message instead of
   a traceback.
8. **Stricter by accident, in good ways.** Duplicate JSON keys become an error
   (Python kept the last one); an out-of-range `seed` or a 14-digit width
   fails to parse rather than overflowing.

## Deletions and documentation

Deleted: `background.py`, `parameters_pb2.py`, `parameters.json`,
`test/__pycache__/`.

Updated: `README.md` (the protobuf error-message example changes wording, the
Run/Check blocks change commands, "one Python file plus a generated protobuf
module" becomes the crate), `CLAUDE.md` (commands and architecture),
`docs/ROADMAP.md` (tick the box).

`docs/mood/samples/*.svg` must stay byte-valid — they are configs plus their
renders, and byte-identical output means they do not move. One is verified as
an early smoke test. If they turn out to have drifted *before* this work, that
gets reported rather than quietly regenerated.

## Verification strategy

The corpus gives a pass/fail signal long before the port is complete, because
an SVG is assembled in document order. Work proceeds by making a growing
*prefix* of one golden match:

1. `rng.rs` against captured Python vectors.
2. `fmt`/`geom`/`style` against literals lifted from a golden.
3. Header + `defs` + `css` → matches a golden's prefix up to `<g>`.
4. Lattice and triangles → the `<g>…</g>` body matches.
5. Icons → the tail matches; one whole golden passes.
6. Starfield, blinds, rain → the axes that remain.
7. CLI, `--configs`, `test/golden.py` → all 42 pass.

A step that cannot match its golden is a defect found within one module rather
than in a 1300-line diff.

## Migration

A hard break on `feature/rust-port`. No Python fallback, no dual
implementation: the corpus is the equivalence proof, and keeping both would
mean keeping the generated `_pb2` the request removes. Python stays in the
dev shell to run `test/golden.py`.
