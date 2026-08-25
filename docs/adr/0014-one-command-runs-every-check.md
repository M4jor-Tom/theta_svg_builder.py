# 0014. Make `cargo test` run every check, with all fixtures under `tests/`

| Field    | Value                          |
|----------|--------------------------------|
| Date     | 2026-08-25                     |
| Status   | Accepted; supersedes the packaging half of [[0013]] |
| Deciders | theta                          |
| Branch   | `master`                       |
| Commit   | pending                        |

## Context

[[0013]] removed the last Python but left the checks spread across three
runners and two directories: `cargo test --workspace`, then
`cargo run --example golden`, then `node test/wasm.mjs`. `CLAUDE.md` opened by
saying "three tests, and a change is unfinished until all three pass" — which
is a sentence that exists because the tooling could not say it for you.

Three runners means three chances to forget one, and the two that lived outside
`cargo test` were exactly the two that catch what the invariant suite cannot:
that the picture did not move, and that the browser build agrees byte for byte.
A check you have to remember is weaker than a check that runs.

The layout said the same thing twice: `test/` held the corpus and the wasm
sweep, `tests/` held the cargo integration tests. Two directories one letter
apart, each holding tests.

## Decision

We will make **`cargo test --workspace` run every check**, and move everything
under **`tests/`**. `test/` is deleted; `examples/` with it.

- `examples/golden.rs` becomes `tests/golden.rs`, a `#[test]`.
- `test/golden/` becomes `tests/golden/` — 42 directories, byte for byte, and
  every file still hashes to its own name.
- `test/wasm.mjs` becomes `tests/wasm.mjs`, run by a new `tests/wasm.rs`.

**Regeneration becomes an `#[ignore]`d test**, `regenerate_the_corpus`, named
in full to run it:

```sh
cargo test --test golden -- --ignored regenerate_the_corpus
```

[[0009]] is why that is the right shape rather than a flag or an environment
variable: a moved golden is a regression until proven otherwise, so rewriting
the corpus should be something you type on purpose and cannot do by accident.
`#[ignore]` also puts the reason in `cargo test`'s own output. Verification and
regeneration share a `Mutex`, because `cargo test -- --include-ignored` would
otherwise run them concurrently and one would delete the corpus the other is
reading.

**The wasm test skips, loudly, when `BGSVG_WASM` is unset.** The module is a
separate derivation and is not in this tree, so the alternative is failing every
plain `cargo test` and the `nix build` sandbox with it. The skip prints the two
commands that turn it into a real run. This is the one place where a green run
does not mean every check passed, and it is stated wherever the command is
documented.

**The corpus now enters the flake's source fileset**, because `nix build` runs
`cargo test` in a sandbox and the check reads those bytes. Editing a golden
therefore changes the package's store path. [[0013]] avoided exactly this cost;
we pay it instead — the store path tracking an input the check depends on is
defensible, and a rebuild when a golden changes is rare and cheap next to a
regression check nobody runs.

The sweep itself stays JavaScript. `wasm-bindgen`'s glue is JavaScript by
construction — see [[0013]]'s alternatives, which this does not revisit.

## Alternatives Considered

### Leave the golden harness an example and keep three runners

What [[0013]] decided, one day old. It keeps the corpus out of the flake fileset
and `--regen` a real flag.

Rejected because the property it protects is smaller than the property it costs.
A store path that does not change when a golden changes is tidiness; a
regression check that runs on every `cargo test` and inside `nix build` is
correctness. The flag is recovered as an `#[ignore]`d test name, which is
arguably more deliberate than `-- --regen`.

### Keep `--regen` by keeping an example alongside the test

Both surfaces, one shared module.

Rejected as two entry points to one behaviour, and a `tests/common/` module to
hold the shared half. `#[ignore]` already provides a deliberate, discoverable
second entry point at zero structural cost.

### Regeneration behind an environment variable

`BGSVG_REGEN=1 cargo test --test golden`.

Rejected: an environment variable can be left exported in a shell and silently
rewrite the corpus on an unrelated run. That is the accident [[0009]] exists to
prevent. A test name has to be typed each time.

### Make the wasm test fail rather than skip when `BGSVG_WASM` is unset

Honest about what did not run, and no silent pass.

Rejected because it makes `cargo test` fail by default and breaks `nix build`,
whose sandbox has neither the module nor Node. A check that cannot pass without
manual setup gets disabled, and then nothing runs. The skip is loud and names
its own fix.

### Move `tests/` to `test/` instead

The other way to end the two-directory split.

Rejected: `tests/` is where Cargo looks for integration tests. Renaming it means
fighting the tool for a directory name.

## Consequences

### Positive

- One command is the whole check: `cargo test --workspace`. `nix build` runs it
  too, so the corpus is verified in the sandbox.
- One directory holds every test and every fixture.
- The golden check can no longer be forgotten, which was its main failure mode.
- Regeneration is harder to do by accident than it has ever been.

### Negative / Trade-offs

- Editing a golden changes the package's store path and triggers a rebuild.
- `cargo test` is slower: the golden sweep renders 42 SVGs at 1080p, about 5–6 s
  in a debug build.
- The wasm test passes without checking anything when `BGSVG_WASM` is unset. It
  says so, but a green `cargo test` alone does not prove the wasm build agrees.
- The 4.2 MB corpus is copied into the Nix store on every build.

### Neutral

- `tests/wasm.mjs` is a fixture as far as Cargo is concerned; only `tests/*.rs`
  become test targets, so it sits beside `tests/golden/` and `tests/data/`
  without becoming one.

## Resumption (for Agent)

### Current state

Complete. `cargo test --workspace` runs 6 targets; with `BGSVG_WASM` set it
reports 42 byte-identical goldens and 42 byte-identical wasm renders. Every one
of the 84 corpus files still hashes to its own name after the move, checked with
`sha512sum` rather than with this program.

### Key files / entry points

| File | Role |
|------|------|
| `tests/golden.rs` | `the_corpus_is_unchanged`, and `regenerate_the_corpus` behind `#[ignore]` |
| `tests/wasm.rs` | runs `tests/wasm.mjs` under Node; skips when `BGSVG_WASM` is unset |
| `tests/wasm.mjs` | the sweep itself — the only non-Rust code in the tree |
| `tests/golden/` | 42 directories, unchanged since [[0002]] created them |
| `flake.nix` | `./tests` carries the corpus into the sandbox; `./examples` is gone |

### Next steps

None.

### How to verify

```bash
nix build .#bgsvg-wasm
BGSVG_WASM=$PWD/result/nodejs nix develop -c cargo test --workspace
nix build          # runs cargo test in the sandbox; the wasm test skips there

# the corpus is self-verifying — check the move without trusting this program:
cd tests/golden && sha512sum */*_background.svg | head

# the golden check must be live, not vacuous:
printf x >> tests/golden/<D>/<D>_background.svg
nix develop -c cargo test --test golden   # 2 problems, naming the byte
git checkout -- tests/golden
```

### Gotchas

- **A green `cargo test` without `BGSVG_WASM` has not checked the wasm build.**
  The test prints `SKIPPED`, which `cargo test` hides unless it fails or you
  pass `--nocapture`. Set the variable before believing a clean run.
- `cargo test -- --include-ignored` will rewrite the corpus. The `Mutex` in
  `tests/golden.rs` keeps it from racing the verify test, but it does not stop
  it happening — nothing can, short of not typing it.
- Moving the corpus back out of `tests/` would take the golden check out of
  `nix build` again. That is the decision this ADR reverses; reversing it back
  needs a new ADR, not an edit.

### Related

- ADRs: [[0013]] the port whose packaging half this supersedes · [[0009]] why
  regeneration is deliberate · [[0002]] the corpus and its self-naming rule ·
  [[0011]] the wasm target the sweep protects
