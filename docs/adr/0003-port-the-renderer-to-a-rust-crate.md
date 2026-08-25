# 0003. Port the renderer from `background.py` to the `bgsvg` crate

| Field    | Value                                  |
|----------|----------------------------------------|
| Date     | 2026-08-19                             |
| Status   | Accepted                               |
| Deciders | theta                                  |
| Branch   | `feature/rust-port`                    |
| Commit   | `1ca96b3` (design), `27192c0` (break)  |

## Context

`background.py` was 1075 lines carrying the whole program: the schema boundary,
the RNG, geometry, four pattern generators, two glyphs, the CSS, the CLI, and
its own test suite. `parameters_pb2.py` was generated Python committed beside
it. The ROADMAP asked for Rust with an object split.

Nothing about the *picture* was wrong. The mood contract in `docs/mood/`, the
42-config corpus from [[0002]] and the `--selftest` invariants all described
behaviour that had to survive unchanged.

## Decision

We will build a Cargo package `bgsvg`: a thin binary over a library of ten flat
`snake_case` modules, one per stage `CLAUDE.md` already names.
`parameters.proto` stays authoritative and stops generating Python —
`build.rs` generates Rust from it via `prost-build` + `pbjson-build`.

**The 42 golden SVGs are the acceptance test: the port is done when
`test/golden.py` passes against the corpus it already holds, unmodified.**

Three supporting decisions:

- **`--selftest` becomes `cargo test`**, with invariants beside the code they
  check, where they can reach private items.
- **`parameters.json` is deleted**; a bare `bgsvg` renders `{}`.
- **`Scene` deletes the vocabulary bridge.** Python's `resolve()` mapped schema
  enums onto lowercase strings (`_BG`, `_IMG`) because it had no better boundary
  type. In Rust the schema enums *are* types, so the renderer takes them
  directly. `Ship`'s lack of a motion becomes a variant without a field rather
  than a `"static"` string, and `overlay: Option<Rain>` restates the schema's
  structural rule in the renderer's own types.

Objects only where they earn their keep — `Lattice`, `PyRandom`, `Scene`. **No
traits:** there is one implementation of everything, and an `IconRenderer` trait
with two impls would be scaffolding for a third icon that does not exist.

## Alternatives Considered

### Port, then regenerate the corpus to match

The obvious shortcut: reimplement freely and re-bless the goldens.

Rejected, and this is the decision the whole port hangs on. A regenerated corpus
passes trivially because it was made to match whatever the new code does. Keeping
it makes the committed corpus *proof* the refactor moved nothing. This is what
forced [[0004]].

### `rust-protobuf` instead of `prost` + `pbjson`

Reading the generated-code templates of both:

| case | pbjson | rust-protobuf's `protobuf-json-mapping` |
|---|---|---|
| unknown field | error | error |
| unknown enum name | error | error |
| two members of one `oneof` | `duplicate_field` error | **not detected — last wins** |

Rejected because `{"output": {"file": {...}, "stdout": {}}}` would parse
*successfully* under rust-protobuf, silently keeping one sink, and `validate()`
could not catch it afterwards because the evidence is gone by then. Closing that
would need a descriptor-driven pre-pass over the raw JSON. pbjson gets it right
for free, at the price of `protoc` at build time — already in the flake.

### `clap`, `anyhow` / `thiserror`

Rejected: two flags do not justify an argument parser, and a three-variant error
enum with `Display` covers the error surface. `std::env::args` and a hand-written
enum keep the dependency list at what the schema actually requires.

### Keep Python as a fallback or dual implementation

Rejected: the corpus is the equivalence proof, and keeping both would mean
keeping the generated `_pb2` module the port exists to remove. Python stays in
the dev shell only to run `test/golden.py`. (That last use was removed by
[[0013]]; the dev shell no longer carries a Python.)

## Consequences

### Positive

- One crate, ten focused modules, invariants as `cargo test` beside their code.
- The vocabulary bridge (`_BG` / `_IMG`) disappeared — the schema enums are the
  renderer's types.
- Stricter by accident in good ways: duplicate JSON keys became an error (Python
  kept the last one), and an out-of-range seed fails to parse rather than
  overflowing.

### Negative / Trade-offs

- **One deliberate behaviour change:** `0x0` resolution. Python's `parse_res`
  accepted it and `lattice()` raised an uncaught `ZeroDivisionError`. Rust would
  divide by zero in floating point and emit a *garbage SVG*, which is worse — so
  `parse_res` now rejects a zero dimension, exiting 2 with a message.
- `protoc` is required at build time.
- Every docstring had to be ported as rustdoc rather than summarised: `CLAUDE.md`
  defers to `ico_ship`'s and `pat_matrix`'s docstrings as the authority for the
  visuals, so dropping them would delete the mood contract's reasoning while
  keeping its output.

### Neutral

- `Cargo.lock` must be committed and git-tracked, or the Nix build fails to see
  it while a plain `cargo build` succeeds.

## Resumption (for Agent)

### Current state

Complete. `background.py`, `parameters_pb2.py` and `parameters.json` are deleted.
All 42 goldens passed unmodified at completion.

### Key files / entry points

| File | Role |
|------|------|
| `build.rs` | `prost-build` → descriptor set → `pbjson-build` |
| `src/lib.rs` | module wiring and `run(args) -> i32`, so tests can drive the CLI boundary |
| `src/params.rs` | generated types, `validate`, `resolve` → `Scene`, `valid_configs` |
| `docs/superpowers/specs/2026-08-19-rust-port-design.md` | the full design, including the port-hazard list |

### Next steps

None.

### How to verify

```bash
nix develop -c cargo test
nix develop -c cargo test --test golden
nix build
```

### Gotchas

The spec's port-hazard list is the accumulated cost of Python/Rust semantic
gaps, and every one of them silently corrupts output rather than failing:

- **`%` on negatives.** `NB[r % 2]`, `(k - 1) % 6`, `(head - j) % N` all run
  negative. Python returns non-negative; Rust follows the dividend's sign. All are
  `rem_euclid`. Missing one silently rearranges the lattice.
- **`round()` is banker's rounding.** `_mix` used Python's half-to-even `round`;
  `f64::round` is half-away-from-zero. Must be `round_ties_even`, or every colour
  in the file shifts.
- **Negative zero.** `fmt(-0.001)` prints `0` in Python; Rust's `{:.2}` prints
  `-0.00`.
- **`math.hypot`.** CPython 3.14 uses its own correctly-rounded implementation;
  Rust calls libm. It is only compared against `clear_r`, so a 1-ulp gap could
  flip one cell's eligibility. The goldens are the detector.
- Do not add a trait to "generalise" the two icons. That was considered and
  rejected as scaffolding for a third icon that does not exist.

### Related

- Commits: `1ca96b3`, `ef39028`, `6acfdea`, `27192c0`, `01db85e`
- ADRs: [[0001]] the schema it kept · [[0002]] the corpus that proved it ·
  [[0004]] the RNG decision this forced
