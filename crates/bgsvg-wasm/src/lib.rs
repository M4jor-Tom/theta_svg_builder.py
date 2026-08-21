//! Browser-callable bindings for `bgsvg`.
//!
//! This crate holds no rendering logic and no markup. It calls
//! `bgsvg::render_to_string` and `bgsvg::params::parse_res` like any other
//! consumer; its only real work is turning a `bgsvg::Error` into an object
//! JavaScript can branch on.

use wasm_bindgen::prelude::*;

/// Which half of the boundary rejected the config, and where.
///
/// Kept as a plain function of the error, separate from the JsValue it becomes,
/// so the classification can be tested on the host -- `cargo test` has no
/// JavaScript to hand.
fn classify(e: &bgsvg::Error) -> (&'static str, Option<(usize, usize)>) {
    match e {
        // serde_json's position is 1-based and points into the text the user typed
        bgsvg::Error::Schema(e) => ("schema", Some((e.line(), e.column()))),
        bgsvg::Error::Invalid(_) => ("invalid", None),
        // unreachable here: neither render_to_string nor parse_res ever touches
        // a filesystem -- Error::Io is only ever constructed on the CLI path
        bgsvg::Error::Io(_) => ("invalid", None),
    }
}

/// `{ kind, message, line?, column? }` -- see the API specification in
/// `docs/superpowers/specs/2026-08-21-wasm-target-design.md`.
fn throw(e: bgsvg::Error) -> JsValue {
    let (kind, at) = classify(&e);
    let o = js_sys::Object::new();
    let set = |k: &str, v: JsValue| {
        js_sys::Reflect::set(&o, &JsValue::from_str(k), &v)
            .expect("a freshly created object accepts new keys");
    };
    set("kind", JsValue::from_str(kind));
    set("message", JsValue::from_str(&e.to_string()));
    if let Some((line, column)) = at {
        set("line", JsValue::from_f64(line as f64));
        set("column", JsValue::from_f64(column as f64));
    }
    o.into()
}

/// A trap poisons the module for every later call, so make it say why.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// One config, one SVG document. `width` and `height` are pixels, both non-zero.
///
/// A config's `output` field is parsed and validated like any other, then
/// ignored: a sink names a destination and there is none here. So a config
/// written for the CLI renders unaltered rather than being rejected.
#[wasm_bindgen]
pub fn render(json: &str, width: u32, height: u32) -> Result<String, JsValue> {
    bgsvg::render_to_string(json, width, height).map_err(throw)
}

/// A preset name or `WIDTHxHEIGHT` -> `[width, height]`. This is `parse_res`
/// and nothing more, exposed so no consumer reimplements its edge cases.
#[wasm_bindgen]
pub fn resolve_resolution(spec: &str) -> Result<Box<[u32]>, JsValue> {
    let (w, h) = bgsvg::params::parse_res(spec).map_err(throw)?;
    Ok(vec![w, h].into_boxed_slice())
}

/// The preset table as JSON, in declaration order.
#[wasm_bindgen]
pub fn resolutions() -> String {
    let v: Vec<serde_json::Value> = bgsvg::params::RESOLUTIONS
        .iter()
        .map(|(name, (w, h))| serde_json::json!({"name": name, "width": w, "height": h}))
        .collect();
    serde_json::Value::Array(v).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `kind` split is the whole contract with a consumer: a syntax error
    /// belongs beside the text someone typed, a rule violation belongs beside
    /// the field it concerns. Tested on the host, where there is no JsValue.
    #[test]
    fn errors_are_classified_by_which_half_rejected_them() {
        let schema = bgsvg::render_to_string(r#"{"backgrond":{}}"#, 640, 360).unwrap_err();
        let (kind, at) = classify(&schema);
        assert_eq!(kind, "schema");
        assert!(
            matches!(at, Some((l, c)) if l >= 1 && c >= 1),
            "a schema rejection must carry a real position"
        );

        let invalid = bgsvg::render_to_string(
            r#"{"background":{"motion":"CLOSEOPEN","image":"NONE"}}"#,
            640,
            360,
        )
        .unwrap_err();
        assert_eq!(classify(&invalid), ("invalid", None));
    }

    #[test]
    fn resolutions_serialises_every_preset() {
        let v: serde_json::Value = serde_json::from_str(&resolutions()).unwrap();
        let a = v.as_array().expect("an array");
        assert_eq!(a.len(), bgsvg::params::RESOLUTIONS.len());
        assert_eq!(a[0]["name"], "1080p");
        assert_eq!(a[0]["width"], 1920);
        assert_eq!(a[0]["height"], 1080);
    }
}
