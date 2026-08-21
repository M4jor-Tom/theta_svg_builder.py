# WASM Target Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `bgsvg` renderer callable from a browser, without moving a single rendered byte.

**Architecture:** The render path is already pure, so this is packaging. A new workspace member `bgsvg-wasm` wraps `bgsvg` with `wasm-bindgen`; `lib.rs` grows `load` and `render_to_string` so the CLI and the WASM build share one pipeline rather than each having their own; `flake.nix` gains a `packages.bgsvg-wasm` output so a consumer can pin this repository as a flake input. A new sweep asserts the WASM build reproduces the corpus byte for byte.

**Tech Stack:** Rust 1.85+ (edition 2024), `wasm-bindgen`, `js-sys`, `console_error_panic_hook`, `wasm-bindgen-cli`, Node (test harness only), Nix flakes.

**Spec:** `docs/superpowers/specs/2026-08-21-wasm-target-design.md`
**ADRs:** `docs/adr/0011-render-in-the-browser-via-wasm.md`, `docs/adr/0012-keep-the-wasm-binding-in-this-repository.md`

## Global Constraints

Every task's requirements implicitly include all of these.

- **No rendered byte may move.** `python3 test/golden.py` must pass unmodified after every task. If a golden moves, that is a regression — `--regen` is never the answer here (ADR 0009).
- **`bgsvg`'s dependency list stays exactly six:** `askama`, `pbjson`, `prost`, `serde`, `serde_json`, `sha2`. `wasm-bindgen` must never appear in `bgsvg`'s `[dependencies]`. `lib.rs`'s module docstring states this count; if it changes, the docstring is wrong (ADR 0012).
- **No SVG markup in Rust sources** outside `#[cfg(test)]` modules — `tests/purity.rs` enforces it for `src/`. Keep markup out of `crates/bgsvg-wasm/src/` by the same rule.
- **`protoc` is required at build time**, supplied by the flake as `PROTOC`.
- **Everything stays on `master` in this repository. Create no branches.**
- **Conventional commits**, each ending with the trailer:
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`
- **Run inside the dev shell:** every `cargo`/`python3`/`node` command below assumes `nix develop -c <cmd>`.

## File Structure

| File | Responsibility |
|---|---|
| `src/params.rs` | *(modify)* `RESOLUTIONS` at module scope; `DESCRIPTOR` bytes |
| `src/lib.rs` | *(modify)* `load` + `render_to_string`; `--descriptor` in `run` |
| `Cargo.toml` | *(modify)* becomes a workspace root |
| `crates/bgsvg-wasm/Cargo.toml` | *(create)* the binding crate's manifest |
| `crates/bgsvg-wasm/src/lib.rs` | *(create)* `render`, `resolve_resolution`, `resolutions`, error mapping |
| `tests/pipeline.rs` | *(create)* `load`/`render_to_string` share the CLI's pipeline |
| `tests/descriptor.rs` | *(create)* `--descriptor` emits the schema |
| `test/wasm.mjs` | *(create)* WASM output == corpus bytes, all 42 configs |
| `flake.nix` | *(modify)* wasm target + `wasm-bindgen-cli` + `nodejs` in the shell; `packages.bgsvg-wasm` |
| `README.md`, `CLAUDE.md` | *(modify)* the new commands |
| `docs/ROADMAP.md`, `docs/adr/0011*`, `docs/adr/0012*` | *(modify)* close-out |

---

### Task 1: Lift the resolution presets to module scope

**Files:**
- Modify: `src/params.rs:166-204` (`parse_res`)

**Interfaces:**
- Consumes: nothing
- Produces: `pub const RESOLUTIONS: [(&str, (u32, u32)); 8]` in `bgsvg::params`

- [ ] **Step 1: Write the failing test**

Append to the `mod tests` block at the bottom of `src/params.rs`:

```rust
    /// The list a consumer offers must be exactly the list `parse_res` accepts,
    /// and exactly the list the error message names. Three copies that can
    /// drift became one; this is what holds them together.
    #[test]
    fn resolutions_is_what_parse_res_accepts() {
        assert_eq!(RESOLUTIONS.len(), 8);
        for (name, wh) in RESOLUTIONS {
            assert_eq!(parse_res(name).unwrap(), wh, "preset {name}");
        }
        let msg = parse_res("nope").unwrap_err().to_string();
        for (name, _) in RESOLUTIONS {
            assert!(msg.contains(name), "error message omits preset {name}: {msg}");
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `nix develop -c cargo test --lib resolutions_is_what_parse_res_accepts`
Expected: FAIL to compile — `cannot find value RESOLUTIONS in this scope`.

- [ ] **Step 3: Move the array out of the function**

In `src/params.rs`, delete the `const PRESETS: [(&str, (u32, u32)); 8] = [...];` line and its array literal from inside `parse_res`, and insert this immediately **above** the `pub fn parse_res` docstring:

```rust
/// The resolution presets, in the order a consumer should offer them.
///
/// At module scope rather than inside `parse_res` because the list itself is
/// now part of the API: a UI builds a dropdown from it, and retyping eight
/// names somewhere else is exactly how they drift apart. `parse_res` reads it
/// too, so the accepted set and the offered set cannot diverge.
pub const RESOLUTIONS: [(&str, (u32, u32)); 8] = [
    ("1080p", (1920, 1080)),
    ("1440p", (2560, 1440)),
    ("4k", (3840, 2160)),
    ("720p", (1280, 720)),
    ("mobile", (1080, 1920)),
    ("square", (1080, 1080)),
    ("tablet", (1536, 2048)),
    ("ultrawide", (3440, 1440)),
];
```

Then replace the two `PRESETS` references in `parse_res`'s body with `RESOLUTIONS`:

```rust
    if let Some((_, wh)) = RESOLUTIONS.iter().find(|(name, _)| *name == s) {
        return Ok(*wh);
    }
    let bad = || {
        let names: Vec<&str> = RESOLUTIONS.iter().map(|(n, _)| *n).collect();
        Error::Invalid(format!(
            "bad resolution '{s}': use WIDTHxHEIGHT or a preset {names:?}"
        ))
    };
```

- [ ] **Step 4: Run the tests**

Run: `nix develop -c cargo test`
Expected: PASS, including the pre-existing `parse_res_takes_presets_and_dimensions`.

- [ ] **Step 5: Verify no rendered byte moved**

Run: `nix develop -c cargo build --release && nix develop -c python3 test/golden.py`
Expected: `golden ok: 42 configs render byte-identical SVGs at 1920x1080`

- [ ] **Step 6: Commit**

```bash
git add src/params.rs
git commit -m "refactor(params): lift the resolution presets to module scope" \
 -m "The list is now part of the API -- a consumer builds a dropdown from it -- so it cannot stay a const inside parse_res's body. parse_res still reads it, which is what keeps the offered set and the accepted set identical." \
 -m "Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: One pipeline for the CLI and any other consumer

**Files:**
- Modify: `src/lib.rs:112-150` (`render`)
- Create: `tests/pipeline.rs`

**Interfaces:**
- Consumes: `bgsvg::params::{parse, validate, resolve}`, `bgsvg::svg::build_svg`
- Produces:
  - `pub fn load(json: &str) -> Result<(params::Parameters, params::Scene), Error>`
  - `pub fn render_to_string(json: &str, w: u32, h: u32) -> Result<String, Error>`

- [ ] **Step 1: Write the failing test**

Create `tests/pipeline.rs`:

```rust
//! `render_to_string` is the CLI's own pipeline with the sink removed. If these
//! ever diverge, a rule added to `validate()` reaches one caller and not the
//! other -- the failure this extraction exists to prevent.
use bgsvg::{Error, load, render_to_string};

#[test]
fn render_to_string_is_the_pipeline_the_cli_runs() {
    let (_, scene) = load("{}").expect("{} is a complete config");
    assert_eq!(
        render_to_string("{}", 640, 360).unwrap(),
        bgsvg::svg::build_svg(640, 360, &scene),
        "render_to_string must produce exactly what the CLI would write"
    );
}

#[test]
fn render_to_string_validates_before_it_renders() {
    // the one rule parameters.proto cannot state
    let e = render_to_string(
        r#"{"background":{"motion":"CLOSEOPEN","image":"NONE"}}"#,
        640,
        360,
    )
    .expect_err("CLOSEOPEN with NONE has nothing to reveal");
    assert!(matches!(e, Error::Invalid(_)), "got {e:?}");

    // a typo'd key is the schema's rejection, not validate()'s
    assert!(matches!(
        render_to_string(r#"{"backgrond":{}}"#, 640, 360).unwrap_err(),
        Error::Schema(_)
    ));
}

#[test]
fn load_returns_the_message_too_so_the_cli_can_pick_a_sink() {
    let (p, scene) = load(r#"{"seed":7,"output":{"stdout":{"resolution":"4k"}}}"#).unwrap();
    assert_eq!(scene.seed, 7);
    assert!(p.output.is_some(), "the sink must survive load()");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `nix develop -c cargo test --test pipeline`
Expected: FAIL to compile — `unresolved import bgsvg::load`.

- [ ] **Step 3: Extract the pipeline**

In `src/lib.rs`, insert these two functions immediately **above** the existing `fn render`:

```rust
/// Parse, validate, flatten — the whole schema boundary in one call.
///
/// Returns the message alongside the `Scene` because the two callers need
/// different halves: the CLI reads `output` to pick a sink, and a renderer
/// needs only the flattened scene. The README already calls this step "load"
/// ("rejected at load"), so it keeps that name.
pub fn load(json: &str) -> Result<(params::Parameters, params::Scene), Error> {
    let p = params::parse(json)?;
    params::validate(&p)?;
    let scene = params::resolve(&p);
    Ok((p, scene))
}

/// One config's text to one SVG document, with no destination involved.
///
/// This is what a consumer without a filesystem calls. It shares `load` with
/// the CLI rather than repeating it, so a rule added to `params::validate`
/// cannot reach one caller and miss the other — the same reason both test
/// surfaces enumerate from `params::valid_configs`.
pub fn render_to_string(json: &str, w: u32, h: u32) -> Result<String, Error> {
    let (_, scene) = load(json)?;
    Ok(svg::build_svg(w, h, &scene))
}
```

Then replace the first four lines of `fn render`'s body so it uses `load`:

```rust
fn render(text: &str) -> Result<(), Error> {
    let (p, scene) = load(text)?;
    let sink = p.output.as_ref().and_then(|o| o.sink.as_ref());
```

Leave the rest of `render` unchanged — it already refers to `p`, `scene` and `sink`.

- [ ] **Step 4: Run the tests**

Run: `nix develop -c cargo test`
Expected: PASS, all suites.

- [ ] **Step 5: Verify no rendered byte moved**

Run: `nix develop -c cargo build --release && nix develop -c python3 test/golden.py`
Expected: `golden ok: 42 configs ...`

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs tests/pipeline.rs
git commit -m "refactor(lib): extract load and render_to_string from render" \
 -m "render() ran parse -> validate -> resolve -> build_svg before choosing a sink. Those four steps are now load() and render_to_string(), so a consumer without a filesystem calls the same pipeline the CLI does instead of assembling its own." \
 -m "Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: `bgsvg --descriptor`

**Files:**
- Modify: `src/params.rs` (add `DESCRIPTOR` near the top, after `pub use generated::*;`)
- Modify: `src/lib.rs` (`run`, the match arms and the usage line)
- Create: `tests/descriptor.rs`
- Modify: `README.md`, `CLAUDE.md`

**Interfaces:**
- Consumes: `OUT_DIR/descriptor.bin`, already written by `build.rs`
- Produces: `pub const DESCRIPTOR: &[u8]` in `bgsvg::params`; the `--descriptor` CLI flag

- [ ] **Step 1: Write the failing test**

Create `tests/descriptor.rs`:

```rust
//! `parameters.proto` is the single source of truth for what a config may
//! contain. `--descriptor` is how a consumer in another language reads it --
//! the UI's form is hand-written, and a CI check diffs it against these bytes.
use bgsvg::params::DESCRIPTOR;

#[test]
fn the_descriptor_carries_the_whole_schema() {
    assert!(!DESCRIPTOR.is_empty(), "build.rs must embed descriptor.bin");

    // Descriptors store names as plain UTF-8, so a substring scan is enough to
    // prove this is our schema and not some other descriptor set.
    let s = String::from_utf8_lossy(DESCRIPTOR);
    for name in [
        "svg_builder", "Parameters", "Output", "Background", "Icon", "Hexatri",
        "Ship", "Overlay", "Matrix", "CLOSEOPEN", "STARFIELD", "ROTATE",
    ] {
        assert!(s.contains(name), "descriptor does not mention {name}");
    }
}

#[test]
fn the_flag_exits_clean() {
    assert_eq!(bgsvg::run(&["--descriptor".to_string()]), 0);
    // a second flag is still a usage error, not a silently ignored argument
    assert_eq!(
        bgsvg::run(&["--descriptor".to_string(), "--configs".to_string()]),
        2
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `nix develop -c cargo test --test descriptor`
Expected: FAIL to compile — `cannot find value DESCRIPTOR`.

- [ ] **Step 3: Embed the descriptor and add the flag**

In `src/params.rs`, immediately after the `pub use generated::*;` line, add:

```rust
/// `parameters.proto` compiled to a `FileDescriptorSet` — the schema itself,
/// machine-readable.
///
/// `build.rs` already produces these bytes for `pbjson-build` and then throws
/// them away; this keeps them. A consumer in another language cannot read the
/// Rust enums, so this is what it checks its own field list against.
pub const DESCRIPTOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/descriptor.bin"));
```

In `src/lib.rs`, add a match arm to `run` immediately **after** the existing `--configs` arm:

```rust
        [a] if a == "--descriptor" => {
            use std::io::Write;
            // binary on stdout: write the bytes, do not print! them
            return match std::io::stdout().write_all(params::DESCRIPTOR) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("--descriptor: {e}");
                    2
                }
            };
        }
```

and update the usage line in the same function:

```rust
            eprintln!("usage: bgsvg [config.json | --configs | --descriptor]");
```

- [ ] **Step 4: Run the tests**

Run: `nix develop -c cargo test`
Expected: PASS.

- [ ] **Step 5: Verify the flag by hand**

```bash
nix develop -c cargo build --release
./target/release/bgsvg --descriptor | wc -c        # non-zero
./target/release/bgsvg --descriptor | strings | grep -c CLOSEOPEN   # >= 1
nix develop -c python3 test/golden.py              # goldens still pass
```

- [ ] **Step 6: Document the flag**

In `README.md`, under `## Run`, add a line to the code block:

```sh
nix run .#bgsvg -- --descriptor      # parameters.proto as a FileDescriptorSet, for other languages
```

In `CLAUDE.md`, in the sentence listing the CLI surface ("the config path and `--configs`, which dumps `params::valid_configs` for the corpus harness, are the only CLI surface left"), replace that clause with:

```
the config path, `--configs`, which dumps `params::valid_configs` for the
corpus harness, and `--descriptor`, which dumps the compiled schema for
consumers that cannot read the Rust enums, are the only CLI surface left
```

- [ ] **Step 7: Commit**

```bash
git add src/params.rs src/lib.rs tests/descriptor.rs README.md CLAUDE.md
git commit -m "feat(cli): add --descriptor to dump the compiled schema" \
 -m "build.rs already produced descriptor.bin for pbjson-build and discarded it. A consumer in another language cannot read the Rust enums, so this is what it checks its field list against -- the same role --configs plays for the corpus harness." \
 -m "Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: The `bgsvg-wasm` binding crate

**Files:**
- Modify: `Cargo.toml` (add `[workspace]`)
- Create: `crates/bgsvg-wasm/Cargo.toml`
- Create: `crates/bgsvg-wasm/src/lib.rs`

**Interfaces:**
- Consumes: `bgsvg::render_to_string`, `bgsvg::params::{parse_res, RESOLUTIONS}`, `bgsvg::Error`
- Produces, as WASM exports:
  - `render(json: &str, width: u32, height: u32) -> Result<String, JsValue>`
  - `resolve_resolution(spec: &str) -> Result<Box<[u32]>, JsValue>`
  - `resolutions() -> String`
  - thrown error object: `{ kind: "schema" | "invalid", message: string, line?: number, column?: number }`

- [ ] **Step 1: Make the root a workspace**

In `Cargo.toml`, insert above the `[package]` section:

```toml
[workspace]
members = ["crates/bgsvg-wasm"]
```

The root package joins its own workspace automatically, so `bgsvg` needs no entry.

- [ ] **Step 2: Create the crate manifest**

Create `crates/bgsvg-wasm/Cargo.toml`:

```toml
[package]
name = "bgsvg-wasm"
version = "0.1.0"
edition = "2024"
description = "Browser-callable bindings for the bgsvg renderer"

[lib]
# cdylib for the wasm artifact; rlib so `cargo test` can build it for the host
crate-type = ["cdylib", "rlib"]

[dependencies]
bgsvg = { path = "../.." }
wasm-bindgen = "0.2"
js-sys = "0.3"
console_error_panic_hook = "0.1"
serde_json = "1"
```

- [ ] **Step 3: Write the failing test**

Create `crates/bgsvg-wasm/src/lib.rs` containing **only** the test module for now, so the test fails on missing items rather than on a missing file:

```rust
//! Browser-callable bindings for `bgsvg`.

#[cfg(test)]
mod tests {
    use super::*;

    /// The `kind` split is the whole contract with a consumer: a syntax error
    /// belongs beside the text someone typed, a rule violation belongs beside
    /// the field it concerns. Tested on the host, where there is no JsValue.
    #[test]
    fn errors_are_classified_by_which_half_rejected_them() {
        let schema = bgsvg::render_to_string(r#"{"backgrond":{}}"#, 640, 360).unwrap_err();
        let (kind, at) = classify(&schema);
        assert_eq!(kind, "schema");
        assert!(at.is_some(), "a syntax error must carry a position");

        let invalid = bgsvg::render_to_string(
            r#"{"background":{"motion":"CLOSEOPEN","image":"NONE"}}"#,
            640,
            360,
        )
        .unwrap_err();
        assert_eq!(classify(&invalid), ("invalid", None));
    }

    #[test]
    fn resolutions_serialises_every_preset() {
        let v: serde_json::Value = serde_json::from_str(&resolutions()).unwrap();
        let a = v.as_array().expect("an array");
        assert_eq!(a.len(), bgsvg::params::RESOLUTIONS.len());
        assert_eq!(a[0]["name"], "1080p");
        assert_eq!(a[0]["width"], 1920);
        assert_eq!(a[0]["height"], 1080);
    }
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `nix develop -c cargo test -p bgsvg-wasm`
Expected: FAIL to compile — `cannot find function classify`, `cannot find function resolutions`.

- [ ] **Step 5: Write the implementation**

Insert above the `#[cfg(test)]` module in `crates/bgsvg-wasm/src/lib.rs`:

```rust
//! This crate holds no rendering logic and no markup. It calls
//! `bgsvg::render_to_string` and `bgsvg::params::parse_res` like any other
//! consumer; its only real work is turning a `bgsvg::Error` into an object
//! JavaScript can branch on.

use wasm_bindgen::prelude::*;

/// Which half of the boundary rejected the config, and where.
///
/// Kept as a plain function of the error, separate from the JsValue it becomes,
/// so the classification can be tested on the host — `cargo test` has no
/// JavaScript to hand.
fn classify(e: &bgsvg::Error) -> (&'static str, Option<(usize, usize)>) {
    match e {
        // serde_json's position is 1-based and points into the text the user typed
        bgsvg::Error::Schema(e) => ("schema", Some((e.line(), e.column()))),
        bgsvg::Error::Invalid(_) => ("invalid", None),
        // unreachable here: there is no filesystem in a browser to fail
        bgsvg::Error::Io(_) => ("invalid", None),
    }
}

/// `{ kind, message, line?, column? }` — see the API specification in
/// `docs/superpowers/specs/2026-08-21-wasm-target-design.md`.
fn throw(e: bgsvg::Error) -> JsValue {
    let (kind, at) = classify(&e);
    let o = js_sys::Object::new();
    let mut set = |k: &str, v: JsValue| {
        js_sys::Reflect::set(&o, &JsValue::from_str(k), &v)
            .expect("a freshly created object accepts new keys");
    };
    set("kind", JsValue::from_str(kind));
    set("message", JsValue::from_str(&e.to_string()));
    if let Some((line, column)) = at {
        set("line", JsValue::from_f64(line as f64));
        set("column", JsValue::from_f64(column as f64));
    }
    o.into()
}

/// A trap poisons the module for every later call, so make it say why.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// One config, one SVG document. `width` and `height` are pixels, both non-zero.
///
/// A config's `output` field is parsed and validated like any other, then
/// ignored: a sink names a destination and there is none here. So a config
/// written for the CLI renders unaltered rather than being rejected.
#[wasm_bindgen]
pub fn render(json: &str, width: u32, height: u32) -> Result<String, JsValue> {
    bgsvg::render_to_string(json, width, height).map_err(throw)
}

/// A preset name or `WIDTHxHEIGHT` -> `[width, height]`. This is `parse_res`
/// and nothing more, exposed so no consumer reimplements its edge cases.
#[wasm_bindgen]
pub fn resolve_resolution(spec: &str) -> Result<Box<[u32]>, JsValue> {
    let (w, h) = bgsvg::params::parse_res(spec).map_err(throw)?;
    Ok(vec![w, h].into_boxed_slice())
}

/// The preset table as JSON, in declaration order.
#[wasm_bindgen]
pub fn resolutions() -> String {
    let v: Vec<serde_json::Value> = bgsvg::params::RESOLUTIONS
        .iter()
        .map(|(name, (w, h))| serde_json::json!({"name": name, "width": w, "height": h}))
        .collect();
    serde_json::Value::Array(v).to_string()
}
```

- [ ] **Step 6: Run the tests**

Run: `nix develop -c cargo test`
Expected: PASS for the whole workspace, including `bgsvg-wasm`'s two tests.

**If the host build of `bgsvg-wasm` fails** (some `wasm-bindgen` versions do not build cleanly off-wasm), gate only the JS-facing items and keep the tested ones unconditional:

```rust
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
```

is *not* sufficient — instead wrap `throw`, `start`, `render`, `resolve_resolution` in
`#[cfg(target_arch = "wasm32")]` and leave `classify` and `resolutions` unconditional. The two
tests only touch the unconditional pair, so they keep working.

- [ ] **Step 7: Confirm `bgsvg` did not gain a dependency**

```bash
nix develop -c cargo tree -p bgsvg --depth 1
```

Expected: exactly `askama`, `pbjson`, `prost`, `serde`, `serde_json`, `sha2`. If `wasm-bindgen`
appears here, the dependency was added to the wrong manifest.

- [ ] **Step 8: Verify no rendered byte moved**

Run: `nix develop -c cargo build --release && nix develop -c python3 test/golden.py`
Expected: `golden ok: 42 configs ...`

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock crates/
git commit -m "feat(wasm): add the bgsvg-wasm binding crate" \
 -m "A workspace member rather than a feature on bgsvg, so the core keeps the six dependencies lib.rs advertises and wasm-bindgen never enters it." \
 -m "Exposes render, resolve_resolution and resolutions, and maps bgsvg::Error onto { kind, message, line?, column? }. The kind split is what lets a consumer put a syntax error beside typed text and a rule violation beside its field; classify() is a plain function so that mapping is testable without a JavaScript runtime." \
 -m "Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Build and expose the module from the flake

**Files:**
- Modify: `flake.nix`
- Modify: `CLAUDE.md`, `README.md`

**Interfaces:**
- Consumes: the `bgsvg-wasm` crate from Task 4
- Produces: `packages.bgsvg-wasm` with `web/` and `nodejs/` subdirectories; a devShell that can build for wasm and run Node

- [ ] **Step 1: Confirm the wasm target is usable before wiring it**

```bash
nix develop -c rustc --print target-list | grep -x wasm32-unknown-unknown
nix develop -c cargo build --target wasm32-unknown-unknown -p bgsvg-wasm
```

Expected: the target is listed and the build succeeds.

**If the build fails with `can't find crate for 'core'`**, nixpkgs' `rustc` lacks the wasm32
standard library. Add `rust-overlay` as a flake input and replace `pkgs.cargo`/`pkgs.rustc` in the
devShell with a toolchain carrying the target:

```nix
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";
  inputs.rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  # then, in the shell's packages list, instead of pkgs.cargo and pkgs.rustc:
  #   (pkgs.rust-bin.stable.latest.default.override {
  #      targets = [ "wasm32-unknown-unknown" ];
  #    })
```

Record which path was taken in the commit message — the consumer's flake follows this repository's
toolchain choice.

- [ ] **Step 2: Add the package output**

In `flake.nix`, inside `packages = forAll (pkgs: rec { ... })`, add **before** `default = bgsvg;`:

```nix
          bgsvg-wasm = pkgs.rustPlatform.buildRustPackage {
            pname = "bgsvg-wasm";
            version = "0.1.0";
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./build.rs
                ./parameters.proto
                ./askama.toml
                ./src
                ./templates
                ./tests
                ./crates
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.protobuf pkgs.wasm-bindgen-cli ];
            PROTOC = "${pkgs.protobuf}/bin/protoc";
            buildPhase = ''
              cargo build --release --target wasm32-unknown-unknown -p bgsvg-wasm
            '';
            # two targets from one .wasm: `web` is what a bundler consumes,
            # `nodejs` is what test/wasm.mjs runs. The glue differs; the
            # rendered bytes cannot.
            installPhase = ''
              for t in web nodejs; do
                wasm-bindgen target/wasm32-unknown-unknown/release/bgsvg_wasm.wasm \
                  --out-dir $out/$t --target $t
              done
            '';
            # the workspace's tests run natively via `cargo test`, not here
            doCheck = false;
          };
```

- [ ] **Step 3: Extend the dev shell**

In `flake.nix`, in the devShell's `packages` list, add after `pkgs.python3`:

```nix
            pkgs.wasm-bindgen-cli
            # nodejs is here to run test/wasm.mjs and nothing else, the same way
            # python3 is here only for test/golden.py
            pkgs.nodejs
```

- [ ] **Step 4: Verify the package builds and carries what it should**

```bash
nix build .#bgsvg-wasm
ls result/web result/nodejs
test -f result/web/bgsvg_wasm.d.ts && echo "types present"
nix build && ls result/bin/bgsvg          # packages.default is still bgsvg
```

Expected: both directories exist, `web/` contains `bgsvg_wasm.d.ts`, and the default package is
still the CLI.

- [ ] **Step 5: Document the commands**

In `README.md`, under `## Run`:

```sh
nix build .#bgsvg-wasm                # the browser-callable module (web/ and nodejs/)
```

In `CLAUDE.md`, in the `## Commands` block:

```sh
nix build .#bgsvg-wasm                # the browser module; web/ for bundlers, nodejs/ for test/wasm.mjs
```

- [ ] **Step 6: Commit**

```bash
git add flake.nix flake.lock README.md CLAUDE.md
git commit -m "build(nix): build and expose the wasm module" \
 -m "packages.bgsvg-wasm emits both wasm-bindgen targets from one .wasm: web/ for a bundler to consume, nodejs/ for the byte-identity sweep. packages.default stays bgsvg." \
 -m "The devShell gains wasm-bindgen-cli and nodejs -- nodejs purely to run test/wasm.mjs, the same way python3 is there only for test/golden.py." \
 -m "Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: The byte-identity sweep

**Files:**
- Create: `test/wasm.mjs`
- Modify: `README.md`, `CLAUDE.md`

**Interfaces:**
- Consumes: `packages.bgsvg-wasm`'s `nodejs/` output; the existing `test/golden/` corpus
- Produces: a pass/fail sweep; exit 0 on success, 1 on any differing byte

- [ ] **Step 1: Write the sweep**

Create `test/wasm.mjs`:

```js
#!/usr/bin/env node
// The WASM build must render the SAME BYTES as the native one -- not merely the
// same picture. Every render in test/golden/ is pinned to the sha512 of its own
// bytes, so comparing against the corpus proves both at once: the wasm build
// matches native, and it matches what native was pinned to.
//
// Float formatting through geom::fmt is the plausible way two targets diverge
// while both still look correct, which is why this exists at all.
//
//   nix build .#bgsvg-wasm
//   BGSVG_WASM=$PWD/result/nodejs nix develop -c node test/wasm.mjs
import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = dirname(dirname(fileURLToPath(import.meta.url)));
const GOLDEN = join(REPO, "test", "golden");
const SIZE = [1920, 1080]; // what test/golden.py renders at
const JSON_SUFFIX = "_parameters.json";
const SVG_SUFFIX = "_background.svg";
const EXPECTED = 42; // same count tests/configs.rs asserts

const pkg = process.env.BGSVG_WASM;
if (!pkg) {
  console.error("set BGSVG_WASM to a wasm-bindgen --target nodejs output directory");
  console.error("  nix build .#bgsvg-wasm && BGSVG_WASM=$PWD/result/nodejs node test/wasm.mjs");
  process.exit(2);
}
const { render } = await import(join(pkg, "bgsvg_wasm.js"));

const sha = (b) => createHash("sha512").update(b).digest("hex");

// Where two documents first part company. The whole SVG is one line, so a line
// diff says nothing -- point at the byte and quote either side of it.
function firstDiff(want, got) {
  let i = 0;
  while (i < want.length && i < got.length && want[i] === got[i]) i++;
  const lo = Math.max(0, i - 30);
  return (
    `      first differs at byte ${i}, ${want.length} -> ${got.length} bytes\n` +
    `      was ...${want.subarray(lo, i + 40)}...\n` +
    `      now ...${got.subarray(lo, i + 40)}...`
  );
}

const bad = [];
let checked = 0;

for (const dir of readdirSync(GOLDEN).sort()) {
  const names = readdirSync(join(GOLDEN, dir));
  const cfgName = names.find((n) => n.endsWith(JSON_SUFFIX));
  const svgName = names.find((n) => n.endsWith(SVG_SUFFIX));
  if (!cfgName || !svgName) {
    bad.push(`${dir.slice(0, 16)}...: not a golden directory; run test/golden.py first`);
    continue;
  }

  const cfg = readFileSync(join(GOLDEN, dir, cfgName), "utf8");
  const want = readFileSync(join(GOLDEN, dir, svgName));
  checked++;

  let got;
  try {
    got = Buffer.from(render(cfg, ...SIZE), "utf8");
  } catch (e) {
    bad.push(`${cfg.trim()}\n      wasm threw ${e.kind ?? "?"}: ${e.message ?? e}`);
    continue;
  }

  if (!got.equals(want)) {
    bad.push(`${cfg.trim()}\n${firstDiff(want, got)}`);
  } else if (sha(got) !== dir) {
    bad.push(`${cfg.trim()}\n      renders bytes that do not hash to their own directory`);
  }
}

if (checked !== EXPECTED) {
  bad.push(`swept ${checked} configs, expected ${EXPECTED} -- the corpus and the sweep disagree`);
}

if (bad.length) {
  console.error(`wasm: ${bad.length} problem(s) across ${checked} configs`);
  for (const b of bad) console.error(`  ${b}`);
  console.error("\nthe wasm build must match the corpus byte for byte; do NOT regenerate it");
  process.exit(1);
}
console.log(`wasm ok: ${checked} configs render byte-identical to the corpus at ${SIZE.join("x")}`);
```

- [ ] **Step 2: Run it and watch it pass**

```bash
nix build .#bgsvg-wasm
BGSVG_WASM=$PWD/result/nodejs nix develop -c node test/wasm.mjs
```

Expected: `wasm ok: 42 configs render byte-identical to the corpus at 1920x1080`

**If it fails**, the diff names the first differing byte. Do not touch the corpus — the WASM build
is wrong, and `geom::fmt`'s float formatting is the first place to look.

- [ ] **Step 3: Prove the sweep is live rather than vacuous**

Temporarily change a literal in a template (for example a stroke width in `templates/trihex.svg`),
then:

```bash
nix build .#bgsvg-wasm
BGSVG_WASM=$PWD/result/nodejs nix develop -c node test/wasm.mjs
```

Expected: FAIL, naming the differing byte. **Revert the template change** and re-run to confirm it
passes again. This is the same check ADR 0009 describes performing on `golden.py`.

- [ ] **Step 4: Document it**

In `README.md`, under `## Check`, after the `golden.py` block:

```sh
nix build .#bgsvg-wasm
BGSVG_WASM=$PWD/result/nodejs nix develop -c node test/wasm.mjs   # the wasm build renders the same bytes
```

In `CLAUDE.md`'s `## Commands` block, add the same two lines, and extend the "Two tests" sentence
to "Three tests, and a change is unfinished until all three pass."

- [ ] **Step 5: Commit**

```bash
git add test/wasm.mjs README.md CLAUDE.md
git commit -m "test(wasm): assert the wasm build reproduces the corpus byte for byte" \
 -m "cargo test says the right code ran and golden.py says the picture did not change; this says the browser-callable build lands on those same sha512s. A wasm build that renders nearly the same picture would pass every other test here, and float formatting through geom::fmt is the plausible way that happens." \
 -m "Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Close out and push

**Files:**
- Modify: `docs/ROADMAP.md`
- Modify: `docs/adr/0011-render-in-the-browser-via-wasm.md`, `docs/adr/0012-keep-the-wasm-binding-in-this-repository.md`

**Interfaces:**
- Consumes: Tasks 1–6
- Produces: a pushed `master`, which is what `svg.studio.ui`'s flake input pins

- [ ] **Step 1: Run every check, from clean**

```bash
nix develop -c cargo test
nix develop -c cargo build --release && nix develop -c python3 test/golden.py
nix build .#bgsvg-wasm && BGSVG_WASM=$PWD/result/nodejs nix develop -c node test/wasm.mjs
nix develop -c cargo clippy --workspace --all-targets
nix develop -c cargo fmt --check
```

Expected: all pass. Do not proceed past a failure.

- [ ] **Step 2: Move the ROADMAP task to Done**

In `docs/ROADMAP.md`, tick all six sub-items, then cut the whole `- [ ] WASM target …` block out of
`# Planned` and add this single line to the end of the `# Done` list:

```markdown
- [x] WASM target — [spec](superpowers/specs/2026-08-21-wasm-target-design.md) · [plan](superpowers/plans/2026-08-21-wasm-target.md) · ADR [0011](adr/0011-render-in-the-browser-via-wasm.md) [0012](adr/0012-keep-the-wasm-binding-in-this-repository.md)
```

Leave the deferred items as a short note under `# Planned`:

```markdown
- [ ] Field paths on `Error::Invalid` — only one cross-field rule exists today
  (`CLOSEOPEN` with `NONE`) and its message names both fields, so a consumer can
  place it without one. Revisit when a second cross-field rule appears.
```

Then update the closing paragraph, replacing the sentence about 0011 and 0012 being recorded before
implementation with:

```markdown
ADRs are numbered in decision order, so `docs/adr/` reads chronologically:
config schema, then the corpus, then the Rust port, then the template split,
then the WASM target.
```

- [ ] **Step 3: Bring both ADRs up to date**

In **both** `0011` and `0012`, change the header table's `Commit` row from
`— (design only; not implemented)` to the real range, and replace the
**Resumption → Current state** section.

For `0011`:

```markdown
### Current state

Complete. All 42 goldens passed unmodified throughout, and `test/wasm.mjs`
reports the WASM build renders byte-identical documents at 1920×1080.
```

For `0012`:

```markdown
### Current state

Complete. `bgsvg-wasm` is a workspace member; `cargo tree -p bgsvg --depth 1`
still lists exactly askama, pbjson, prost, serde, serde_json and sha2.
```

In `0011`'s **Next steps**, replace "Implement the six items…" with:

```markdown
None here. The consumer's editor is specified in the `svg.studio.ui`
repository and is out of scope for this one.
```

In `0012`'s **Next steps**, replace "Implement alongside [[0011]]…" with `None.`

Record the measured artifact size in `0011`'s **Negative / Trade-offs**, replacing the estimate:

```bash
du -h result/web/bgsvg_wasm_bg.wasm     # put the real number in the ADR
```

- [ ] **Step 4: Commit the close-out**

```bash
git add docs/ROADMAP.md docs/adr/0011-render-in-the-browser-via-wasm.md docs/adr/0012-keep-the-wasm-binding-in-this-repository.md
git commit -m "docs: close out the WASM target" \
 -m "Both ADRs move from designed to complete and record the measured module size; the ROADMAP task moves to Done, leaving only the deferred Error::Invalid field paths under Planned." \
 -m "Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 5: Push**

```bash
git push origin master
```

This is the step that unblocks `svg.studio.ui`: its `flake.nix` pins this repository by remote
revision, so nothing lands there until this is pushed.

- [ ] **Step 6: Hand the revision over**

Report the pushed commit hash. In `svg.studio.ui`, the follow-up is:

```bash
nix flake lock          # first lock, now that packages.bgsvg-wasm exists
nix develop             # bun + bgsvg available
```

and wiring `BGSVG_WASM` in that repository's `flake.nix` to
`svg_builder.packages.${system}.bgsvg-wasm` — the comment there names this as the pending step.

---

## Self-Review

**Spec coverage.** All six required changes map to tasks: `bgsvg-wasm` → Task 4; `render_to_string`
→ Task 2; `--descriptor` → Task 3; `RESOLUTIONS` → Task 1; flake toolchain and
`packages.bgsvg-wasm` → Task 5; byte-identity test → Task 6. The API specification's three exports
and its error shape are all implemented in Task 4. The spec's "to verify" items are covered:
artifact size is measured in Task 7 Step 3, and the panic-safety question is addressed by
`console_error_panic_hook` in Task 4 Step 5.

**Deviation from the spec, deliberate.** The spec describes `packages.bgsvg-wasm` as "the
wasm-bindgen output directory". Task 5 emits two subdirectories, `web/` and `nodejs/`, because the
sweep in Task 6 needs Node glue while a bundler needs web glue. The glue differs; the `.wasm` is one
build, so rendered bytes cannot differ between them. The consumer's spec refers to
`packages.bgsvg-wasm` generally, so it should be updated to say `${bgsvg-wasm}/web` once this lands.

**Type consistency.** `load` returns `(params::Parameters, params::Scene)` in Task 2 and is consumed
with that shape in Task 2's own test and in `render`. `classify` returns
`(&'static str, Option<(usize, usize)>)` in Task 4 and is asserted against that shape in the same
task's tests. `render(json, width, height)` has one signature across Tasks 4 and 6.

**Known risk, with a decision point rather than an assumption.** Task 5 Step 1 verifies that
nixpkgs' `rustc` ships a `wasm32-unknown-unknown` standard library *before* the flake depends on it,
and gives the `rust-overlay` fallback if it does not. Task 4 Step 6 does the same for building
`bgsvg-wasm` on the host. Neither is asserted as fact.
