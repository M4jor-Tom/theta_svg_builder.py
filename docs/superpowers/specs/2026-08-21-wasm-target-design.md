# WASM target — a browser-callable build of the renderer

**Status:** designed, not implemented · **Date:** 2026-08-21

## Problem

`bgsvg` renders one SVG from one JSON config on a machine with a filesystem and
a process. A consumer that has neither cannot call it — even though nothing in
the render path needs either.

## Scope

This spec covers what this repository produces: a WASM artifact, the API it
exposes, and the tests that keep that API honest.

The first consumer is a config editor in the `svg.studio.ui` repository, which
has its own spec. This document is the supply side of that boundary. Everything
downstream of the API below is specified there and decided there.

## Non-goals

- **No user interface.** No layout, theme, typography, form design, framework,
  or TypeScript is decided here. The editor's spec owns all of it.
- **No hosting or deployment** of any page that consumes the module.
- **No change to rendered bytes.** Every change below is additive; the golden
  corpus must pass untouched. That is the point of the byte-identity test.
- **No new renderer features.** Sharing starfields through `<defs>`/`<use>`,
  which `docs/mood/README.md` names as the first lever if `CLOSEOPEN`'s ~222 KB
  at 4k stops being acceptable, changes rendered bytes and is out of scope. The
  editor will make that size visible for the first time, which may be what
  eventually prompts it.
- **Not a published package.** The artifact is built from this repository and
  consumed by a known repository. No npm publishing, no semver contract for
  third parties, no bundler-agnostic packaging work.

## Why this needs no logic change

The render path is already pure. `svg::build_svg` touches no filesystem, no
clock and no OS entropy — `rng.rs` reimplements CPython's MT19937 seeded from a
string, precisely so a render depends on nothing but its inputs. `std::fs`
appears only in `lib.rs`'s CLI body and in tests. The six dependencies
(`askama`, `prost`, `pbjson`, `serde`, `serde_json`, `sha2`) are pure Rust with
no C shims.

So compiling to `wasm32-unknown-unknown` is a packaging exercise, not a port.

## API specification

This section is the contract. A consumer may rely on exactly what is below.

### The WASM module

```rust
#[wasm_bindgen]
pub fn render(json: &str, width: u32, height: u32) -> Result<String, JsValue>;

#[wasm_bindgen]
pub fn resolve_resolution(spec: &str) -> Result<Box<[u32]>, JsValue>;

#[wasm_bindgen]
pub fn resolutions() -> String;
```

**`render`** runs parse → validate → resolve → `build_svg` and returns the SVG
document as a string. `width` and `height` are pixels and must both be
non-zero. It is the only function that renders anything.

The config's `output` field is parsed and validated like any other field, then
ignored: sinks name a destination, and there is no destination here. A config
carrying `output` is accepted rather than rejected, so that a file written for
the CLI renders unaltered.

**`resolve_resolution`** maps a resolution spec — one of the eight preset names,
or `WIDTHxHEIGHT` — to `[width, height]`. It is `parse_res` and nothing more.
It exists so a consumer never reimplements that parsing, including its
edge cases: empty means `1080p`, a whitespace-only string is rejected, and a
zero dimension is rejected.

**`resolutions()`** returns the preset table as a JSON array, in declaration
order:

```json
[{"name":"1080p","width":1920,"height":1080}, …]
```

### Errors

Both fallible functions throw a plain JavaScript object, built with `js-sys`:

```ts
{ kind: "schema" | "invalid", message: string, line?: number, column?: number }
```

- **`kind: "schema"`** — the JSON is malformed, names an unknown key, sets two
  members of one `oneof`, or carries a value of the wrong type. `line` and
  `column` are present, taken from `serde_json::Error`, and are 1-based.
- **`kind: "invalid"`** — a rule `parameters.proto` cannot express, from
  `params::validate`. `line` and `column` are absent.

`message` is `Error`'s existing `Display` output, unchanged.

The distinction is the load-bearing part: it is what lets a consumer route a
syntax error to the text the user typed and a semantic error to the field it
concerns. `Error::Io` is unreachable in this build and has no mapping.

The error carries no machine-readable field path. Only one cross-field rule
exists — `CLOSEOPEN` with `NONE` — and its message already names both fields.
See **Deferred**.

### The CLI

```sh
bgsvg --descriptor    # descriptor.bin to stdout
```

`build.rs` already emits the proto descriptor set at `OUT_DIR/descriptor.bin`
and discards it. This flag is the schema itself, machine-readable, for a
consumer in another language — the exact precedent `--configs` set.

It is deliberately a CLI flag rather than a WASM export: its consumer is a
build-time check, not a running page, and it must be reachable without loading
a renderer.

## Changes required

**1 — `bgsvg-wasm`, a new workspace member.** The `wasm-bindgen` crate
implementing the API above. Separate from `bgsvg` so the core keeps the six
dependencies `lib.rs` advertises and `wasm-bindgen` never enters it. It owns
`console_error_panic_hook`, since a trap poisons the module for every later
call.

**2 — `pub fn render_to_string(json: &str, w: u32, h: u32) -> Result<String, Error>`
in `lib.rs`.** The private `render()` already runs parse → validate → resolve →
`build_svg` before choosing a sink. Extract those four steps; `render()` calls
the new function once per resolution, and `bgsvg-wasm::render` calls the same
one.

Every piece is already `pub`, so this is not what unblocks the WASM build — it
is what stops the pipeline existing twice. A rule added to `validate()` must
reach both callers without anyone remembering to wire it, the same reason both
test surfaces enumerate from `valid_configs`.

**3 — `bgsvg --descriptor`**, as specified above.

**4 — `pub const RESOLUTIONS`.** Lift the `PRESETS` array out of the body of
`parse_res` (`params.rs:167`) to module scope, still used by `parse_res`. It
backs both `resolutions()` and `resolve_resolution`.

**5 — `flake.nix` builds and exposes the module.** Two parts:

- The devShell gains the `wasm32-unknown-unknown` target and
  `wasm-bindgen-cli`; `Cargo.toml` becomes a workspace. Nothing builds for the
  browser today.
- A new output, **`packages.bgsvg-wasm`** — the `wasm-bindgen` output directory
  containing the `.wasm`, its JavaScript glue and its generated `.d.ts`.

The package output is what makes this consumable. A consumer declares this
repository as a flake input and takes the artifact from the store, so
`flake.lock` pins the revision and the module, the `.d.ts` and `--descriptor`
are guaranteed to come from one revision — which is the property a drift check
depends on and the one a hand-managed vendor step gets wrong.

`packages.default` stays `bgsvg`.

**6 — a byte-identity test.** Assert the WASM build renders byte-identical SVGs
to the native build across all 42 configs from `--configs`.

"Looks the same" is not this repository's standard — every render is pinned to
a sha512 for exactly that reason. A WASM build that renders *nearly* the same
picture would pass every other test here, and float formatting through
`geom::fmt` is the plausible way it would happen. The corpus already holds the
expected bytes, so the test is a sweep comparing both builds against them.

## Deferred

**Field paths on `Error::Invalid`.** A machine-readable path would let a
consumer place an error on the exact field that caused it. Today's
`Error::Invalid(String)` cannot carry one. It is not needed yet: only one
cross-field rule exists, and its message names both fields it concerns.
Revisit when a second appears — at which point the `kind` union above gains a
`field` member rather than changing shape.

## To verify during implementation

**The artifact's size.** Estimated at 300–500 KB before compression, but not
measured. It is the whole download budget for any page that loads it, so if it
lands far above that, `opt-level = "z"` and `wasm-opt` come before any other
optimisation.

**That `render` cannot panic on validated input.** `console_error_panic_hook`
reports a trap; it does not prevent one. If any input that passes `validate()`
can still panic inside `build_svg`, that is a bug in this crate, and the WASM
build is simply where it becomes visible.

## ADRs due

Two decisions here deserve records once implementation starts, continuing the
existing numbering: rendering in the browser via WASM rather than behind a
server, and placing the binding crate in this repository rather than in the
consumer's.
