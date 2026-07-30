//! The request an operation makes, built from the operation's own emitted Flux.
//!
//! # Why the declaration, and not a second lowering
//!
//! [`crate::spec`] projects the *contract* by reading the shipped `op` declaration back, because two
//! surfaces generated from one IR can drift into disagreeing about the same operation — the risk
//! [the design](../../../docs/designs/connector-tool-pack.md) names first. The **request** is the
//! other half of that risk and it is the more dangerous half: a contract that drifts tells a model
//! something slightly wrong, while a request that drifts sends a vendor a call the module would
//! never have made.
//!
//! So this module does not re-lower `connector_spec`'s IR. It **evaluates the emitted body** — the
//! same statements `connectors/<provider>.flux` carries, the same statements flux itself would run —
//! and reads `{ method, url, headers, body }` off the `http.request` call at the end of it. The
//! pack's request is the module's request by construction, which is the only form of agreement that
//! cannot rot. C-117 asserts it differentially; this is what makes that assertion cheap to satisfy.
//!
//! # The evaluator is deliberately tiny, and deliberately closed
//!
//! `connector-flux` emits exactly one shape ([`connector_flux::op`]'s module documentation states
//! it in full), so the node set this has to understand is small and fixed:
//!
//! | node | where it appears |
//! |---|---|
//! | `Bind` of a `Lit` | `base`, `sep`, `content_type`, a `const`-pinned body field |
//! | `Bind` of a `Fmt` | `url`, and each optional query parameter's re-binding |
//! | `Bind` of an `Obj` | `payload`, nested at the wire paths the emitter honours |
//! | `Bind` of a `Parse` | `payload`, for a free-form body supplied whole by the caller |
//! | `Bind` of a `Call` | `response = http.request(…)` — the request itself |
//! | `When` over a `Var` | the guard that makes an unsupplied filter *not sent* |
//! | `Return` of a `Var` | the end of the body |
//!
//! Anything else is [`Error::Unbuildable`](crate::Error::Unbuildable) rather than ignored. That is
//! the load-bearing choice: an emitter that grows a node this does not model — a `retry`, a quirk
//! compiled into control flow (C-12) — must fail loudly here, because the alternative is a request
//! assembled from *part* of an operation's body and sent anyway. A partly-evaluated request is not a
//! degraded request; it is a different call, and the vendor answers it.
//!
//! # Where the semantics come from
//!
//! Interpolation, truthiness and value-to-text are flux-lang's, reproduced against it rather than
//! invented — `interpolate_str`, `json_truthy` and `lit_text` in flux-lang's `runtime.rs`. The two
//! that matter for a request:
//!
//! - **An unbound `{name}` stays verbatim.** That is why the emitter guards optional query
//!   parameters with `when` instead of interpolating them unconditionally, and reproducing it here
//!   keeps the guard meaningful.
//! - **`null` renders as the empty string**, the `""`-is-absent idiom flux-lang documents on
//!   `lit_text`. A parameter a caller passed `null` for is falsey, so a guarded filter is not sent;
//!   rendering it as the literal text `null` would put `?page=null` on the wire in the one case the
//!   guard exists to prevent.

use std::collections::BTreeMap;

use flux_lang::ast::Node;
use flux_lang::program::CompositeOpDecl;
use serde_json::Value;

use crate::Error;

/// The op the whole pack delegates to. Named once so the check below is a comparison rather than a
/// scattered string.
const HTTP_REQUEST: &str = "http.request";

/// **The request**: `{ method, url, headers, body }`, exactly `http.request`'s own input.
///
/// A typed value rather than a bare `serde_json::Value` so a test can assert on the pieces that go
/// wrong silently — a flattened body, a missing `?`/`&` separator — instead of on a blob.
/// [`Request::to_params`] is the one place it becomes the JSON `http.request` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The HTTP method, as `http.request` spells it (`GET`, `PUT`, …).
    pub method: String,
    /// The absolute request URL, query string included.
    pub url: String,
    /// The request headers. `BTreeMap` because the emitted record is one, so the order is the
    /// module's order rather than a hash seed's.
    pub headers: BTreeMap<String, String>,
    /// The request body as the text `http.request` sends, or `None` for a request that has none.
    ///
    /// Text rather than a `Value` because that is what actually travels: `http.request` reads its
    /// `body` argument with `Value::as_str`, and the emitter binds the payload to a symbol
    /// precisely so flux stores it as canonical JSON *text* rather than handing over an object that
    /// would be dropped without a word.
    pub body: Option<String>,
}

impl Request {
    /// The params `http.request` is called with.
    ///
    /// `headers` and `body` are omitted when empty rather than sent as `{}`/`""`, so a request this
    /// pack builds is the same JSON a hand-written `http.request` call would carry.
    pub fn to_params(&self) -> Value {
        let mut params = serde_json::Map::new();
        params.insert("url".to_string(), Value::String(self.url.clone()));
        params.insert("method".to_string(), Value::String(self.method.clone()));
        if !self.headers.is_empty() {
            params.insert(
                "headers".to_string(),
                Value::Object(
                    self.headers
                        .iter()
                        .map(|(name, value)| (name.clone(), Value::String(value.clone())))
                        .collect(),
                ),
            );
        }
        if let Some(body) = &self.body {
            params.insert("body".to_string(), Value::String(body.clone()));
        }
        Value::Object(params)
    }
}

/// Build the request `declaration` makes when called with `params`.
///
/// # Errors
///
/// [`Error::MissingParameter`] when the caller omitted a parameter the operation declares, and
/// [`Error::Unbuildable`] when the body contains something this evaluator does not model or a
/// free-form body is not JSON. Both refuse; neither repairs.
pub(crate) fn build(
    operation: &str,
    declaration: &CompositeOpDecl,
    params: &Value,
) -> Result<Request, Error> {
    let mut env = Env::new();

    // Every declared parameter must be supplied, exactly as flux's own composite dispatch requires
    // — `execute_composite_call` fails with "missing required param" and `composite_signature` puts
    // every parameter in `required_params`. An *optional* parameter is one a caller may pass `null`
    // for, not one they may omit: the emitted `when` guard turns null into "do not send this
    // filter", while an absent path parameter would quietly leave `{ticket_id}` in the URL.
    for param in &declaration.params {
        let name = param.name.0.as_str();
        let value = params
            .get(name)
            .ok_or_else(|| Error::MissingParameter {
                operation: operation.to_owned(),
                parameter: name.to_owned(),
            })?
            .clone();
        env.insert(name.to_string(), value);
    }

    match run(operation, &declaration.body.body, &mut env)? {
        Some(request) => Ok(request),
        None => Err(Error::Unbuildable {
            operation: operation.to_owned(),
            message: format!("its body makes no `{HTTP_REQUEST}` call"),
        }),
    }
}

/// The symbols an op body has bound so far.
type Env = BTreeMap<String, Value>;

/// Run `nodes`, stopping at the `http.request` call.
fn run(operation: &str, nodes: &[Node], env: &mut Env) -> Result<Option<Request>, Error> {
    for node in nodes {
        match node {
            // `response = http.request(…)`: the request is the whole point of the body, so it ends
            // the walk. Nothing after it can change what is sent.
            Node::Bind { value, .. } if matches!(value.as_ref(), Node::Call { .. }) => {
                let Node::Call { op, args } = value.as_ref() else {
                    unreachable!("the guard above matched a call");
                };
                return request_of(operation, op, args, env).map(Some);
            }
            // A bare statement-position call. The emitter binds its response rather than discarding
            // it (C-9), so this is not a shape the catalogue carries — it is here because reading
            // the request off it is the same operation, and refusing it would be a refusal of
            // spelling rather than of substance.
            Node::Call { op, args } => return request_of(operation, op, args, env).map(Some),
            Node::Bind { name, value, .. } => {
                let value = eval(operation, value, env)?;
                env.insert(name.0.clone(), value);
            }
            Node::When {
                cond,
                then,
                otherwise,
            } => {
                let taken = if truthy(&eval(operation, cond, env)?) {
                    then
                } else {
                    otherwise
                };
                if let Some(request) = run(operation, taken, env)? {
                    return Ok(Some(request));
                }
            }
            // The emitted body ends `return $response`, which cannot be reached before the request
            // is built. Anything the walk has not modelled is refused rather than skipped.
            Node::Return { .. } => break,
            other => {
                return Err(Error::Unbuildable {
                    operation: operation.to_owned(),
                    message: format!(
                        "its body contains {}, which this pack does not evaluate",
                        kind(other)
                    ),
                })
            }
        }
    }
    Ok(None)
}

/// Read `{ method, url, headers, body }` off the `http.request` call the body ends in.
fn request_of(operation: &str, op: &str, args: &[Node], env: &Env) -> Result<Request, Error> {
    if op != HTTP_REQUEST {
        return Err(Error::Unbuildable {
            operation: operation.to_owned(),
            message: format!(
                "its body calls `{op}`, and this pack delegates only `{HTTP_REQUEST}`"
            ),
        });
    }
    let [Node::Obj { fields }] = args else {
        return Err(Error::Unbuildable {
            operation: operation.to_owned(),
            message: format!(
                "its `{HTTP_REQUEST}` call takes something other than one named record"
            ),
        });
    };

    let mut request = Request {
        // `http.request` itself defaults an absent method to GET; the emitter always states one, so
        // this default is the same answer arrived at twice rather than a guess.
        method: "GET".to_string(),
        url: String::new(),
        headers: BTreeMap::new(),
        body: None,
    };
    for (name, value) in fields {
        match name.as_str() {
            "url" => request.url = text(&eval(operation, value, env)?),
            "method" => request.method = text(&eval(operation, value, env)?),
            "body" => request.body = Some(text(&eval(operation, value, env)?)),
            "headers" => {
                let Node::Obj { fields } = value.as_ref() else {
                    return Err(Error::Unbuildable {
                        operation: operation.to_owned(),
                        message: "its request headers are not a record".to_string(),
                    });
                };
                for (header, value) in fields {
                    request
                        .headers
                        .insert(header.clone(), text(&eval(operation, value, env)?));
                }
            }
            other => {
                return Err(Error::Unbuildable {
                    operation: operation.to_owned(),
                    message: format!("its request names `{other}`, which this pack does not carry"),
                })
            }
        }
    }

    if request.url.is_empty() {
        return Err(Error::Unbuildable {
            operation: operation.to_owned(),
            message: "its request has no URL".to_string(),
        });
    }
    Ok(request)
}

/// Evaluate one pure value node.
fn eval(operation: &str, node: &Node, env: &Env) -> Result<Value, Error> {
    match node {
        Node::Lit { value } => Ok(value.clone()),
        Node::Var { name } => env.get(&name.0).cloned().ok_or_else(|| Error::Unbuildable {
            operation: operation.to_owned(),
            message: format!("its body reads `{}` before binding it", name.0),
        }),
        Node::Fmt { template } => Ok(Value::String(interpolate(template, env))),
        Node::Obj { fields } => {
            let mut object = serde_json::Map::new();
            for (name, value) in fields {
                object.insert(name.clone(), eval(operation, value, env)?);
            }
            Ok(Value::Object(object))
        }
        // `parse($body, as: "json")` is how a free-form body reaches the vendor at all: it
        // canonicalizes a record *and* validates a JSON string, so both spellings of "here is my
        // body" arrive intact. A string that is not JSON is refused here rather than sent.
        Node::Parse { value, as_type } if as_type == "json" => match eval(operation, value, env)? {
            Value::String(text) => {
                serde_json::from_str(&text).map_err(|source| Error::Unbuildable {
                    operation: operation.to_owned(),
                    message: format!("its body was supplied as text that is not JSON: {source}"),
                })
            }
            other => Ok(other),
        },
        other => Err(Error::Unbuildable {
            operation: operation.to_owned(),
            message: format!(
                "its body computes {}, which this pack does not evaluate",
                kind(other)
            ),
        }),
    }
}

/// flux-lang's `interpolate_str`, over the symbols an op body has bound.
///
/// An unbound `{name}` is left verbatim, braces included — the behaviour the emitter's `when` guards
/// exist because of, so reproducing it is what keeps a guarded filter genuinely unsent rather than
/// sent as the literal text `{page}`.
fn interpolate(template: &str, env: &Env) -> String {
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
        match env.get(inner[..close].trim()) {
            Some(value) => {
                out.push_str(&text(value));
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

/// flux-lang's `lit_text`: a string is itself, `null` is empty, anything else is compact JSON.
///
/// The `null` arm is the one with consequences — see this module's documentation.
fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// flux-lang's `json_truthy`: null/false/0/empty are falsey, and so is the *text* `"false"`/`"0"`.
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

/// A node's kind, for a refusal that names what it refused.
fn kind(node: &Node) -> &'static str {
    match node {
        Node::Call { .. } => "a call",
        Node::Bind { .. } => "a binding",
        Node::When { .. } => "a conditional",
        Node::Return { .. } => "a return",
        Node::Var { .. } => "a symbol",
        Node::Lit { .. } => "a literal",
        Node::Fmt { .. } => "an interpolation",
        Node::Obj { .. } => "a record",
        Node::List { .. } => "a list",
        Node::Parse { .. } => "a coercion",
        Node::Jq { .. } => "a path extraction",
        Node::Expr { .. } => "a formula",
        _ => "a statement",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn env(pairs: &[(&str, Value)]) -> Env {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), value.clone()))
            .collect()
    }

    #[test]
    fn an_unbound_placeholder_stays_verbatim() {
        assert_eq!(
            interpolate(
                "{base}/tickets/{id}.json",
                &env(&[("base", json!("https://x"))])
            ),
            "https://x/tickets/{id}.json"
        );
    }

    /// The `""`-is-absent idiom. `null` rendering as the text `null` would put `?page=null` on the
    /// wire in exactly the case a `when` guard exists to prevent.
    #[test]
    fn null_renders_as_nothing_rather_than_as_the_word() {
        assert_eq!(text(&Value::Null), "");
        assert_eq!(text(&json!(1)), "1");
        assert_eq!(text(&json!("x")), "x");
        assert_eq!(text(&json!({"a": 1})), r#"{"a":1}"#);
    }

    /// Truthiness is what makes an unsupplied filter unsent, so it must be flux's own rather than
    /// Rust's `Option`-shaped instinct.
    #[test]
    fn truthiness_is_fluxs_own() {
        assert!(!truthy(&Value::Null));
        assert!(!truthy(&json!("")));
        assert!(!truthy(&json!("false")));
        assert!(!truthy(&json!("0")));
        assert!(!truthy(&json!(0)));
        assert!(!truthy(&json!(false)));
        assert!(truthy(&json!("a")));
        assert!(truthy(&json!(1)));
    }

    /// An empty header map and an absent body are omitted rather than sent as `{}` and `""`.
    #[test]
    fn a_bare_request_carries_only_what_it_has() {
        let request = Request {
            method: "GET".to_string(),
            url: "https://x/y".to_string(),
            headers: BTreeMap::new(),
            body: None,
        };
        assert_eq!(
            request.to_params(),
            json!({"url": "https://x/y", "method": "GET"})
        );
    }
}
