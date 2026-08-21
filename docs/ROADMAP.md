# Done

- [x] Json as parameters — [spec](superpowers/specs/2026-08-19-parameters-json-design.md) · ADR [0001](adr/0001-one-json-config-with-a-protobuf-schema.md)
- [x] Determined hash-map test — ADR [0002](adr/0002-pin-every-render-with-a-sha512-corpus.md)
- [x] Refactor to rust with object split — [spec](superpowers/specs/2026-08-19-rust-port-design.md) · ADR [0003](adr/0003-port-the-renderer-to-a-rust-crate.md) [0004](adr/0004-reimplement-cpython-mt19937-in-rust.md)
- [x] SVG / RS split refactor — [spec](superpowers/specs/2026-08-20-svg-templates-design.md) · ADR [0005](adr/0005-use-askama-for-svg-templating.md) [0006](adr/0006-keep-composition-in-rust-not-templates.md) [0007](adr/0007-rust-computes-templates-substitute.md) [0008](adr/0008-disable-escaping-for-svg-templates.md) [0009](adr/0009-never-regenerate-the-golden-corpus.md) [0010](adr/0010-suppress-whitespace-and-indent-templates.md)
- [x] WASM target — [spec](superpowers/specs/2026-08-21-wasm-target-design.md) · [plan](superpowers/plans/2026-08-21-wasm-target.md) · ADR [0011](adr/0011-render-in-the-browser-via-wasm.md) [0012](adr/0012-keep-the-wasm-binding-in-this-repository.md)

# Planned

- [ ] Field paths on `Error::Invalid` — only one cross-field rule exists today
  (`CLOSEOPEN` with `NONE`) and its message names both fields, so a consumer can
  place it without one. Revisit when a second cross-field rule appears.

ADRs are numbered in decision order, so `docs/adr/` reads chronologically:
config schema, then the corpus, then the Rust port, then the template split,
then the WASM target.
