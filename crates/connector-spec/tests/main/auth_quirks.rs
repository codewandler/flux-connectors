//! **Quirks on the authentication surface** — C-440's third declaration.
//!
//! This repository already carries the word and its discipline: `quirks.pagination` and
//! `quirks.rate_limit` are *declarations, not behaviour*, reaching the IR and the loader and no
//! artifact. What C-440 adds is not a new kind of statement but a new **scope** — today `Quirks`
//! hangs off an operation, and a token endpoint is not one. An authentication endpoint is never a
//! connector operation (`AGENTS.md` § Authentication contract), so a measured departure of a token
//! endpoint from its own document has nowhere to go until `[[auth]]` can hold it.
//!
//! # Why the shape is prose and a grant, and not a knob
//!
//! Owner-decided 2026-08-02, in flux-exchange: **if it is not in the specification, it does not
//! become a general thing.** The occasion was a token lifetime. babelforce's token endpoint accepts
//! one; `specs/babelforce/auth-2026-06-25.openapi.yaml`'s `OAuthTokenRequest` declares eleven
//! properties and none is a lifetime. Both were true — the vendor's controller reads `expires_in`
//! straight out of the request parameters, which is exactly why no generated document shows it —
//! and it reads it **differently for each grant**: a default of `-1` meaning *never expires* for
//! one, a hard 60-second clamp for another, and not at all for a third.
//!
//! One field, five behaviours, one vendor. A general `requested_ttl` would be a hard cap here,
//! ignored there, and the difference between an hour and forever somewhere else, while inviting the
//! other fifty-four providers to be assumed to honour something none of them declares. So a quirk
//! records **what was measured, on which grant, by whom, and when** — and a host that wants to act
//! on one has to have read it.
//!
//! # Attribution is not decoration
//!
//! Every quirk here is asserted against a vendor's *implementation* and contradicted by that
//! vendor's own *document*. A reader a year from now needs to know which of the two this repository
//! checked and when, or the declaration is indistinguishable from a guess that aged. That is why
//! `attribution` and `measured` are required rather than optional, and why the loader refuses a
//! `measured` that is not a date.

use connector_spec::Connector;

use crate::shipped_provider;

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

/// The OAuth2 credential every case below builds on, with `extra` spliced in after its grant block.
///
/// The grant block is not scenery: a token-endpoint quirk describes an endpoint, and a credential
/// with no `[auth.oauth2]` declares no token endpoint for it to be about. The `hazard` is not
/// scenery either — the loader refuses a `password` grant that does not declare one, which is
/// `auth_hazard.rs`'s subject rather than this file's.
fn credential(extra: &str) -> String {
    format!(
        r#"
[[auth]]
name = "acme.access_token"
scheme = "bearer"
env = ["ACME_ACCESS_TOKEN"]
hazard = "resource_owner_secret_shared"

[auth.oauth2]
endpoint = "login"
token_path = "/oauth/token"
grants = ["password", "refresh_token"]
{extra}
"#
    )
}

/// One ordinary read, so the connector describes something.
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

/// Two measured departures of one token endpoint from its own document.
const QUIRKS: &str = r#"
[[auth.quirks.token_endpoint]]
grant = "client_credentials"
behaviour = "`expires_in` is read from the request and defaults to -1, which means never expires."
attribution = "the vendor's own token controller, read beside the document that omits it"
measured = "2026-08-02"

[[auth.quirks.token_endpoint]]
grant = "refresh_token"
behaviour = "`account_id` on the request switches the account the new token belongs to."
attribution = "the vendor's own token controller, read beside the document that omits it"
measured = "2026-08-02"
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

/// The connector's declared auth surface, in the form a manifest publishes it.
///
/// Read through the serialized IR rather than a typed accessor deliberately. A leak is a question
/// about what a *published* declaration carries — a host reads the artifact, not this crate's
/// structs — and a typed getter that walked the right credential would answer the narrower question
/// while a shared or defaulted field went on leaking underneath it.
fn published_auth(connector: &Connector) -> String {
    serde_json::to_string_pretty(&connector.auth).expect("the connector's auth surface serializes")
}

/// **A quirk declared on one connector's auth surface does not reach another's.**
///
/// The failure this refuses is not hypothetical in shape: a quirk is vendor-specific by
/// construction — a 60-second clamp, an account switch on refresh — and a host that read one
/// connector's measured departure while talking to a different vendor would be acting on a fact
/// about somebody else's server. `providers/gitlab.toml` is the real neighbour here: it is the other
/// shipped connector declaring `[auth.oauth2]`, so it is the one a leak would land on first.
#[test]
fn a_quirk_declared_on_one_connectors_auth_surface_does_not_reach_another() {
    let declaring = load(&provider(&format!("{}{READ}", credential(QUIRKS))))
        .expect("a connector declaring auth quirks must load");
    let silent = load(&provider(&format!("{}{READ}", credential(""))))
        .expect("a connector declaring none must load");
    let shipped = shipped_provider::load("gitlab").connector;

    let declared = published_auth(&declaring);
    assert!(
        declared.contains("client_credentials") && declared.contains("account_id"),
        "the declaring connector must carry its own measurements: {declared}"
    );

    for (id, connector) in [("the synthetic neighbour", &silent), ("gitlab", &shipped)] {
        let published = published_auth(connector);
        assert!(
            !published.contains("token_endpoint"),
            "{id} declares no auth quirks and must carry none: {published}"
        );
        assert!(
            !published.contains("account_id"),
            "{id} gained a measurement made against another vendor's server: {published}"
        );
    }
}

/// A quirk on one credential does not reach a **sibling credential in the same connector**.
///
/// The nearer miss, and the one a per-connector store would get wrong. GitLab already ships two
/// credentials on one connector — a personal access token and an OAuth token — and only one of them
/// has a token endpoint at all.
#[test]
fn a_quirk_does_not_reach_a_sibling_credential_in_the_same_connector() {
    let source = provider(&format!(
        r#"
{}
[[auth]]
name = "acme.api_key"
scheme = {{ header = {{ name = "X-Api-Key" }} }}
env = ["ACME_API_KEY"]
{READ}
[[default_auth]]
credentials = ["acme.access_token"]
"#,
        credential(QUIRKS)
    ));

    let connector = load(&source).expect("two credentials, one declaring quirks, must load");
    let published =
        serde_json::to_string_pretty(&connector.auth[1]).expect("the sibling serializes");

    assert!(
        !published.contains("token_endpoint"),
        "the sibling credential declares no token endpoint and must carry no quirk about one: \
         {published}"
    );
}

/// **A quirk with no attribution is refused**, naming the credential and the grant.
///
/// An unattributed quirk is the cost this story was written to avoid paying twice:
/// `providers/babelforce.toml` already carries one open question to the vendor's API owners that
/// nobody can now answer, because whoever raised it did not record what they had read.
#[test]
fn a_quirk_without_attribution_is_refused() {
    let source = provider(&format!(
        "{}{READ}",
        credential(
            r#"
[[auth.quirks.token_endpoint]]
grant = "client_credentials"
behaviour = "`expires_in` is read from the request and defaults to -1."
attribution = ""
measured = "2026-08-02"
"#
        )
    ));

    let refusal = refusal(&source);
    assert!(
        refusal.contains("acme.access_token") && refusal.contains("client_credentials"),
        "the refusal must name the credential and the grant: {refusal}"
    );
    assert!(
        refusal.contains("attribution"),
        "the refusal must name the missing field: {refusal}"
    );
}

/// **A `measured` that is not a date is refused.**
///
/// "recently", "when we looked" and "2026" are all things a hurried author writes, and none of them
/// lets a reader decide whether the measurement predates the vendor release they are debugging. The
/// field is a date or it is not a measurement.
#[test]
fn a_quirk_measured_on_a_non_date_is_refused() {
    let source = provider(&format!(
        "{}{READ}",
        credential(
            r#"
[[auth.quirks.token_endpoint]]
grant = "client_credentials"
behaviour = "`expires_in` is read from the request and defaults to -1."
attribution = "the vendor's own token controller"
measured = "recently"
"#
        )
    ));

    let refusal = refusal(&source);
    assert!(
        refusal.contains("recently") && refusal.contains("YYYY-MM-DD"),
        "the refusal must name the value and the shape a date takes: {refusal}"
    );
}

/// **A token-endpoint quirk on a credential with no grant is refused.**
///
/// The same rule `oauth.redirect_uri` already carries: a declaration about an endpoint the
/// connector never declared is one nothing will ever read, and a quirk nobody reads is worse than
/// none because it reads as a fact somebody checked.
#[test]
fn a_token_endpoint_quirk_without_a_grant_is_refused() {
    let source = provider(&format!(
        r#"
[[auth]]
name = "acme.access_token"
scheme = "bearer"
env = ["ACME_ACCESS_TOKEN"]

[[auth.quirks.token_endpoint]]
grant = "client_credentials"
behaviour = "`expires_in` is read from the request and defaults to -1."
attribution = "the vendor's own token controller"
measured = "2026-08-02"
{READ}"#
    ));

    let refusal = refusal(&source);
    assert!(
        refusal.contains("acme.access_token") && refusal.contains("oauth2"),
        "the refusal must name the credential and the grant block it lacks: {refusal}"
    );
}

/// **Two measurements of one grant are refused**, because they are two answers to one question.
#[test]
fn two_quirks_for_one_grant_are_refused() {
    let source = provider(&format!(
        "{}{READ}",
        credential(
            r#"
[[auth.quirks.token_endpoint]]
grant = "password"
behaviour = "`expires_in` is read when present."
attribution = "the vendor's own token controller"
measured = "2026-08-02"

[[auth.quirks.token_endpoint]]
grant = "password"
behaviour = "`expires_in` is ignored."
attribution = "the vendor's own token controller"
measured = "2026-08-02"
"#
        )
    ));

    let refusal = refusal(&source);
    assert!(
        refusal.contains("password") && refusal.contains("acme.access_token"),
        "the refusal must name the grant measured twice and the credential: {refusal}"
    );
}
