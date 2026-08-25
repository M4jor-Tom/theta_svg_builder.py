# 0006. Keep composition in Rust — no `.svg` file is ever "morphy"

| Field    | Value                                  |
|----------|----------------------------------------|
| Date     | 2026-08-21                             |
| Status   | Accepted                               |
| Deciders | theta                                  |
| Branch   | `feature/svg-templates` (merged to `master`) |
| Commit   | `c00edee`                              |

## Context

Having chosen a template engine ([[0005]]), the question was where *composition*
lives: the act of assembling a finished document out of independently rendered
fragments.

A four-tier vocabulary was proposed for classifying the pieces:

| Tier | Meaning |
|---|---|
| **pure** | markup with no substitution |
| **parametrized** | markup with value slots |
| **controlled** | markup with `if` / `for` |
| **morphy** | assembles rendered fragments |

The initial design applied this taxonomy to *files*, and labelled a
`document.svg` template as "morphy" because it composed the others via nested
template structs.

That was wrong, and correcting it changed the architecture.

## Decision

We will treat the taxonomy as classifying **components, not files**, and
**morphy is a `.rs` tier**. No `.svg` file is ever morphy.

Each morphy module (`svg.rs`, `trihex.rs`, `matrix.rs`) holds its own `String`
buffer and appends rendered fragments into it in whatever order it chooses.
Templates only ever emit markup from data handed to them.

The *pure* tier is expected to stay empty in this crate: `PAL.a` is computed at
runtime (`darken("#6fb7d1", 0.58)`), so even the ship glyph needs substitution.
The tier costs nothing to keep.

## Alternatives Considered

### `document.svg` as a composing template (the original design)

A root template holding nested `Template` structs as fields, letting askama
render the tree.

Rejected because it puts the byte-level joins between fragments under the
template engine's control. Askama then decides what sits between two elements,
which is precisely the surface where `whitespace = "suppress"` misbehaves
(see [[0010]]). It also inverts the responsibility: composition is ordering
logic, and ordering logic belongs with the code that knows the seeded, ordered
data.

### One file per element, composed in Rust

Maximal fragmentation — a `.svg` per `<polygon>`, `<line>`, `<circle>` — with
Rust joining everything.

Rejected as over-fragmentation. It would turn the ship's ten elements into
seven files and buys nothing once Rust owns the joins. Granularity settled at
**one file per coherent visual unit**.

### Per-item `{% include %}` inside a loop

Letting a template loop and include a per-cell template.

Rejected in favour of nested loops in a single template plus Rust-side
iteration. `trihex.svg` holds three sequential `{% for %}` loops matching the
concatenation order (`voids`, `tris`, `hexes`); `matrix.svg` nests a glyph loop
inside a column loop. Fewer files, no include indirection.

## Consequences

### Positive

- **Rust owns every byte between fragments.** The class of "the engine inserted a
  newline at a block boundary" bugs is confined to the inside of six small files.
- The escaping surface shrank to the five markup slots in `root.svg` — every other
  interpolation is an attribute value (a number, a colour, an id). See [[0008]].
- Composition sits next to the seeded, ordered data it depends on, which is what
  keeps determinism intact.
- `Template::render_into(&mut dyn fmt::Write)` is available if per-fragment
  allocation ever matters; today plain `.render()` is used and allocates trivially.

### Negative / Trade-offs

- `root.svg` needs five markup slots rather than the one the design initially
  claimed, because the shell interleaves its own elements (`<rect>`, the halo
  `<circle>`, the vignette, the icon `<g>`) between the parts it composes.
- A glyph's definition is split across two files: coordinates and decisions in
  `.rs`, markup in `.svg`. `ico_ship` was one readable function and is now a struct
  in one file and a template in another.

### Neutral

- The taxonomy vocabulary itself lives only in `docs/superpowers/specs/` and in
  this ADR; the source does not use the words.

## Resumption (for Agent)

### Current state

Complete and merged.

### Key files / entry points

| File | Role |
|------|------|
| `src/svg.rs` | morphy root: builds `Defs`, then `Root`, composing the rest |
| `src/trihex.rs` | morphy: one eager pass builds `voids`/`tris`/`hexes`, then renders |
| `src/matrix.rs` | morphy: builds columns and cells, then renders |
| `templates/root.svg` | the five composition slots |
| `templates/trihex.svg` | three sequential loops in concatenation order |

### Next steps

None. Optionally: record the four-tier vocabulary in `CLAUDE.md`, since it is
currently only in dated design docs and this ADR.

### How to verify

```bash
# No template composes another; composition is Rust-side only.
grep -rn 'render()' src/          # 6 call sites, all in .rs
grep -c '{% include' templates/*.svg   # expect 0 everywhere
nix develop -c cargo run --example golden
```

### Gotchas

- **The concatenation order is the layering.** `voids` sit under the triangles so a
  translucent triangle crossing a window reads as a shard catching light; borders
  go on top of everything. That order is enforced by the order of the three loops
  in `templates/trihex.svg` — **not** by the field declaration order on `Trihex`,
  which askama binds by name and ignores.
- The blind is emitted *inside its own cell's iteration*, not in a batched second
  pass. See [[0007]].

### Related

- Commits: `c00edee`, `f6f6500`, `9c2cb78`
- ADRs: [[0005]] engine choice · [[0007]] the seam · [[0008]] escaper
