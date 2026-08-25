# 0009. Treat zero golden regeneration as the refactor's correctness proof

| Field    | Value                                  |
|----------|----------------------------------------|
| Date     | 2026-08-21                             |
| Status   | Accepted                               |
| Deciders | theta                                  |
| Branch   | `feature/svg-templates` (merged to `master`) |
| Commit   | `5a7e96a..10e6c23`                     |

## Context

`test/golden/` holds 42 directories, each named by the sha512 of the SVG it
contains, covering every valid `background.motion` × `background.image` × `icon`
× `overlay` combination. `test/golden.py` re-renders and compares bytes; a single
moved byte fails it.

Rewriting the entire rendering layer ([[0005]]) is exactly the kind of change most
likely to move a byte by accident. Regenerating the corpus was explicitly offered
as an acceptable cost up front.

The question was whether to spend it.

## Decision

We will **not regenerate the corpus at any point**, and will treat that as the
refactor's primary correctness proof.

The rationale inverts the usual framing: if a full rewrite of the rendering layer
leaves 42 byte-identical renders, that is about as strong a correctness proof as
this codebase can produce — far stronger than regenerating and eyeballing a diff.
The corpus stops being a cost and becomes the thing that makes the refactor safe.

Two supporting rules followed from it:

- **Task 1 was a go/no-go gate.** Convert the smallest glyph first and stop if its
  goldens moved, rather than discovering the approach was unviable on file six.
- **Never regenerate mid-migration.** That would blind the oracle precisely when
  risk is highest. If bytes had to move, it would be once, at the end, as a single
  reviewable commit.

Each conversion used the same TDD spine, which gave a faster signal than the
goldens: rename the existing function to `*_legacy` untouched, assert the new
template renders *exactly* what it returns, then delete the legacy function once
green. That assertion fails on the differing byte, in one small string, naming the
config.

## Alternatives Considered

### Regenerate once at the end

Convert everything, then regenerate and review the diff.

Rejected because it discards the only oracle that would catch a mistake during
exactly the change most likely to make one. Reviewing a diff of 42 SVGs, each
thousands of polygons, is not a real check.

### Regenerate per task

Simplest to operate: convert, regen, move on.

Rejected outright — it makes every subsequent task's "goldens pass" vacuous.

### Replace sha512 goldens with structural assertions

Parse the SVG and compare a normalised tree, removing the byte constraint
entirely. This would make the `svg` builder crate viable ([[0005]]).

Rejected as out of scope: a larger change than the one being made, and it
discards a working oracle. Noted as the real fork in the road if byte-pinning
ever becomes intolerable.

## Consequences

### Positive

- All 42 goldens are byte-identical across a complete rewrite of the rendering
  layer, verified after every one of the six tasks.
- The existing invariant suite survived **untouched** — `the_ship_is_a_folded_solid`,
  `roles_are_exclusive_and_the_centre_is_clear`,
  `the_rain_lights_a_column_without_moving_anything`,
  `hexatri_spins_only_its_triangles`, `the_document_shell_matches_the_corpus`.
  No test was added, removed or weakened to make the refactor pass.
- Byte-exactness forced good discipline elsewhere: it is what surfaced the
  template-side value assembly that [[0007]] now forbids.

### Negative / Trade-offs

- Template formatting was constrained by output bytes throughout, which is what
  produced the one-line templates later corrected in [[0010]].
- Some structures exist to preserve bytes rather than because they read best
  (`so: so.to_string()` allocating per hexagon for one of two constants).

### Neutral

- The corpus remains sha512-of-bytes, so this constraint continues to apply to
  every future change.

## Resumption (for Agent)

### Current state

Complete. The corpus was never regenerated; `git diff --stat 5a7e96a..10e6c23 -- test/`
is empty.

### Key files / entry points

| File | Role |
|------|------|
| `examples/golden.rs` | the harness; `--regen` rewrites the corpus |
| `test/golden/` | 42 directories, each named by the sha512 of its own SVG |
| `tests/configs.rs` | enumerates the same 42 configs from `params::valid_configs` |

The harness was `test/golden.py` when this refactor was proved; [[0013]] later
ported it to Rust, again without touching a golden.

### Next steps

The follow-up this ADR recommended — the harness rendered through whatever binary
sat in `target/release/`, so a stale one could make the check a silent no-op — is
done in [[0013]]. The harness now renders in-process and `cargo run` builds first.

### How to verify

```bash
nix develop -c cargo run --example golden
```

To confirm the harness is live rather than vacuous, perturb one byte in a
template, rebuild, and watch it fail naming the differing bytes; then revert.
This was done and it behaves correctly.

### Gotchas

- **`--regen` is almost never the right answer.** If the goldens move, the default
  assumption is a regression, not an intended visual change. Read the diff first.
- The stale-binary trap this section used to warn about is gone since [[0013]]:
  `cargo run --example golden` rebuilds whatever it renders with.

### Related

- Commits: `5a7e96a..10e6c23` (the whole refactor, corpus untouched throughout)
- ADRs: [[0002]] the corpus this relies on, and why it is self-naming ·
  [[0005]] engine choice · [[0007]] the rule byte-exactness helped enforce ·
  [[0010]] the formatting cost it imposed · [[0013]] the harness rewritten in Rust
