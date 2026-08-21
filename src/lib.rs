//! Animated SVG background builder — trihexagonal blueprint.
//!
//! A hexagon lattice with sparse equilateral triangles, plus a center icon.
//! Light theme only. One render is described by one JSON file; parameters.proto
//! is its schema, and every zero value there is this program's default, so `{}`
//! is a complete config.
//!
//!   background.motion  STATIC     no background animation
//!                      SCAN       hexagon (and triangle) opacities sweep diagonally
//!                      LIGHTS     hexagons light their border, then fill, then rest
//!                      CLOSEOPEN  every hexagon is a window, mostly shut; a few
//!                                 shrink open onto the image at a time, then close
//!   background.image   NONE       plain lattice
//!                      STARFIELD  a few hexagons become windows onto a procedural
//!                                 starfield
//!   icon.hexatri       nested hexagon/triangle glyph, motion ROTATE or STATIC
//!   icon.ship          cloaked delta spaceship: one folded sheet read as four
//!                      translucent facets under one light. Declares no motion --
//!                      it is static by design, and nothing assumes the next glyph
//!                      rotates
//!   overlay.matrix     columns of characters at `angle` in `color`. The characters
//!                      never move: a lit head walks down each column into the next
//!                      fixed glyph while the ones behind it fade out in place
//!
//! Two rules that used to be runtime rejections are now unrepresentable: a motion
//! belongs to the icon that has it (only hexatri declares one), and the matrix
//! angle and colour live inside the matrix message, so they cannot be written
//! without rain to steer. The one rule the schema cannot state -- CLOSEOPEN needs
//! an image to open onto -- is checked in `params::validate`.
//!
//! Animation is pure CSS (no SMIL, no JS) and honours prefers-reduced-motion
//! (which falls back to the clean static look). It runs on six crates --
//! `askama`, `prost`, `pbjson`, `serde`, `serde_json` and `sha2`, next to the
//! `prost-build`/`pbjson-build` pair that compiles the schema and is gone by
//! run time -- and `askama` is the one that shapes the source tree: every SVG
//! element lives in a template under `templates/`, inlined into the binary at
//! compile time, so nothing is read from disk at runtime. There are no
//! external assets either -- the starfield is drawn, not embedded -- so
//! output stays a small self-contained .svg that is crisp at any resolution.
//! Sizes scale with min(w,h) so pattern density is constant across
//! resolutions.
//!
//! Triangle logic: every hexagon is EITHER a "holder" (one of its edges is a
//! triangle's base) OR an "intersector" (a triangle's tip pokes into it), never
//! both. Triangles never overlap and never sit behind the center icon.
//!
//! Space cells sit fully outside the icon's clear zone, and under LIGHTS they
//! pulse their border only -- a pale fill flash would wash the stars out.
//!
//! Everything except the animations depends only on `seed`, so a given seed
//! yields the same layout across every background / icon / overlay combination --
//! the rain draws from its own stream and never moves a hexagon.
//!
//! Examples:
//!   bgsvg                      # the schema's defaults
//!   bgsvg docs/mood/samples/matrix-hexatri.json

pub mod geom;
pub mod icon;
pub mod matrix;
pub mod params;
pub mod rng;
pub mod style;
pub mod svg;
pub mod trihex;

use std::fmt;

/// Everything that can go wrong between a path on the command line and an SVG
/// on disk. Three variants is the whole surface, so a hand-written enum beats
/// pulling in an error library.
#[derive(Debug)]
pub enum Error {
    /// The config could not be read, or the output could not be written.
    Io(std::io::Error),
    /// The JSON is malformed, has a typo'd key, sets two members of one oneof,
    /// or names a value the schema does not know.
    Schema(serde_json::Error),
    /// A rule `parameters.proto` cannot state — see `params::validate`.
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Schema(e) => write!(f, "{e}"),
            Error::Invalid(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Schema(e)
    }
}

/// Parse, validate, flatten -- the whole schema boundary in one call.
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
/// cannot reach one caller and miss the other -- the same reason both test
/// surfaces enumerate from `params::valid_configs`. A zero `w` or `h` is
/// rejected here rather than in `Lattice::new`, where it would divide by a
/// zero row height and overflow the row count -- this is the function every
/// caller funnels through, so this is where that guard has to live.
pub fn render_to_string(json: &str, w: u32, h: u32) -> Result<String, Error> {
    if w == 0 || h == 0 {
        return Err(Error::Invalid("width and height must be non-zero".into()));
    }
    let (_, scene) = load(json)?;
    Ok(svg::build_svg(w, h, &scene))
}

/// Parse, validate, resolve and write -- porting `background.py:696-727`. The
/// sink decides both the resolution list and where the bytes go: stdout and a
/// file each carry exactly one resolution, a directory (also the unset
/// default) carries a list, empty meaning one entry at `parse_res("")`'s
/// default.
fn render(text: &str) -> Result<(), Error> {
    let (p, scene) = load(text)?;
    let sink = p.output.as_ref().and_then(|o| o.sink.as_ref());

    if let Some(params::output::Sink::Stdout(s)) = sink {
        let (w, h) = params::parse_res(&s.resolution)?;
        print!("{}", svg::build_svg(w, h, &scene));
        return Ok(());
    }
    if let Some(params::output::Sink::File(f)) = sink {
        let (w, h) = params::parse_res(&f.resolution)?;
        std::fs::write(&f.path, svg::build_svg(w, h, &scene))?;
        println!("{}", f.path);
        return Ok(());
    }

    let (dir, resolutions): (&str, &[String]) = match sink {
        Some(params::output::Sink::Directory(d)) => (&d.path, &d.resolutions),
        _ => ("", &[]),
    };
    let dir = if dir.is_empty() { "out" } else { dir };
    std::fs::create_dir_all(dir)?;
    let sizes: Vec<(u32, u32)> = if resolutions.is_empty() {
        vec![params::parse_res("")?]
    } else {
        resolutions
            .iter()
            .map(|r| params::parse_res(r))
            .collect::<Result<_, _>>()?
    };
    for (w, h) in sizes {
        let path = format!("{dir}/trihex-{}-{w}x{h}.svg", scene.slug());
        std::fs::write(&path, svg::build_svg(w, h, &scene))?;
        println!("{path}");
    }
    Ok(())
}

/// The CLI body. It lives in the library rather than in main.rs so tests can
/// drive the whole boundary -- read, parse, validate, render, report -- without
/// spawning a process.
pub fn run(args: &[String]) -> i32 {
    let (label, text) = match args {
        [] => ("(defaults)".to_string(), Ok("{}".to_string())),
        [a] if a == "--configs" => {
            for c in params::valid_configs(0) {
                println!(
                    "{}",
                    serde_json::to_string(&c).expect("a config must serialise")
                );
            }
            return 0;
        }
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
        [a] => (a.clone(), std::fs::read_to_string(a).map_err(Error::Io)),
        _ => {
            eprintln!("usage: bgsvg [config.json | --configs | --descriptor]");
            return 2;
        }
    };
    match text.and_then(|t| render(&t)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{label}: {e}");
            2
        }
    }
}
