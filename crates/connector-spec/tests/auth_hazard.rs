//! **A declared weakness in how a credential is obtained, and why the vocabulary is closed** — C-440.
//!
//! A host refuses a hazardous acquisition **by property, not by connector name**. An operator who
//! wants no password-grant authentication anywhere says that once, about a declared property, and
//! every connector carrying it refuses — including the fifty-sixth, added next month by somebody who
//! never read their policy. A list of connector names is correct on the day it is written and
//! silently wrong afterwards.
//!
//! That only works if the spelling is closed. A free-form `hazard = "..."` string is more expressive
//! and strictly worse, because it turns the consumer's filter into a string match: a near-miss
//! spelling matches no allow-list entry, reads as *no hazard declared*, and is admitted by the very
//! deployment that refused the thing it names. So an unrecognised spelling is a **loader refusal**,
//! and this file is the whole of that.
//!
//! # The spelling is a contract with a consumer that already shipped
//!
//! flux-exchange's `crates/exchange-host/src/acquisition.rs:285` maps its `AuthHazard` variant to
//! the string `resource_owner_secret_shared`, and `FLUX_EXCHANGE_ALLOW_AUTH_HAZARDS` is the
//! deployment gate written against exactly that word. This repository emits the value that filter
//! reads; the near-miss `resource_owner_secret_sharing` is one word away, means the same thing to a
//! human, and is what a filter silently admits.

use connector_spec::Connector;

/// A minimal well-formed provider with `body` spliced in after the connector-level keys.
fn provider(body: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
base_url = "https://api.acme.example"
description = "A provider that exists to be checked."
{body}
"#
    )
}

/// The bearer credential every case below builds on, with `extra` spliced into its block.
fn credential(extra: &str) -> String {
    format!(
        r#"
[[auth]]
name = "acme.access_token"
scheme = "bearer"
env = ["ACME_ACCESS_TOKEN"]
{extra}
"#
    )
}

/// One ordinary read, so the connector describes something. A provider declaring no operations at
/// all is refused before any credential rule is reached.
const READ: &str = r#"
[[operations]]
id = "acme-thing-list"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List the things."
risk = "low"
idempotency = "idempotent"
"#;

fn load(source: &str) -> connector_spec::Result<Connector> {
    connector_spec::provider::load("providers/acme.toml", source).map(|loaded| loaded.connector)
}

/// The rendered refusal, or a panic naming the provider that was wrongly accepted.
fn refusal(source: &str) -> String {
    match load(source) {
        Ok(_) => panic!("the loader accepted a provider it must refuse:\n{source}"),
        Err(error) => error.to_string(),
    }
}

/// **The near-miss spelling is refused, and the refusal names both words.**
///
/// `resource_owner_secret_sharing` is one letter away from the declared spelling and means the same
/// thing to a reader, which is exactly what makes it dangerous: it is what a hurried author types,
/// and a consumer filtering on the real word sees *no hazard declared* rather than an error.
///
/// Two assertions, and the second is the one that makes the refusal useful. Naming the rejected
/// value alone is not enough — a plain unknown-key refusal already quotes the offending source line,
/// so a test asserting only that would pass against a loader that had never heard of `hazard`. The
/// refusal must also name the spelling that **is** recognised, because that is the edit the author
/// has to make and the word the consuming deployment gate is written against.
#[test]
fn the_near_miss_hazard_spelling_is_refused_naming_the_value() {
    let source = provider(&format!(
        "{}{READ}",
        credential(r#"hazard = "resource_owner_secret_sharing""#)
    ));
    let refusal = refusal(&source);

    assert!(
        refusal.contains("resource_owner_secret_sharing"),
        "the refusal must name the value it rejected: {refusal}"
    );
    assert!(
        refusal.contains("resource_owner_secret_shared"),
        "the refusal must name the spelling that is recognised, which is the edit the author has \
         to make and the word flux-exchange's deployment gate filters on: {refusal}"
    );
    assert!(
        !refusal.contains("unknown field `hazard`"),
        "`hazard` must be a declared key whose *value* was refused, not a key the loader has never \
         heard of — a connector declaring a hazard it spelled correctly must load: {refusal}"
    );
}

/// The spelling this repository emits is the one flux-exchange already filters on.
///
/// Pinned as a literal rather than derived, because the whole value of the closed set is that this
/// exact byte sequence reaches a deployment gate in another repository. A serde rename that changed
/// it would be invisible here and would silently admit a hazardous acquisition there.
#[test]
fn the_declared_hazard_is_the_word_the_consuming_deployment_gate_reads() {
    let source = provider(&format!(
        "{}{READ}",
        credential(r#"hazard = "resource_owner_secret_shared""#)
    ));
    let connector = load(&source).expect("a correctly spelled hazard must load");

    let published = serde_json::to_string(&connector.auth).expect("the IR serializes");
    assert!(
        published.contains("resource_owner_secret_shared"),
        "the declared hazard must reach the published form as the word the consumer reads: \
         {published}"
    );
}

/// **A `password` grant that declares no hazard is refused.**
///
/// The closed vocabulary is only worth having if a connector cannot opt out of it by silence. A
/// deployment filter refuses on the *presence* of a declared hazard, so a connector allowing the
/// resource-owner password grant with no `hazard` line is admitted by the very deployment that set
/// out to refuse exactly this — and the omission is one line nobody wrote rather than anything a
/// reviewer sees. `AGENTS.md` states the general form: a marking that reads as a safety decision
/// while recording only that the question was never asked is worse than no marking.
#[test]
fn a_password_grant_that_declares_no_hazard_is_refused() {
    let source = provider(&format!(
        "{}{READ}",
        credential(
            r#"
[auth.oauth2]
endpoint = "login"
token_path = "/oauth/token"
grants = ["password", "refresh_token"]
"#
        )
    ));

    let refusal = refusal(&source);
    assert!(
        refusal.contains("acme.access_token") && refusal.contains("password"),
        "the refusal must name the credential and the grant: {refusal}"
    );
    assert!(
        refusal.contains("resource_owner_secret_shared"),
        "the refusal must name the hazard the author has to declare: {refusal}"
    );
}

/// The rule runs **one way**: a grant list without `password` needs no hazard.
///
/// `providers/gitlab.toml` is the shipped case — `authorization_code` and `refresh_token`, neither
/// of which shares the resource owner's secret with this host — and it must keep loading untouched.
#[test]
fn a_grant_list_without_the_password_grant_needs_no_hazard() {
    let source = provider(&format!(
        "{}{READ}",
        credential(
            r#"
[auth.oauth2]
endpoint = "login"
token_path = "/oauth/token"
grants = ["authorization_code", "refresh_token"]
"#
        )
    ));

    load(&source).expect("a grant list carrying no declared weakness must load without a hazard");
}

/// A credential that declares no hazard carries none — the case all 55 shipped connectors are in.
///
/// The default has to be *absent* rather than a benign-sounding value, for the same reason
/// `Subject::Unstated` is not `App`: a hazard nobody declared is a question nobody asked, and the
/// consumer's fail-closed default is written against the absence.
#[test]
fn a_credential_declaring_no_hazard_carries_none() {
    let source = provider(&format!("{}{READ}", credential("")));
    let connector = load(&source).expect("a plain credential must load");

    let published = serde_json::to_string(&connector.auth).expect("the IR serializes");
    assert!(
        !published.contains("hazard"),
        "a credential declaring no hazard gained one: {published}"
    );
}
