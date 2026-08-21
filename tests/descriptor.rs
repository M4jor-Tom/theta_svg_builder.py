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
