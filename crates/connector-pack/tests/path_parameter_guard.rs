//! C-478: caller-owned path values stay inside the one reviewed URL segment.
//!
//! The fixture is emitted-Flux-shaped and deliberately puts one path value inside a `when` branch.
//! That makes the test about the pack's declaration analysis rather than about a provider-specific
//! parameter name or catalogue metadata. Query, header and body values carry the same characters in
//! the positive test so a broad "reject suspicious strings" check cannot satisfy it.

use std::sync::Arc;

use connector_pack::{Configuration, Error, MemoryConfig, Rehearsal};
use serde_json::json;

const GUARDED_PATH: &str = r#"op probe-item-update(id: String, child_id: String, filter: String, header_value: String, body_value: String) -> Any
  description "Update one guarded child"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.probe.example"
  url = fmt("{base}/items/{id}")
  when child_id
    url = fmt("{url}/children/{child_id}")
  sep = "?"
  when filter
    url = fmt("{url}{sep}filter={filter}")
    sep = "&"
  payload = { value: $body_value }
  response = http.request(body: payload, headers: { "X-Probe": $header_value }, method: "POST", url)
  return response
"#;

const NUMERIC_PATH: &str = r#"op probe-item-get(id: Number) -> Any
  description "Get one numbered item"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.probe.example"
  url = fmt("{base}/items/{id}")
  response = http.request(method: "GET", url)
  return response
"#;

fn configuration() -> Configuration {
    Configuration::new(Arc::new(MemoryConfig::new()), "t-path-guard").expect("a valid tenant id")
}

fn guarded() -> Rehearsal {
    Rehearsal::of("probe-item-update", "probe", "default", GUARDED_PATH)
        .expect("the emitted fixture projects")
}

fn params(id: &str, child_id: &str, filter: &str, header: &str, body: &str) -> serde_json::Value {
    json!({
        "id": id,
        "child_id": child_id,
        "filter": filter,
        "header_value": header,
        "body_value": body,
    })
}

/// The failing-first mutation set. Before C-478 every value here was interpolated verbatim, so `/`
/// added a path segment, `?`/`#` ended the path, `%` introduced an encoded delimiter, and dot
/// segments changed which resource the transport resolved.
#[test]
fn every_path_delimiter_is_refused_for_direct_and_guarded_parameters() {
    for unsafe_value in [
        "a/b", "a?b", "a#b", "a%b", "a\\b", "a b", "a\nb", "a\u{0}b", ".", "..",
    ] {
        for parameter in ["id", "child_id"] {
            let (id, child_id) = match parameter {
                "id" => (unsafe_value, "child-1"),
                "child_id" => ("item-1", unsafe_value),
                _ => unreachable!(),
            };
            let error = guarded()
                .request(
                    &configuration(),
                    &params(id, child_id, "active", "safe", "safe"),
                )
                .expect_err("a caller path value may not escape its segment");

            assert!(
                matches!(
                    &error,
                    Error::UnsafePathParameter {
                        operation,
                        parameter: refused,
                        ..
                    } if operation == "probe-item-update" && refused == parameter
                ),
                "{parameter}={unsafe_value:?}: {error}"
            );
        }
    }
}

#[test]
fn safe_string_and_numeric_paths_are_byte_identical() {
    let request = guarded()
        .request(
            &configuration(),
            &params("item-1", "child_2", "", "safe", "safe"),
        )
        .expect("ordinary string segments build");
    assert_eq!(
        request.url,
        "https://api.probe.example/items/item-1/children/child_2"
    );

    let request = Rehearsal::of("probe-item-get", "probe", "default", NUMERIC_PATH)
        .expect("the numeric fixture projects")
        .request(&configuration(), &json!({"id": 42}))
        .expect("a number is a safe path segment");
    assert_eq!(request.url, "https://api.probe.example/items/42");
}

#[test]
fn query_header_and_body_values_do_not_inherit_the_path_rule() {
    let structural = "a/b?c#d%e\\f g";
    let request = guarded()
        .request(
            &configuration(),
            &params("item-1", "child-1", structural, structural, structural),
        )
        .expect("only path placements use the segment guard");

    assert_eq!(
        request.url,
        format!("https://api.probe.example/items/item-1/children/child-1?filter={structural}")
    );
    assert_eq!(
        request.headers.get("X-Probe").map(String::as_str),
        Some(structural)
    );
    assert_eq!(
        request.body.as_deref(),
        Some(r#"{"value":"a/b?c#d%e\\f g"}"#)
    );
}
