//! **A credential-producing operation returns a handle, or it does not load** — C-136's declaration
//! and the six refusals that keep it true.
//!
//! `AGENTS.md` § Authentication contract records why an operation whose response carries a token was
//! withheld at all: the host's redactor holds only values the host itself resolved, and cannot know
//! a secret minted by the very call returning it. C-430 built the gate that withholds one and
//! established the constraint this story is shaped by — **removing the field from the published
//! schema is strictly worse than withholding the operation**, because nothing between the vendor and
//! a model-visible symbol projects a response, so deleting a location removes the *disclosure* and
//! leaves the *exposure*.
//!
//! `produces_credential` is the answer that removes the exposure. The secret travels into the host's
//! bound `CredentialStore` and the caller receives the handle, so the operation's declared output —
//! [`Operation::effective_response_schema`] — is `{ "credential": "tenants/…" }` and contains no
//! field the vendor's body has. The runtime half is `connector_pack::mint`; this file is the
//! declaration half, and every rule in it is a refusal.
//!
//! # Three the story names, and three without which they cannot hold
//!
//! Each has its own test below. The first three are the story's: a declared output that still
//! exposes the secret, a declaration naming no secret field, and an operation declared `idempotent`.
//! The other three are the ones that make the mechanism *possible* rather than merely correct — a
//! credential nothing declares has no leaf, a connector with no authority has no address, and two
//! operations minting one credential leave "which call put the value there" unanswerable.

use connector_spec::{Connector, CREDENTIAL_HANDLE_FIELD};
use serde_json::json;

use crate::shipped_provider;

/// A minimal well-formed provider that declares an authority and one credential, with `body`
/// spliced in after the connector-level keys.
fn provider(body: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
base_url = "https://api.acme.example"
description = "A provider that exists to be checked."

[[auth]]
name = "acme.access_token"
scheme = "bearer"
env = ["ACME_ACCESS_TOKEN"]
{body}
"#
    )
}

/// A login: a `POST` that mints `credential`, taking the secret from `secret`.
///
/// `tables` is spliced in *after* the operation's own sub-tables, because a TOML sub-table closes
/// the block it belongs to — writing a `response_schema` before `[operations.produces_credential]`
/// would leave the second one parsed under the first.
fn login(id: &str, secret: &str, credential: &str, tables: &str) -> String {
    format!(
        r#"
[[operations]]
id = "{id}"
method = "POST"
direction = "write"
path = "/oauth/token"
description = "Exchange client credentials for an access token."
risk = "medium"
idempotency = "non_idempotent"

[operations.produces_credential]
secret = "{secret}"
credential = "{credential}"
{tables}"#
    )
}

/// A `response_schema` describing an object with each of `properties` as a string member.
fn describing(properties: &[&str]) -> String {
    let mut tables = String::from(
        r#"
[operations.response_schema]
type = "object"
"#,
    );
    for property in properties {
        tables.push_str(&format!(
            r#"
[operations.response_schema.properties.{property}]
type = "string"
"#
        ));
    }
    tables
}

fn load(source: &str) -> connector_spec::Result<Connector> {
    connector_spec::provider::load("providers/acme.toml", source).map(|loaded| loaded.connector)
}

/// The rendered refusal, or a panic naming the connector that was wrongly accepted.
fn refusal(source: &str) -> String {
    match load(source) {
        Ok(_) => panic!("the loader accepted a provider it must refuse:\n{source}"),
        Err(error) => error.to_string(),
    }
}

/// **The declaration loads, and the declared output is the handle.**
///
/// The positive case first, because every refusal below is only meaningful against a shape that is
/// otherwise accepted — and because this is the acceptance criterion itself: the secret field is
/// absent from the effective output *entirely*, and what is present is the address.
#[test]
fn a_credential_producing_operation_declares_the_handle_as_its_output() {
    let source = provider(&login(
        "acme-oauth-token",
        "/access_token",
        "acme.access_token",
        "",
    ));
    let connector = load(&source).expect("a well-formed credential-producing operation loads");

    let operation = &connector.operations[0];
    let produced = operation
        .produces_credential
        .as_ref()
        .expect("the declaration survives the loader");
    assert_eq!(produced.secret, "/access_token");
    assert_eq!(produced.credential, "acme.access_token");

    let output = operation
        .effective_response_schema()
        .expect("a credential-producing operation always declares an output");
    assert_eq!(
        output["properties"][CREDENTIAL_HANDLE_FIELD]["type"],
        json!("string"),
        "the declared output is not the handle: {output}"
    );
    assert_eq!(
        output["properties"]
            .as_object()
            .map(serde_json::Map::len)
            .expect("the handle is an object schema"),
        1,
        "the declared output carries more than the handle: {output}"
    );
    assert_eq!(
        output["additionalProperties"],
        json!(false),
        "the declared output admits fields beside the handle: {output}"
    );
}

/// **The word the handle is returned under**, pinned against the runtime's own copy.
///
/// `connector-pack` mirrors this constant rather than importing it — it depends on neither this
/// crate nor the compiler, and `crates/connector-cli/tests/dependency_fence.rs` keeps it that way,
/// so no test can see both. Its counterpart is
/// `connector_pack::mint::tests::the_handle_field_is_the_word_the_declared_output_uses`, which
/// spells the same word. Changing either fails a test that names the other.
#[test]
fn the_handle_field_is_the_word_the_runtime_answers_with() {
    assert_eq!(CREDENTIAL_HANDLE_FIELD, "credential");
    assert!(connector_spec::credential_handle_schema()["required"]
        .as_array()
        .expect("the handle requires its one property")
        .contains(&json!(CREDENTIAL_HANDLE_FIELD)));
}

/// **Refusal 1 — the declared output still exposes the secret field.**
///
/// The operation describes the vendor's wire body, and that description reaches
/// `web/public/catalog.json`. A `response_schema` naming the very location the diversion takes the
/// value out of is a published contract offering a caller the secret. Refused, and the refusal says
/// what C-430 established, so nobody re-derives it: deleting the location is not the fix.
#[test]
fn a_produces_credential_operation_whose_response_schema_exposes_the_secret_is_refused() {
    let source = provider(&login(
        "acme-oauth-token",
        "/access_token",
        "acme.access_token",
        &describing(&["access_token", "expires_in"]),
    ));

    let error = refusal(&source);
    assert!(
        error.contains("acme-oauth-token") && error.contains("/access_token"),
        "the refusal must name the operation and the location: {error}"
    );
    assert!(
        error.contains("response_schema"),
        "the refusal must name the field that exposes it: {error}"
    );

    // The control: the identical operation with the secret *absent* from the described body loads,
    // so the refusal above is about the exposure rather than about declaring a schema at all.
    load(&provider(&login(
        "acme-oauth-token",
        "/access_token",
        "acme.access_token",
        &describing(&["expires_in"]),
    )))
    .expect("a wire body that does not describe the secret is fine to document");
}

/// **Refusal 2 — the declaration names no secret field.**
///
/// The extractor would not know what to divert, so the operation would hand back the vendor's body —
/// the unsafe operation wearing the safe operation's declaration, which is the worst of the two
/// states. An empty string and a location that is not a pointer are the same mistake and take the
/// same refusal.
#[test]
fn a_produces_credential_operation_naming_no_secret_field_is_refused() {
    for secret in ["", "access_token"] {
        let source = provider(&login("acme-oauth-token", secret, "acme.access_token", ""));
        let error = refusal(&source);
        assert!(
            error.contains("acme-oauth-token") && error.contains("names no field"),
            "the refusal for secret = {secret:?} must say the location names nothing: {error}"
        );
    }
}

/// **A location naming more than one value is refused too**, and it is the same rule read from the
/// other end.
///
/// `credential_response` admits `*` for every element of an array — postmark's
/// `Servers[]/ApiTokens` is a real array of live tokens, which is why that extension exists. A mint
/// is one call storing one value at one address, so `*` would name several secrets with nothing to
/// say which is the credential. Refusing at load is also what stops a published crate's
/// documentation from asserting a runtime behaviour it does not have: the diversion resolves the
/// location with `serde_json::Value::pointer`, which has no wildcard, so a `*` this validator let
/// through would load, cross-check, and then refuse at every single call.
#[test]
fn a_produces_credential_location_naming_every_element_of_an_array_is_refused() {
    let source = provider(&login(
        "acme-oauth-token",
        "/tokens/*/value",
        "acme.access_token",
        "",
    ));

    let error = refusal(&source);
    assert!(
        error.contains("acme-oauth-token") && error.contains("/tokens/*/value"),
        "the refusal must name the operation and the location: {error}"
    );
    assert!(
        error.contains("exactly one value") || error.contains("one value at exactly one address"),
        "the refusal must say why one location is required: {error}"
    );
}

/// **Refusal 3 — a login declared `idempotent`.**
///
/// Minting a token is a write: some vendors invalidate the previous one, so a repeat is not the
/// no-op the value claims. And `Idempotent` licenses flux's op cache to serve a stored result
/// *instead of executing*, which for a login means answering with an address whose value has since
/// been replaced — a handle that resolves to a dead token, with nothing to say so.
#[test]
fn a_produces_credential_operation_declared_idempotent_is_refused() {
    let source = provider(&login(
        "acme-oauth-token",
        "/access_token",
        "acme.access_token",
        "",
    ))
    .replace(
        r#"idempotency = "non_idempotent""#,
        r#"idempotency = "idempotent""#,
    );

    let error = refusal(&source);
    assert!(
        error.contains("acme-oauth-token") && error.contains("idempotent"),
        "the refusal must name the operation and the claim: {error}"
    );
    assert!(
        error.contains("write") || error.contains("invalidate"),
        "the refusal must say why minting is not repeatable: {error}"
    );
}

/// **A credential nothing declares has no leaf, so it has no address.** Without this the three
/// refusals above would sit on a mechanism that cannot store anything.
#[test]
fn a_produces_credential_operation_storing_an_undeclared_credential_is_refused() {
    let source = provider(&login(
        "acme-oauth-token",
        "/access_token",
        "acme.session_token",
        "",
    ));

    let error = refusal(&source);
    assert!(
        error.contains("acme.session_token") && error.contains("auth"),
        "the refusal must name the credential and where it would have been declared: {error}"
    );
}

/// **A connector with no `authority` has no second path segment**, so `tenants/<tenant>/…/…` cannot
/// be composed and the minted value has nowhere to go. `connector-pack` refuses the same
/// arrangement at resolve time; refusing here makes it a build failure rather than a first-call one.
#[test]
fn a_produces_credential_operation_on_a_connector_with_no_authority_is_refused() {
    let source = provider(&login(
        "acme-oauth-token",
        "/access_token",
        "acme.access_token",
        "",
    ))
    .replace("authority = \"com.acme.api\"\n", "");

    let error = refusal(&source);
    assert!(
        error.contains("acme-oauth-token") && error.contains("authority"),
        "the refusal must name the operation and the missing fact: {error}"
    );
}

/// **Two operations minting one credential are refused.**
///
/// The catalogue records the mint as "this credential is minted by *that* operation", so two
/// producers have no representation — and a downstream operation naming the credential could not say
/// which login it needs. The same ambiguity C-406 refuses for two connections of one vendor, one
/// level down.
#[test]
fn two_operations_minting_one_credential_are_refused() {
    let source = provider(&format!(
        "{}{}",
        login("acme-oauth-token", "/access_token", "acme.access_token", ""),
        login(
            "acme-oauth-refresh",
            "/access_token",
            "acme.access_token",
            ""
        )
    ));

    let error = refusal(&source);
    assert!(
        error.contains("acme-oauth-token") && error.contains("acme-oauth-refresh"),
        "the refusal must name both operations: {error}"
    );
}

/// **One fact, two dispositions, and the operation may not claim both** (C-432).
///
/// `credential_response` and `produces_credential` state the *same* fact — a credential arrives in
/// this operation's response — and prescribe opposite outcomes: withhold the operation, or ship it
/// and hand back a handle. An operation declaring both asks the loader to do both, and before this
/// story it got both answers: `credential_response`'s stock refusal telling the author to withhold
/// the operation, rendered beside a `produces_credential` declaration whose whole meaning is that
/// the operation ships. Two instructions, contradicting, with nothing saying which governs.
///
/// The rule that resolves it is **purpose, not shape**: if the credential *is* the answer, the
/// operation is a mint and diverts it (`produces_credential`); if the credential arrives *beside*
/// the answer, diverting would delete the answer, so the operation is withheld until it can be
/// redacted in place (`credential_response`, and C-79). That discriminator is what the refusal has
/// to carry, because it is the thing an author cannot re-derive from either field alone.
///
/// The second assertion is the load-bearing half: exactly one disposition is stated. A refusal that
/// merely *added* a sentence about the conflict, while still telling the author to withhold an
/// operation the other field says ships, would leave the reader exactly where they started.
#[test]
fn an_operation_declaring_both_credential_declarations_is_refused_naming_which_governs() {
    let source = provider(
        r#"
[[operations]]
id = "acme-oauth-token"
method = "POST"
direction = "write"
path = "/oauth/token"
description = "Exchange client credentials for an access token."
risk = "medium"
idempotency = "non_idempotent"
credential_response = ["/access_token"]

[operations.produces_credential]
secret = "/access_token"
credential = "acme.access_token"

[operations.response_schema]
type = "object"

[operations.response_schema.properties.access_token]
type = "string"
"#,
    );

    let error = refusal(&source);
    assert!(
        error.contains("credential_response") && error.contains("produces_credential"),
        "the refusal must name both declarations, or it does not say what conflicts: {error}"
    );
    assert!(
        error.contains("acme-oauth-token"),
        "the refusal must name the operation: {error}"
    );
    assert!(
        error.contains("purpose") && error.contains("incidental"),
        "the refusal must carry the discriminator that says which declaration governs — purpose \
         versus incidental — not merely report that two were found: {error}"
    );
    assert!(
        !error.contains("Withhold the operation and name it as an exclusion"),
        "the refusal still prescribes withholding beside a declaration that says the operation \
         ships, which is the contradiction this story exists to remove: {error}"
    );
}

/// **Nothing shipped declares one**, and that is the honest state of the catalogue rather than an
/// oversight.
///
/// The four operations v0.9.0 and v0.9.1 withheld are withheld under C-430's `credential_response`;
/// reinstating one through this mechanism is a change to its provider file and to
/// `tests/credential_response.rs`'s register, in the same commit. This assertion is what makes that
/// a deliberate act: the day it goes red, somebody landed the first credential-producing operation
/// and owes the review that comes with it.
#[test]
fn no_shipped_operation_declares_produces_credential_yet() {
    let mut declared: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(shipped_provider::providers_dir())
        .expect("the repository's providers/ directory is readable")
    {
        let path = entry.expect("readable directory entry").path();
        if path.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        let stem = path
            .file_stem()
            .expect("a provider file has a stem")
            .to_string_lossy()
            .into_owned();
        declared.extend(
            shipped_provider::connector(&stem)
                .operations
                .iter()
                .filter(|operation| operation.produces_credential.is_some())
                .map(|operation| operation.id.clone()),
        );
    }

    assert!(
        declared.is_empty(),
        "the first credential-producing operations have shipped: {declared:?}. That is what C-136 \
         is for — review the diversion end to end and update this test's reason rather than \
         deleting it"
    );
}
