# 0007. Rust computes; templates only substitute complete values

| Field    | Value                                  |
|----------|----------------------------------------|
| Date     | 2026-08-21                             |
| Status   | Accepted                               |
| Deciders | theta                                  |
| Branch   | `feature/svg-templates` (merged to `master`) |
| Commit   | `671f5bb`, `e351f04`                   |

## Context

`CLAUDE.md`'s central invariant is that geometry depends **only** on `seed`:
the animation, icon, image and overlay choices must never move a hexagon. The
same seed must give the same layout across all 42 valid configurations.

Introducing a template layer ([[0005]]) creates a new place where a value can be
constructed. If a template can compute, then rendering can vary independently of
the seed, and the determinism guarantee stops being enforceable by inspection.

Askama *can* do arithmetic — `{{ 24 - 3 * loop.index0 }}` is valid, and
`loop.index`/`loop.index0` are available. So this is a constraint we impose, not
one the tool imposes on us.

## Decision

We will hold a one-way seam: **Rust computes every value that reaches a
template, and a template only substitutes complete values into markup.**

Concretely, a template:

- never computes a coordinate and never derives a vertex;
- never introduces a **sign, digit, unit or separator** adjacent to an interpolation;
- never calls the RNG.

Style attribute *values*, element ids, and the CSS rules are built in Rust and
arrive as finished strings. The template writes `style="{{ … }}"`, never
`style="animation-delay:-{{ delay }}s"`.

The rule is enforced mechanically by `tests/purity.rs`, which scans each `src/*.rs`
file up to its first `#[cfg(test)]` line for a quote immediately followed by an
angle bracket.

## Alternatives Considered

### Let templates do arithmetic where it reads better

Askama supports it, and `{{ 24 - 3 * loop.index0 }}` is arguably clearer at the
point of use than a pre-computed field.

Rejected because the boundary must be bright to be checkable. Once *some*
arithmetic is allowed, "does this template compute geometry?" becomes a judgement
call on every review rather than a grep. The `dur = 24 - idx * 3` case is exactly
the sympathetic example that would have opened the door.

### Allow signs and units next to interpolations as a pragmatic exception

`x1="-{{ hw }}"` and `animation-delay:-{{ delay }}s` are terse and obviously
correct at a glance.

Rejected after this exact construct shipped twice and was caught twice. In
`ship.svg` the value `-14` existed nowhere in Rust — it was assembled by text
concatenation at render time. In `root.svg`, `viewBox="0 0 {{ ws }} {{ hs }}"`
had the template supplying the origin *and* the separating space. Both were
fixed (`Exhaust` carries both endpoints; `Root` carries a whole `view_box`
string). Permitting the pattern would have left the rule unstatable.

### Enforce by convention and review only

Rejected: the whole point of the refactor was that the previous arrangement
relied on nobody doing the wrong thing. A grep-able rule with a test is cheap.

## Consequences

### Positive

- Determinism is structurally protected: nothing a template does can vary between
  renders of the same seed, because a template cannot vary anything on its own.
- The rule is checkable by `grep`, not by judgement.
- Two "both or neither" couplings became type-level rather than conventional:
  `RingMotion { cls, style }` and `HexAnim { cls, style }` replaced pairs of
  independent `Option`s whose disagreement would have panicked at render time.

### Negative / Trade-offs

- Templates carry less of the structure they render. `style="{{ r.style }}"` shows
  less than `style="animation-duration:{{ dur }}s;transform-origin:50% {{ oy }}%"`.
- Context structs gain fields that exist purely to pre-format
  (`view_box`, `icon_transform`, `rot`, `so`).
- `tests/purity.rs` is a smoke test, not a proof: `format!("{x}<polygon")` would
  slip past its needle.

### Neutral

- Non-numeric plumbing still sits adjacent to interpolations — `clip-path="url(#{{ v.cid }})"`,
  `id="shp{{ r.ramp }}"`. These are id references, not the coordinate-assembly
  pattern the rule targets, and are deliberately in scope of the *spirit* but not
  the letter.

## Resumption (for Agent)

### Current state

Complete and merged, and enforced by a guard test that has been demonstrated
failing on a deliberate violation.

### Key files / entry points

| File | Role |
|------|------|
| `tests/purity.rs` | the guard: scans non-test code for `"<` |
| `src/icon.rs` | `Exhaust` carries both x endpoints — the canonical worked example |
| `src/svg.rs` | `view_box` / `icon_transform` built in Rust |
| `src/trihex.rs` | `rot` carries the whole `rotate(...)` value |
| `CLAUDE.md` | states the rule for humans |

### Next steps

None.

### How to verify

```bash
nix develop -c cargo test --test purity
# and the same logic by hand:
awk '/#\[cfg\(test\)\]/{exit} /"</{print FILENAME":"NR}' src/*.rs   # must print nothing
```

To confirm the guard is load-bearing rather than decorative, inject
`format!("<polygon/>")` into non-test code and watch it fail, then revert.

### Gotchas

- The guard truncates each file at the **first** `#[cfg(test)]`. A test-only helper
  placed early in a file would blind the rest of that file. All eight files
  currently have exactly one, at the end.
- `read_dir("src")` is non-recursive — correct for the current flat layout, but it
  would silently stop guarding a future `src/` subdirectory.
- Test modules legitimately contain `"<polygon"` and similar. Those are search
  needles parsing rendered output, not authored markup. Do not "fix" them.

### Related

- Commits: `671f5bb`, `e351f04`, `42e11bf`, `a201ffa`
- ADRs: [[0004]] the RNG whose draw order this rule protects ·
  [[0006]] where composition lives · [[0009]] how the rule was proven safe
