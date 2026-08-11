//! **One deployment, one origin question** — C-529's shared endpoint slot and its four refusals.
//!
//! A self-managed GitLab serves its REST API at `{origin}/api/v4` and its OAuth2 authorize and token
//! endpoints at `{origin}`. That is one server and therefore one fact, but the two live in different
//! *services*, and a configuration value is addressed by `(tenant, provider, service, kind, name)` —
//! so without this the connector must declare the origin twice and ask the operator one question two
//! ways.
//!
//! Two slots that must agree and are not forced to is the Contentful defect running backwards.
//! Contentful's `delivery_space_id` and `management_space_id` both bind `endpoint.space_id` and must
//! stay separate: keyed as one, a management write went to whichever space the delivery reads had
//! been configured with, and got a `200` from a real server rather than a refusal. Here the same
//! machinery has the opposite requirement, and the difference is not derivable from the shape — one
//! `{origin}` approved for the API must be the one the token exchange uses, or a token endpoint can
//! be pointed at a host the API never approved.
//!
//! So sharing is **stated**, never inferred, and the default remains two slots.

use connector_spec::Connector;

/// A two-service connector shaped exactly like a self-managed GitLab: an API surface owning a path
/// suffix, and an OAuth surface at the bare origin.
fn provider(config: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
base_url = "{{origin}}/api/v4"
description = "A provider that exists to be checked."

[[services]]
name = "default"
legacy = true

[[services]]
name = "login"
description = "The vendor's OAuth endpoints, at the deployment origin."
base_url = "{{origin}}"

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]

[[operations]]
id = "acme-thing-list"
service = "default"
method = "GET"
direction = "read"
path = "/things"
description = "List the things."
risk = "low"
idempotency = "idempotent"
{config}
"#
    )
}

/// The shared declaration, as an author writes it.
const SHARED_ORIGIN: &str = r#"
[[config]]
name = "origin"
service = "default"
also_services = ["login"]
label = "Acme origin"
help = "The HTTPS origin only, without a path."
example = "https://acme.company.example"
format = "origin"
required = false
default = "https://acme.example"
approval = "operator"
binds = "endpoint.origin"
"#;

fn load(source: &str) -> connector_spec::Result<Connector> {
    connector_spec::provider::load("providers/acme.toml", source).map(|loaded| loaded.connector)
}

fn refusal(source: &str) -> String {
    match load(source) {
        Ok(_) => panic!("the loader accepted a provider it must refuse:\n{source}"),
        Err(error) => error.to_string(),
    }
}

/// **One field binds the origin for both services**, and it is one address with one approval.
#[test]
fn one_field_fills_the_placeholder_of_a_sibling_service() {
    let connector = load(&provider(SHARED_ORIGIN)).expect("a shared endpoint slot must load");

    let field = connector
        .config_field("origin")
        .expect("the connector declares `origin`");
    assert_eq!(
        field.service, "default",
        "the head service still carries the address"
    );
    assert_eq!(field.also_services, ["login"]);
    assert_eq!(
        connector.config.len(),
        1,
        "one question, one slot — a second field would be a second value that must agree"
    );

    assert_eq!(connector.base_url_of("default"), "{origin}/api/v4");
    assert_eq!(connector.base_url_of("login"), "{origin}");
}

/// Without the declaration, the sibling service's placeholder is unbound and the loader says so.
///
/// This is the control: it proves the test above passes because of `also_services` and not because
/// the coverage check stopped looking at named services.
#[test]
fn a_sibling_service_placeholder_is_refused_when_nothing_shares_the_slot() {
    let unshared = SHARED_ORIGIN.replace("also_services = [\"login\"]\n", "");
    let refusal = refusal(&provider(&unshared));
    assert!(
        refusal.contains("login") && refusal.contains("{origin}"),
        "the refusal must name the unbound service and variable: {refusal}"
    );
}

/// A misspelled sibling is refused **against the field that misspelled it**.
///
/// Reported here rather than only as "service `login` is unbound", because the typo is the cause and
/// the unbound service is the symptom — a message naming only the symptom sends an author to the
/// wrong file.
#[test]
fn a_sibling_service_that_does_not_exist_is_refused() {
    let typo = SHARED_ORIGIN.replace("[\"login\"]", "[\"logon\"]");
    let refusal = refusal(&provider(&typo));
    assert!(
        refusal.contains("\"origin\"") && refusal.contains("logon"),
        "the refusal must name the field and the misspelled service: {refusal}"
    );
}

/// Repeating the head service is one slot spelled twice.
#[test]
fn listing_the_head_service_again_is_refused() {
    let repeated = SHARED_ORIGIN.replace("[\"login\"]", "[\"default\", \"login\"]");
    let refusal = refusal(&provider(&repeated));
    assert!(
        refusal.contains("\"origin\"") && refusal.contains("default"),
        "the refusal must name the field and the repeated head: {refusal}"
    );
}

/// **Only an endpoint slot may be shared.** A credential has no per-service placeholder, so an
/// entry on one would name a service without reaching anything there.
#[test]
fn sharing_a_non_endpoint_binding_is_refused() {
    let credential = format!(
        r#"{SHARED_ORIGIN}
[[config]]
name = "token"
service = "default"
also_services = ["login"]
label = "Acme token"
help = "Create one in your account settings."
format = "token"
secret = true
binds = "credential.acme.token"
"#
    );
    let refusal = refusal(&provider(&credential));
    assert!(
        refusal.contains("\"token\"") && refusal.contains("endpoint"),
        "the refusal must name the field and say only an endpoint slot is shareable: {refusal}"
    );
}
