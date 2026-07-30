//! **The request**, against real shipped operations.
//!
//! Every assertion here is on the request *before* it is sent, which is deliberate and is the
//! opposite of a compromise. The two mistakes this story can make are both ones a vendor answers
//! `200` to and then ignores: a body flattened out of the wire nesting the emitter honours
//! (`{"ticket.comment.body": …}` instead of `{"ticket": {"comment": {"body": …}}}`), and a query
//! string assembled without its `?`/`&` separators. A live call would prove neither, and a green
//! integration suite against a real vendor is exactly how both ship.
//!
//! **These requests are the unauthenticated ones**, and that is still true after C-116. Every
//! assertion below goes through `Operation::build_request`, which applies no credential — it is the
//! request the operation's own emitted module describes and nothing more. `build_authenticated_request`
//! is the one that resolves and places a credential, and `tests/credentials.rs` is where it is
//! followed. Keeping the two apart is what lets this file assert a header set *exactly*.

use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{Credentials, Egress, MemoryStore, Operation, Request};
use flux_runtime::Tool;
use serde_json::{json, Value};

/// A stand-in for flux's `http.request`. Nothing here reaches it — `execute` needs a real
/// `ToolContext`, which needs a `flux_system::System` over a real workspace root — but a projected
/// operation needs *a* transport, and taking one is the seam the story is about.
fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        },
        |params| async move { Ok(params) },
    ))
}

/// A bound credential port over an **empty** store (C-116).
///
/// The pack requires one; this file asserts the unauthenticated request, so it must hold nothing.
/// An empty store here is what keeps the header assertions below a statement about the *emitter*
/// rather than about whichever credential happened to resolve.
fn credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), "t-request").expect("a valid tenant id")
}

/// One shipped operation, projected.
fn projected(id: &str) -> Operation {
    let entry = catalog::operation(OperationKey::id(id))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"));
    Operation::project(entry, http(), credentials())
        .unwrap_or_else(|error| panic!("`{id}`: {error}"))
}

/// The request `id` makes when called with `params`.
fn request(id: &str, params: Value) -> Request {
    projected(id)
        .build_request(&params)
        .unwrap_or_else(|error| panic!("`{id}`: {error}"))
}

/// **A nested body nests.** `zendesk-ticket-comment-add` writes `ticket.comment.body`, and the flat
/// spelling is a request Zendesk accepts and silently ignores — the comment never appears. The
/// emitter assembles the wire paths into one record ([`connector_flux`]'s `body_tree`); the pack
/// must land the caller's values at the same paths, or the two surfaces of one operation make two
/// different calls.
#[test]
fn a_nested_body_operation_nests_rather_than_flattening() {
    let request = request(
        "zendesk-ticket-comment-add",
        json!({
            "ticket_id": 42,
            "updated_stamp": "2026-07-30T00:00:00Z",
            "body": "the comment text",
            "public": false,
        }),
    );

    assert_eq!(request.method, "PUT");
    assert_eq!(
        request.url,
        "https://{subdomain}.zendesk.com/api/v2/tickets/42.json"
    );

    let body: Value = serde_json::from_str(
        request
            .body
            .as_deref()
            .expect("a comment travels in a request body"),
    )
    .expect("the body is the JSON text `http.request` sends");

    assert_eq!(
        body,
        json!({
            "ticket": {
                "comment": {"body": "the comment text", "public": false},
                "safe_update": true,
                "updated_stamp": "2026-07-30T00:00:00Z",
            }
        }),
        "the wire paths must nest, and `ticket.safe_update` must be sent without being asked for"
    );

    // Not `{"ticket.comment.body": …}` — stated separately because it is the shape that would pass
    // a "the body mentions the text" assertion while being the wrong request.
    assert!(
        !request.body.as_deref().unwrap().contains("ticket.comment"),
        "a flattened dotted key is a request Zendesk accepts and ignores: {:?}",
        request.body
    );

    // Credentials are C-116. Asserting the whole header set rather than an absence keeps that
    // story's addition visible here.
    assert_eq!(
        request.headers.iter().collect::<Vec<_>>(),
        vec![(&"content-type".to_string(), &"application/json".to_string())]
    );
}

/// **A query string opens with `?` and continues with `&`.** `freshdesk-ticket-list` has four
/// optional filters and no required one, so the separator is carried by the emitted `$sep` symbol
/// and only the first *surviving* filter opens the query. Getting this wrong sends the vendor
/// `...tickets?requester_id=7?email=…`, which parses to one filter and drops the rest — answered
/// `200`, with a list that is simply not the list that was asked for.
#[test]
fn a_query_string_operation_separates_its_parameters() {
    // Every filter supplied: `?` then `&&&`.
    let all = request(
        "freshdesk-ticket-list",
        json!({
            "req_id": "7",
            "req_email": "a@b.c",
            "company_id": "9",
            "updated": "2026-07-30",
        }),
    );
    assert_eq!(
        all.url,
        "https://{domain}/api/v2/tickets?requester_id=7&email=a@b.c&company_id=9\
         &updated_since=2026-07-30"
    );
    assert_eq!(all.method, "GET");
    assert!(all.body.is_none(), "a listing sends no body");
    assert!(all.headers.is_empty(), "a listing sets no content type");

    // A middle filter only: it must be the one that opens the query with `?`, not the one that
    // inherits an `&` from a filter that was never sent.
    let one = request(
        "freshdesk-ticket-list",
        json!({
            "req_id": Value::Null,
            "req_email": Value::Null,
            "company_id": "9",
            "updated": Value::Null,
        }),
    );
    assert_eq!(one.url, "https://{domain}/api/v2/tickets?company_id=9");

    // No filter at all: no `?`, and no dangling separator.
    let none = request(
        "freshdesk-ticket-list",
        json!({
            "req_id": Value::Null,
            "req_email": Value::Null,
            "company_id": Value::Null,
            "updated": Value::Null,
        }),
    );
    assert_eq!(none.url, "https://{domain}/api/v2/tickets");
}

/// A required query parameter goes in the template and an optional one is guarded, so the two kinds
/// have to agree about who owns the `?`. `zendesk-ticket-search` is the shipped case with both.
#[test]
fn a_required_query_parameter_opens_the_string_and_optional_ones_follow() {
    let both = request(
        "zendesk-ticket-search",
        json!({"query": "type:ticket status:new", "page": 2, "per_page": 50}),
    );
    assert_eq!(
        both.url,
        "https://{subdomain}.zendesk.com/api/v2/search.json\
         ?query=type:ticket status:new&page=2&per_page=50"
    );

    let required_only = request(
        "zendesk-ticket-search",
        json!({"query": "type:ticket", "page": Value::Null, "per_page": Value::Null}),
    );
    assert_eq!(
        required_only.url,
        "https://{subdomain}.zendesk.com/api/v2/search.json?query=type:ticket"
    );
}

/// A free-form body reaches the vendor whole, whether the caller spells it as a record or as JSON
/// text. Both spellings are why the emitter re-binds it through `parse(…, as: "json")` rather than
/// passing it straight to `http.request`.
#[test]
fn a_free_form_body_travels_whole_in_either_spelling() {
    let as_record = request(
        "babelforce-call-session-set",
        json!({"id": "c-1", "body": {"appFoo": "bar"}}),
    );
    let as_text = request(
        "babelforce-call-session-set",
        json!({"id": "c-1", "body": "{\"appFoo\": \"bar\"}"}),
    );

    assert_eq!(as_record.body.as_deref(), Some(r#"{"appFoo":"bar"}"#));
    assert_eq!(as_record.body, as_text.body);
    assert_eq!(
        as_record.url,
        "https://services.babelforce.com/api/v2/calls/c-1/session/set"
    );
}

/// The params handed to `http.request` are the shape its own input schema declares — `url` and
/// `method` always, `headers` and `body` only when there are any.
#[test]
fn the_request_becomes_the_params_http_request_declares() {
    let show = request("zendesk-ticket-show", json!({"ticket_id": 7}));
    assert_eq!(
        show.to_params(),
        json!({
            "url": "https://{subdomain}.zendesk.com/api/v2/tickets/7.json",
            "method": "GET",
        })
    );

    let add = request(
        "zendesk-ticket-comment-add",
        json!({"ticket_id": 7, "updated_stamp": "s", "body": "b", "public": true}),
    );
    let params = add.to_params();
    assert_eq!(
        params["headers"],
        json!({"content-type": "application/json"})
    );
    assert!(
        params["body"].is_string(),
        "`http.request` reads `body` with `Value::as_str`, so an object would be dropped without a \
         word: {params}"
    );
}

/// **Every shipped operation builds a request**, not just the four this file names. A projection
/// that is right for Zendesk and refuses `google-calendar-calendar-get` is a pack that installs and
/// then cannot call half of what it advertised.
#[test]
fn every_shipped_operation_builds_an_absolute_request() {
    let mut built = 0usize;
    for entry in catalog::operations() {
        let operation = Operation::project(entry, http(), credentials())
            .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));

        let params = params_from_schema(&operation);
        let request = operation
            .build_request(&params)
            .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));

        assert!(
            request.url.starts_with("https://"),
            "`{}` builds `{}`, which is not an absolute https URL",
            entry.id,
            request.url
        );
        for host in entry.hosts {
            assert!(
                request.url.contains(host),
                "`{}` builds `{}`, which does not reach its declared `{host}`",
                entry.id,
                request.url
            );
        }
        assert!(
            !request.method.is_empty(),
            "`{}` builds a request with no method",
            entry.id
        );
        built += 1;
    }
    assert!(built > 0, "an empty catalogue would pass the loop above");
}

/// A plausible value for every parameter an operation declares, from its own input schema.
fn params_from_schema(operation: &Operation) -> Value {
    let spec = operation.spec();
    let mut params = serde_json::Map::new();
    if let Some(properties) = spec
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    {
        for (name, schema) in properties {
            let value = match schema.get("type").and_then(Value::as_str) {
                Some("number") | Some("integer") => json!(1),
                Some("boolean") => json!(true),
                Some("array") => json!([]),
                Some("object") => json!({}),
                Some(_) => Value::String(format!("a-{name}")),
                // An untyped schema is a free-form body (`Any`), which travels through
                // `parse(…, as: "json")` — a bare string is not JSON and would be refused.
                None => json!({}),
            };
            params.insert(name.clone(), value);
        }
    }
    Value::Object(params)
}
