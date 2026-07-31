//! Linear (C-110) is the fleet's only **GraphQL** vendor, and this file is the record of what that
//! cost. The story asked one question — can a connector describe a vendor with *one endpoint and a
//! query language*? — and allowed a documented refusal as a first-class answer.
//!
//! **The answer is yes, and `providers/linear.toml` ships.** Four properties a GraphQL vendor
//! needs turned out to be expressible with mechanisms this repository already had, and two are
//! genuinely not. This file asserts all six rather than describing them, because the two that are
//! not expressible are the ones a later reader will otherwise assume were never checked.
//!
//! # What works, and why
//!
//! 1. **The path stops identifying the operation, and nothing cared.** All eight operations are
//!    `POST /graphql`. Operation identity in this repository is `id` and only `id` — the uniqueness
//!    pass in `connector_spec::provider` is over ids, `catalog::Operation` carries no `path` field
//!    at all, and `providers/zendesk.toml` has shipped three operations on one `PUT` path since
//!    long before this connector. `every_operation_is_one_post_to_one_graphql_endpoint` pins it.
//! 2. **The query document is a vendor constant, and C-55's mechanism already covered it.** The
//!    story's note asked whether `crates/connector-flux/src/op.rs`'s `constant` — a body field
//!    pinned with a JSON Schema `const`, sent on every call and never declared as a parameter —
//!    actually covers a GraphQL query or merely resembles the case. It covers it: `constant` is a
//!    bare `schema.get("const")` with no type, length or newline restriction, and the emitter
//!    filters constants out of the op's parameter list so no caller and no model can choose one.
//!    `the_query_document_is_pinned_and_no_caller_can_choose_it` pins both halves.
//! 3. **A multi-line query document survives the emitter.** flux-lang formats a newline-bearing
//!    string literal as a verbatim `"""…"""` block, and the CST formatter treats such a block as a
//!    single token it never reaches into. No provider had exercised that path before this one.
//!    `a_multiline_query_document_round_trips_through_the_emitter` is the first test that does.
//! 4. **The `data.<field>` envelope is declarable as a shape.** Every response schema here is
//!    `{data: {<fieldName>: …}}`, which is what the vendor actually sends.
//!    `every_response_schema_is_nested_under_data` pins it.
//!
//! # What does not work, and is not papered over
//!
//! 5. **Every operation is forced to `risk >= medium` and `non_idempotent`, including the reads.**
//!    `check_write_metadata` derives write-ness from the HTTP verb, so a `POST` may not declare
//!    `risk = "low"` or `idempotency = "idempotent"`. That rule is right for REST and it is the
//!    single most load-bearing safety check in the emitter — but under GraphQL *every* operation is
//!    a `POST`, so it applies to `linear-viewer` and `linear-issue-archive` alike. The floor of the
//!    risk axis rises from `low` to `medium` for the whole connector, and `idempotency` carries no
//!    authored information at all: no operation here could have said anything else.
//!
//!    The direction matters and is why this connector still ships: the forced value is
//!    **conservative**. A read is over-stated as `medium`, never a write under-stated as `low`, so
//!    a host's approval gate errs toward asking. Gradation above the floor survives — the reads are
//!    `medium`, `linear-issue-archive` is `destructive` — so the axis is compressed, not erased.
//!    `a_graphql_read_cannot_declare_itself_low_risk` and
//!    `a_graphql_read_cannot_declare_itself_idempotent` pin the boundary by constructing exactly
//!    the declaration a REST author would have written and asserting the emitter refuses it.
//!
//! 6. **A failed Linear call arrives as HTTP 200, and nothing in this repository can say so.** This
//!    is [C-57]'s exact case and it is the safety-relevant finding of this story. Linear answers a
//!    validation error, a permission denial and an unknown field alike with `200` and an `errors`
//!    array beside a `null` `data`. This repository's success signal is the transport's: flux-web
//!    hardcodes `is_error: false` for any completed request, the emitted Flux asserts nothing on
//!    status, and `ErrorEnvelope` has no success predicate — its own documentation scopes it to a
//!    *non-2xx* body. So **every Linear failure reads as a success** to anything switching on
//!    status.
//!
//!    The consequence for this file is concrete and is the reason it asserts a *negative*: this
//!    connector declares **no** `[operations.quirks.error_envelope]`, unlike every REST connector in
//!    the fleet. Declaring one would make `description()` append the sentence "A non-2xx response is
//!    returned as data, not a failure…" to the contract a model reads — and for Linear that sentence
//!    is not merely unhelpful, it is false, and it points a model at the wrong branch. The choice is
//!    between no machine-readable envelope and a false statement of one.
//!    `no_operation_declares_an_error_envelope_because_the_prose_it_emits_would_be_false` pins the
//!    decision together with the reason, so that closing C-57 makes this test fail loudly and
//!    demand revisiting rather than leaving it quietly stale.
//!
//! [C-57]: ../../../docs/stories/C-57-quirks-beyond-http-shape.md

use std::path::{Path, PathBuf};

use connector_flux::{emit_operation, parameter_symbols};
use connector_spec::{provider, Connector, HttpMethod, Idempotency, Operation, Risk};

/// `<repo root>/providers/linear.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("linear.toml")
}

fn linear() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-110 ships the Linear connector",
            path.display()
        )
    });
    provider::load("providers/linear.toml", &source)
        .expect("providers/linear.toml does not load")
        .connector
}

fn op<'a>(connector: &'a Connector, id: &str) -> &'a Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("no operation named {id:?} in the curated set"))
}

fn emit(connector: &Connector, id: &str) -> String {
    emit_operation(connector, op(connector, id))
        .unwrap_or_else(|error| panic!("`{id}` is not emittable: {error}"))
}

/// The constant query document pinned on an operation's `query` body field.
fn query_document(operation: &Operation) -> &str {
    let param = operation
        .params
        .body
        .iter()
        .find(|param| param.name == "query")
        .unwrap_or_else(|| panic!("`{}` declares no `query` body field", operation.id));
    param
        .schema
        .get("const")
        .unwrap_or_else(|| panic!("`{}`'s `query` field is not pinned", operation.id))
        .as_str()
        .unwrap_or_else(|| panic!("`{}`'s pinned query is not a string", operation.id))
}

// -------------------------------------------------------------------------------------------
// 1. The path stops identifying the operation.
// -------------------------------------------------------------------------------------------

/// **Every operation is `POST /graphql`** — the property that makes this connector the probe it is.
///
/// A REST connector's operations are distinguished by method and path. Here they are distinguished
/// by nothing on the wire except the body, so this asserts the *collision* rather than merely
/// tolerating it: more than one operation shares the single endpoint, which is exactly the shape a
/// path-keyed pipeline would have refused.
#[test]
fn every_operation_is_one_post_to_one_graphql_endpoint() {
    let connector = linear();
    assert!(
        connector.operations.len() > 1,
        "the probe is meaningless with fewer than two operations on the shared endpoint"
    );
    for operation in &connector.operations {
        assert_eq!(
            operation.method,
            HttpMethod::Post,
            "`{}` is not a POST — every GraphQL operation is",
            operation.id
        );
        assert_eq!(
            operation.path, "/graphql",
            "`{}` does not address the single GraphQL endpoint",
            operation.id
        );
    }
}

// -------------------------------------------------------------------------------------------
// 2 & 3. The query document is a vendor constant the caller never chooses.
// -------------------------------------------------------------------------------------------

/// **The query is pinned with a JSON Schema `const`, and the emitter refuses to let a caller pick
/// it.** This is the assertion the story's third acceptance item is about: the tempting wrong shape
/// is one `linear-graphql(query)` operation taking an arbitrary query string, which hands a model a
/// language instead of an operation. Both halves are checked — the document is a constant, and the
/// emitted op's parameter list does not contain it.
#[test]
fn the_query_document_is_pinned_and_no_caller_can_choose_it() {
    let connector = linear();
    for operation in &connector.operations {
        let document = query_document(operation);
        assert!(
            document.contains('{'),
            "`{}`'s pinned value is not a GraphQL document",
            operation.id
        );

        let symbols = parameter_symbols(operation)
            .unwrap_or_else(|error| panic!("`{}` has no parameter list: {error}", operation.id));
        assert!(
            !symbols.contains_key("query"),
            "`{}` exposes `query` as a parameter — a caller could author the document, which is \
             the remote-expression-evaluator shape C-110 exists to refuse",
            operation.id
        );

        // The document must nevertheless reach the wire: it is *sent* on every call, bound as a
        // literal in the emitted body rather than declared in the signature.
        let flux = emit(&connector, &operation.id);
        assert!(
            flux.contains("query ="),
            "`{}` does not bind its pinned query document in the emitted body",
            operation.id
        );
    }
}

/// **A multi-line query document survives the emitter.** No provider in the fleet had pinned a
/// `const` string containing a newline before this one, so the `"""…"""` path through flux-lang's
/// formatter was carrying this connector without ever having been exercised here.
///
/// The assertion is deliberately about the *emitted module*, not about the string: a query document
/// that arrived escaped onto one line would still be correct GraphQL, but it would be unreadable in
/// a rendering a human reviews, and the CST formatter's guarantee that it never reaches into such a
/// block is what this connector relies on.
#[test]
fn a_multiline_query_document_round_trips_through_the_emitter() {
    let connector = linear();
    let multiline: Vec<&Operation> = connector
        .operations
        .iter()
        .filter(|operation| query_document(operation).contains('\n'))
        .collect();
    assert!(
        !multiline.is_empty(),
        "no operation pins a multi-line document, so this connector no longer exercises the \
         `\"\"\"` path it was the first to reach"
    );

    for operation in multiline {
        let flux = emit(&connector, &operation.id);
        assert!(
            flux.contains("\"\"\""),
            "`{}`'s multi-line query document was not emitted as a `\"\"\"` block",
            operation.id
        );
        // The document's own text must survive verbatim — no escaping, no re-indentation.
        let document = query_document(operation);
        assert!(
            flux.contains(document),
            "`{}`'s query document was altered on the way into the module",
            operation.id
        );
    }
}

/// **The variables are typed parameters, not a blob.** Each caller-supplied value is a body field
/// under `variables.<name>`, so the operation keeps a real signature. This is the other half of the
/// story's "real typed signature" requirement.
#[test]
fn the_variables_are_typed_parameters_under_the_variables_key() {
    let connector = linear();
    let mut with_variables = 0;
    for operation in &connector.operations {
        for param in &operation.params.body {
            if param.name == "query" {
                continue;
            }
            let wire = param.wire.as_deref().unwrap_or_else(|| {
                panic!(
                    "`{}`'s `{}` declares no wire path",
                    operation.id, param.name
                )
            });
            assert!(
                wire.starts_with("variables."),
                "`{}`'s `{}` is not sent under `variables.` — GraphQL takes its arguments there",
                operation.id,
                param.name
            );
            assert!(
                param.schema.get("type").is_some(),
                "`{}`'s `{}` is untyped; a GraphQL variable has a declared type and so must its \
                 parameter",
                operation.id,
                param.name
            );
            with_variables += 1;
        }
    }
    assert!(
        with_variables > 0,
        "no operation takes a variable, so nothing here is a typed signature"
    );
}

// -------------------------------------------------------------------------------------------
// 4. The `data.<field>` envelope.
// -------------------------------------------------------------------------------------------

/// **Every response schema describes the `data.<field>` envelope the vendor actually sends.**
///
/// This is the surface the story called the weak point and expected to be *stronger* than REST,
/// since the query is fixed at build time: the shape under `data` is not a guess about what the
/// vendor might return, it is a consequence of the document pinned three assertions above.
#[test]
fn every_response_schema_is_nested_under_data() {
    let connector = linear();
    for operation in &connector.operations {
        let schema = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("`{}` publishes no response schema", operation.id));
        let data = schema
            .get("properties")
            .and_then(|properties| properties.get("data"))
            .unwrap_or_else(|| {
                panic!(
                    "`{}`'s response schema has no `data` envelope",
                    operation.id
                )
            });
        let fields = data
            .get("properties")
            .and_then(|properties| properties.as_object())
            .unwrap_or_else(|| panic!("`{}`'s `data` names no field", operation.id));
        assert_eq!(
            fields.len(),
            1,
            "`{}`'s `data` should carry exactly the one field its pinned document selects",
            operation.id
        );

        // The field under `data` is the GraphQL root field the pinned document selects, so the two
        // must agree. This is what makes the response schema a consequence of the query rather than
        // a hand-written guess beside it.
        let field = fields.keys().next().expect("one field");
        assert!(
            query_document(operation).contains(field.as_str()),
            "`{}` publishes `data.{field}` but its pinned document does not select it",
            operation.id
        );
    }
}

// -------------------------------------------------------------------------------------------
// 5. The boundary: a GraphQL read cannot declare a read's metadata.
// -------------------------------------------------------------------------------------------

/// **A GraphQL read may not declare `risk = "low"`.** The finding, pinned by construction: take a
/// real read from this connector, declare it the way its REST equivalent would be declared, and
/// watch the emitter refuse.
///
/// `check_write_metadata` reads the HTTP verb, and under GraphQL the verb is `POST` for everything.
/// The rule is not wrong — it is what stops a write being waved through an approval gate — but its
/// premise, that the verb distinguishes reads from writes, is a REST premise that GraphQL does not
/// satisfy. The cost is stated in this connector's header and in this file's own: the risk floor
/// rises to `medium` for reads that genuinely are `low`.
#[test]
fn a_graphql_read_cannot_declare_itself_low_risk() {
    let connector = linear();
    let mut read = op(&connector, "linear-viewer").clone();
    assert_eq!(
        read.risk,
        Risk::Medium,
        "the shipped declaration should be at the floor this test explains"
    );

    read.risk = Risk::Low;
    let error = emit_operation(&connector, &read)
        .expect_err("a POST declared `low` must be refused, however plainly it reads");
    let message = error.to_string();
    assert!(
        message.contains("may not declare `risk = \"low\"`"),
        "refused for the wrong reason: {message}"
    );
}

/// **A GraphQL read may not declare itself idempotent**, for the same reason and with a sharper
/// consequence: `idempotency` is what tells flux whether a `retry` around the call is sound. A
/// GraphQL query *is* idempotent, and no operation in this connector can say so, so the field
/// carries no authored information anywhere in the file — every value is `non_idempotent` because
/// every other value is unreachable.
#[test]
fn a_graphql_read_cannot_declare_itself_idempotent() {
    let connector = linear();
    let mut read = op(&connector, "linear-viewer").clone();
    read.idempotency = Idempotency::Idempotent;
    let error =
        emit_operation(&connector, &read).expect_err("a POST declared idempotent must be refused");
    let message = error.to_string();
    assert!(
        message.contains("may not declare `idempotency = \"idempotent\"`"),
        "refused for the wrong reason: {message}"
    );

    // And the consequence, asserted over the shipped file rather than inferred: the axis is flat.
    for operation in &connector.operations {
        assert_eq!(
            operation.idempotency,
            Idempotency::NonIdempotent,
            "`{}` carries an idempotency this connector cannot actually have chosen",
            operation.id
        );
    }
}

/// The risk axis is **compressed, not erased** — which is why the connector ships rather than being
/// refused. `medium` is a floor forced on the reads, and the writes are still graded above it.
#[test]
fn the_risk_axis_is_compressed_to_a_floor_but_still_grades_the_writes() {
    let connector = linear();
    assert!(
        connector
            .operations
            .iter()
            .all(|operation| operation.risk != Risk::Low),
        "a `low` operation cannot exist here; if one does, `check_write_metadata` changed"
    );
    assert!(
        connector
            .operations
            .iter()
            .any(|operation| operation.risk == Risk::Medium),
        "the reads sit at the forced floor"
    );
    assert!(
        connector
            .operations
            .iter()
            .any(|operation| operation.risk == Risk::Destructive),
        "gradation above the floor is what makes the axis still worth reading"
    );
}

// -------------------------------------------------------------------------------------------
// 6. The safety finding: a failure arrives as HTTP 200.
// -------------------------------------------------------------------------------------------

/// **No operation declares an error envelope, because the prose that declaring one emits would be
/// false.** See this file's header, item 6, and [C-57].
///
/// Linear signals every failure with `200` and an `errors` array. `ErrorEnvelope` cannot say that —
/// it has no success predicate and its own documentation scopes it to a non-2xx body — and
/// `connector_flux`'s `description()` appends "A non-2xx response is returned as data, not a
/// failure…" to any operation that declares one. For Linear that sentence points a model at a
/// branch that never occurs.
///
/// This test asserts the *negative* together with the reason it exists, by reproducing the false
/// sentence that would ship. When C-57 lands a success predicate, this test fails and demands the
/// envelope be declared properly rather than remaining absent by inertia.
///
/// [C-57]: ../../../docs/stories/C-57-quirks-beyond-http-shape.md
#[test]
fn no_operation_declares_an_error_envelope_because_the_prose_it_emits_would_be_false() {
    let connector = linear();
    for operation in &connector.operations {
        assert!(
            operation.quirks.error_envelope.is_none(),
            "`{}` declares an error envelope; under GraphQL that emits a claim about non-2xx \
             responses that never occur. See C-57",
            operation.id
        );
    }

    // The claim above, demonstrated rather than asserted from memory: give one operation the
    // envelope a REST author would have written and read what reaches the model's contract.
    let mut with_envelope = op(&connector, "linear-viewer").clone();
    with_envelope.quirks.error_envelope = Some(connector_spec::ErrorEnvelope {
        message_pointer: "/errors/0/message".to_string(),
        code_pointer: None,
    });
    let flux = emit_operation(&connector, &with_envelope)
        .expect("declaring an envelope is legal — that is the problem");
    assert!(
        flux.contains("A non-2xx response is returned as data"),
        "the emitter no longer states the non-2xx claim; if C-57 landed, this connector's \
         envelope decision must be revisited rather than left absent"
    );
}

// -------------------------------------------------------------------------------------------
// The connector's own shape: auth, config, verify.
// -------------------------------------------------------------------------------------------

/// Auth is a single bearer API key, there is a `[[config]]` surface for it, and `verify` names a
/// read — the story's fourth acceptance item.
///
/// `verify` is the one place risk is checked *without* reference to the HTTP method: the loader
/// refuses a `high` or `destructive` verify on the operation's own declared risk. That is what lets
/// a `POST /graphql` be a "Test connection" button at all, and it is worth pinning, because it is
/// the single rule in the repository that already read risk the way GraphQL needs it read.
#[test]
fn the_connector_authenticates_with_one_bearer_key_and_verifies_with_a_read() {
    let connector = linear();

    assert_eq!(connector.auth.len(), 1, "one credential, one alternative");
    let credential = &connector.auth[0];
    assert_eq!(credential.name, "linear.api_key");

    // The authority is the `<authority>` segment of a credential path and is **permanent** — a
    // different spelling mints a new address under which no tenant has provisioned anything. It is
    // pinned here rather than only by the fleet-wide sweep because the fleet-wide test can only say
    // "some provider declares none"; this one says which string it must be. C-110 shipped without it
    // on the first pass and the connector could not authenticate at all.
    assert_eq!(
        connector.authority.as_deref(),
        Some("app.linear.api"),
        "the credential address changed; every provisioned Linear secret is under the old one"
    );

    let field = connector
        .config
        .iter()
        .find(|field| field.binds == "credential.linear.api_key")
        .expect("no config field binds the API key — an operator would have nothing to fill in");
    assert!(field.secret, "the API key is a secret");
    assert!(
        !field.label.is_empty() && !field.help.is_empty(),
        "a config field must be renderable"
    );

    let verify = connector.verify.as_deref().expect("no verify operation");
    let verify_op = op(&connector, verify);
    assert!(
        verify_op.risk != Risk::High && verify_op.risk != Risk::Destructive,
        "a Test-connection button that could do damage is a button nobody presses"
    );
    assert!(
        parameter_symbols(verify_op).expect("signature").is_empty(),
        "`{verify}` takes an argument, so it is not a button"
    );
}

/// Every operation emits analyzable Flux. The fleet-wide checks cover this too, but a per-provider
/// contract test that never emits would pass while the connector was broken.
#[test]
fn every_operation_emits() {
    let connector = linear();
    for operation in &connector.operations {
        let flux = emit(&connector, &operation.id);
        assert!(
            flux.contains(&format!("op {}", operation.id)),
            "`{}` did not emit its own declaration",
            operation.id
        );
    }
}
