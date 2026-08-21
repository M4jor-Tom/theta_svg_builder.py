//! `render_to_string` is the CLI's own pipeline with the sink removed. If these
//! ever diverge, a rule added to `validate()` reaches one caller and not the
//! other -- the failure this extraction exists to prevent.
use bgsvg::{Error, load, render_to_string};

#[test]
fn render_to_string_is_the_pipeline_the_cli_runs() {
    let (_, scene) = load("{}").expect("{} is a complete config");
    assert_eq!(
        render_to_string("{}", 640, 360).unwrap(),
        bgsvg::svg::build_svg(640, 360, &scene),
        "render_to_string must produce exactly what the CLI would write"
    );
}

#[test]
fn render_to_string_validates_before_it_renders() {
    // the one rule parameters.proto cannot state
    let e = render_to_string(
        r#"{"background":{"motion":"CLOSEOPEN","image":"NONE"}}"#,
        640,
        360,
    )
    .expect_err("CLOSEOPEN with NONE has nothing to reveal");
    assert!(matches!(e, Error::Invalid(_)), "got {e:?}");

    // a typo'd key is the schema's rejection, not validate()'s
    assert!(matches!(
        render_to_string(r#"{"backgrond":{}}"#, 640, 360).unwrap_err(),
        Error::Schema(_)
    ));
}

#[test]
fn load_returns_the_message_too_so_the_cli_can_pick_a_sink() {
    let (p, scene) = load(r#"{"seed":7,"output":{"stdout":{"resolution":"4k"}}}"#).unwrap();
    assert_eq!(scene.seed, 7);
    assert!(p.output.is_some(), "the sink must survive load()");
}
