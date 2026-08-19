//! Animated SVG background builder — trihexagonal blueprint.
//!
//! (Task 12 replaces this placeholder with the full module docstring ported
//! from background.py:1-52.)

pub mod params;
pub mod rng;

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
