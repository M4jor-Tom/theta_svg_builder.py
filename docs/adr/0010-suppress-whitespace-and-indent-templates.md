# 0010. Use `whitespace = "suppress"` and indent the templates

| Field    | Value                                  |
|----------|----------------------------------------|
| Date     | 2026-08-21                             |
| Status   | Accepted                               |
| Deciders | theta                                  |
| Branch   | `feature/svg-templates` (merged to `master`) |
| Commit   | `10e6c23`                              |

## Context

The rendered SVG contains no newlines: the previous `format!` strings used
backslash line-continuation, which eats the newline and following indentation
while preserving the space before the backslash. Every byte of whitespace in the
output is therefore deliberate, and the golden corpus pins all of them ([[0009]]).

A template file's whitespace *is* output. Indenting a template for readability
would inject that indentation into every rendered SVG — unless the engine is told
to trim it.

This is the decision the original design got wrong in a way that survived all six
implementation tasks: it mandated `whitespace = "suppress"` **so that templates
could be indented**, and simultaneously mandated one-line templates. The two
requirements cancelled. The result was six single-line files — 338 to 1267
characters each, `trihex.svg` being one 1267-character line holding three loops,
two nested loops and six conditionals — which paid `suppress`'s complexity cost
while collecting `preserve`'s readability. Readability was the entire point of the
refactor.

## Decision

We will set `whitespace = "suppress"` in `askama.toml` **and actually use it**:
templates are indented, multi-line files.

Where a literal space must survive next to a tag, we use askama's `+`
whitespace-preservation markers (`{{+ x +}}`, `{% if … +%}`, `{% endif +%}`), and
only there. Each such marker is documented at its site, because it looks like
noise and removing it silently breaks the corpus.

## Alternatives Considered

### `whitespace = "preserve"` with no markers, templates single-line

The state the branch was in before this decision was corrected. Verified in a
throwaway worktree: setting `preserve` and stripping every `+` marker leaves all
42 goldens byte-identical.

Rejected because it delivers nothing. It is a valid configuration, but it makes
the templates unreadable — which is the thing the refactor existed to fix.

### `whitespace = "suppress"` with single-line templates

The accidental status quo. Rejected: strictly the worst option, paying the cost of
one mode and taking the benefit of neither.

### `whitespace = "minimize"`

Collapses runs of whitespace to a single character rather than removing them.

Rejected: this crate's output has *no* whitespace between elements, so collapsing
to one character still moves every byte.

## Consequences

### Positive

- All six templates are now indented and readable — `trihex.svg`'s three loops and
  six conditionals are legible as structure rather than as one long line.
- Verified byte-neutral: reformatting moved no golden.

### Negative / Trade-offs

- Ten `+` markers exist across the templates — five pairs, two in `hexatri.svg` and
  eight in `trihex.svg` — and each is load-bearing. Deleting one produces malformed
  output such as `<gclass="win"` — still well-formed XML, which is why the failure
  is not obvious by eye. Note there are six `{% if let %}` conditional-attribute
  sites but only five marker pairs: one conditional correctly needs none, because it
  abuts `<polygon`/`<circle` rather than a space.
- Two long lines survive and **cannot** be split: `defs.svg` line 2 (751 chars) and
  `trihex.svg` line 2 (351 chars). `suppress` only trims whitespace *adjacent to a
  tag*, so a newline between two directly-abutting static elements lands in literal
  text and is emitted verbatim.

### Neutral

- `root.svg` must end with **two** trailing newlines to emit one, because askama
  unconditionally trims exactly one trailing newline from every render. The other
  five templates end with none.

## Resumption (for Agent)

### Current state

Complete and merged. All six templates multi-line; the two unsplittable lines are
documented in place.

### Key files / entry points

| File | Role |
|------|------|
| `askama.toml` | `whitespace = "suppress"` |
| `templates/root.svg` | ends `\n\n`; the only template that emits a trailing newline |
| `templates/defs.svg` | line 2 unsplittable, carries a `{# … #}` note explaining why |
| `templates/trihex.svg` | line 2 unsplittable; four `+` marker pairs |
| `src/trihex.rs` | `Trihex` docstring explains why the opening `+` is required |
| `src/svg.rs` | `Root` docstring explains the trailing-newline trim |

### Next steps

None.

### How to verify

```bash
nix develop -c cargo test --test golden

# trailing-byte state (xxd is unavailable in this environment; use od)
tail -c 2 templates/root.svg | od -c      # must show \n \n
tail -c 1 templates/defs.svg | od -c      # must NOT be \n
```

### Gotchas

- **`suppress` trims whitespace adjacent to `{{ }}` expressions, not only `{% %}`
  blocks.** This is why `{{+ … +}}` markers were once needed inside `viewBox`. That
  particular case is gone (the whole value is now built in Rust, see [[0007]]) but
  the semantics still apply everywhere else.
- The opening `+` on `{% if … +%}` is **not** redundant. The text following it is
  ` class="win" style="` — not whitespace-only — yet its leading space is still
  trimmed without the marker. A reader who assumes `suppress` only affects
  whitespace-only text will delete it and break the corpus.
- When reformatting, break lines only at tag boundaries that carry **no** `+`
  marker. A marker exists precisely because the whitespace there is load-bearing.

### Related

- Commits: `10e6c23` (the reformat), `f23eebd` (documenting the markers)
- ADRs: [[0005]] engine choice · [[0007]] why `viewBox` no longer needs markers ·
  [[0009]] the byte constraint that made this subtle
