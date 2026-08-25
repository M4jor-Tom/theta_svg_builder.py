# 0013. Run the golden corpus from Rust, as an example rather than a test

| Field    | Value                          |
|----------|--------------------------------|
| Date     | 2026-08-25                     |
| Status   | Accepted; the example/`#[test]` half superseded by [[0014]] |
| Deciders | theta                          |
| Branch   | `master`                       |
| Commit   | `40ef3a8`                      |

> **Superseded in part.** Dropping Python stands and is not revisited. The
> packaging choice below — an example rather than a `#[test]`, `test/golden/`
> kept out of the flake fileset — was reversed by [[0014]] within the day:
> `cargo test` now runs the corpus and the fixtures live under `tests/`. The
> "`tests/golden.rs`, a real `#[test]`" alternative below is what shipped, and
> the cost it names was accepted rather than avoided. Read this for the port;
> read [[0014]] for where the harness is now.

## Context

[[0003]] ported the renderer to Rust and left exactly one Python file behind:
`test/golden.py`, the harness for the corpus [[0002]] created. Its own
Alternatives section said so plainly — "Python stays in the dev shell only to
run `test/golden.py`" — which made the dev shell carry a whole language runtime
for one 220-line script.

That script also shelled out to `target/release/bgsvg`. Both [[0002]] and
[[0009]] recorded the same unfixed consequence: with no build step of its own,
the harness could print `golden ok` against a binary from an hour ago, and
`CLAUDE.md` documented running it exactly that way. A silent no-op is the worst
failure mode available to a regression test.

## Decision

We will rewrite the harness as **`examples/golden.rs`** and delete
`test/golden.py`. `pkgs.python3` leaves the dev shell.

The corpus does not move and not one golden byte changes — the same bar
[[0003]] and [[0009]] were held to. `test/golden/` keeps its path, so the
`sha512sum` recipe in `README.md`, `test/wasm.mjs` and every ADR that names a
directory stay correct.

Three things change in the port:

- **It renders in-process** through `render_to_string` instead of spawning the
  binary into a scratch directory. `tests/pipeline.rs` already pins that
  function to exactly what the CLI writes, and `cargo run` builds before it
  runs — which closes the stale-binary gap [[0002]] and [[0009]] both left open.
- **It calls `params::valid_configs` directly** rather than parsing
  `bgsvg --configs` stdout. Same bytes, one less hop, and [[0002]]'s "both
  surfaces read one enumeration" is now a function call instead of a pipe.
- **The size comes from `parse_res("")`.** The goldens carry no `output` key
  precisely because they inherit whatever the default sink picks; the Python
  restated `SIZE = (1920, 1080)` next to that fact, and a second copy of a
  constant is a second place for it to drift.

`roxmltree` joins as a **dev-dependency** to replace
`xml.dom.minidom.parseString`. `lib.rs`'s six-dependency claim is about the
shipped binary and is unaffected: nothing in `bgsvg` parses XML — it writes XML
and never reads it back.

**An example, not a `#[test]`.** `--regen` stays a flag, and the corpus stays
out of the flake's source fileset, which `flake.nix` keeps deliberately narrow
so that editing a golden cannot trigger a package rebuild. `cargo test` still
compiles examples, so the harness cannot rot unnoticed.

Node stays for `test/wasm.mjs`. See below.

## Alternatives Considered

### `tests/golden.rs`, a real `#[test]`

The obvious shape, and genuinely stronger in one way: `cargo test --workspace`
and `nix build` would both verify the corpus, so it could not be forgotten.

Rejected on the flake. `nix build` runs the test suite in a sandbox, so
`test/golden/` would have to enter the source fileset — and `flake.nix:15`
excludes it on purpose, because the built binary does not depend on the corpus
and a golden edit must not change the package's store path. The second cost is
smaller but real: a `#[test]` has no argv, so `--regen` would become
`BGSVG_REGEN=1`, an environment variable that is easy to set by accident and
impossible to discover from `--help`.

### `src/bin/golden.rs`

Same ergonomics as an example, but a `[[bin]]` is installed. `nix build` would
put a test harness in `$out/bin` beside `bgsvg`, which is a packaging mistake in
exchange for nothing.

### Keep shelling out to the binary

Faithful to what the Python did, and it exercises the CLI's default sink rather
than the library.

Rejected because it is precisely the property that produced the stale-binary
gap. The sink is not what the corpus pins — `tests/pipeline.rs` and
`tests/reject.rs` cover the CLI surface, and neither can be satisfied by a stale
artifact.

### Remove Node as well, so the toolchain is Rust and nothing else

The obvious extension of "Rust only", and it was considered.

Rejected as a different project. `wasm-bindgen`'s glue is JavaScript by
construction: driving `render(string) -> string` from a Rust host means either
reimplementing that glue against the raw wasm exports under `wasmtime` — hand
marshalling pointers and lengths, refreshed on every `wasm-bindgen` bump — or
building for `wasm32-wasip1`, which is no longer the artifact [[0011]] and
[[0012]] specify for the browser. Deleting Python removed a runtime that ran one
script; deleting Node would mean rewriting what the check actually tests.

## Consequences

### Positive

- The dev shell drops a language runtime. Rust, `protoc`, and Node for the one
  thing that cannot be Rust.
- The stale-binary gap open since [[0002]] is closed, not documented around.
- Failures now carry a Rust backtrace and the same byte-level diff as before.
- One enumeration, one resolution default, no subprocess between the sweep and
  the renderer.

### Negative / Trade-offs

- The harness needs a compile before it runs, so the first invocation after a
  change is slower than starting a Python interpreter.
- `roxmltree` is a new dev-dependency, where the Python got XML parsing from its
  standard library.
- The golden check still has to be run deliberately; `cargo test` compiles it but
  does not run it. That is the price of keeping the corpus out of the fileset,
  and it is the same situation the Python harness was in.

### Neutral

- `test/` now holds the corpus and `test/wasm.mjs` — the assets cargo does not
  own — while `tests/` holds the integration tests. That split already existed.

## Resumption (for Agent)

### Current state

The Python is gone and stays gone. The packaging half is superseded — see
[[0014]] for where the harness lives, how it is run, and how it is regenerated.
`examples/` no longer exists.

### Key files / entry points

| File | Role |
|------|------|
| `tests/golden.rs` | the harness — an example at this ADR's commit, a `#[test]` since [[0014]] |
| `tests/golden/` | 42 directories, unchanged since [[0002]] created them; at `test/golden/` until [[0014]] |
| `src/params.rs` | `valid_configs` and `parse_res` — the two things the harness reads instead of restating |
| `Cargo.toml` | `[dev-dependencies] roxmltree` — the only XML parser in the tree |

### Next steps

None here. The packaging follow-up is [[0014]].

### How to verify

```bash
nix develop -c cargo test --workspace
nix build .#bgsvg-wasm --no-link --print-out-paths   # then, against that path:
BGSVG_WASM=<out-path>/nodejs nix develop -c cargo test --workspace
```

There must be no Python left in the toolchain:

```bash
grep -rn python3 flake.nix CLAUDE.md README.md   # expect nothing
```

`src/rng.rs` and `src/geom.rs` still say "python" in test names. That is
[[0004]] — the RNG *is* CPython's by decision, and those tests pin its
behaviour. They are not a dependency on a Python being installed.

### Gotchas

- **Regenerating is still almost never right.** [[0009]] is the standing rule and
  neither this port nor [[0014]] softened it; only the incantation changed.
- The flake-fileset cost this ADR avoided is real and was simply paid instead —
  [[0014]] argues why. Do not "fix" it by moving the corpus back out.
- `roxmltree` earns its keep at regeneration time, where a malformed template
  would otherwise be pinned into the corpus. On a verify run the bytes already
  have to match, so the parse is close to redundant — do not delete it on that
  reasoning.

### Related

- Commits: `40ef3a8`
- ADRs: [[0014]] supersedes the packaging half · [[0002]] the corpus and the gap
  this closes · [[0003]] the port that left the last Python file · [[0009]] the
  standing rule against regenerating · [[0012]] the six-dependency claim a
  dev-dependency does not touch
