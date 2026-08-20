//! The constraint that motivated the whole refactor, enforced rather than
//! remembered: no SVG markup in Rust source. Askama templates under
//! `templates/` are the only place a `<tag` may appear.
//!
//! `#[cfg(test)]` modules are exempt: `src/` legitimately holds string
//! needles like `svg.matches("<polygon")` that search *rendered* output,
//! which is not markup being authored. So each file is truncated at its
//! first `#[cfg(test)]` line and only the part before that is scanned --
//! the same check this project has been running by hand:
//! `awk '/#\[cfg\(test\)\]/{exit} /"</{print FILENAME":"NR}' src/*.rs`

/// A smoke test, not a proof -- `format!("{x}<polygon")` would slip through.
/// It catches the realistic case: a string literal that opens with a tag,
/// anywhere in a file's non-test code.
#[test]
fn no_svg_markup_in_rust_sources() {
    for entry in std::fs::read_dir("src").expect("src/ exists") {
        let path = entry.expect("a readable entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("valid utf-8");
        // Only the code before the first #[cfg(test)] module counts -- test
        // code searches rendered SVG text, it doesn't author any.
        let production = match src.find("#[cfg(test)]") {
            Some(i) => &src[..i],
            None => &src[..],
        };
        assert!(
            !production.contains("\"<"),
            "{} contains SVG markup outside its #[cfg(test)] module; \
             move it into templates/ instead of building it in Rust",
            path.display()
        );
    }
}
