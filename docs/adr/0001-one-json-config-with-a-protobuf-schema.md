# 0001. Replace the flag surface with one JSON config and a protobuf schema

| Field    | Value                                  |
|----------|----------------------------------------|
| Date     | 2026-08-19                             |
| Status   | Accepted                               |
| Deciders | theta                                  |
| Commit   | `b4fddc7` (design), `b9344df` (break)  |

## Context

`background.py` exposed eleven flags across five visual axes plus output
plumbing. Three cross-flag rules were hand-written rejections in `main()`:
`--fg rotate` needed `--icon hexatri`, `--bg closeopen` needed
`--bg-image space`, and `--matrix-angle` / `--matrix-color` needed
`--overlay matrix`.

Every new axis added another flag and another hand-written rule. Worse, `--fg`
was modelled as a *global foreground axis* when it is really one glyph's own
animation — the ship has no use for it, and neither may the next icon.

There was also a live bug the flag surface made possible: `-o wall.svg -r 4k,mobile`
fell past the single-file branch and silently created a *directory* named
`wall.svg`.

## Decision

We will describe a render with a single `parameters.json`, with
`parameters.proto` as its schema, parsed through the generated protobuf message.

The governing rule: **conditional flags become structural wherever the model
allows it.** A rule the schema can express is one nobody has to check, because
the invalid configuration cannot be written down. Two of the three qualify —
`rotate` exists only inside `Hexatri`, `angle`/`color` only inside `Matrix`.
The third (`CLOSEOPEN` needs an image) crosses two orthogonal fields and stays
an explicit check.

Supporting rules:

- **Every proto zero equals the old CLI default**, so an empty `{}` renders what
  bare `bgsvg` rendered. Hence `Hexatri.ROTATE = 0` rather than a `bool rotate`
  that would default false and quietly flip hexatri to static.
- **`none` stops being a value where absence says it better** — `overlay: none`
  is an unset `oneof`. `Image.NONE` stays a named value, because an image is a
  property of the background rather than a layer that is present or absent.
- **Cardinality attaches to the output sink**, which makes the `wall.svg`
  directory bug unrepresentable.
- Unknown keys are rejected (`ignore_unknown_fields=False`), so a typo'd key
  fails loudly instead of silently rendering the wrong wallpaper.

## Alternatives Considered

### Keep the flags and add rules as axes grow

The status quo. Rejected: each new axis costs a flag *and* a hand-written
cross-flag rejection, and the rules live far from the thing they constrain.

### Make the background image the outer discriminator

Nesting motion inside image would have bought the third structural rule —
`CLOSEOPEN` could only exist where windows do.

Rejected because it distorts the model to win a check. Motion and image are
independent, exactly as `pat_trihex` already takes them; motion is motion and
image is image. The `CLOSEOPEN`-needs-`STARFIELD` rule stays a runtime check
rather than warping the schema around it.

### `protovalidate` for the remaining rules

Would express the four residual checks (CLOSEOPEN needs an image, angle range,
colour format, resolution format) declaratively.

Rejected: a second dependency for four checks.

### A flag shim / deprecation period

Rejected. The request was to stop using options; a compatibility layer would
preserve exactly the surface being deleted. This is a personal repo on
`master`, so a hard break is cheap.

## Consequences

### Positive

- Two of three conditional rules are now unrepresentable rather than rejected.
- A future icon declares its own motion vocabulary; nothing assumes the next
  glyph rotates.
- The valid config space became enumerable and *exact*: 7 backgrounds × 3 glyphs
  × 2 overlays = **42 valid configs**, against the previous 64 that included
  three impossible ones. That enumeration is the basis of [[0002]].
- A silent output bug became unrepresentable.

### Negative / Trade-offs

- **`protobuf` became the project's first runtime dependency**, ending the repo's
  stdlib-only and single-file properties. Accepted deliberately.
- One new failure mode was introduced — a typo'd JSON key — and closed by
  default with `ignore_unknown_fields=False`.
- A generated `parameters_pb2.py` had to be committed to keep the
  `python3 background.py` path working. (Later removed by [[0003]].)

### Neutral

- Seeds became non-negative (`uint32`); nothing depended on that.

## Resumption (for Agent)

### Current state

Complete. `parameters.proto` survived the Rust port unchanged and is still the
schema — see [[0003]].

### Key files / entry points

| File | Role |
|------|------|
| `parameters.proto` | the schema; still authoritative after the Rust port |
| `src/params.rs` | `parse`, `validate`, `resolve`, `valid_configs` |
| `docs/superpowers/specs/2026-08-19-parameters-json-design.md` | the full design |

### Next steps

None.

### How to verify

```bash
nix develop -c cargo run -- --configs | wc -l   # 42
nix develop -c cargo test --test reject         # the rejection cases
```

### Gotchas

- **Try moving a field before adding a check.** The rule this ADR establishes is
  that conditionals are structural where the model allows and rejected only where
  it does not. Adding a `validate()` rule for something the schema could have made
  unrepresentable is a regression against this decision.
- Two rejection cases in `tests/reject.rs` exist specifically as regression tests
  against someone "simplifying" the schema by flattening a `oneof` back into a
  plain field. They are not redundant with the others.

### Related

- Commits: `b4fddc7`, `ab99d28`, `ef58e01`, `b9344df`, `5de905c`, `e527b84`
- ADRs: [[0002]] the corpus built on this config space · [[0003]] the port that
  kept this schema
