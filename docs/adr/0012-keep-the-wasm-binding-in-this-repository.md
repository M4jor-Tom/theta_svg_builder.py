# 0012. Keep the WASM binding in this repository, as its own crate

| Field    | Value                          |
|----------|--------------------------------|
| Date     | 2026-08-21                     |
| Status   | Accepted                       |
| Deciders | theta                          |
| Branch   | `master`                       |
| Commit   | — (design only; not implemented) |

## Context

[[0011]] decided the renderer would be called from a browser. That needs a
`wasm-bindgen` layer somewhere, and it could live in either of two repositories:
here, or in `svg.studio.ui` where the editor that consumes it lives.

Two facts shaped the answer. `lib.rs` opens by stating the crate runs on six
crates, and that count is a maintained claim rather than trivia. And two checks
this design depends on need inputs from *both* sides of the boundary: the
byte-identity sweep compares a WASM build against a native build, and the
consumer's form is validated against `descriptor.bin`.

## Decision

We will add `bgsvg-wasm` as a **workspace member of this repository**, and
leave `bgsvg` itself with its six dependencies untouched.

`bgsvg-wasm` holds the `wasm-bindgen` surface, `console_error_panic_hook`, and
the mapping from `Error` onto the thrown
`{ kind, message, line?, column? }` object. It holds no rendering logic; it
calls `render_to_string` and `parse_res` like any other consumer.

The API it exposes is specified in
`docs/superpowers/specs/2026-08-21-wasm-target-design.md` and is the entire
boundary. Nothing about the editor — its layout, framework, or appearance — is
decided in this repository.

**`bgsvg --descriptor` is part of the same surface**, deliberately as a CLI
flag rather than a WASM export: its consumer is a build-time check, and it must
be reachable without loading a renderer.

## Alternatives Considered

### The binding lives in `svg.studio.ui`, depending on `bgsvg` by git revision

Keeps this repository free of any web concern — arguably the cleaner separation,
and the reason this was seriously considered.

Rejected on the two checks. The byte-identity sweep needs a WASM build and a
native build of the *same* revision side by side; split across repositories it
either does not run or runs against a stale pin. And a binding to the schema
should version with the schema — `descriptor.bin` and the module must come from
one revision or the drift check proves nothing.

### Feature-gate `wasm-bindgen` inside `bgsvg`

`--features wasm` on the existing crate, no new member. Fewer moving parts.

Rejected. It puts `wasm-bindgen` and its macro machinery in the core crate's
dependency graph whether or not the feature is on, and falsifies the claim
`lib.rs` makes about running on six crates. The separation costs one directory.

### Publish `bgsvg-wasm` to npm and consume it as a normal package

Rejected as scope this does not have. There is one known consumer in one known
repository. Publishing buys a semver contract, bundler-agnostic packaging and a
release process for an audience of one.

## Consequences

### Positive

- `bgsvg` keeps its six dependencies; `wasm-bindgen` never enters the core.
- Both checks that span the boundary run in one place against one revision.
- The API is specified in the repository that must not break it.

### Negative / Trade-offs

- This repository now owns a web-facing artifact and an API contract, having
  previously owned only a CLI. A change to the thrown-error shape is a breaking
  change for a consumer that is not in this tree.
- `Cargo.toml` becomes a workspace, which changes how `nix build` and
  `cargo test` are invoked at the root.

### Neutral

- The consumer pins a revision of this repository and is responsible for
  updating it. Nothing here tracks who has pinned what.

## Resumption (for Agent)

### Current state

Designed, not started. No code exists.

### Key files / entry points

| File | Role |
|------|------|
| `docs/superpowers/specs/2026-08-21-wasm-target-design.md` | the API specification this crate must satisfy |
| `Cargo.toml` | becomes a workspace root |
| `src/params.rs:167` | `PRESETS`, to be lifted to `pub const RESOLUTIONS` |
| `build.rs` | already writes `descriptor.bin` to `OUT_DIR` and discards it |

### Next steps

Implement alongside [[0011]]; they are one piece of work split across two
decisions.

### How to verify

```bash
nix develop -c cargo test --workspace  # the workspace, including bgsvg-wasm
nix build                    # default package still bgsvg
```

`bgsvg`'s dependency list must still read `askama`, `pbjson`, `prost`, `serde`,
`serde_json`, `sha2` — if `wasm-bindgen` appears there, this decision was
implemented wrongly.

### Gotchas

- `bgsvg-wasm` must contain no rendering logic and no SVG markup. `tests/purity.rs`
  scans `src/` in this crate only, so the rule it enforces is not automatically
  enforced in the new member — keep markup out of it by discipline, or extend
  the scan.
- The thrown error's `kind` distinction is load-bearing for any consumer: it is
  what separates a syntax error in typed text from a semantic rule violation.
  Do not flatten it to a bare string.

### Related

- ADRs: [[0011]] the decision this implements · [[0003]] the six-dependency
  claim in `lib.rs` this protects · [[0001]] the schema `--descriptor` exposes
