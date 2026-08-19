//! parameters.proto -> Rust. prost-build generates the messages; pbjson-build
//! reads the descriptor set it emits and adds the canonical proto3 JSON mapping
//! on top, which is the part this program actually uses -- nothing here ever
//! touches the wire format.
use std::{env, fs, io::Result, path::PathBuf};

fn main() -> Result<()> {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let descriptor = out.join("descriptor.bin");

    prost_build::Config::new()
        .file_descriptor_set_path(&descriptor)
        .compile_protos(&["parameters.proto"], &["."])?;

    pbjson_build::Builder::new()
        .register_descriptors(&fs::read(&descriptor)?)?
        .build(&[".svg_builder"])?;

    println!("cargo:rerun-if-changed=parameters.proto");
    Ok(())
}
