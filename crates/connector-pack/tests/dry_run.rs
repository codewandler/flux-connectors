//! **The dry run**: what an operation would send, answered without sending it (C-145).
//!
//! The whole value of this surface is that the question can be asked *before* a real token reaches
//! a real vendor. That makes two properties load-bearing, and both are asserted here rather than
//! described:
//!
//! 1. **The transport cannot send.** Not "does not", not "is configured not to" — a flag on a live
//!    client is something a caller forgets, and what that forgetting produces is a live vendor call
//!    where a rehearsal was asked for.
//!    [`a_dry_run_transport_cannot_be_constructed_with_a_live_client`] is the structural form of
//!    the claim.
//! 2. **The answer holds no credential value.** C-159 made `Request`'s `Debug` redact, and
//!    redaction is *not* absence: a redacting `Debug` still has the plaintext in the struct behind
//!    it, one `.headers` away from any surface. The assertions below read fields directly and never
//!    a `{:?}` rendering, precisely so that the redactor cannot be what makes them pass. The stores
//!    they run over **hold** the credential, so a dry run omitting it is omitting something it
//!    could have had.

use std::sync::Arc;

use catalog::{OperationKey, Placement};
use connector_pack::{
    Configuration, CredentialRef, Credentials, DryRunTransport, Egress, Error, MemoryConfig,
    MemoryStore, Operation, Secret, SecretStore,
};
use flux_runtime::Tool;
use serde_json::{json, Value};

/// The tenant both ports answer for. One constant, because a pack whose two ports name different
/// tenants is refused at install.
const TENANT: &str = "t-dry-run";

/// A value that is not a real secret and is long enough for flux's redactor to hold — so a test
/// finding it in a dry run has found a genuine leak rather than a value below the threshold.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-C145";

/// A stand-in for flux's `http.request`. **Nothing here reaches it**, which is the point: every
/// assertion below goes through the dry-run transport, and this exists only because
/// [`Operation::project`] takes the live seam as its constructor argument.
fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in no dry run reaches".into(),
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

/// A configuration port carrying a value for **every** endpoint variable the shipped catalogue
/// declares, discovered from the catalogue rather than listed — so a templated connector shipped
/// after this file was written is covered rather than silently skipped.
fn configuration() -> Configuration {
    let mut values = MemoryConfig::new();
    for entry in catalog::operations() {
        let operation = Operation::project(entry, http(), empty_credentials(), unconfigured())
            .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));
        for variable in operation.endpoint_variables() {
            values = match variable.strip_prefix("username.") {
                Some(credential) => {
                    values.with_username(TENANT, entry.provider, entry.service, credential, "acme")
                }
                None => {
                    values.with_endpoint(TENANT, entry.provider, entry.service, variable, "acme")
                }
            };
        }
    }
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

/// An empty configuration port. Projection reads no *values* — only the variables an operation's
/// own Flux names — so this is enough to ask an entry what it needs.
fn unconfigured() -> Configuration {
    Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant id")
}

/// A bound credential port over an empty store.
fn empty_credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant id")
}

/// A bound credential port over a store that **does** hold the credential. See this module's
/// documentation for why the stocked case is the one worth asserting on.
async fn stocked_credentials(authority: &str, leaves: &[&str]) -> Credentials {
    let store = MemoryStore::new();
    for leaf in leaves {
        store
            .put(
                &CredentialRef::new(TENANT, authority, "default", leaf).expect("a valid address"),
                &Secret::new(SENTINEL),
            )
            .await
            .expect("an in-memory put cannot fail");
    }
    Credentials::new(Arc::new(store), TENANT).expect("a valid tenant id")
}

fn entry(id: &str) -> &'static catalog::Operation {
    catalog::operation(OperationKey::id(id))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"))
}

fn projected(id: &str, credentials: Credentials) -> Operation {
    Operation::project(entry(id), http(), credentials, configuration())
        .unwrap_or_else(|error| panic!("`{id}`: {error}"))
}

/// **The structural claim** (C-145's hard acceptance item).
///
/// A dry-run transport that held a client and declined to use it would be one `if` away from
/// sending, and the caller who forgets to set the flag gets a live vendor call where they asked for
/// a rehearsal. So the claim is made about the *type* rather than about its behaviour:
/// [`DryRunTransport`] is zero bytes wide, and a zero-sized type has room for no `Arc<dyn Tool>` (a
/// fat pointer, 16 bytes), no file descriptor (4), no channel handle, and not even the `bool` (1) a
/// flag would need.
///
/// The second assertion reads as a formality and is not: the type's only constructor takes **no
/// arguments**, so the day someone gives it a field to fill, this line stops compiling rather than
/// starting to lie.
///
/// The control comes last. `Egress` — the live seam, unchanged since C-115 — is *not* zero-sized,
/// so the size check above distinguishes the two rather than measuring something every transport
/// happens to have.
#[test]
fn a_dry_run_transport_cannot_be_constructed_with_a_live_client() {
    assert_eq!(
        std::mem::size_of::<DryRunTransport>(),
        0,
        "a dry-run transport with room for a field has room for a client"
    );

    let transport = DryRunTransport::new();

    assert!(
        std::mem::size_of::<Egress>() > 0,
        "the live seam is zero-sized too, so the assertion above distinguishes nothing"
    );

    // And it answers: a transport that cannot send is only worth having if it still reports the
    // request.
    let operation = projected("trello-board-get", empty_credentials());
    let dry = transport
        .dry_run(&operation, &json!({"id": "b-1"}))
        .expect("a shipped operation rehearses");
    assert_eq!(dry.request().method, "GET");
    assert!(
        dry.request()
            .url
            .starts_with("https://api.trello.com/1/boards/b-1"),
        "{}",
        dry.request().url
    );
}

/// A dry run reports the four things a request is: method, URL, headers and body.
#[test]
fn a_dry_run_reports_the_request_it_would_have_sent() {
    let operation = projected("trello-card-create", empty_credentials());
    let dry = operation
        .dry_run(&json!({"list_id": "l-1", "name": "a card", "description": "why"}))
        .expect("a shipped operation rehearses");

    assert_eq!(dry.operation(), "trello-card-create");
    assert_eq!(dry.tool(), "trello.card.create");
    assert_eq!(dry.request().method, "POST");
    assert!(
        dry.request()
            .url
            .starts_with("https://api.trello.com/1/cards"),
        "{}",
        dry.request().url
    );
    assert_eq!(
        dry.request()
            .headers
            .get("content-type")
            .map(String::as_str),
        Some("application/json")
    );

    let body: Value = serde_json::from_str(
        dry.request()
            .body
            .as_deref()
            .expect("a card creation travels in a body"),
    )
    .expect("the body is the JSON text `http.request` would send");
    assert_eq!(
        body,
        json!({"desc": "why", "idList": "l-1", "name": "a card"})
    );
}

/// **Absence, not redaction** — a header placement.
#[tokio::test]
async fn a_dry_run_carries_the_credential_reference_and_never_the_value() {
    let credentials = stocked_credentials("com.slack.api", &["bot_token"]).await;
    let operation = projected("slack-chat-post-message", credentials);

    let dry = operation
        .dry_run(&json!({"channel": "C1", "text": "hello", "thread_ts": Value::Null}))
        .expect("a shipped operation rehearses");

    assert!(
        !dry.render().contains(SENTINEL),
        "a resolved credential reached the dry run: {}",
        dry.render()
    );

    // And the reference is where the value would have been, carrying the vendor's own prefix —
    // which is the half that makes a dry run worth reading.
    assert_eq!(
        dry.request()
            .headers
            .get("Authorization")
            .map(String::as_str),
        Some("Bearer ~credential.slack.bot_token~")
    );

    let referenced = dry.credentials();
    assert_eq!(referenced.len(), 1);
    assert_eq!(referenced[0].credential(), "slack.bot_token");
    assert_eq!(
        referenced[0].place(),
        Placement::Header {
            name: "Authorization",
            prefix: "Bearer ",
        }
    );
}

/// **Absence, not redaction** — a query placement, which goes wrong differently.
///
/// `trello` is the catalogue's first `Placement::Query` connector and it places *two* credentials,
/// so this pins the separator as well: the first opens the query with `?` and the second continues
/// it with `&`, exactly as a real call would.
#[tokio::test]
async fn a_query_placed_credential_is_referenced_on_the_url_and_never_resolved() {
    let credentials = stocked_credentials("com.trello.api", &["key", "token"]).await;
    let operation = projected("trello-board-list", credentials);

    let dry = operation
        .dry_run(&json!({}))
        .expect("a shipped operation rehearses");

    assert!(
        !dry.render().contains(SENTINEL),
        "a resolved credential reached the dry run: {}",
        dry.render()
    );
    assert_eq!(
        dry.request().url,
        "https://api.trello.com/1/members/me/boards\
         ?key=~credential.trello.key~&token=~credential.trello.token~"
    );
}

/// **The reference reads the same wherever the credential goes.**
///
/// A query placement percent-encodes on its way onto a URL (C-159), and the point of a dry run is
/// to be read — a reference arriving as `%7Ecredential...` in a URL and plainly in a header would
/// be two spellings of one fact. The reference is therefore drawn from RFC 3986's *unreserved*
/// alphabet, so the encoder is the identity over it and the dry run can go through the **real**
/// placement path rather than a second copy of it that could drift.
#[test]
fn a_reference_survives_both_placements_unchanged() {
    let header = projected("slack-users-info", empty_credentials())
        .dry_run(&json!({"user": "U1", "include_locale": true}))
        .expect("a shipped operation rehearses");
    let query = projected("trello-board-list", empty_credentials())
        .dry_run(&json!({}))
        .expect("a shipped operation rehearses");

    for reference in header.credentials().iter().chain(query.credentials()) {
        assert!(
            !reference.reference().contains('%'),
            "`{}` is spelled so that a query placement escapes it: {}",
            reference.credential(),
            reference.reference()
        );
    }

    assert!(header
        .request()
        .headers
        .values()
        .any(|value| value.contains(header.credentials()[0].reference())));
    assert!(query
        .request()
        .url
        .contains(query.credentials()[0].reference()));
}

/// **The refusal discipline holds** (the story's closing note).
///
/// A signing secret verifies bytes that arrived and never leaves, so a real call refuses to place
/// one. A dry run reporting the request *without* it would describe a call the pack would never
/// make — a partly-built request presented as the real one, which is worse than reporting nothing.
#[test]
fn a_dry_run_refuses_an_inbound_credential_rather_than_reporting_a_partial_request() {
    // Doctored deliberately: `slack.signing_secret` is a real catalogue credential with a real
    // `Placement::Inbound`, and no shipped operation names it in a mechanism — which is exactly why
    // the branch needs a test of its own.
    let mut doctored = *entry("slack-chat-post-message");
    doctored.credentials = &[&["slack.signing_secret"]];
    let doctored: &'static catalog::Operation = Box::leak(Box::new(doctored));

    let operation = Operation::project(doctored, http(), empty_credentials(), configuration())
        .expect("the entry projects");
    let error = operation
        .dry_run(&json!({"channel": "C1", "text": "hello", "thread_ts": Value::Null}))
        .expect_err("a signing secret never goes out, rehearsal or not");

    assert!(matches!(error, Error::InboundCredential { .. }), "{error}");
}

/// A parameter the caller omitted is refused here for the same reason it is refused on the live
/// path: a dry run of a request that cannot be built would report a URL still carrying `{id}`, and
/// a reader would take it for the call that was going to be made.
#[test]
fn a_dry_run_refuses_a_request_it_cannot_build() {
    let error = projected("trello-board-get", empty_credentials())
        .dry_run(&json!({}))
        .expect_err("a missing path parameter is not a request");

    assert!(
        matches!(&error, Error::MissingParameter { parameter, .. } if parameter == "id"),
        "{error}"
    );
}

/// **Every shipped operation rehearses**, and none of them carries a value. A dry run that is right
/// for Slack and refuses half the catalogue is a playground an operator cannot use.
#[test]
fn every_shipped_operation_rehearses_and_reports_its_credentials_by_reference() {
    let configuration = configuration();
    let mut rehearsed = 0usize;
    let mut referenced = 0usize;

    for entry in catalog::operations() {
        let operation =
            Operation::project(entry, http(), empty_credentials(), configuration.clone())
                .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));
        let params = params_from_schema(&operation);
        let dry = operation
            .dry_run(&params)
            .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));

        assert!(
            dry.request().url.starts_with("https://"),
            "`{}` rehearses `{}`, which is not an absolute https URL",
            entry.id,
            dry.request().url
        );
        for reference in dry.credentials() {
            assert!(
                dry.render().contains(reference.reference()),
                "`{}` reports `{}` and does not carry its reference",
                entry.id,
                reference.credential()
            );
            referenced += 1;
        }
        rehearsed += 1;
    }

    assert!(
        rehearsed > 0,
        "an empty catalogue would pass the loop above"
    );
    assert!(
        referenced > 0,
        "no shipped operation named a credential, so the reference assertion asserted nothing"
    );
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
