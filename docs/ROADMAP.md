# Done

- [x] Json as parameters — [spec](superpowers/specs/2026-08-19-parameters-json-design.md) · ADR [0001](adr/0001-one-json-config-with-a-protobuf-schema.md)
- [x] Determined hash-map test — ADR [0002](adr/0002-pin-every-render-with-a-sha512-corpus.md)
- [x] Refactor to rust with object split — [spec](superpowers/specs/2026-08-19-rust-port-design.md) · ADR [0003](adr/0003-port-the-renderer-to-a-rust-crate.md) [0004](adr/0004-reimplement-cpython-mt19937-in-rust.md)
- [x] SVG / RS split refactor — [spec](superpowers/specs/2026-08-20-svg-templates-design.md) · ADR [0005](adr/0005-use-askama-for-svg-templating.md) [0006](adr/0006-keep-composition-in-rust-not-templates.md) [0007](adr/0007-rust-computes-templates-substitute.md) [0008](adr/0008-disable-escaping-for-svg-templates.md) [0009](adr/0009-never-regenerate-the-golden-corpus.md) [0010](adr/0010-suppress-whitespace-and-indent-templates.md)

# Planned

- [ ] WASM target — [spec](superpowers/specs/2026-08-21-wasm-target-design.md) · ADR [0011](adr/0011-render-in-the-browser-via-wasm.md) [0012](adr/0012-keep-the-wasm-binding-in-this-repository.md)

  A browser-callable build of the renderer, plus the API it exposes. The render
  path is already pure, so this is packaging, not a port.

  It exists for a config editor that lives in its own repository
  (`svg.studio.ui`) and has its own spec. That editor is out of scope here, and
  nothing about it is decided in this repository — the API below is the whole
  boundary. None of these changes alters a rendered byte:

  - [ ] **`bgsvg-wasm`, a new workspace member.** The `wasm-bindgen` crate
    implementing the API: `render(json, w, h)`, `resolve_resolution(spec)` and
    `resolutions()`. Separate so `bgsvg` keeps the six dependencies `lib.rs`
    advertises and `wasm-bindgen` never enters the core. It owns
    `console_error_panic_hook`, since a trap poisons the module for every later
    call.
  - [ ] **`pub fn render_to_string(json, w, h) -> Result<String, Error>`.**
    `render()` already runs parse → validate → resolve → `build_svg` before
    picking a sink; extract those four steps so the CLI and the WASM export call
    one pipeline instead of two. Every piece is already `pub`, so this is not
    what unblocks the build — it is what keeps a rule added to `validate()` from
    reaching only one caller, the same reason both test surfaces enumerate from
    `valid_configs`.
  - [ ] **`bgsvg --descriptor`.** Dump `descriptor.bin` to stdout. `build.rs`
    already emits it at `OUT_DIR` and discards it. Mirrors `--configs`: a
    machine-readable dump of something the build already knows, for a consumer
    in another language. A CLI flag rather than a WASM export, because its
    consumer is a build-time check that must not have to load a renderer.
  - [ ] **`pub const RESOLUTIONS`.** Lift the `PRESETS` array out of the body of
    `parse_res` to module scope, still used by `parse_res`. It backs both
    `resolutions()` and `resolve_resolution`.
  - [ ] **`flake.nix` builds and exposes the module.** The devShell gains the
    `wasm32-unknown-unknown` target and `wasm-bindgen-cli`, and `Cargo.toml`
    becomes a workspace — nothing builds for the browser today. Plus a new
    `packages.bgsvg-wasm` output holding the `wasm-bindgen` directory (`.wasm`,
    glue, generated `.d.ts`), which is what lets a consumer declare this
    repository as a flake input and take the module, its types and
    `--descriptor` from one locked revision. `packages.default` stays `bgsvg`.
  - [ ] **Byte-identity test: WASM output == native output, all 42 configs.**
    "Looks the same" is not this repository's standard — a WASM build that
    renders *nearly* the same picture passes every other test here, and float
    formatting through `geom::fmt` is the plausible way it happens. The corpus
    already holds the expected bytes.

  Deferred, with reasons in the spec: field paths on `Error::Invalid` (only one
  cross-field rule exists, and its message already names both fields), and
  sharing starfields through `<defs>`/`<use>` (changes rendered bytes, so it is
  a renderer decision rather than part of this).

ADRs are numbered in decision order, so `docs/adr/` reads chronologically:
config schema, then the corpus, then the Rust port, then the template split,
then the WASM target. [[0011]] and [[0012]] are the first recorded *before*
their implementation rather than after it, so their Resumption sections say
what to build rather than what was built.
