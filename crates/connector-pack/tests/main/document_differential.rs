//! **The document differential** — the canonical document's request template against the
//! Flux-derived request, field for field, for one whole provider (C-536).
//!
//! Decision 0022's migration rule is that the old and new derivations run side by side until they
//! are proven byte-identical. `tests/differential.rs` already holds the pack's request to the
//! shipped module's; this file lands the second comparison — the **document's request template**
//! (`catalog/<name>.catalog.json`, written by C-536's lowering in `connector-cli`) against the
//! request this pack evaluates from the same operation's emitted Flux. The whole-catalogue gate,
//! covering every provider plus the configuration surface, is C-538's; what must exist first is
//! the *mechanism*, proven on one provider with every request shape the catalogue ships: a
//! templated host, three services, path/query/body parameters, a path **pin** (messaging's
//! `appId`), and a `form`-free JSON body.
//!
//! Zendesk is that provider, and it is deliberately the same connector the README snippet, the
//! rehearsal worked examples and the lockfile fixtures all use — a divergence here is legible
//! against every other artifact of the same operation.
//!
//! # How the template is evaluated
//!
//! The evaluator below reproduces, in the template's own closed vocabulary, exactly the semantics
//! `connector-pack/src/request.rs` gives the emitted Flux — and each rule cites where it comes
//! from. That duplication is the *point* of a differential: two independent derivations that agree
//! are evidence; one derivation asserted equal to itself is not. C-538 replaces the pack's half
//! with a production reader of this same template, and this file's evaluator retires into it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use connector_pack::{Configuration, MemoryConfig, Rehearsal, Request};
use serde_json::{Map, Value};

const TENANT: &str = "t-document-differential";
const PROVIDER: &str = "zendesk";

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// The committed canonical document — a full build's artifact, read exactly as a consumer would.
fn document() -> Value {
    let path = root().join(format!("catalog/{PROVIDER}.catalog.json"));
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read `catalog/{PROVIDER}.catalog.json`: {error}\n\
             the canonical per-provider document is a committed artifact of a full\n\
             `cargo run -p connector-cli -- build` (C-536)"
        )
    });
    serde_json::from_str(&text).expect("the committed document is JSON")
}

/// The emitted Flux of one operation — the per-provider artifact the rehearsal evaluates.
fn flux_of(id: &str) -> String {
    let path = root().join(format!("crates/catalog/ops/{PROVIDER}/{id}.flux"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read the emitted Flux for `{id}`: {error}"))
}

// ---------------------------------------------------------------------------------------------
// The template evaluator: request.rs's semantics, over the document's closed vocabulary
// ---------------------------------------------------------------------------------------------

/// The one brace grammar — `request.rs::scan_template`'s behaviour: `{{`/`}}` escapes pass
/// through, an unterminated brace stays verbatim, and a name `fill` declines stays verbatim,
/// braces included. A filled value is never rescanned.
fn scan(template: &str, mut fill: impl FnMut(&str) -> Option<String>) -> String {
    if !template.contains('{') {
        return template.to_string();
    }
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let at_brace = &rest[open..];
        let (open_token, close_token) = if at_brace.starts_with("{{") {
            ("{{", "}}")
        } else {
            ("{", "}")
        };
        let inner = &at_brace[open_token.len()..];
        let Some(close) = inner.find(close_token) else {
            out.push_str(at_brace);
            return out;
        };
        match fill(inner[..close].trim()) {
            Some(value) => {
                out.push_str(&value);
                rest = &inner[close + close_token.len()..];
            }
            None => {
                out.push_str(open_token);
                rest = inner;
            }
        }
    }
    out.push_str(rest);
    out
}

/// flux-lang's `lit_text`, as `request.rs::text` reproduces it: a string is itself, `null` is
/// empty, anything else is compact JSON.
fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// flux-lang's `json_truthy`, as `request.rs::truthy` reproduces it — what decides whether an
/// optional form pair travels.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(bit) => *bit,
        Value::Number(number) => number.as_f64().map(|n| n != 0.0).unwrap_or(false),
        Value::String(text) => {
            let trimmed = text.trim();
            !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("false") && trimmed != "0"
        }
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
    }
}

/// The RFC 3986 component encoder Flux 0.54 applies to every structured query key and value —
/// `connector-pack/src/auth.rs::query_encode`.
fn query_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// A `{"$param": name}` splice, or `None` for anything else in value position.
fn splice(value: &Value) -> Option<&str> {
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    object.get("$param").and_then(Value::as_str)
}

/// Instantiate a JSON body template: objects and arrays recurse, a `$param` splices the caller's
/// value verbatim, and every other literal is itself.
fn instantiate(template: &Value, params: &Value) -> Value {
    if let Some(name) = splice(template) {
        return params
            .get(name)
            .unwrap_or_else(|| panic!("the template splices `{name}`, which the caller omitted"))
            .clone();
    }
    match template {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(name, child)| (name.clone(), instantiate(child, params)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|child| instantiate(child, params))
                .collect(),
        ),
        literal => literal.clone(),
    }
}

/// **Evaluate one operation's request template** over the caller's `params` and the tenant's
/// `endpoints` — the document-side half of the differential.
fn evaluate(
    operation: &Value,
    base_url: &str,
    params: &Value,
    endpoints: &BTreeMap<String, String>,
) -> Request {
    let request = &operation["request"];

    // `{base}` is the operation's service base URL with its endpoint slots filled — the emitter
    // binds the same literal and the pack substitutes into literals only (C-193). A filled value
    // is not rescanned, so nothing a tenant configures can splice a caller's parameter in.
    let base = scan(base_url.trim_end_matches('/'), |name| {
        endpoints.get(name).cloned()
    });

    let mut url = scan(request["url"].as_str().expect("a URL template"), |name| {
        if name == "base" {
            return Some(base.clone());
        }
        if let Some(value) = params.get(name) {
            return Some(text(value));
        }
        endpoints.get(name).cloned()
    });

    // The structured query record: null omitted, everything encoded exactly once (C-30).
    let mut pairs = Vec::new();
    if let Some(entries) = request["query"].as_array() {
        for entry in entries {
            let name = entry["name"].as_str().expect("a query entry has a name");
            let value = &entry["value"];
            let rendered = match splice(value) {
                Some(param) => match params.get(param).unwrap_or(&Value::Null) {
                    Value::Null => continue,
                    Value::String(value) => value.clone(),
                    Value::Number(value) => value.to_string(),
                    Value::Bool(value) => value.to_string(),
                    other => panic!("query field `{name}` is {other}, which Flux refuses"),
                },
                None => scan(value.as_str().expect("a query literal"), |slot| {
                    endpoints.get(slot).cloned()
                }),
            };
            pairs.push(format!(
                "{}={}",
                query_encode(name),
                query_encode(&rendered)
            ));
        }
    }
    if !pairs.is_empty() {
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(&pairs.join("&"));
    }

    let mut headers = BTreeMap::new();
    if let Some(entries) = request["headers"].as_object() {
        for (name, value) in entries {
            let rendered = match splice(value) {
                Some(param) => text(params.get(param).unwrap_or(&Value::Null)),
                None => scan(value.as_str().expect("a header literal"), |slot| {
                    endpoints.get(slot).cloned()
                }),
            };
            headers.insert(name.clone(), rendered);
        }
    }

    let body = match request["body"]["encoding"].as_str() {
        None => None,
        Some("json") => {
            let template = &request["body"]["template"];
            let mut value = instantiate(template, params);
            // A free-form body supplied as text travels through `parse(…, as: "json")`: it is
            // validated and canonicalized rather than sent verbatim (`connector-flux`'s free-form
            // lowering; `request.rs`'s `Node::Parse` arm).
            if splice(template).is_some() {
                if let Value::String(text) = &value {
                    value = serde_json::from_str(text).expect("a free-form body must be JSON");
                }
            }
            Some(value.to_string())
        }
        Some("form") => {
            let fields = request["body"]["fields"]
                .as_array()
                .expect("a form body carries its fields");
            let mut pairs = Vec::new();
            for field in fields {
                let name = field["name"].as_str().expect("a form field has a name");
                let value = match splice(&field["value"]) {
                    Some(param) => params.get(param).unwrap_or(&Value::Null).clone(),
                    None => field["value"].clone(),
                };
                let always = field["required"].as_bool().unwrap_or(false);
                if always || truthy(&value) {
                    pairs.push(format!("{name}={}", text(&value)));
                }
            }
            Some(pairs.join("&"))
        }
        Some(other) => panic!("the template vocabulary has no `{other}` encoding"),
    };

    let mut request = Request {
        method: request["method"]
            .as_str()
            .expect("the template states a method")
            .to_string(),
        url,
        headers,
        body,
    };
    identify(&mut request);
    request
}

/// `request.rs::identify` — the pack gives every request this software's `User-Agent` unless the
/// connector stated one (C-223). Reconstructed from the same manifest fields the pack reads, so a
/// release bump moves both sides together.
fn identify(request: &mut Request) {
    if request
        .headers
        .keys()
        .any(|name| name.eq_ignore_ascii_case("user-agent"))
    {
        return;
    }
    request.headers.insert(
        "User-Agent".to_string(),
        format!(
            "flux-connectors/{} (+{})",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_REPOSITORY")
        ),
    );
}

// ---------------------------------------------------------------------------------------------
// Inputs both derivations share
// ---------------------------------------------------------------------------------------------

/// A plausible value for every parameter, from the rehearsed contract's own input schema — the
/// same shapes `tests/differential.rs::params_from_schema` uses.
fn params_for(rehearsal: &Rehearsal) -> Value {
    let mut params = Map::new();
    if let Some(properties) = rehearsal
        .spec()
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    {
        for (name, schema) in properties {
            let value = match schema.get("type").and_then(Value::as_str) {
                Some("number") | Some("integer") => Value::from(1),
                Some("boolean") => Value::from(true),
                Some("array") => Value::Array(Vec::new()),
                Some("object") => Value::Object(Map::new()),
                Some(_) => Value::String(format!("a-{name}")),
                // An untyped parameter is a free-form body, which travels through
                // `parse(…, as: "json")` — a bare word is not JSON and would be refused.
                None => Value::Object(Map::new()),
            };
            params.insert(name.clone(), value);
        }
    }
    Value::Object(params)
}

/// The tenant value each endpoint variable resolves to, read off the **document's own
/// configuration surface**: the declared `example` of the field binding that slot for that
/// service. The document is the artifact under test, so it must be able to answer this — a
/// variable no field declares is a defect the assertion should surface, not paper over.
fn declared_values(document: &Value, service: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for field in document["config"].as_array().expect("config").iter() {
        let owns = field["service"] == service
            || field["also_services"]
                .as_array()
                .is_some_and(|widened| widened.iter().any(|entry| entry == service));
        if !owns {
            continue;
        }
        let binds = field["binds"].as_str().expect("a field states `binds`");
        let variable = match binds.split_once('.') {
            Some(("endpoint", variable)) => variable,
            Some(("path" | "query" | "header", name)) => name,
            _ => continue,
        };
        if let Some(example) = field["example"].as_str() {
            values.insert(variable.to_string(), example.to_string());
        }
    }
    values
}

// ---------------------------------------------------------------------------------------------
// The differential
// ---------------------------------------------------------------------------------------------

/// **The mechanism.** For every operation zendesk's document carries, the request evaluated from
/// the document's template equals the request the pack evaluates from the same operation's
/// emitted Flux — method, URL, headers and body, exactly.
#[test]
fn the_document_template_and_the_flux_derived_request_agree_for_zendesk() {
    let document = document();
    let base_urls: BTreeMap<String, String> = document["services"]
        .as_array()
        .expect("the document carries its services")
        .iter()
        .map(|service| {
            (
                service["name"].as_str().expect("a name").to_string(),
                service["base_url"]
                    .as_str()
                    .expect("a base URL")
                    .to_string(),
            )
        })
        .collect();

    let operations = document["operations"]
        .as_array()
        .expect("the document carries its operations");
    assert!(
        !operations.is_empty(),
        "an empty document differentials nothing"
    );

    let mut divergences: Vec<String> = Vec::new();
    let mut compared = 0usize;
    for entry in operations {
        let id = entry["id"].as_str().expect("an operation id");
        let service = entry["service"].as_str().expect("an operation's service");
        let flux = flux_of(id);

        let rehearsal = Rehearsal::of(id, PROVIDER, service, &flux)
            .unwrap_or_else(|error| panic!("`{id}` does not rehearse: {error}"));
        let params = params_for(&rehearsal);

        let declared = declared_values(&document, service);
        let mut endpoints = BTreeMap::new();
        let mut values = MemoryConfig::new();
        for variable in rehearsal.endpoint_variables() {
            let value = declared.get(variable).unwrap_or_else(|| {
                panic!("`{id}` needs `{variable}`, which the document's config declares no example for")
            });
            values = values.with_endpoint(TENANT, PROVIDER, service, variable, value);
            endpoints.insert(variable.clone(), value.clone());
        }
        let configuration =
            Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id");

        let from_flux = rehearsal
            .request(&configuration, &params)
            .unwrap_or_else(|error| panic!("`{id}`'s Flux-derived request refuses: {error}"));
        let base_url = base_urls.get(service).unwrap_or_else(|| {
            panic!("`{id}` names service `{service}`, which the document does not carry")
        });
        let from_document = evaluate(entry, base_url, &params, &endpoints);

        for problem in disagreements(id, &from_document, &from_flux) {
            divergences.push(problem);
        }
        compared += 1;
    }

    assert!(
        divergences.is_empty(),
        "{} of {compared} operations diverge between the document template and the emitted Flux:\n{}",
        divergences.len(),
        divergences.join("\n")
    );
    assert_eq!(
        compared,
        operations.len(),
        "every operation the document carries must be compared"
    );
}

/// The document names exactly the operations the per-provider renderings ship — neither side may
/// carry an operation the other does not, or the differential above compares a subset and calls
/// it the provider.
#[test]
fn the_document_and_the_renderings_carry_the_same_operations() {
    let document = document();
    let documented: BTreeSet<String> = document["operations"]
        .as_array()
        .expect("operations")
        .iter()
        .map(|operation| operation["id"].as_str().expect("an id").to_string())
        .collect();

    let mut rendered = BTreeSet::new();
    let renderings = root().join(format!("crates/catalog/ops/{PROVIDER}"));
    for entry in fs::read_dir(&renderings).expect("the zendesk renderings exist") {
        let path = entry.expect("a readable entry").path();
        if path
            .extension()
            .is_some_and(|extension| extension == "flux")
        {
            rendered.insert(
                path.file_stem()
                    .expect("a rendering has a stem")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }

    assert_eq!(
        documented, rendered,
        "the document and crates/catalog/ops/{PROVIDER}/ must name the same operations"
    );
}

/// A seeded divergence is *reported*, naming the field — the assertion above is only trustworthy
/// if a disagreement actually fails it.
#[test]
fn a_seeded_divergence_is_caught() {
    let request = Request {
        method: "GET".to_string(),
        url: "https://acme.zendesk.com/api/v2/tickets".to_string(),
        headers: BTreeMap::new(),
        body: None,
    };
    let mut doctored = request.clone();
    doctored.method = "POST".to_string();
    let problems = disagreements("seeded", &doctored, &request);
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].contains("method") && problems[0].contains("seeded"),
        "a divergence names the operation and the field: {problems:?}"
    );
}

/// Every way the two requests can disagree, each naming the operation and the field.
fn disagreements(operation: &str, document: &Request, flux: &Request) -> Vec<String> {
    let mut problems = Vec::new();
    if document.method != flux.method {
        problems.push(format!(
            "`{operation}`: method `{}` (document) vs `{}` (flux)",
            document.method, flux.method
        ));
    }
    if document.url != flux.url {
        problems.push(format!(
            "`{operation}`: url `{}` (document) vs `{}` (flux)",
            document.url, flux.url
        ));
    }
    if document.headers != flux.headers {
        problems.push(format!(
            "`{operation}`: headers {:?} (document) vs {:?} (flux)",
            document.headers.keys().collect::<Vec<_>>(),
            flux.headers.keys().collect::<Vec<_>>()
        ));
    }
    if document.body != flux.body {
        problems.push(format!(
            "`{operation}`: body {:?} (document) vs {:?} (flux)",
            document.body, flux.body
        ));
    }
    problems
}
