//! The WASM build must render the same bytes as the native one. The sweep
//! itself is `tests/wasm.mjs` and has to stay JavaScript -- `wasm-bindgen`'s
//! glue is JS by construction, so the only way to call `render()` the way a
//! browser calls it is to call it from a JS host. This target exists so that
//! `cargo test --workspace` is the one command that runs every check, rather
//! than two of three plus a `node` invocation somebody has to remember.
//!
//! ```sh
//! nix build .#bgsvg-wasm
//! BGSVG_WASM=$PWD/result/nodejs nix develop -c cargo test --workspace
//! ```
use std::process::Command;

/// Skipped, loudly, when `BGSVG_WASM` is unset: the module is built by a
/// separate derivation (`nix build .#bgsvg-wasm`) and is not in this tree, so
/// the alternative to skipping is failing every plain `cargo test` and the
/// `nix build` sandbox with it. The message names the command that turns the
/// skip into a real run -- a silent pass is the failure mode this whole suite
/// exists to avoid.
#[test]
fn the_wasm_build_renders_the_corpus_byte_for_byte() {
    let Ok(pkg) = std::env::var("BGSVG_WASM") else {
        println!(
            "SKIPPED: BGSVG_WASM is unset, so the wasm build was not swept.\n  \
             nix build .#bgsvg-wasm\n  \
             BGSVG_WASM=$PWD/result/nodejs cargo test --workspace"
        );
        return;
    };

    let sweep = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/wasm.mjs");
    let out = Command::new("node")
        .arg(sweep)
        .env("BGSVG_WASM", &pkg)
        .output()
        .unwrap_or_else(|e| panic!("BGSVG_WASM is set but node could not run {sweep}: {e}"));

    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    print!("{}", String::from_utf8_lossy(&out.stdout));
}
