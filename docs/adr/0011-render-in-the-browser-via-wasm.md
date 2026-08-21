# 0011. Render in the browser via WASM rather than behind a server

| Field    | Value                          |
|----------|--------------------------------|
| Date     | 2026-08-21                     |
| Status   | Accepted                       |
| Deciders | theta                          |
| Branch   | `master`                       |
| Commit   | `d510469..5644c3f`             |

## Context

The ROADMAP's one planned item was a UI. Rendering happens in `svg::build_svg`,
reached through a CLI that reads a file and writes files — so any consumer
without a filesystem and a process is locked out.

The question was where a render runs when a user drags a slider.

`build_svg` touches no filesystem, no clock and no OS entropy: `rng.rs`
reimplements CPython's MT19937 seeded from a string, precisely so a render
depends on nothing but its inputs ([[0004]]). `std::fs` appears only in
`lib.rs`'s CLI body and in tests, and all six dependencies are pure Rust with
no C shims. The renderer was therefore already portable; only its packaging
was not.

## Decision

We will build for `wasm32-unknown-unknown` and let the browser call the
renderer directly, through a `wasm-bindgen` crate ([[0012]]) exposing
`render(json, w, h)`, `resolve_resolution(spec)` and `resolutions()`.

`bgsvg-wasm` is a consumer of the same pipeline as the CLI, not a second one:
`lib.rs` gains `pub fn render_to_string(json, w, h)` holding parse → validate →
resolve → `build_svg`, and both callers use it. A rule added to `validate()`
must reach both without anyone remembering to wire it — the same principle that
makes both test surfaces enumerate from `valid_configs` ([[0002]]).

**The corpus extends to cover it.** A byte-identity test asserts the WASM build
renders the same bytes as the native build across all 42 configs. "Looks the
same" has never been this repository's standard ([[0009]]), and a WASM build
that renders *nearly* the same picture would pass every other test here.

## Alternatives Considered

### A local Rust server wrapping the crate

A small `axum` binary serving `POST /render`; the page stays plain HTML with no
WASM toolchain and no module to download.

Rejected. It costs a resident process (ballpark 5–15 MB) and an HTTP round trip
per keystroke, and — decisively — the editor then only exists while a process
is running. It cannot be opened offline or hosted as a static page.

### A server shelling out to the `bgsvg` binary

Needs no change to this crate at all: spawn the binary per request with a temp
config.

Rejected as strictly the worst on CPU. Every input event pays `fork`/`exec`,
dynamic linking and a temp-file write and read, from cold, on top of the same
render — and a Node or Bun host adds the fattest baseline RSS of the three.

### Reimplement the renderer in TypeScript

Rejected without much deliberation: it would require reimplementing CPython's
MT19937 a second time, and the corpus pins every draw. This is the one option
that could not be proven correct against `test/golden/`.

### An honest note on why this was *not* decided on resource grounds

All three options paint the identical SVG in the identical browser, and that
paint is the dominant cost — `docs/mood/README.md` measures 1080p `CLOSEOPEN`
at 46–59 fps and ~1031–1142 MiB renderer RSS. A 10 MB server process is a
rounding error against it. WASM does win on resources, but by a margin that
does not matter; it was chosen for deployment, offline use, and having one
fewer moving part.

The levers that *do* matter belong to the consumer, not here: preview at a
small pixel area (density is resolution-independent, so composition is
unchanged), deliver through an `<img>` to keep ~1800 animated nodes out of the
main document, and revoke each blob URL.

## Consequences

### Positive

- The editor is a static page: hostable, offline, no install, no process.
- `render_to_string` removes the possibility of the pipeline existing twice.
- The corpus's reach grows — 42 configs now pin two builds instead of one.

### Negative / Trade-offs

- A download of the module: **measured** at 366,312 bytes (~358 KiB) for
  `bgsvg_wasm_bg.wasm`, comfortably inside the pre-implementation 300–500 KB
  estimate — a measurement, not a guess (`nix build .#bgsvg-wasm --no-link
  --print-out-paths`, then `du -h`/`stat` on the result). If a future revision
  lands far above that, `opt-level = "z"` and `wasm-opt` come before any other
  optimisation.
- `packages.bgsvg-wasm` emits **two** subdirectories from one `.wasm`: `web/`
  for a bundler consumer and `nodejs/` for the byte-identity sweep in
  `test/wasm.mjs`. Only the `wasm-bindgen` JavaScript glue differs between
  them — the `.wasm` is one build, so rendered bytes cannot differ between the
  two.
- A panic traps the module and poisons every later call, so `bgsvg-wasm` carries
  `console_error_panic_hook`. That reports a trap; it does not prevent one.
- `Cargo.toml` becomes a workspace, and the devShell grows the
  `wasm32-unknown-unknown` target and `wasm-bindgen-cli`. nixpkgs' `rustc`
  shipped the `wasm32-unknown-unknown` standard library as-is, so no
  `rust-overlay` was needed; only `pkgs.lld` had to be added, because the
  linker binary itself was absent.

### Neutral

- The `output` field is parsed and validated in the WASM path, then ignored:
  sinks name a destination and there is no destination in a browser. A config
  written for the CLI therefore renders unaltered rather than being rejected.
- `Error::Io` is unreachable in this build and has no mapping to the thrown
  error object.

## Resumption (for Agent)

### Current state

Complete. All 42 goldens passed unmodified throughout, and `test/wasm.mjs`
reports the WASM build renders byte-identical documents at 1920×1080.

### Key files / entry points

| File | Role |
|------|------|
| `docs/superpowers/specs/2026-08-21-wasm-target-design.md` | the design, including the full API specification |
| `src/lib.rs:112` | private `render()`, whose first four steps become `render_to_string` |
| `src/svg.rs:76` | `build_svg(w, h, &Scene)` — the pure function this exposes |
| `flake.nix` | gains the wasm target and `wasm-bindgen-cli` |

### Next steps

None here. The consumer's editor is specified in the `svg.studio.ui`
repository and is out of scope for this one.

### How to verify

```bash
nix develop -c cargo test --workspace  # invariants, including bgsvg-wasm
nix develop -c python3 test/golden.py  # the picture did not change
nix build .#bgsvg-wasm --no-link --print-out-paths   # then, against that path:
BGSVG_WASM=<out-path>/nodejs nix develop -c node test/wasm.mjs   # WASM == native, all 42 configs
```

### Gotchas

- **Byte identity is the whole risk.** Float formatting through `geom::fmt` is
  the plausible way a WASM build diverges while still looking correct. The
  byte-identity test is not optional politeness; it is what makes this decision
  safe.
- Do not add `wasm-bindgen` to `bgsvg` itself — see [[0012]].
- Nothing here may move a rendered byte. The goldens must pass untouched, and
  `--regen` is not the answer if they do not ([[0009]]).
- **`Cargo.lock`'s `wasm-bindgen`/`js-sys` versions must match
  `pkgs.wasm-bindgen-cli`'s schema version exactly**, or `nix build
  .#bgsvg-wasm` fails with a confusing schema-mismatch error rather than a
  normal compile error. A `nixpkgs.url` bump is the likely trigger; the repair
  is `cargo update -p wasm-bindgen --precise <version>`. Commented at the pin's
  site in `flake.nix`.

### Related

- ADRs: [[0012]] where the binding crate lives · [[0004]] the RNG that makes a
  render depend only on its inputs · [[0009]] the standard the byte-identity
  test inherits · [[0002]] the corpus it extends
- The consumer's own design lives in the `svg.studio.ui` repository and is out
  of scope here.
