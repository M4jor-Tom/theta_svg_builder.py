# Done

- [x] Json as parameters — [spec](superpowers/specs/2026-08-19-parameters-json-design.md) · no ADR
- [x] Determined hash-map test — no ADR
- [x] Refactor to rust with object split — [spec](superpowers/specs/2026-08-19-rust-port-design.md) · no ADR
- [x] SVG / RS split refactor — [spec](superpowers/specs/2026-08-20-svg-templates-design.md) · ADR [0001](adr/0001-use-askama-for-svg-templating.md) [0002](adr/0002-keep-composition-in-rust-not-templates.md) [0003](adr/0003-rust-computes-templates-substitute.md) [0004](adr/0004-disable-escaping-for-svg-templates.md) [0005](adr/0005-never-regenerate-the-golden-corpus.md) [0006](adr/0006-suppress-whitespace-and-indent-templates.md)

# Planned

- [ ] UI — not yet designed, so no ADR is due

# Without an ADR

Decisions taken before `docs/adr/` existed. Each has a design doc, but no record
of the alternatives rejected — which is the part a design doc does not carry.
What each missing ADR would have to argue:

- **Json as parameters** — why protobuf is the schema; why conditional rules are
  made structural where the model allows it (a motion belongs to the icon that
  declares it) and rejected by `validate()` where it does not.
- **Determined hash-map test** — why the corpus is named by the sha512 of each
  file's own bytes; why byte comparison rather than a structural one. [[0005]]
  records how this refactor *used* the corpus, not why it was built that way.
- **Refactor to rust with object split** — why the port at all; why `src/rng.rs`
  reimplements CPython's MT19937, string seeding and `_randbelow`/`shuffle`/`sample`
  rather than using a Rust RNG.

Backfilling these is optional — they are settled and working. Write one only if a
decision is about to be revisited.
