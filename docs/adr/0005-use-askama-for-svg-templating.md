# 0005. Use askama to move SVG markup out of Rust

| Field    | Value                                  |
|----------|----------------------------------------|
| Date     | 2026-08-21                             |
| Status   | Accepted                               |
| Deciders | theta                                  |
| Branch   | `feature/svg-templates` (merged to `master`) |
| Commit   | `5473be9`                              |

## Context

Every SVG element this crate emitted was a `format!` string literal inside
`src/*.rs` — 47 `format!` sites, concentrated in `trihex.rs` (16), `icon.rs`
(11) and `svg.rs` (6). The markup was invisible to every tool that understands
SVG, unreadable next to its own escaped quotes, and impossible to inspect
without running the binary.

The requirement was absolute: **no `<` in `src/*.rs`** outside test modules,
with control flow (`if` / `for`) available in the template layer.

Two constraints narrowed the field sharply:

- **`test/golden.py` pins exact bytes.** 42 golden SVGs are named by the
  sha512 of their own contents. Any tool that owns the serializer owns whether
  those hashes move.
- **The crate is self-contained.** `CLAUDE.md` states the binary reads nothing
  from disk at runtime, so any solution had to inline templates at compile time.

## Decision

We will use **askama** (0.16.0) — compile-time Jinja-style templates that
generate type-checked Rust code — with six templates under `templates/`.

Askama is the only Rust option that provides control flow *without surrendering
control of the output bytes*: it generates Rust that writes exactly the literal
text of the template, so byte-exactness remains ours to hold.

## Alternatives Considered

### XSLT — the actual W3C standard

The standards-track answer for parametrised XML with control flow since 1999:
`<xsl:param>`, `<xsl:if>`, `<xsl:choose>`, `<xsl:for-each>`, XPath. SVG is XML,
so it applies natively.

Rejected on three counts. Rust support is not viable — `xrust` targets XSLT 3.0
but sits at XSLT 1.0 equivalence, and `rust-libxslt` self-describes as "an
infant proof of concept … not ready for production use". XSLT also transforms a
*document* into a document, while our input is a `Scene` struct, so adoption
would mean serialising `Scene` → XML → transform → SVG: a pipeline with two
extra formats. Finally its verbosity is severe — the four-facet ship loop would
become roughly 15 lines of `<xsl:for-each>` with `<xsl:value-of>` per attribute.

### Natural templates (TAL / Genshi / Thymeleaf / Angular `*ngFor`)

Control flow expressed as attributes in a foreign XML namespace, so the
template file stays a valid, renderable document that a vector editor can open.
This is the design that best fits the stated goal, and Wikipedia's description
of TAL is almost verbatim the requirement: template logic in namespaced
attributes so "templates remain well-formed documents that can be viewed and
edited with standard XML or HTML authoring systems".

Rejected because **no Rust implementation exists**. Java, Python and JS have
this family; Rust has none. Building one means `quick-xml` plus a tree-walking
directive interpreter, and moves rendering to runtime XML parsing — a project,
not a refactor, for two glyphs and four patterns.

### maud / hypertext — Rust markup macros

Compile-time and type-checked, the closest thing to JSX in Rust.

Rejected because maud is HTML-first by design and its documentation is
explicit: void elements "will use HTML syntax (e.g. `<br>`) rather than XHTML
(`<br />`)". `polygon` is not an HTML void element, so maud emits
`<polygon …></polygon>` — valid SVG, but nine extra bytes on *every* polygon,
and `pat_trihex` emits hundreds per render. There is no XHTML mode. For a crate
whose stated goal is small output this is disqualifying before the 42 broken
goldens are even considered.

### `svg` (bodoni) / `domrs` — builder crates

The de-facto Rust choice for programmatic SVG, actively maintained, fluent
builder API.

Rejected because both **own the serializer**. Exact bytes are the entire
mechanism of `test/golden.py`; handing that to a dependency means any upstream
patch touching whitespace or attribute order breaks all 42 goldens
indistinguishably from a real visual regression.

### Do nothing / a local `el()` helper

A ~10-line element helper alongside `pts`/`fmt` would have removed the escaped
quotes at zero dependency cost. Rejected because it does not satisfy the
requirement — the markup would still live in `.rs` — and it offers no control
flow.

## Consequences

### Positive

- Markup is in `.svg` files, inspectable and editable without running the crate.
- Control flow (`{% if %}` / `{% for %}`) is available and type-checked at compile time.
- `trihex.rs` improved materially: the polygon attribute list was written out four
  times across four `format!` arms and is now written once in the template.
- Self-containment is preserved — askama inlines templates at compile time.

### Negative / Trade-offs

- One new dependency (plus eight transitive crates).
- Templates are `.svg` files containing `{% %}`, so they are **not** openable in a
  vector editor. The editable-artwork property was never achievable in Rust
  (see the natural-templates alternative) and is explicitly not delivered.
- The goldens remain sha512-of-bytes, so an askama upgrade that changes whitespace
  handling can break all 42 at once. This is a recurring tax, not a one-off.
- Askama's whitespace semantics are subtle enough to need their own ADR — see [[0010]].

### Neutral

- `flake.nix` must include `./templates` and `./askama.toml` in its `fileset.unions`
  allowlist, because the proc macro reads them at compile time.

## Resumption (for Agent)

### Current state

Complete and merged. All six templates exist; `src/*.rs` contains no SVG markup
outside test modules; all 42 goldens are byte-identical to their pre-refactor
values.

### Key files / entry points

| File | Role |
|------|------|
| `askama.toml` | engine config: template dir, whitespace mode, escaper registration |
| `templates/*.svg` | the six templates (`root`, `defs`, `ship`, `hexatri`, `matrix`, `trihex`) |
| `Cargo.toml` | `askama = "0.16.0"` |
| `flake.nix` | `fileset.unions` must list `./templates` and `./askama.toml` |
| `tests/purity.rs` | guards that markup does not return to `src/` |

### Next steps

None. The decision is fully applied.

### How to verify

```bash
nix develop -c cargo test
nix develop -c cargo test --test golden  # must report 42 byte-identical
nix build                                  # proves the Nix sandbox sees templates/
```

### Gotchas

- **`nix build` is not optional as a check.** Askama's proc macro reads `templates/`
  at compile time; if the flake's source filter omits it, the sandboxed build fails
  while `cargo build` succeeds. Same failure mode as an untracked `Cargo.lock`.
- Do not swap askama for a builder crate without first replacing the byte-exact
  golden corpus with structural assertions — the two are mutually exclusive.

### Related

- Commits: `5473be9` (wiring), `5a7e96a..10e6c23` (full refactor)
- ADRs: [[0002]] the byte-exact corpus that ruled out every serializer-owning option ·
  [[0006]] where composition lives · [[0007]] what a template may do ·
  [[0008]] escaper · [[0009]] byte-exactness as proof · [[0010]] whitespace mode
