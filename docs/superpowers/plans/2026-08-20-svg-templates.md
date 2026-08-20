# SVG Templates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move every SVG element out of `src/*.rs` and into askama templates under `templates/`, with zero change to the 42 golden renders.

**Architecture:** Rust computes (seeded RNG, lattice, roles, coordinates, animation phases) and produces plain data structs; askama templates receive that data and emit markup. Composition stays in Rust — each morphy module holds one `String` buffer and calls `Template::render_into` on fragments in order, so Rust owns every byte between elements.

**Tech Stack:** Rust 2024, askama (compile-time Jinja templates), existing `prost`/`pbjson` config layer, `test/golden.py` corpus, Nix flake.

**Spec:** `docs/superpowers/specs/2026-08-20-svg-templates-design.md`

## Global Constraints

- **No `<` in `src/*.rs`.** Every SVG element lives in a `.svg` file. Style attribute *values*, element ids, and `style::css()` stay in Rust.
- **Zero golden regeneration.** All 42 files under `test/golden/` must remain byte-identical through every task. If a task cannot achieve this, STOP and report — do not regenerate.
- **Geometry stays in Rust.** No template computes coordinates. Arithmetic on `loop.index0` for animation durations is permitted; deriving vertices is not.
- **`askama.toml`** at crate root: `dirs = ["templates"]`, `whitespace = "suppress"`, and `[[escaper]] path = "askama::filters::Text"` / `extensions = ["svg"]`.
- **Determinism is untouchable.** RNG draw order must not change. Every draw stays in Rust, in its current sequence. A template never calls the RNG.
- Verification after every task: `nix develop -c cargo test` AND `nix develop -c python3 test/golden.py`. Both must pass.
- Commit style: conventional commits, `refactor(scope):` for the conversions.
- Branch: all work on `feature/svg-templates`, never on `master`.

## Two corrections to the spec

Reading `trihex.rs` and `matrix.rs` in full changed two details. The spec's architecture is unaffected; these are refinements to its file table.

1. **`cell.svg` does not exist.** `pat_trihex` builds three separate `Vec`s (`voids`, `fills`, `out`) and concatenates them in that fixed order — a hexagon's border, its triangle, and its starfield window are emitted in three different passes, not one. So `trihex.svg` is **one template containing three sequential `{% for %}` loops**, matching the concatenation order exactly. Likewise `column.svg` is folded into `matrix.svg` as a nested loop.

2. **`root.svg` has five markup slots, not one.** The shell (`svg.rs:93-101`) interleaves its own elements — `<rect fill="url(#bg)">`, the halo `<circle>`, the vignette `<rect>`, the icon `<g transform=…>` — between the parts it composes. Those elements belong in `root.svg`, which means slots for `defs`, `css`, `bg`, `rain` and `glyph`. The spec's "only one interpolation carries markup" was over-simplified. The escaping analysis is unchanged: all five slots need `|safe`, and all five carry internally-generated markup.

## File Structure

```
askama.toml                 create — engine config
templates/root.svg          create — <svg> shell, 5 slots
templates/defs.svg          create — bg/vig/halo/ink defs + starfield nebulae
templates/ship.svg          create — ship glyph
templates/hexatri.svg       create — hexatri glyph
templates/trihex.svg        create — voids loop, tris loop, hexes loop
templates/matrix.svg        create — rain wrapper, column loop, glyph loop
src/icon.rs                 modify — Ship/Hexatri structs, markup deleted
src/svg.rs                  modify — Root struct, buffer assembly
src/trihex.rs               modify — Void/Tri/Hex data structs, markup deleted
src/matrix.rs               modify — Column/Glyph data structs, markup deleted
tests/purity.rs             create — the no-angle-brackets guard
tests/data/two_hulls.txt    create — moved fixture from icon.rs:265
flake.nix                   modify — include templates/ in the source filter
CLAUDE.md                   modify — build pipeline, no-external-assets wording
```

## The refactor's TDD spine

Every conversion task uses the same cycle, and it is what makes "zero golden regeneration" verifiable *before* the goldens run:

1. Rename the existing function to `<name>_legacy`, leaving it byte-for-byte untouched.
2. Write a test asserting the new template renders **exactly** what the legacy function returns. Run it — it fails, because the template does not exist yet.
3. Build the template and its context struct until the equality test passes.
4. Run `cargo test` + `golden.py`.
5. Delete the legacy function and its equality test; the goldens become the standing oracle.
6. Commit.

The equality assertion is a stronger and faster signal than the goldens: it fails on the exact differing byte in one small string rather than after a full render.

---

### Task 1: Wire askama and convert the ship — THE GO/NO-GO GATE

**Files:**
- Create: `askama.toml`, `templates/ship.svg`
- Modify: `Cargo.toml`, `src/icon.rs:80-147`, `flake.nix`
- Test: `src/icon.rs` (tests module)

**Interfaces:**
- Produces: `struct Ship` implementing `askama::Template`, with fields `a: &'static str`, `b: &'static str`, `lit: &'static str`, `cy0: String`, `cy1: String`, `ramps: Vec<(&'static str, &'static str)>`, `facets: Vec<ShipFacet>`, `hull: String`, `core: String`, `exhaust: Vec<(i32, i32, String)>`.
- Produces: `struct ShipFacet { points: String, k: &'static str, v: String }`.
- Produces: `pub fn ico_ship() -> String` — unchanged signature, so `svg.rs:62` needs no edit.

**Why the ship first:** it takes no arguments, owns its own `<defs>`, and 14 of the 42 configs use it (two per motion × image group, of which there are seven). It is the smallest change that exercises the whole toolchain — dependency, config, escaper, Nix sandbox, byte-exactness.

- [ ] **Step 1: Create the branch**

```bash
git checkout -b feature/svg-templates
git branch --show-current   # MUST print feature/svg-templates before continuing
```

- [ ] **Step 2: Add the dependency and config**

```bash
nix develop -c cargo add askama
```

Create `askama.toml` at the crate root:

```toml
[general]
dirs = ["templates"]
whitespace = "suppress"

[[escaper]]
path = "askama::filters::Text"
extensions = ["svg"]
```

The escaper block is mandatory: `svg` is not in askama's default table (which covers `html`/`htm`/`xml`/`j2`/`jinja`/`jinja2` for HTML escaping and `md`/`yml`/`none`/`txt`/`""` for none). `askama::filters::Text` is the no-op escaper. Do not omit it and hope for a sensible default.

- [ ] **Step 3: Rename the legacy function**

In `src/icon.rs`, rename `ico_ship` to `ico_ship_legacy` and mark it `#[cfg(test)]`. Change **nothing** inside it. Add the new public entry point that will call the template:

```rust
pub fn ico_ship() -> String {
    todo!("template")
}
```

- [ ] **Step 4: Write the failing equality test**

Add to `src/icon.rs`'s tests module:

```rust
/// The template must reproduce the legacy glyph byte-for-byte. This is the
/// gate: if it holds, the 14 ship goldens cannot move.
#[test]
fn the_ship_template_matches_the_legacy_glyph() {
    assert_eq!(ico_ship(), ico_ship_legacy());
}
```

- [ ] **Step 5: Run it to verify it fails**

Run: `nix develop -c cargo test the_ship_template_matches_the_legacy_glyph`
Expected: FAIL — panics on `todo!("template")`.

- [ ] **Step 6: Write `templates/ship.svg`**

Every element goes on **one line**. The legacy `format!` strings use `\`-continuation, which preserves the space before the backslash and eats the newline plus following indentation — so the current output contains no newlines at all. A newline in this template is a changed byte.

```jinja
<defs>{% for r in ramps %}<linearGradient id="shp{{ r.0 }}" gradientUnits="userSpaceOnUse" x1="-70" y1="-80" x2="70" y2="60"><stop offset="0%" stop-color="{{ r.1 }}" stop-opacity="0.5"/><stop offset="100%" stop-color="{{ r.1 }}"/></linearGradient>{% endfor %}<linearGradient id="shpe" gradientUnits="userSpaceOnUse" x1="0" y1="{{ cy0 }}" x2="0" y2="{{ cy1 }}"><stop offset="0%" stop-color="{{ lit }}" stop-opacity="0.95"/><stop offset="100%" stop-color="{{ lit }}" stop-opacity="0"/></linearGradient></defs>{% for f in facets %}<polygon points="{{ f.points }}" fill="url(#shp{{ f.k }})" fill-opacity="{{ f.v }}"/>{% endfor %}<line x1="0" y1="{{ cy0 }}" x2="0" y2="{{ cy1 }}" stroke="url(#shpe)" stroke-width="2.2"/><polygon points="{{ hull }}" fill="none" stroke="{{ a }}" stroke-width="3.6"/><polygon points="{{ core }}" fill="{{ a }}" fill-opacity="0.28" stroke="{{ a }}" stroke-width="2"/>{% for e in exhaust %}<line x1="-{{ e.0 }}" y1="{{ e.1 }}" x2="{{ e.0 }}" y2="{{ e.1 }}" stroke="{{ b }}" stroke-width="2.4" stroke-opacity="{{ e.2 }}"/>{% endfor %}
```

The file must end **without** a trailing newline. Check with `tail -c 1 templates/ship.svg | xxd` — the last byte must be `>` (0x3e), not `0a`.

- [ ] **Step 7: Write the context struct and the new `ico_ship`**

The docstring at `icon.rs:47-79` — the argument for why the facets are lit and valued as they are — **stays on `ico_ship`**. CLAUDE.md points readers at it by name. Move the geometry comments (`// ridge feet, 35% along the trailing edges`, the crest and ramp rationale) onto the corresponding `let` bindings.

```rust
pub struct ShipFacet {
    pub points: String,
    pub k: &'static str,
    pub v: String,
}

#[derive(askama::Template)]
#[template(path = "ship.svg")]
struct Ship {
    a: &'static str,
    b: &'static str,
    lit: &'static str,
    cy0: String,
    cy1: String,
    ramps: Vec<(&'static str, &'static str)>,
    facets: Vec<ShipFacet>,
    hull: String,
    core: String,
    exhaust: Vec<(i32, i32, String)>,
}
```

`ico_ship` keeps every existing binding (`n`, `r`, `l`, `t`, `kl`, `kr`, `cy0`, `cy1`) and the same four-facet table, then fills the struct and returns `Ship { … }.render().unwrap()`. `PAL.a` is a `String` behind a `LazyLock`, so the `&'static str` fields take `PAL.a.as_str()` exactly as the legacy code does.

- [ ] **Step 8: Run the equality test**

Run: `nix develop -c cargo test the_ship_template_matches_the_legacy_glyph`
Expected: PASS.

If it fails, the assertion prints both strings — diff them and fix the template's whitespace. Do **not** proceed to Step 9 until it passes.

- [ ] **Step 9: Run the full suite and the goldens**

Run: `nix develop -c cargo test`
Expected: PASS, including `the_ship_is_a_folded_solid` unchanged.

Run: `nix develop -c python3 test/golden.py`
Expected: PASS, all 42 unchanged.

**If the goldens moved, STOP.** Report it and do not regenerate. This is the go/no-go gate the spec defines.

- [ ] **Step 10: Verify the Nix sandbox sees `templates/`**

Run: `nix build`
Expected: PASS.

Askama's proc macro reads `templates/` at compile time. If the flake's source filter excludes it, this fails while `cargo build` succeeds — the same failure mode as the untracked `Cargo.lock`. If it fails, add `templates` and `askama.toml` to the flake's `src` filter and re-run.

- [ ] **Step 11: Delete the legacy function**

Remove `ico_ship_legacy` and `the_ship_template_matches_the_legacy_glyph`. Re-run `nix develop -c cargo test` and `nix develop -c python3 test/golden.py` — both must still pass.

- [ ] **Step 12: Commit**

```bash
git add askama.toml templates/ship.svg Cargo.toml Cargo.lock src/icon.rs flake.nix
git status --short   # verify the index actually holds these before committing
git commit -m "refactor(icon): move the ship glyph into templates/ship.svg"
```

---

### Task 2: Convert the hexatri glyph

**Files:**
- Create: `templates/hexatri.svg`
- Modify: `src/icon.rs:11-45`
- Test: `src/icon.rs` (tests module)

**Interfaces:**
- Consumes: the askama setup from Task 1.
- Produces: `struct Hexatri { rings: Vec<HexRing>, a: &'static str, core: String }`, `struct HexRing { points: String, col: &'static str, swd: String, o: String, cls: Option<&'static str>, dur: usize, oy: String }`.
- Produces: `pub fn ico_hexatri(rotate: bool) -> String` — unchanged signature.

**Why second:** it proves `{% if %}` and loop arithmetic, which Task 1 did not exercise.

- [ ] **Step 1: Rename the legacy function**

Rename `ico_hexatri` to `ico_hexatri_legacy`, mark it `#[cfg(test)]`, change nothing inside. Add `pub fn ico_hexatri(rotate: bool) -> String { todo!("template") }`.

- [ ] **Step 2: Write the failing equality test**

```rust
/// Both variants, because rotate is the whole point of this glyph.
#[test]
fn the_hexatri_template_matches_the_legacy_glyph() {
    assert_eq!(ico_hexatri(true), ico_hexatri_legacy(true));
    assert_eq!(ico_hexatri(false), ico_hexatri_legacy(false));
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `nix develop -c cargo test the_hexatri_template_matches_the_legacy_glyph`
Expected: FAIL — panics on `todo!("template")`.

- [ ] **Step 4: Write `templates/hexatri.svg`**

One line, no trailing newline. The legacy attribute string is built conditionally at `icon.rs:25-31` and inserted directly after `<polygon`, so the leading space belongs inside the `{% if %}`:

```jinja
{% for r in rings %}<polygon{% if let Some(cls) = r.cls %} class="{{ cls }}" style="animation-duration:{{ r.dur }}s;transform-origin:50% {{ r.oy }}%"{% endif %} points="{{ r.points }}" fill="none" stroke="{{ r.col }}" stroke-width="{{ r.swd }}" stroke-opacity="{{ r.o }}"/>{% endfor %}<polygon points="{{ core }}" fill="{{ a }}" fill-opacity="0.28" stroke="{{ a }}" stroke-width="2"/>
```

- [ ] **Step 5: Write the context struct**

The `dur` and `cls` decisions stay in Rust — `cls` is `Some("rspin")` at index 1, `Some("spin")` at index 3, `None` where the legacy `oy` is `None` or `rotate` is false; `dur` is `24 - idx * 3`. Keep the existing `Ring` tuple type and the four-row table verbatim; the loop that walks it now fills `Vec<HexRing>` instead of pushing markup.

`{{ 24 - 3 * loop.index0 }}` in the template is an alternative, but the ring table already carries `idx`, and keeping the arithmetic in Rust matches the Global Constraint. Use the struct field.

- [ ] **Step 6: Run the equality test**

Run: `nix develop -c cargo test the_hexatri_template_matches_the_legacy_glyph`
Expected: PASS.

- [ ] **Step 7: Run the full suite and the goldens**

Run: `nix develop -c cargo test` — `hexatri_spins_only_its_triangles` must pass unchanged.
Run: `nix develop -c python3 test/golden.py` — all 42 unchanged.

- [ ] **Step 8: Delete the legacy function and commit**

```bash
git add templates/hexatri.svg src/icon.rs
git status --short
git commit -m "refactor(icon): move the hexatri glyph into templates/hexatri.svg"
```

At this point `src/icon.rs` must contain no `<`. Verify: `rg '"<' src/icon.rs` returns nothing (the test fixture at the old `icon.rs:265` still holds one — it moves in Task 6).

---

### Task 3: Convert the document shell and defs

**Files:**
- Create: `templates/root.svg`, `templates/defs.svg`
- Modify: `src/svg.rs:14-102`
- Test: `src/svg.rs` (tests module)

**Interfaces:**
- Consumes: `ico_ship()`, `ico_hexatri(rotate)` from Tasks 1-2.
- Produces: `struct Root { ws: String, hs: String, cx: String, cy: String, clear_r: String, k: String, label: String, defs: String, css: String, bg: String, rain: String, glyph: String }`.
- Produces: `struct Defs { w04: String, h: String, bg0: &'static str, bg1: &'static str, starfield: bool }`.
- Produces: `pub fn build_svg(w: u32, h: u32, scene: &Scene) -> String` — unchanged signature.

- [ ] **Step 1: Rename the legacy function**

Rename `build_svg` to `build_svg_legacy`, mark `#[cfg(test)]`, change nothing. Add `pub fn build_svg(w: u32, h: u32, scene: &Scene) -> String { todo!("template") }`.

`trihex.rs` and `matrix.rs` tests call `build_svg` — they will exercise the new path automatically.

- [ ] **Step 2: Write the failing equality test**

```rust
/// Every valid config, not just the default: the shell branches on glyph,
/// image, motion and overlay, and each branch changes the label.
#[test]
fn the_root_template_matches_the_legacy_document() {
    for cfg in crate::params::valid_configs() {
        let scene = resolve(&parse(&cfg).expect("a valid config"));
        assert_eq!(
            build_svg(1920, 1080, &scene),
            build_svg_legacy(1920, 1080, &scene),
            "config: {cfg}"
        );
    }
}
```

If `valid_configs()` returns something other than `Vec<String>` of JSON, adapt the call to its real signature — check `src/params.rs` before writing this.

- [ ] **Step 3: Run it to verify it fails**

Run: `nix develop -c cargo test the_root_template_matches_the_legacy_document`
Expected: FAIL — panics on `todo!("template")`.

- [ ] **Step 4: Write `templates/defs.svg`**

One line, no trailing newline. The nebula pair at `svg.rs:45-52` is the same gradient twice with different id and colour — a two-row loop, not two literals:

```jinja
<linearGradient id="bg" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="{{ w04 }}" y2="{{ h }}"><stop offset="0%" stop-color="{{ bg0 }}"/><stop offset="100%" stop-color="{{ bg1 }}"/></linearGradient><radialGradient id="vig"><stop offset="55%" stop-color="#8fa3b8" stop-opacity="0"/><stop offset="100%" stop-color="#8fa3b8" stop-opacity="0.16"/></radialGradient><radialGradient id="halo"><stop offset="0%" stop-color="{{ bg0 }}" stop-opacity="0.92"/><stop offset="55%" stop-color="{{ bg0 }}" stop-opacity="0.65"/><stop offset="100%" stop-color="{{ bg0 }}" stop-opacity="0"/></radialGradient><filter id="ink" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="2" stdDeviation="3" flood-color="#1e293b" flood-opacity="0.25"/></filter>{% if starfield %}{% for n in [("neba", "#6fb7d1"), ("nebb", "#77c9a6")] %}<radialGradient id="{{ n.0 }}"><stop offset="0%" stop-color="{{ n.1 }}" stop-opacity="0.3"/><stop offset="100%" stop-color="{{ n.1 }}" stop-opacity="0"/></radialGradient>{% endfor %}{% endif %}
```

If askama rejects the inline tuple array in `{% for %}`, move the pair to a `nebulae: Vec<(&'static str, &'static str)>` field on `Defs` populated only when `starfield` — and drop the `{% if %}`, since an empty Vec renders nothing. Prefer that form if there is any doubt; it is plainly valid.

- [ ] **Step 5: Write `templates/root.svg`**

All five slots need `|safe`, and every one carries internally-generated markup. One line, but this template **does** end with a newline — `svg.rs:100` emits `</svg>\n`:

```jinja
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {{ ws }} {{ hs }}" width="{{ ws }}" height="{{ hs }}" preserveAspectRatio="xMidYMid slice" role="img" aria-label="{{ label }}"><title>{{ label }}</title><defs>{{ defs|safe }}</defs>{{ css|safe }}<rect width="{{ ws }}" height="{{ hs }}" fill="url(#bg)"/><g>{{ bg|safe }}</g>{{ rain|safe }}<circle cx="{{ cx }}" cy="{{ cy }}" r="{{ clear_r }}" fill="url(#halo)"/><rect width="{{ ws }}" height="{{ hs }}" fill="url(#vig)"/><g transform="translate({{ cx }},{{ cy }}) scale({{ k }})" filter="url(#ink)">{{ glyph|safe }}</g></svg>
```

Confirm the trailing newline is present: `tail -c 1 templates/root.svg | xxd` must show `0a`.

- [ ] **Step 6: Rewrite `build_svg`**

Keep the RNG seeding first (`svg.rs:15`) — it is what makes layout depend only on `seed`. Keep the label assembly (`svg.rs:72-88`) verbatim in Rust; it is prose, not markup. Replace the `defs` vec with `Defs { … }.render()`, and the final `format!` with `Root { … }.render()`.

- [ ] **Step 7: Run the equality test**

Run: `nix develop -c cargo test the_root_template_matches_the_legacy_document`
Expected: PASS for all 42 configs.

- [ ] **Step 8: Run the full suite and the goldens**

Run: `nix develop -c cargo test` — `the_document_shell_matches_the_corpus` must pass against the unmodified `tests/data/shell.txt`.
Run: `nix develop -c python3 test/golden.py` — all 42 unchanged.

- [ ] **Step 9: Delete the legacy function and commit**

```bash
git add templates/root.svg templates/defs.svg src/svg.rs
git status --short
git commit -m "refactor(svg): move the document shell and defs into templates"
```

---

### Task 4: Convert the matrix rain

**Files:**
- Create: `templates/matrix.svg`
- Modify: `src/matrix.rs:40-101`
- Test: `src/matrix.rs` (tests module)

**Interfaces:**
- Produces: `struct Matrix { fs: String, rgb: String, o: String, t: String, columns: Vec<RainColumn> }`, `struct RainColumn { dur: String, cells: Vec<RainCell> }`, `struct RainCell { x: String, y: String, delay: String, o: String, ch: char }`.
- Produces: `pub fn pat_matrix(w: u32, h: u32, lat: &Lattice, seed: u32, angle: f64, color: &str) -> String` — unchanged signature.

**The determinism trap:** the RNG draws in `matrix.rs:62-93` happen in a precise order — `g.random()` for the frac test, `g.uniform` for duration, `g.randrange` for the head, then one `g.choice` per cell *interleaved with nothing else*. Building `Vec<RainColumn>` must perform those draws in exactly that order. A column that fails the `MATRIX_FRAC` test must still consume exactly the one `g.random()` it consumes today and nothing more.

- [ ] **Step 1: Rename the legacy function**

Rename to `pat_matrix_legacy`, mark `#[cfg(test)]`, change nothing. Add the `todo!("template")` stub.

- [ ] **Step 2: Write the failing equality test**

```rust
/// Two angles and two colours: the band geometry, the cell count and the
/// column keying all follow the angle, so one angle proves little.
#[test]
fn the_matrix_template_matches_the_legacy_rain() {
    let lat = crate::geom::Lattice::new(1920, 1080);
    for angle in [250.0, 90.0] {
        for color in [MATRIX_COLOR, "#395e53"] {
            assert_eq!(
                pat_matrix(1920, 1080, &lat, 3, angle, color),
                pat_matrix_legacy(1920, 1080, &lat, 3, angle, color),
                "angle {angle}, color {color}"
            );
        }
    }
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `nix develop -c cargo test the_matrix_template_matches_the_legacy_rain`
Expected: FAIL — panics on `todo!("template")`.

- [ ] **Step 4: Write `templates/matrix.svg`**

One line, no trailing newline. Nested loops — no markup slot:

```jinja
<g class="matrix" font-family="monospace" font-size="{{ fs }}" text-anchor="middle" fill="{{ rgb }}" style="--o:{{ o }};--t:{{ t }}">{% for col in columns %}<g style="--d:{{ col.dur }}s">{% for c in col.cells %}<text class="rain" x="{{ c.x }}" y="{{ c.y }}" style="animation-delay:-{{ c.delay }}s" fill-opacity="{{ c.o }}">{{ c.ch }}</text>{% endfor %}</g>{% endfor %}</g>
```

- [ ] **Step 5: Build the context, preserving draw order**

The opacity-at-t=0 computation (`matrix.rs:76-84`) and the delay (`matrix.rs:91`) stay in Rust verbatim. The `{{ c.ch }}` interpolation is safe unescaped because `MATRIX_GLYPHS` excludes `< > & " '` by design (`style.rs:48`) — leave that comment in place, it is now load-bearing for the escaper choice too.

- [ ] **Step 6: Run the equality test**

Run: `nix develop -c cargo test the_matrix_template_matches_the_legacy_rain`
Expected: PASS.

- [ ] **Step 7: Run the full suite and the goldens**

Run: `nix develop -c cargo test` — `the_rain_lights_a_column_without_moving_anything` must pass unchanged. That test compares every `<polygon>` overlay-on vs overlay-off; if it fails, a draw moved.
Run: `nix develop -c python3 test/golden.py` — all 42 unchanged.

- [ ] **Step 8: Delete the legacy function and commit**

```bash
git add templates/matrix.svg src/matrix.rs
git status --short
git commit -m "refactor(matrix): move the character rain into templates/matrix.svg"
```

---

### Task 5: Convert the lattice — highest risk

**Files:**
- Create: `templates/trihex.svg`
- Modify: `src/trihex.rs:102-287`
- Test: `src/trihex.rs` (tests module)

**Interfaces:**
- Produces: `struct Trihex { voids: Vec<VoidCell>, tris: Vec<TriFill>, hexes: Vec<HexBorder> }`.
- Produces: `struct VoidCell { cid: String, poly: String, win: Option<String>, void: &'static str, nx: String, ny: String, nrx: String, nry: String, neb: &'static str, ang: String, stars: Vec<Star>, blind: Option<String> }`.
- Produces: `struct Star { x: String, y: String, r: String, o: String, bloom: Option<String> }`.
- Produces: `struct TriFill { points: String, col: String, delay: Option<String> }`.
- Produces: `struct HexBorder { poly: String, so: String, kind: HexKind }` where `HexKind` is an enum mirroring the four `match scene.motion` arms at `trihex.rs:237-268`.
- Produces: `pub fn pat_trihex(w: u32, h: u32, lat: &Lattice, scene: &Scene, rng: &mut PyRandom) -> String` — unchanged signature.

**Two traps, both called out in CLAUDE.md:**

1. **`blind_phase` is one value with two users.** `trihex.rs:209` computes it once; the space cell's `class="win" style="{phase}"` and the blind's `class="blind" style="{phase}"` both receive that same string. The template must take **one** `phase` and interpolate it into both places. Do not give `VoidCell` separate `win_phase` and `blind_phase` fields — that is exactly the split CLAUDE.md warns desyncs a window from its blind.

2. **The eager `out` loop must stay eager and stay in `lat.hexes` order.** `trihex.rs:198-203` explains why: the `lights` non-void branch draws two values from the *global* stream per hexagon, and those draws must land immediately after `assign()`'s. Building `Vec<HexBorder>` must walk `lat.hexes` in order and perform those draws in place. Do not make it lazy, do not reorder, do not build `voids` and `hexes` in separate passes over the lattice.

- [ ] **Step 1: Rename the legacy functions**

Rename `pat_trihex` to `pat_trihex_legacy` and `space_cell` to `space_cell_legacy`, mark both `#[cfg(test)]`, change nothing inside. Add the `pat_trihex` stub with `todo!("template")`.

- [ ] **Step 2: Write the failing equality test**

```rust
/// Every motion, both images, on a fresh RNG each time. The RNG is threaded
/// through, so each call needs its own seeded instance to compare fairly.
#[test]
fn the_trihex_template_matches_the_legacy_lattice() {
    use background::{Image, Motion};
    let lat = Lattice::new(1920, 1080);
    for motion in [Motion::Static, Motion::Scan, Motion::Lights, Motion::Closeopen] {
        for image in [Image::None, Image::Starfield] {
            if motion == Motion::Closeopen && image == Image::None {
                continue; // validate() rejects this pair
            }
            let scene = Scene { seed: 3, motion, image, glyph: Glyph::Ship, overlay: None };
            let mut r1 = PyRandom::new("trihex:3");
            let mut r2 = PyRandom::new("trihex:3");
            assert_eq!(
                pat_trihex(1920, 1080, &lat, &scene, &mut r1),
                pat_trihex_legacy(1920, 1080, &lat, &scene, &mut r2),
                "{motion:?} / {image:?}"
            );
        }
    }
}
```

Check the real variant names in `src/params.rs` before writing this — the enum arms are generated by prost and may be spelled differently.

- [ ] **Step 3: Run it to verify it fails**

Run: `nix develop -c cargo test the_trihex_template_matches_the_legacy_lattice`
Expected: FAIL — panics on `todo!("template")`.

- [ ] **Step 4: Write `templates/trihex.svg`**

One line, no trailing newline. Three sequential loops in the concatenation order of `trihex.rs:286` — voids, then fills, then borders. Note the blind follows its own space cell inside the same iteration, matching `trihex.rs:210-227`:

```jinja
{% for v in voids %}<clipPath id="{{ v.cid }}"><polygon points="{{ v.poly }}"/></clipPath><g{% if let Some(p) = v.win %} class="win" style="{{ p }}"{% endif %} clip-path="url(#{{ v.cid }})"><polygon points="{{ v.poly }}" fill="{{ v.void }}"/><ellipse cx="{{ v.nx }}" cy="{{ v.ny }}" rx="{{ v.nrx }}" ry="{{ v.nry }}" fill="url(#{{ v.neb }})" transform="rotate({{ v.ang }} {{ v.nx }} {{ v.ny }})"/>{% for s in v.stars %}{% if let Some(b) = s.bloom %}<circle cx="{{ s.x }}" cy="{{ s.y }}" r="{{ b }}" fill="{{ star }}" fill-opacity="0.12"/>{% endif %}<circle cx="{{ s.x }}" cy="{{ s.y }}" r="{{ s.r }}" fill="{{ star }}" fill-opacity="{{ s.o }}"/>{% endfor %}</g>{% if let Some(p) = v.blind %}<polygon class="blind" style="{{ p }}" points="{{ v.poly }}" fill="url(#bg)"/>{% endif %}{% endfor %}{% for t in tris %}<polygon{% if let Some(d) = t.delay %} class="wavef" style="animation-delay:{{ d }}s"{% endif %} points="{{ t.points }}" fill="{{ t.col }}" fill-opacity="{{ fill_o }}"/>{% endfor %}{% for hx in hexes %}<polygon{% match hx.kind %}{% when HexKind::Scan with { delay } %} class="scan" style="animation-delay:{{ delay }}s"{% when HexKind::LightBorder with { delay } %} class="lightb" style="animation-delay:-{{ delay }}s"{% when HexKind::Light with { delay, dur } %} class="light" style="animation-delay:-{{ delay }}s;animation-duration:{{ dur }}s"{% when HexKind::Plain %}{% endmatch %} points="{{ hx.poly }}" fill="{% if let HexKind::Light { .. } = hx.kind %}{{ a }}" fill-opacity="0{% else %}none{% endif %}" stroke="{{ ink }}" stroke-opacity="{{ hx.so }}" stroke-width="{{ sw }}"/>{% endfor %}
```

The `fill` attribute differs between arms: the `light` arm emits `fill="{a}" fill-opacity="0"` (`trihex.rs:259-261`), every other arm emits `fill="none"` (`trihex.rs:265`). The inline `{% if %}` written above splits an attribute across a conditional, which is fragile.

**Preferred form:** give `HexBorder` two fields, `fill: String` (either `PAL.a` or `"none"`) and `fill_opacity: Option<&'static str>` (`Some("0")` only on the `light` arm), then write the attributes plainly:

```jinja
fill="{{ hx.fill }}"{% if let Some(o) = hx.fill_opacity %} fill-opacity="{{ o }}"{% endif %}
```

Take this form from the start. The decision stays in Rust either way; this just avoids interpolating a partial attribute.

- [ ] **Step 5: Build the context, preserving draw order**

Walk `lat.hexes` once, exactly as `trihex.rs:204-269` does, appending to `voids` and `hexes` in the same iteration. Keep `space_cells`, `blind_phase`, `assign` and `scan_delay` unchanged — they compute, they do not emit. `space_cell`'s star loop becomes `Vec<Star>`, with the `i < 2` bloom becoming `bloom: Some(fmt(rad * 3.5))` and the `rad *= 1.4` applied after, in that order.

`star`, `fill_o`, `a`, `ink` and `sw` are whole-render constants — put them on `Trihex` as fields, not on every child struct.

- [ ] **Step 6: Run the equality test**

Run: `nix develop -c cargo test the_trihex_template_matches_the_legacy_lattice`
Expected: PASS for all seven valid motion × image pairs.

- [ ] **Step 7: Run the full suite and the goldens**

Run: `nix develop -c cargo test` — `roles_are_exclusive_and_the_centre_is_clear` must pass unchanged.
Run: `nix develop -c python3 test/golden.py` — all 42 unchanged.

- [ ] **Step 8: Delete the legacy functions and commit**

```bash
git add templates/trihex.svg src/trihex.rs
git status --short
git commit -m "refactor(trihex): move the lattice and starfield into templates/trihex.svg"
```

---

### Task 6: The purity guard and the docs

**Files:**
- Create: `tests/purity.rs`, `tests/data/two_hulls.txt`
- Modify: `src/icon.rs` (test fixture), `CLAUDE.md`

- [ ] **Step 1: Move the test fixture out of `icon.rs`**

`hull_poly_rejects_a_second_silhouette` builds a fake two-hull glyph as a raw string literal containing `<polygon`. Write those exact bytes to `tests/data/two_hulls.txt` and replace the literal with `include_str!("../../tests/data/two_hulls.txt")` (adjust the relative path to match `src/icon.rs`'s location). The test's behaviour must not change — it still expects a panic reading "the ship needs exactly one silhouette outline".

- [ ] **Step 2: Write the failing purity test**

```rust
//! The constraint that motivated the whole refactor, enforced rather than
//! remembered: no SVG markup in Rust source.

/// A smoke test, not a proof -- `format!("{x}<polygon")` would slip through.
/// It catches the realistic case: a string literal that opens with a tag.
#[test]
fn no_svg_markup_in_rust_sources() {
    for entry in std::fs::read_dir("src").expect("src/ exists") {
        let path = entry.expect("a readable entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("valid utf-8");
        assert!(
            !src.contains("\"<"),
            "{} contains SVG markup; it belongs in templates/",
            path.display()
        );
    }
}
```

- [ ] **Step 3: Run it**

Run: `nix develop -c cargo test no_svg_markup_in_rust_sources`
Expected: PASS if Tasks 1-5 are complete and Step 1 is done. If it FAILS, it has found real markup left in `src/` — fix that file rather than weakening the test.

- [ ] **Step 4: Update CLAUDE.md**

Three edits:

- **Build pipeline** section: the chain now ends in template rendering. Name `templates/` and the six files.
- **"no external assets"**: still true, because askama inlines templates at compile time — but reword so it does not read as though the crate now loads files at runtime.
- Add a short paragraph on the seam (Rust computes, templates render) and on the escaper being `Text` by deliberate choice, with the standing obligation from the spec: any new config field reaching a template must pass `params::validate()` first.

Do not restate the template contents in CLAUDE.md. Point at `templates/`.

- [ ] **Step 5: Full verification**

```bash
nix develop -c cargo test
nix develop -c python3 test/golden.py
nix build
rg '"<' src/          # must return nothing
```

- [ ] **Step 6: Commit**

```bash
git add tests/purity.rs tests/data/two_hulls.txt src/icon.rs CLAUDE.md
git status --short
git commit -m "test: guard the no-markup-in-rust rule, and document the seam"
```

---

## Self-review notes

**Spec coverage.** Every spec section maps to a task: askama adoption and config → Task 1; the four-tier taxonomy → realised across Tasks 1-5 (the *pure* tier stays empty as the spec predicts); `render_into` composition → Tasks 3-5; escaper and its standing obligation → Task 1 Step 2 and Task 6 Step 4; byte-exactness gate → Task 1 Steps 8-9; the invariant suite surviving untouched → asserted in every task's verification step; purity guard and fixture move → Task 6; Nix source filter → Task 1 Step 10; CLAUDE.md → Task 6.

**Deviation from the spec.** `render_into` is not used in the plan — every template renders via `.render()` into a `String` that Rust then composes. The spec's rationale for `render_into` was avoiding per-element allocation, which only mattered under the rejected one-file-per-element design. With six templates and at most one render per module, `.render()` is simpler and allocates trivially. If profiling later shows it matters, `render_into` is a drop-in change.

**Known weak point.** Task 5 Step 4's `{% match %}` inside an attribute is the least certain construct in this plan, and its `fill` handling needs the Rust-side fallback described in that step. An executor who finds askama rejects the inline form should take the fallback without hesitation rather than fighting the template syntax — the Global Constraint is that markup leaves Rust, not that every branch is expressed in Jinja.

**Open question resolved.** The spec left `fmt()` call style open. It does not arise: every task pre-formats numbers into `String` fields in Rust, so no template calls `fmt`. No `#[askama::filter_fn]` is needed.
