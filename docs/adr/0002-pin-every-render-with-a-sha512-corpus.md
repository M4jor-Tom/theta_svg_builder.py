# 0002. Pin every schema-valid render with a self-naming sha512 corpus

| Field    | Value                                  |
|----------|----------------------------------------|
| Date     | 2026-08-19                             |
| Status   | Accepted                               |
| Deciders | theta                                  |
| Commit   | `7b8c8dc`                              |

## Context

After [[0001]] the valid configuration space became exactly enumerable: 42
configs. `--selftest` already asserted that a render was *well-formed* — the
triangle invariant, the clear centre, the blind layering. Nothing asserted that
a render was *unchanged*.

Those are different questions. A refactor can keep every invariant and still
move the picture.

A Rust port was already anticipated, which sharpened the need: without a
byte-level oracle, a reimplementation can only be checked by looking at it.

## Decision

We will keep `test/golden/<sha512 of the SVG>/`, each directory holding a
config beside the SVG it renders.

**One rule covers both files: each is named by the sha512 of its own bytes,
exactly as written.** So `sha512sum` reproduces every name in the corpus
unaided, and the SVG additionally reproduces its own directory name. Nothing
has to trust this program to check its own work.

We will keep the **SVG itself**, not only its hash, so a failure can report the
first differing byte with the text either side.

Both the Rust test sweep and the Python harness enumerate from
`valid_configs()` rather than each carrying its own loop, so a new axis cannot
reach one surface and miss the other.

The corpus fixes `seed` at 0 and carries no `output` key: geometry depends only
on the seed, and the sink picks a destination rather than pixels.

## Alternatives Considered

### Store hashes only

Smaller and sufficient to detect change.

Rejected because a bare hash mismatch says only "something moved". Keeping the
SVG lets a failure point at the first differing byte with its surrounding text.
These are one-line documents — a line diff says nothing useful about them.

### Rely on `--selftest` alone

It already covered every valid config.

Rejected: it proves a render is well-formed, not that it is unchanged. Both are
wanted, and they are separate tests. A change is unfinished until both pass.

### Name directories by config, or by an index

Human-readable names like `closeopen-starfield-ship-matrix/`.

Rejected in favour of content-addressing. A self-naming corpus is verifiable by
an outsider with `sha512sum` and no knowledge of this program's naming scheme;
a descriptive name is an assertion the program makes about itself.

### Let each test surface enumerate its own config list

Rejected: two copies of the sweep drift. Lifting `valid_configs()` out of
`selftest()` and having both surfaces read it is what makes "a new axis cannot
reach one surface and miss the other" true by construction.

### Include multi-resolution directory sinks

Rejected as unnameable in this layout: a directory sink with several
resolutions is one config with several SVGs, and the corpus holds
single-render configs only.

## Consequences

### Positive

- The corpus later became the acceptance test for the entire Rust port ([[0003]])
  and then for the SVG template refactor ([[0009]]) — in both cases it proved
  the change moved nothing, which no other test in this repo can do.
- Verifiable without trusting the program: `sha512sum` reproduces every name.
- A failure names the differing byte rather than the moved hash.

### Negative / Trade-offs

- Every intentional visual change now requires a deliberate `--regen`, and the
  diff must be read before accepting it.
- The corpus is bytes, so it is sensitive to formatting choices that are not
  visual — this later constrained template formatting ([[0010]]).
- 42 SVGs are committed to the repo.

### Neutral

- Fixing `seed` at 0 means the corpus proves reproducibility at one seed, not
  across seeds. The determinism argument for other seeds rests on the code, not
  on the corpus.

## Resumption (for Agent)

### Current state

Complete and in continuous use. The corpus survived both the Rust port and the
SVG template refactor without regeneration.

### Key files / entry points

| File | Role |
|------|------|
| `examples/golden.rs` | the harness; `--regen` rewrites the corpus |
| `test/golden/` | 42 directories, each named by the sha512 of its own SVG |
| `src/params.rs` | `valid_configs` — the single enumeration both sweeps read |
| `tests/configs.rs` | the Rust sweep over the same 42 |

The harness was `test/golden.py` until [[0013]] ported it to Rust; the corpus
crossed that port unmodified, as it had the two before it.

### Next steps

The known gap this ADR recorded — **the harness did not build the binary**, so it
could print `golden ok` against a stale `target/release/bgsvg` — is closed by
[[0013]]: it renders in-process through `render_to_string`, and `cargo run`
always builds first.

### How to verify

```bash
nix develop -c cargo run --example golden

# the corpus is self-verifying; check it without trusting the program:
cd test/golden && sha512sum */*_background.svg | head
```

### Gotchas

- **`--regen` is almost never right.** A moved golden means a regression until
  proven otherwise. Read the diff first.
- The JSON side of each pair is byte-sensitive too: it is
  `json.dumps(sort_keys=True, separators=(",", ":"))`, which `serde_json`'s
  compact output over its `BTreeMap`-backed map matches exactly. That match is
  why the port did not move the JSON hashes.

### Related

- Commits: `7b8c8dc`
- ADRs: [[0001]] the config space this enumerates · [[0003]] the port it proved ·
  [[0009]] the refactor it proved · [[0013]] the harness rewritten in Rust
