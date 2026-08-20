//! Animated SVG background builder — trihexagonal blueprint.
//!
//! (Task 12 replaces this placeholder with the full module docstring ported
//! from background.py:1-52.)

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

/// Parse, validate, resolve and write -- porting `background.py:696-727`. The
/// sink decides both the resolution list and where the bytes go: stdout and a
/// file each carry exactly one resolution, a directory (also the unset
/// default) carries a list, empty meaning one entry at `parse_res("")`'s
/// default.
fn render(text: &str) -> Result<(), Error> {
    let p = params::parse(text)?;
    params::validate(&p)?;
    let scene = params::resolve(&p);
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
        [a] => (a.clone(), std::fs::read_to_string(a).map_err(Error::Io)),
        _ => {
            eprintln!("usage: bgsvg [config.json]");
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
