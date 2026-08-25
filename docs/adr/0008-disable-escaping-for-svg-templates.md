# 0008. Register the no-op `Text` escaper for `.svg` templates

| Field    | Value                                  |
|----------|----------------------------------------|
| Date     | 2026-08-21                             |
| Status   | Accepted                               |
| Deciders | theta                                  |
| Branch   | `feature/svg-templates` (merged to `master`) |
| Commit   | `5473be9`                              |

## Context

Askama chooses an escaper by file extension and escapes by default. Its built-in
mapping covers `html`/`htm`/`xml`/`j2`/`jinja`/`jinja2` (HTML escaping) and
`md`/`yml`/`none`/`txt`/`""` (no escaping).

**`svg` is in neither list.** An escaper must therefore be registered explicitly;
there is no sensible default to fall back on.

The choice matters because escaping is not free here. `root.svg` has five slots
that carry *rendered markup* — the defs block, the stylesheet, the lattice, the
rain, the glyph. HTML-escaping those would turn `<polygon` into `&lt;polygon`
and destroy the document.

Disabling escaping is normally a defect, so the decision needs its safety argument
recorded rather than assumed.

## Decision

We will register **`askama::filters::Text`** — the no-op escaper — for extension
`svg` in `askama.toml`:

```toml
[[escaper]]
path = "askama::filters::Text"
extensions = ["svg"]
```

This is safe because **every value that reaches a template is either computed
internally or validated at the `params` boundary**:

- colours pass `style::hex_rgba`, which rejects anything but `#rrggbb` / `#rrggbbaa`;
- the matrix angle, the seed and the resolution are numbers checked by `params::validate()`;
- `MATRIX_GLYPHS` deliberately excludes `< > & " '` so glyphs never need escaping;
- the aria label is assembled from fixed literals.

`Rain.color` is the **only** user-controlled string that reaches a template, and it
is the one `validate()` checks. `src/lib.rs` runs `validate()` before `resolve()`,
and `src/matrix.rs` documents the dependency at the point of use:
`hex_rgba(color).expect("validate() already accepted the colour")`.

The consequence is a property worth stating plainly: **this crate never emits an
escapable character anywhere, by design.**

## Alternatives Considered

### Register the HTML/XML escaper and mark markup slots `|safe`

The conventional arrangement: escape by default, opt out where markup is
intended.

Rejected because it inverts the risk for no gain. Every one of the ~200
interpolations in this crate is either an internally-computed number/colour or a
validated string; none can contain an escapable character. Escaping by default
would mean five `|safe` opt-outs carrying all the risk, rather than one
documented, crate-wide property. It would also silently change output bytes if a
value ever *did* contain such a character, rather than failing loudly.

### Add `|safe` to `root.svg`'s five markup slots anyway, for documentation

Proposed in the final review as a marker of which slots carry markup versus
attribute values.

**Declined.** With the `Text` escaper registered, `|safe` is a no-op. A marker
that looks like an escaping decision while doing nothing is worse than the
documentation it buys — it suggests escaping is active when it is not.

### Rely on askama's default for `.svg`

Not viable: there is no default for `svg`. Omitting the registration is a
compile-time failure, not a silent fallback.

## Consequences

### Positive

- The five markup slots in `root.svg` compose correctly with no per-slot opt-outs.
- No escaping work at render time.
- The safety argument is written down as a standing obligation rather than
  rediscovered.

### Negative / Trade-offs

- **The escaper's correctness is proven only by compile-time resolution, never
  behaviourally.** Because the crate emits no escapable character by design, no
  test can distinguish a correct registration from a subtly wrong one at runtime.
  The one reachable failure mode — registering the *HTML* escaper for `.svg` —
  would escape all five markup slots and break all 42 goldens immediately, so the
  corpus does cover the realistic accident.
- The safety property depends on `params::validate()` continuing to be exhaustive.

### Neutral

- `askama.toml`'s escaper block is mandatory boilerplate, not a tuning knob.

## Resumption (for Agent)

### Current state

Complete and merged. Recorded in `CLAUDE.md` as a standing obligation.

### Key files / entry points

| File | Role |
|------|------|
| `askama.toml` | the `[[escaper]]` registration |
| `src/params.rs` | `validate()` — the boundary the safety argument rests on |
| `src/style.rs` | `hex_rgba` (colour format); `MATRIX_GLYPHS` and its exclusion comment |
| `src/svg.rs` | the aria label, assembled from fixed literals |
| `CLAUDE.md` | states the standing obligation for humans |

### Next steps

None — but see Gotchas, which describes a *permanent* obligation rather than a task.

### How to verify

```bash
grep -A2 '\[\[escaper\]\]' askama.toml   # must name askama::filters::Text for svg
nix develop -c cargo test
nix develop -c cargo run --example golden
```

### Gotchas

- **This is a standing obligation, not a one-time audit.** Anyone adding a config
  field that reaches a template must route it through `params::validate()` first,
  or the `Text` escaper stops being safe. There is no test that will catch this —
  a newly-added unvalidated field carrying `<` would simply emit broken SVG.
- Do not "improve" `MATRIX_GLYPHS` by adding characters. Its exclusion of
  `< > & " '` is load-bearing for this decision, not merely aesthetic.

### Related

- Commits: `5473be9` (registration), `a201ffa` (the obligation in `CLAUDE.md`)
- ADRs: [[0005]] engine choice · [[0006]] why only `root.svg` has markup slots
