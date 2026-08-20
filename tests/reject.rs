//! The config boundary: every rejection exits 2, whether the schema catches it
//! or `validate` does.
use std::io::Write;

fn rejected(config: Option<&str>, why: &str) {
    let dir = std::env::temp_dir().join(format!("bgsvg-reject-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("parameters.json");
    if let Some(text) = config {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(text.as_bytes()).unwrap();
    } else {
        let _ = std::fs::remove_file(&path);
    }
    let code = bgsvg::run(&[path.to_string_lossy().into_owned()]);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(code, 2, "{why}: expected exit 2, got {code}");
}

#[test]
fn every_bad_config_exits_two() {
    rejected(
        Some(r#"{"icon":{"ship":{"motion":"ROTATE"}}}"#),
        "the ship must not accept a motion",
    );
    rejected(
        Some(r#"{"overlay":{"angle":90}}"#),
        "matrix knobs outside matrix must be rejected",
    );
    rejected(
        Some(r#"{"output":{"file":{"path":"a.svg"},"stdout":{}}}"#),
        "two output sinks at once must be rejected",
    );
    rejected(
        Some(r#"{"backgrond":{}}"#),
        "a typo'd key must be rejected, not silently ignored",
    );
    rejected(
        Some(r#"{"background":{"motion":"CLOSEOPEN","image":"NONE"}}"#),
        "closeopen without an image must be rejected",
    );
    rejected(
        Some(r#"{"overlay":{"matrix":{"angle":400}}}"#),
        "an out-of-range matrix angle must be rejected",
    );
    rejected(
        Some(r#"{"overlay":{"matrix":{"color":"395e53"}}}"#),
        "a malformed matrix colour must be rejected",
    );
    rejected(
        Some(r#"{"output":{"directory":{"resolutions":["1080"]}}}"#),
        "a malformed resolution must be rejected",
    );
    rejected(Some("{not json"), "malformed JSON must be rejected");
    rejected(None, "a missing config file must be rejected");
}
