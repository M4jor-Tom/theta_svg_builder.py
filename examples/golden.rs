//! Golden corpus: every schema-valid config, kept beside the SVG it renders.
//!
//! ```text
//! test/golden/<sha512 of the SVG>/<sha512 of the JSON>_parameters.json
//!                                /<sha512 of the SVG>_background.svg
//! ```
//!
//! One rule covers both files: each is named by the sha512 of its own bytes,
//! exactly as written. So `sha512sum` reproduces every name in the corpus, and
//! the SVG additionally reproduces the directory holding it -- nothing has to
//! trust this program to check its own work:
//!
//! ```sh
//! sha512sum test/golden/<D>/*                             # -> <F>, <D>
//! nix run .#bgsvg -- test/golden/<D>/<F>_parameters.json
//! sha512sum out/trihex-*.svg                              # -> D
//! ```
//!
//! `cargo test` asserts the invariants a render must satisfy; this asserts the
//! render did not change at all, and parses each one as XML on the way past.
//! Keeping the SVG rather than only its hash is what lets a failure say *what*
//! moved instead of just *that* something did.
//!
//! ```sh
//! cargo run --example golden             # verify
//! cargo run --example golden -- --regen  # rewrite after an intended change
//! ```
//!
//! An example rather than a `#[test]`, for two reasons. `--regen` stays a flag
//! instead of becoming an environment variable, and the corpus stays out of the
//! flake's source fileset -- editing a golden must not be able to trigger a
//! package rebuild, since the binary does not depend on it. Cargo still
//! compiles this during `cargo test`, so it cannot rot silently.
//!
//! It renders in-process through `render_to_string` rather than spawning the
//! binary: `tests/pipeline.rs` pins that function to exactly what the CLI
//! writes, and going through cargo means the code under test is always freshly
//! built -- a harness that shells out to `target/release/bgsvg` can report
//! success against a binary from an hour ago.
use bgsvg::params::{parse_res, valid_configs};
use bgsvg::render_to_string;
use sha2::{Digest, Sha512};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test/golden");
const JSON_SUFFIX: &str = "_parameters.json";
const SVG_SUFFIX: &str = "_background.svg";

/// What the corpus should hold, keyed by the JSON's hash: `{JSON bytes, SVG bytes}`.
type Corpus = BTreeMap<String, (Vec<u8>, Vec<u8>)>;

/// What it does hold: the same, plus the directory each pair was found in. The
/// SVG is `None` when the directory did not hold exactly one, which `scan` has
/// already reported -- the comparison below then has nothing to compare against.
type Found = BTreeMap<String, (String, Vec<u8>, Option<Vec<u8>>)>;

fn sha(blob: &[u8]) -> String {
    Sha512::digest(blob)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// What the goldens render at. They carry no `output` key, so they inherit
/// whatever the CLI's default sink picks -- restating "1920x1080" here would be
/// a second copy of a constant that already has one home.
fn size() -> (u32, u32) {
    parse_res("").expect("the empty resolution is the 1080p default")
}

/// A config as you would read it -- short enough that every problem below names
/// the config itself instead of the hash it happens to live under.
fn text(blob: &[u8]) -> String {
    String::from_utf8_lossy(blob).trim().to_string()
}

/// The file names in a directory, unsorted. Both levels of the corpus want the
/// same thing -- names as `String`, sorted, so the report reads the same way twice.
fn names_in(dir: std::fs::ReadDir) -> Vec<String> {
    dir.map(|e| {
        e.expect("a readable entry")
            .file_name()
            .to_string_lossy()
            .into_owned()
    })
    .collect()
}

/// `{JSON hash: (JSON bytes, SVG bytes)}` -- the corpus as it should exist.
///
/// The enumeration lives in `params::valid_configs`, which `tests/configs.rs`
/// also reads, so that a new enum value grows this corpus and the cargo tests
/// together -- describing the axes twice is how one surface silently stops
/// covering them.
fn corpus() -> Corpus {
    let (w, h) = size();
    let configs = valid_configs(0);
    let mut out = Corpus::new();
    for cfg in &configs {
        let line = serde_json::to_string(cfg).expect("a config must serialise");
        let svg = render_to_string(&line, w, h).unwrap_or_else(|e| panic!("{line}: {e}"));
        // well-formed, not merely unchanged
        roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{line}: {e}"));
        // what lands on disk: exactly what `bgsvg --configs` prints, plus the
        // trailing newline that keeps it a POSIX text file. The name hashes
        // these bytes and not some other rendering of them.
        let json = format!("{line}\n").into_bytes();
        out.insert(sha(&json), (json, svg.into_bytes()));
    }
    assert_eq!(
        out.len(),
        configs.len(),
        "two configs serialised to the same JSON"
    );
    out
}

/// Where two SVGs first part company. A line diff says nothing here -- the whole
/// document is one line -- so point at the byte and quote around it.
fn first_diff(old: &[u8], new: &[u8]) -> String {
    let i = old
        .iter()
        .zip(new)
        .position(|(a, b)| a != b)
        .unwrap_or(old.len().min(new.len()));
    let window = |b: &[u8]| text(&b[i.saturating_sub(30)..(i + 40).min(b.len())]);
    format!(
        "      first differs at byte {i}, {} -> {} bytes\n      was ...{}...\n      now ...{}...",
        old.len(),
        new.len(),
        window(old),
        window(new),
    )
}

/// Read the corpus off disk. Enforces the shape here so the comparison below
/// only has to care about content: one directory per render, holding one config
/// and the one SVG it renders, every file named by its own hash.
fn scan() -> (Found, Vec<String>) {
    let (mut found, mut bad) = (Found::new(), Vec::new());
    let Ok(dir) = std::fs::read_dir(ROOT) else {
        return (
            found,
            vec!["test/golden does not exist; run --regen".into()],
        );
    };
    let mut entries = names_in(dir);
    entries.sort();

    for entry in entries {
        let d = Path::new(ROOT).join(&entry);
        let tag = format!("{}...", &entry[..entry.len().min(16)]);
        let Ok(names) = std::fs::read_dir(&d) else {
            bad.push(format!("{entry}: stray file in the corpus root"));
            continue;
        };
        let mut names = names_in(names);
        names.sort();

        let (mut jsons, mut svgs) = (Vec::new(), Vec::new());
        let mut blobs = BTreeMap::new();
        for n in &names {
            let p = d.join(n);
            // one guard for both ways of not being a golden: a subdirectory, or
            // a file whose name carries neither suffix
            let stem = p
                .is_file()
                .then(|| {
                    n.strip_suffix(JSON_SUFFIX)
                        .or_else(|| n.strip_suffix(SVG_SUFFIX))
                })
                .flatten();
            let Some(stem) = stem else {
                bad.push(format!("{tag}/{n}: not a golden file"));
                continue;
            };
            let blob = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            blobs.insert(n.clone(), blob);
            if stem != sha(&blobs[n]) {
                bad.push(format!(
                    "{tag}/{}...: contents do not hash to its own name (edited by hand?)",
                    &n[..n.len().min(16)]
                ));
            }
            if n.ends_with(JSON_SUFFIX) {
                jsons.push(n.clone());
            } else {
                svgs.push(n.clone());
            }
        }

        if svgs.len() != 1 {
            bad.push(format!(
                "{tag}: holds {} SVGs, expected the 1 it is named after",
                svgs.len()
            ));
        } else if svgs[0].strip_suffix(SVG_SUFFIX) != Some(entry.as_str()) {
            bad.push(format!(
                "{tag}: the SVG here does not hash to this directory"
            ));
        }
        if jsons.len() > 1 {
            let configs: Vec<String> = jsons.iter().map(|n| text(&blobs[n])).collect();
            bad.push(format!(
                "{tag}: {} configs render this one SVG, so an axis stopped changing the picture: {}",
                jsons.len(),
                configs.join(" ")
            ));
        } else if jsons.is_empty() {
            bad.push(format!("{tag}: holds no config"));
        }

        let svg = (svgs.len() == 1).then(|| blobs[&svgs[0]].clone());
        for n in jsons {
            let hash = n
                .strip_suffix(JSON_SUFFIX)
                .expect("a json name")
                .to_string();
            found.insert(hash, (entry.clone(), blobs[&n].clone(), svg.clone()));
        }
    }
    (found, bad)
}

fn verify() -> ExitCode {
    let want = corpus();
    let (found, mut bad) = scan();

    // Both loops walk in config order rather than hash order, so the report
    // reads like the sweep that produced it instead of like a directory listing.
    let mut wanted: Vec<_> = want.iter().collect();
    wanted.sort_by(|a, b| (a.1).0.cmp(&(b.1).0));
    let mut stored_configs: Vec<_> = found.iter().collect();
    stored_configs.sort_by(|a, b| (a.1).1.cmp(&(b.1).1));

    for (json_hash, (json_blob, svg_blob)) in wanted {
        let Some((entry, _, stored)) = found.get(json_hash.as_str()) else {
            bad.push(format!("missing: {}", text(json_blob)));
            continue;
        };
        match stored {
            Some(s) if s != svg_blob => bad.push(format!(
                "changed: {}\n{}",
                text(json_blob),
                first_diff(s, svg_blob)
            )),
            _ if *entry != sha(svg_blob) => bad.push(format!(
                "changed: {}\n      {}... -> {}...",
                text(json_blob),
                &entry[..entry.len().min(16)],
                &sha(svg_blob)[..16]
            )),
            _ => {}
        }
    }
    for (json_hash, (_, json_blob, _)) in stored_configs {
        if !want.contains_key(json_hash.as_str()) {
            bad.push(format!("stale: {}", text(json_blob)));
        }
    }

    if !bad.is_empty() {
        eprintln!("golden: {} problem(s)", bad.len());
        for line in &bad {
            eprintln!("  {line}");
        }
        eprintln!("\nif the picture was meant to change: cargo run --example golden -- --regen");
        return ExitCode::from(1);
    }
    let (w, h) = size();
    println!(
        "golden ok: {} configs render byte-identical SVGs at {w}x{h}, \
         and every file hashes to its own name",
        want.len()
    );
    ExitCode::SUCCESS
}

fn regen() -> ExitCode {
    let want = corpus();
    let _ = std::fs::remove_dir_all(ROOT);
    for (json_hash, (json_blob, svg_blob)) in &want {
        let svg_hash = sha(svg_blob);
        let d = Path::new(ROOT).join(&svg_hash);
        std::fs::create_dir_all(&d).expect("the corpus directory must be writable");
        for (name, blob) in [
            (format!("{json_hash}{JSON_SUFFIX}"), json_blob),
            (format!("{svg_hash}{SVG_SUFFIX}"), svg_blob),
        ] {
            std::fs::write(d.join(&name), blob).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
    println!("regenerated {} goldens under test/golden", want.len());
    verify() // a fresh corpus still has to pass every check (~1s)
}

fn main() -> ExitCode {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => verify(),
        [a] if a == "--regen" => regen(),
        _ => {
            eprintln!("usage: cargo run --example golden [-- --regen]");
            eprintln!("  --regen  rewrite the corpus after an intended visual change");
            ExitCode::from(2)
        }
    }
}
