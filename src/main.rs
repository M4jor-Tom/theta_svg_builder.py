//! `bgsvg [config.json]` -- see `parameters.proto` for the config schema.
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(bgsvg::run(&args) as u8)
}
