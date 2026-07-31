//! Linear (C-110) is the fleet's GraphQL probe, and **it ships no `providers/linear.toml`**. This
//! file is the record of why, in the shape [C-164] used for Algolia: a fixture, the real loader and
//! the real emitter, asserting each boundary rather than describing it.
//!
//! The story allowed either outcome — "ship the Linear connector, **or** record why a GraphQL vendor
//! cannot be one" — and the honest answer turned out to be the second, for a reason that is not
//! visible at this layer at all. A complete eight-operation connector was written, emitted, and
//! passed every check in this crate. It could not make a single call.
//!
//! # The blocking finding is in the host library, not in the compiler
//!
//! `connector-pack` derives an operation's configuration variables by scanning **every string
//! literal in the emitted body for `{…}`** (`crates/connector-pack/src/request.rs`,
//! `endpoint_variables` / `scan_node`). That is sound for the two kinds of brace-bearing literal the
//! shipped catalogue has, and that module's own documentation names them: templated base URLs, and
//! C-187's pin binds. In both, a brace really does name a value a host supplies.
//!
//! **A GraphQL query document is a third kind.** It is a string literal, it is full of braces, and
//! not one of them is configuration — they are the vendor's own selection-set syntax. So the pack
//! reads `{ viewer { … } }` as a configuration placeholder, and both outcomes are wrong:
//!
//! - **Unconfigured** — the production shape, since no `[[config]]` field could sensibly declare one
//!   — every operation refuses before assembling anything, naming a "variable" that is a fragment of
//!   GraphQL.
//! - **Configured**, the document is *rewritten*: the brace run is replaced by the host's value, so
//!   the constant query a caller must not choose is chosen after all, by whoever supplies the
//!   tenant's settings. That falsifies the single property this connector existed to demonstrate.
//!
//! Both halves are measured in `crates/connector-pack/src/request.rs`'s own tests —
//! `a_graphql_document_in_a_literal_is_read_as_configuration_variables` and
//! `a_graphql_operation_cannot_be_called_and_is_corrupted_when_it_is`. They are there rather than
//! here because this crate cannot depend on the host library, and because the defect is the pack's.
//!
//! **This is not a reason to weaken the pack.** The scan is a stand-in for a published configuration
//! surface, which is [C-87] — "the pack's request is the module's request by construction", adopted
//! because the catalogue does not yet carry an operation's configuration variables. When it does,
//! the pack reads them instead of inferring them from syntax, and a brace inside a vendor constant
//! stops being anybody's business. The full argument is `docs/designs/graphql-vendors.md`.
//!
//! # What this probe established that a later attempt should not re-derive
//!
//! Four of the six things a GraphQL vendor needs are already expressible, three with no new
//! mechanism at all, and the tests below pin all four so the next attempt starts from here:
//!
//! 1. **Nothing keys an operation by its path**
//!    (`two_operations_share_one_endpoint_and_nothing_objects`).
//! 2. **C-55's constant body field genuinely covers a query document**
//!    (`the_query_document_is_pinned_and_absent_from_the_signature`) — the specific thing the
//!    story's note asked to be checked. It is a bare `schema.get("const")`: no type, length or
//!    newline restriction.
//! 3. **A multi-line document survives the emitter** as a verbatim `"""…"""` block
//!    (`a_multiline_query_document_round_trips_through_the_emitter`).
//! 4. **The `data.<field>` envelope is declarable**
//!    (`a_response_schema_can_describe_the_data_envelope`).
//!
//! And two are not, both on the safety axes:
//!
//! 5. **Every operation is forced to `risk >= medium` and `non_idempotent`, reads included**
//!    (`a_graphql_read_cannot_declare_itself_low_risk`, `..._idempotent`). `check_write_metadata`
//!    derives write-ness from the verb, and under GraphQL the verb is always `POST`. This one is
//!    *conservative* — a read over-stated, never a write under-stated — and would not on its own
//!    have withdrawn the connector.
//! 6. **A failed call arrives as HTTP 200 and nothing here can say so.** [C-57]'s exact case:
//!    Linear answers a validation error, a permission denial and an expired key alike with `200`, a
//!    `null` `data` and an `errors` array, and this repository's success signal is the transport's.
//!    `an_error_envelope_would_emit_a_false_claim_about_non_2xx_responses` pins the sharp end — a
//!    declared `error_envelope` makes `description()` append a sentence that is *false* for a
//!    GraphQL vendor, and the envelope reaches no artifact anyway, so there is no machine-readable
//!    compensation for leaving it out.
//!
//! [C-57]: ../../../docs/stories/C-57-quirks-beyond-http-shape.md
//! [C-87]: ../../../docs/stories/C-87-configuration-codegen.md
//! [C-164]: ../../../docs/stories/C-164-provider-algolia.md

use connector_flux::{emit_operation, parameter_symbols};
use connector_spec::{
    provider, Connector, ErrorEnvelope, HttpMethod, Idempotency, Operation, Risk,
};

/// A minimal but *real* GraphQL connector: one endpoint, two operations, a pinned query document
/// each, and typed variables. It is the withdrawn `providers/linear.toml` reduced to the parts under
/// test.
///
/// It is a fixture rather than a shipped file because shipping it is precisely what this story
/// refuses. Everything below goes through the real loader and the real emitter, so a change to
/// either surfaces here rather than in a comment that has quietly gone stale.
fn fixture() -> String {
    r#"
id = "linear"
vendor = "Linear"
base_url = "https://api.linear.app"
authority = "app.linear.api"
description = "GraphQL probe fixture for C-110"
verify = "linear-viewer"

default_auth = [{ credentials = ["linear.api_key"] }]

[[auth]]
name = "linear.api_key"
scheme = "bearer"
env = ["LINEAR_API_KEY"]
description = "Linear personal API key, for the probe fixture only"

[[config]]
name = "api_key"
label = "Linear API key"
help = "For the probe fixture only"
format = "token"
secret = true
binds = "credential.linear.api_key"

[[operations]]
id = "linear-viewer"
method = "POST"
path = "/graphql"
description = "Read the user this key belongs to"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "query"
required = true
description = "The pinned GraphQL document. Constant, not caller-supplied"

[operations.params.body.schema]
type = "string"
const = """
query Viewer {
  viewer {
    id
    name
    displayName
    email
    admin
  }
}
"""

[operations.response_schema]
type = "object"
required = ["data"]
properties = { data = { type = "object", properties = { viewer = { type = "object", properties = { id = { type = "string" }, name = { type = "string" } } } } } }

[[operations]]
id = "linear-issue-get"
method = "POST"
path = "/graphql"
description = "Read one issue by id"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "query"
required = true
description = "The pinned GraphQL document. Constant, not caller-supplied"

[operations.params.body.schema]
type = "string"
const = """
query Issue($id: String!) {
  issue(id: $id) {
    id
    identifier
    title
  }
}
"""

[[operations.params.body]]
name = "id"
wire = "variables.id"
required = true
description = "The issue's UUID or its human identifier such as ENG-42"

[operations.params.body.schema]
type = "string"
minLength = 1

[operations.response_schema]
type = "object"
required = ["data"]
properties = { data = { type = "object", properties = { issue = { type = "object", properties = { id = { type = "string" }, identifier = { type = "string" }, title = { type = "string" } } } } } }
"#
    .to_string()
}

fn linear() -> Connector {
    provider::load("providers/linear.toml", &fixture())
        .expect("the C-110 fixture does not load")
        .connector
}

fn op<'a>(connector: &'a Connector, id: &str) -> &'a Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("no operation named {id:?} in the fixture"))
}

fn emit(connector: &Connector, id: &str) -> String {
    emit_operation(connector, op(connector, id))
        .unwrap_or_else(|error| panic!("`{id}` is not emittable: {error}"))
}

fn query_document(operation: &Operation) -> &str {
    operation
        .params
        .body
        .iter()
        .find(|param| param.name == "query")
        .and_then(|param| param.schema.get("const"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("`{}` pins no query document", operation.id))
}

// -------------------------------------------------------------------------------------------
// The four that work. A later attempt starts from these rather than re-deriving them.
// -------------------------------------------------------------------------------------------

/// **The path stops identifying the operation, and nothing objects.** Operation identity in this
/// repository is `id` and only `id`: the uniqueness pass is over ids and `catalog::Operation` has no
/// `path` field at all. `providers/zendesk.toml` has shipped three operations on one `PUT` path
/// since long before this probe, so two on one `POST` path is a difference of degree.
///
/// This was the finding most likely to sink the story, and it is a non-event.
#[test]
fn two_operations_share_one_endpoint_and_nothing_objects() {
    let connector = linear();
    assert_eq!(connector.operations.len(), 2);
    for operation in &connector.operations {
        assert_eq!(operation.method, HttpMethod::Post);
        assert_eq!(operation.path, "/graphql");
        emit(&connector, &operation.id);
    }
}

/// **C-55's constant body field covers a query document** rather than merely resembling the case,
/// which is what the story's note asked to be checked. `connector-flux`'s `constant` is a bare
/// `schema.get("const")` — no type, length or newline restriction — and the emitter filters
/// constants out of the operation's signature, so the document is *sent on every call and
/// declarable by nobody*.
///
/// This is the property that makes a GraphQL connector typed rather than a remote expression
/// evaluator, and it is the story's third acceptance item. It holds at this layer. What it does not
/// survive is `connector-pack`'s configuration substitution — see this file's header.
#[test]
fn the_query_document_is_pinned_and_absent_from_the_signature() {
    let connector = linear();
    for operation in &connector.operations {
        let symbols = parameter_symbols(operation).expect("a parameter list");
        assert!(
            !symbols.contains_key("query"),
            "`{}` exposes `query` as a parameter, which hands a model a language instead of an \
             operation",
            operation.id
        );
        let flux = emit(&connector, &operation.id);
        assert!(
            flux.contains("query ="),
            "`{}` does not bind its pinned document in the emitted body",
            operation.id
        );
    }

    // The variables, by contrast, *are* the signature.
    let symbols = parameter_symbols(op(&connector, "linear-issue-get")).expect("a parameter list");
    assert!(
        symbols.contains_key("id"),
        "the GraphQL variable is typed and declared"
    );
}

/// **A multi-line document survives the emitter** as a verbatim `"""…"""` block. No provider in the
/// fleet had pinned a `const` string containing a newline before this probe, so the path was
/// carrying a hypothetical connector without ever having been exercised.
#[test]
fn a_multiline_query_document_round_trips_through_the_emitter() {
    let connector = linear();
    let operation = op(&connector, "linear-viewer");
    let document = query_document(operation);
    assert!(
        document.contains('\n'),
        "the fixture's document is not multi-line"
    );

    let flux = emit(&connector, "linear-viewer");
    assert!(
        flux.contains("\"\"\""),
        "the document was not emitted as a `\"\"\"` block: {flux}"
    );
    assert!(
        flux.contains(document),
        "the document was altered on the way into the module: {flux}"
    );
}

/// **The `data.<field>` envelope is declarable**, and is *stronger* than a REST response schema
/// exactly as the story's Notes hoped: the shape under `data` is a consequence of the document
/// pinned beside it rather than a guess about what the vendor might return.
#[test]
fn a_response_schema_can_describe_the_data_envelope() {
    let connector = linear();
    for operation in &connector.operations {
        let schema = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("`{}` publishes no response schema", operation.id));
        let fields = schema
            .get("properties")
            .and_then(|properties| properties.get("data"))
            .and_then(|data| data.get("properties"))
            .and_then(serde_json::Value::as_object)
            .unwrap_or_else(|| panic!("`{}` describes no `data` envelope", operation.id));
        let field = fields.keys().next().expect("one root field");
        assert!(
            query_document(operation).contains(field.as_str()),
            "`{}` publishes `data.{field}` but its pinned document does not select it",
            operation.id
        );
    }
}

// -------------------------------------------------------------------------------------------
// The two safety axes that do not work. Conservative, and not on their own disqualifying.
// -------------------------------------------------------------------------------------------

/// **A GraphQL read may not declare `risk = "low"`.** `check_write_metadata` derives write-ness from
/// the HTTP verb, and under GraphQL the verb is `POST` for everything — so the rule that keeps a
/// REST write out of an auto-approving gate also catches every GraphQL read.
///
/// The rule is not wrong and should not be weakened; its *premise*, that the verb distinguishes a
/// read from a write, is a REST premise GraphQL does not satisfy. The effect is a risk floor of
/// `medium` for a whole connector — conservative, since it over-states a read rather than
/// under-stating a write.
#[test]
fn a_graphql_read_cannot_declare_itself_low_risk() {
    let connector = linear();
    let mut read = op(&connector, "linear-viewer").clone();
    read.risk = Risk::Low;
    let error = emit_operation(&connector, &read).expect_err("a POST declared `low` is refused");
    assert!(
        error
            .to_string()
            .contains("may not declare `risk = \"low\"`"),
        "refused for the wrong reason: {error}"
    );
}

/// **A GraphQL read may not declare itself idempotent**, with the sharper consequence: `idempotency`
/// is what tells flux whether a `retry` around the call is sound. A GraphQL *query* is idempotent
/// and no operation can say so, so across a GraphQL connector the field carries no authored
/// information — every value is `non_idempotent` because every other value is unreachable.
#[test]
fn a_graphql_read_cannot_declare_itself_idempotent() {
    let connector = linear();
    let mut read = op(&connector, "linear-viewer").clone();
    read.idempotency = Idempotency::Idempotent;
    let error =
        emit_operation(&connector, &read).expect_err("a POST declared idempotent is refused");
    assert!(
        error
            .to_string()
            .contains("may not declare `idempotency = \"idempotent\"`"),
        "refused for the wrong reason: {error}"
    );
}

/// **Declaring the vendor's error envelope emits a claim that is false for a GraphQL vendor.**
///
/// Linear signals every failure with `200`, a `null` `data` and an `errors` array. `ErrorEnvelope`
/// cannot say that — it has no success predicate and its own documentation scopes it to a *non-2xx*
/// body ([C-57]) — and `description()` appends "A non-2xx response is returned as data, not a
/// failure…" to any operation declaring one. For a GraphQL vendor that sentence points a model at a
/// branch that never occurs.
///
/// So the withdrawn connector declared no envelope anywhere, and the cost of that was **nothing
/// machine-readable**: `quirks` reaches no artifact at all — `connector-cli`'s catalogue and site
/// emitters both write `Quirks::default()` — so a declared envelope would have been a false sentence
/// in prose with no compensating structured data. This test pins the false sentence, so closing
/// C-57 breaks it and forces the decision to be retaken rather than inherited.
///
/// [C-57]: ../../../docs/stories/C-57-quirks-beyond-http-shape.md
#[test]
fn an_error_envelope_would_emit_a_false_claim_about_non_2xx_responses() {
    let connector = linear();
    for operation in &connector.operations {
        assert!(
            operation.quirks.error_envelope.is_none(),
            "`{}` declares an envelope; under GraphQL that emits a claim about non-2xx responses \
             that never occur",
            operation.id
        );
    }

    let mut with_envelope = op(&connector, "linear-viewer").clone();
    with_envelope.quirks.error_envelope = Some(ErrorEnvelope {
        message_pointer: "/errors/0/message".to_string(),
        code_pointer: None,
    });
    let flux = emit_operation(&connector, &with_envelope)
        .expect("declaring an envelope is legal, which is the problem");
    assert!(
        flux.contains("A non-2xx response is returned as data"),
        "the emitter no longer states the non-2xx claim; if C-57 landed, this finding must be \
         retaken rather than left as written"
    );
}

// -------------------------------------------------------------------------------------------
// The blocking finding, at the only layer this crate can see it.
// -------------------------------------------------------------------------------------------

/// **The emitted module binds a string literal full of braces, and that is what the host library
/// misreads.** This crate cannot depend on `connector-pack`, so it asserts the *shape* the pack
/// trips over rather than the tripping; the two halves of the actual defect are measured in
/// `crates/connector-pack/src/request.rs`'s own tests.
///
/// The assertion is deliberately about a brace surviving into a **`lit`** rather than a `fmt`. That
/// distinction is the whole of `connector-pack`'s reasoning: flux interpolates `fmt` and never
/// `lit`, so a brace inside a bound literal is taken to name something no evaluation fills — which
/// is true of a templated base URL and a C-187 pin, and false of a GraphQL selection set.
#[test]
fn the_emitted_document_is_a_brace_bearing_literal_which_is_the_shape_the_pack_misreads() {
    let connector = linear();
    let flux = emit(&connector, "linear-viewer");

    let document = query_document(op(&connector, "linear-viewer"));
    assert!(
        document.contains('{') && document.contains('}'),
        "a GraphQL document without a selection set would not exercise the finding"
    );

    // The document reaches the module as a bound literal, not through `fmt`. `base` is the
    // counter-example in the same module: it is a literal too, and *its* placeholder genuinely is
    // configuration, which is why the pack cannot tell the two apart by looking.
    let bound = flux
        .lines()
        .find(|line| line.trim_start().starts_with("query = "))
        .expect("the document is bound as its own symbol");
    assert!(
        bound.contains('"'),
        "the document is not bound as a string literal: {bound}"
    );
    assert!(
        !flux.contains("fmt(\"query"),
        "the document is interpolated rather than bound as a literal, which would change the \
         finding entirely"
    );
}
