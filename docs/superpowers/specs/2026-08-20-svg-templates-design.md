# SVG templates: moving markup out of `.rs` and into `.svg`

*2026-08-20*

## Problem

Every SVG element this crate emits is a `format!` string literal inside a `.rs`
file — 47 `format!` sites across `src/`, with the markup-emitting ones
concentrated in `trihex.rs` (16), `icon.rs` (11) and `svg.rs` (6). The
remainder are error messages and number formatting. The markup is invisible to
every tool that
understands SVG, unreadable next to its own escaped quotes, and impossible to
inspect without running the binary.

## Constraint

**No `<` in `src/*.rs`.** Every SVG element moves to a `.svg` file. Style
attribute *values*, element ids, and `style::css()` stay in Rust — they are not
markup, and their timing constants (`BLIND_KF`, `MATRIX_KF`) belong next to the
arithmetic that derives them.

This is a hard requirement, not a preference to be traded away.

## Decision

Adopt **askama**: compile-time Jinja-style templates that generate Rust code,
type-checked against a context struct. It is the only Rust option that provides
control flow without surrendering control of the output bytes.

Rejected, with reasons:

- **XSLT** — the actual W3C standard for this. Rust support is `xrust` (at
  XSLT 1.0 equivalence against a 3.0 goal) and `rust-libxslt` (self-described
  "infant proof of concept ... not ready for production"). Also transforms
  document→document, but our input is a `Scene` struct.
- **Natural templates** (TAL / Genshi / Thymeleaf / Angular `*ngFor`) — the
  design that would keep each file an openable SVG, because directives live in
  an ignored XML namespace. No Rust implementation exists. Building one means
  `quick-xml` plus a tree-walking interpreter, and moves rendering to runtime.
- **maud / hypertext** — emit HTML syntax, so `<polygon …></polygon>` rather
  than `<polygon …/>`. Nine extra bytes on every polygon, and `pat_trihex`
  emits hundreds per render.
- **`svg` / `domrs` builder crates** — own the serializer. Exact bytes are the
  entire mechanism of `test/golden.py`.

## Architecture

### The seam: Rust computes, templates render

Rust keeps every decision — the seeded RNG, the lattice, the triangle-role
assignment, the facet coordinates, the animation phases — and produces plain
data. Templates receive that data and emit markup. No template ever computes
geometry.

This is not a compromise imposed by the tool. It is the same seam CLAUDE.md
already mandates: geometry depends only on `seed`, and nothing about the
rendering path may perturb it. Putting `cy0 = n.1 + 6.0` into a template is
possible (askama does arithmetic) and is explicitly **out of scope**.

### The four tiers

The taxonomy applies to *components*, not to files. No `.svg` file is ever
"morphy" — morphy is a property of the `.rs` that composes.

| Tier | Meaning | Where it lives |
|---|---|---|
| pure | markup, no substitution | a `.svg` with no `{{ }}` |
| parametrized | markup + value slots | a `.svg` with `{{ }}` |
| controlled | markup + `{% if %}` / `{% for %}` | a `.svg` with blocks |
| **morphy** | assembles rendered fragments | **a `.rs` file** |

The *pure* tier is expected to stay empty: `PAL.a` is computed at runtime
(`darken("#6fb7d1", 0.58)`), so even the ship needs `{{ a }}`. The tier costs
nothing to keep.

### Composition is Rust's job

`Template::render_into(&self, writer: &mut dyn fmt::Write)` renders into an
existing buffer. Every morphy `.rs` therefore holds one `String` and appends
fragments into it in whatever order it likes — byte-for-byte the `push_str`
pattern the code already uses, with no per-element allocation.

Two consequences, both load-bearing:

1. **Rust owns every byte between fragments.** The template engine never
   decides what sits between two elements. The entire class of "askama inserted
   a newline at a block boundary" bugs is confined to the inside of six small
   files.
2. **Only one interpolation in the codebase carries markup** — the `{{ body }}`
   slot in `root.svg`. Every other interpolation is an attribute value: a
   number, a colour, an id. The escaping surface is a single point.

### File layout

```
templates/
  root.svg      param'd  <svg …>{{ body|safe }}</svg>  — the only markup slot
  defs.svg      ctrl     four always-on defs + {% if starfield %} nebulae
  hexatri.svg   ctrl     ring loop + {% if rotate %}
  ship.svg      ctrl     facet loop + ramp loop
  cell.svg      ctrl     one hexagon + optional window / blind / triangle
  column.svg    ctrl     one matrix column
```

Granularity is one file per coherent visual unit. One-file-per-element was
considered and rejected: it buys nothing once Rust owns the joins, and turns
the ship's ten elements into seven files.

`root.svg` exists solely because `<svg …>` and `</svg>` cannot leave a file
under the constraint. It is parametrized, not morphy.

### Module responsibilities after the change

| Module | Keeps | Gains |
|---|---|---|
| `svg.rs` | scene wiring, the aria label | `Root`, `Defs` template structs; buffer assembly |
| `icon.rs` | ring table, facet coordinates, gradient stops | `Hexatri`, `Ship` structs |
| `trihex.rs` | lattice walk, roles, `space_cells`, blind phases | `Cell` struct; Rust loop calling `render_into` |
| `matrix.rs` | column selection, glyph draw, timing | `Column` struct; Rust loop calling `render_into` |
| `style.rs` | unchanged — `css()` emits CSS, not markup | — |
| `geom.rs`, `rng.rs`, `params.rs` | unchanged | — |

## Configuration

`askama.toml` at the crate root:

```toml
[general]
dirs = ["templates"]
whitespace = "suppress"

[[escaper]]
path = "askama::filters::Text"
extensions = ["svg"]
```

### The escaper is not optional

`svg` is **not** in askama's default escaper table. The built-in mapping covers
`html`/`htm`/`xml`/`j2`/`jinja`/`jinja2` (HTML escaping) and
`md`/`yml`/`none`/`txt`/`""` (no escaping). `svg` matches neither, so it must
be registered explicitly. `askama::filters::Text` is the no-op escaper.

### Why disabling escaping is safe here — and what keeps it safe

Turning escaping off is normally a defect. It is correct in this crate because
**every value that reaches a template is either computed internally or
validated at the params boundary**:

- colours pass `style::hex_rgba`, which rejects anything but `#rrggbb` /
  `#rrggbbaa`
- the matrix angle, seed and resolution are numbers checked by
  `params::validate()`
- `MATRIX_GLYPHS` deliberately excludes `< > & " '` so glyphs never need
  escaping (`style.rs:48`)
- the aria label is assembled from fixed literals

**This is a standing obligation, not a one-time check.** Any future field that
reaches a template from user JSON without passing `params::validate()` breaks
the assumption. The spec records it so the next person adding a config field
knows the escaper is off and why.

## Byte-exactness: the goal is zero golden regeneration

The 42 goldens are `sha512` of exact bytes. The target is that **not one of
them moves** across the entire refactor.

This is not thrift. If a full rewrite of the rendering layer leaves 42
byte-identical renders, that is the strongest correctness proof this codebase
can produce — far stronger than regenerating and eyeballing a diff. The corpus
stops being a cost and becomes the thing that makes the refactor safe.

What makes it achievable:

- Rust owns the joins, so between-fragment bytes are unchanged by construction.
- `whitespace = "suppress"` trims whitespace around `{% %}` blocks, so a loop
  body can be indented for readability without emitting the indentation.
- The current document shell (`svg.rs:93-101`) already emits **no** newlines —
  it uses `\`-continuations inside one string. `root.svg` written as long lines
  reproduces it exactly.

### Step 1 is a go/no-go gate

Convert `ship.svg` alone. Fourteen of the 42 configs use `Glyph::Ship` (two per
motion × image group, of which there are seven); if their goldens are
byte-identical, the approach is proven and the rest is mechanical.
If they are not, that is discovered on one small file rather than six.

**If step 1 cannot be made byte-exact**, stop and re-decide. The fallback —
regenerate once at the very end as a single reviewable commit, after diffing
through a whitespace-normalizing pass — is acceptable but strictly worse, and
should not be entered by drift.

Never regenerate mid-migration. That blinds the oracle precisely when the risk
is highest.

## Verification

### The invariant suite survives untouched

Every `cargo test` invariant parses the *rendered output string*:
`the_ship_is_a_folded_solid`, `roles_are_exclusive_and_the_centre_is_clear`,
`the_rain_lights_a_column_without_moving_anything`,
`hexatri_spins_only_its_triangles`. None inspects Rust internals. They keep
asserting the real properties through every step of the rewrite with no
modification. This is the primary safety net; the goldens are the second.

One exception: `the_document_shell_matches_the_corpus` compares against
`tests/data/shell.txt`. If the shell moves at all, that fixture moves with it.

### The purity guard

One test asserting no file in `src/` contains the byte pair `"<`. A smoke test,
not a proof — `format!("{x}<polygon")` would slip through — but it catches the
realistic case and makes the constraint mechanically enforced rather than
remembered.

`icon.rs`'s test fixtures currently build fake glyph strings
(`hull_poly_rejects_a_second_silhouette`, `icon.rs:265`). Those move to
`tests/data/` so the guard can stay a dumb grep with no `#[cfg(test)]` carve-out.

### Nix

Askama's proc macro reads `templates/` **at compile time**. The flake's source
filter must include the directory, or the build fails inside the sandbox while
`cargo build` succeeds outside it — the same failure mode as the untracked
`Cargo.lock`. Verify in step 1, not step 5.

## Sequencing

Risk ascending. Each step gated on `cargo test` plus `test/golden.py`.

1. **Wire askama** — dependency, `askama.toml`, escaper, Nix source filter.
   Convert `ship.svg`. **Go/no-go on byte-exactness.**
2. **`hexatri.svg`** — proves `{% if rotate %}` and loop arithmetic
   (`icon.rs:27`'s `dur = 24 - idx * 3` becomes `{{ 24 - 3 * loop.index0 }}`).
3. **`root.svg` + `defs.svg`** — proves the `|safe` body slot and buffer
   assembly. `tests/data/shell.txt` is the oracle.
4. **`column.svg`** — matrix rain.
5. **`cell.svg`** — last and highest risk. `trihex.rs` holds the triangle
   invariant and the `closeopen` coupling, where the window's off-span and the
   blind's shut-span derive from one `blind_phase` constant. CLAUDE.md warns
   that splitting them desyncs a window from its blind; moving the emission
   into a template is exactly the change that could split them. The template
   must receive **one** phase value and derive both spans from it, as today.
6. **Purity guard + docs** — the guard test, fixture move, CLAUDE.md update.

## CLAUDE.md changes required

- **Build pipeline** section: the chain now ends in template rendering.
- **"no external assets"**: still true — askama inlines templates at compile
  time — but the sentence as written reads as though it is not. Reword.
- A new short section on the seam (Rust computes, templates render) and on the
  escaper being off by deliberate choice.

## Out of scope

- Moving geometry arithmetic into templates.
- Converting `style::css()` to a template.
- Replacing the `sha512` goldens with structural assertions. That would remove
  the byte constraint entirely and make the `svg` builder crate viable, but it
  is a different decision and discards a working oracle.
- Making any `.svg` openable in a vector editor. Askama syntax forecloses it,
  and no Rust library offers the natural-template design that would not.

## Open questions

1. **askama version.** Pin whatever `cargo add askama` resolves. Step 1 must
   confirm the pinned version provides: `whitespace` config, `[[escaper]]`,
   `render_into`, `loop.index0`, and static function calls
   (`{{ crate::geom::fmt(x) }}`) — all documented, none verified against a
   pinned version yet.
2. **`fmt()` call style.** Either `{{ crate::geom::fmt(x) }}` or a
   `#[askama::filter_fn]` giving `{{ x|fmt }}`. Decide in step 1; the pipe form
   reads better at the ~200 call sites this will produce.
