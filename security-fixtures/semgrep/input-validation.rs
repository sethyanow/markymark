fn bad_uri(raw: &str) {
    // ruleid: markymark.rust.boundary-unwrap
    let _ = markymark_core::DocumentUri::new(raw).unwrap();
}

fn good_uri(raw: &str) {
    // ok: markymark.rust.boundary-unwrap
    let _ = markymark_core::DocumentUri::new(raw).map_err(|_e| ());
}

fn bad_json(raw: &str) {
    // ruleid: markymark.rust.boundary-unwrap
    let _parsed: serde_json::Value = serde_json::from_str(raw).expect("valid JSON");
}

fn good_json(raw: &str) {
    // ok: markymark.rust.boundary-unwrap
    let _ = serde_json::from_str::<serde_json::Value>(raw);
}
