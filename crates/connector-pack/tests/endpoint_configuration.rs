//! **The templated-host gap (C-193).**
//!
//! Seven shipped connectors declare a `base_url` whose *host* carries a `{placeholder}`, and two
//! more — `contentful` and `statuspage` — carry one in the *path*. Before this story nothing
//! substituted a tenant's value into any of them, so the request went out to a host containing a
//! brace and `permission_subjects` declared that same unresolvable string as the subject a host's
//! egress allow-list was asked to match.
//!
//! The two halves are asserted separately on purpose. Building a working URL while leaving the
//! subject templated produces a request that is either refused by the gate for a reason naming
//! nothing an operator can fix, or admitted against a subject nobody can audit — and every other
//! test in this crate passes in both of those states.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{
    ConfigStore, Configuration, Credentials, Egress, Error, Field, MemoryConfig, MemoryStore,
    Operation,
};
use flux_runtime::Tool;
use serde_json::json;

/// The tenant both ports answer for.
const TENANT: &str = "t-endpoint";

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

fn credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant id")
}

/// A configuration port holding exactly the `(provider, service, variable, value)` rows given, for
/// this file's tenant.
///
/// The service joined the tuple with C-197: it is part of a value's address, and every connector
/// this file names has exactly one, the reserved `default`. `tests/service_scoped_configuration.rs`
/// is where a connector with two of them is exercised.
fn configured(rows: &[(&str, &str, &str, &str)]) -> Configuration {
    let mut values = MemoryConfig::new();
    for (provider, service, variable, value) in rows {
        values = values.with_endpoint(TENANT, provider, service, variable, value);
    }
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

fn projected(id: &str, configuration: Configuration) -> Operation {
    let entry = catalog::operation(OperationKey::id(id)).expect("the shipped catalogue carries it");
    Operation::project(entry, http(), credentials(), configuration)
        .unwrap_or_else(|error| panic!("`{id}`: {error}"))
}

/// A `zendesk` operation with `{subdomain}` bound to `acme`.
fn zendesk(id: &str) -> Operation {
    projected(
        id,
        configured(&[("zendesk", "default", "subdomain", "acme")]),
    )
}

/// **The first half.** A tenant's value reaches the URL, through the bound port and nowhere else.
#[test]
fn a_templated_host_is_substituted_into_the_request_url() {
    let request = zendesk("zendesk-ticket-show")
        .build_request(&json!({ "ticket_id": 1 }))
        .expect("the request builds");

    assert_eq!(
        request.url, "https://acme.zendesk.com/api/v2/tickets/1.json",
        "the host a request reaches must be the tenant's, not the template's"
    );
}

/// **The second half, and the one that looks like it works.** The pack calls `http.request`'s
/// `execute` directly, bypassing `Executor::dispatch`, so this is the only place flux's egress
/// allow-list is consulted for the inner call. A subject naming `{subdomain}.zendesk.com` cannot be
/// matched by any rule an operator would write.
#[test]
fn the_permission_subject_is_the_host_the_request_reaches() {
    let subjects = zendesk("zendesk-ticket-show").permission_subjects(&json!({ "ticket_id": 1 }));

    assert_eq!(
        subjects,
        vec!["https://acme.zendesk.com/api/v2/tickets/1.json".to_string()],
        "the gate must be shown the host the request actually reaches"
    );
}

/// The subject on the **fallback** path — the one taken when the request cannot be built — is
/// substituted too. This is the case a gate is most likely to be wrong in, because it fires exactly
/// when a call is malformed.
#[test]
fn the_fallback_subject_is_substituted_too() {
    let subjects = zendesk("zendesk-ticket-show").permission_subjects(&json!({}));

    assert_eq!(
        subjects,
        vec!["acme.zendesk.com".to_string()],
        "a call that cannot be built must still be gated against a host that resolves"
    );
}

/// **Total or refused.** A connector whose tenant has supplied nothing does not send a request to a
/// host with a brace in it — it refuses, naming the field.
#[test]
fn an_unconfigured_endpoint_is_refused_by_name_rather_than_sent() {
    let error = projected("zendesk-ticket-show", configured(&[]))
        .build_request(&json!({ "ticket_id": 1 }))
        .expect_err("without a subdomain there is no URL");

    assert!(
        matches!(&error, Error::MissingConfig { field, provider, .. }
            if field == "endpoint.subdomain" && provider == "zendesk"),
        "{error}"
    );
    let rendered = error.to_string();
    assert!(rendered.contains(TENANT), "{rendered}");
    assert!(rendered.contains("was not sent"), "{rendered}");
}

/// A **partial** answer is refused on the same terms as no answer. `docusign` is the case that
/// makes this more than a restatement: its base URL carries two variables, one in the authority and
/// one in the path, so binding either alone would produce a URL that is *almost* right — and a
/// request to `https://na4.docusign.net/restapi/v2.1/accounts/{account_id}` is a call the vendor
/// answers.
#[test]
fn a_half_configured_host_is_refused_rather_than_half_substituted() {
    let operation = projected(
        "docusign-envelope-get",
        configured(&[("docusign", "default", "account_host", "na4.docusign.net")]),
    );

    assert_eq!(
        operation.endpoint_variables(),
        ["account_host".to_string(), "account_id".to_string()],
        "docusign is the two-placeholder case this test exists for"
    );

    let error = operation
        .build_request(&json!({ "envelope_id": "e-1" }))
        .expect_err("one of two variables is not a configured connector");
    assert!(
        matches!(&error, Error::MissingConfig { field, .. } if field == "endpoint.account_id"),
        "{error}"
    );
}

/// Both of docusign's variables, substituted — including the one in the path rather than the
/// authority, which is the half a host-only implementation would have missed.
#[test]
fn every_placeholder_of_a_two_variable_host_is_filled() {
    let request = projected(
        "docusign-envelope-get",
        configured(&[
            ("docusign", "default", "account_host", "na4.docusign.net"),
            ("docusign", "default", "account_id", "acme-account"),
        ]),
    )
    .build_request(&json!({ "envelope_id": "e-1" }))
    .expect("the request builds");

    assert!(
        !request.url.contains('{'),
        "docusign's two-placeholder host must resolve completely: {}",
        request.url
    );
    assert!(
        request
            .url
            .starts_with("https://na4.docusign.net/restapi/v2.1/accounts/acme-account/"),
        "both the authority and the path variable must be filled: {}",
        request.url
    );
}

/// **A caller cannot spend a tenant's configuration.** Substitution happens over the emitter's
/// *literals*, never over the finished URL, so a parameter whose text spells a variable is not
/// filled in — and C-478's caller path guard refuses the brace before a URL is sent.
///
/// Getting this backwards is an easy and expensive mistake: substituting over the finished URL is
/// one line shorter and would let a caller's argument reach the wire as a tenant's configured value.
#[test]
fn a_parameter_that_spells_a_variable_is_not_substituted() {
    let error = zendesk("zendesk-ticket-show")
        .build_request(&json!({ "ticket_id": "{subdomain}" }))
        .expect_err("a parameter is not configuration");

    assert!(
        matches!(
            &error,
            Error::UnsafePathParameter { parameter, .. } if parameter == "ticket_id"
        ),
        "{error}"
    );
}

/// An operation on a connector with a literal base URL needs no configuration, and asking for some
/// would make every untemplated connector refuse until a host bound values nobody needs.
#[test]
fn an_untemplated_connector_needs_no_configuration() {
    let operation = projected("slack-chat-post-message", configured(&[]));

    assert!(operation.endpoint_variables().is_empty());
    let request = operation
        .build_request(&json!({ "channel": "C0FLUX", "text": "hi", "thread_ts": null }))
        .expect("a literal base URL needs nothing bound");
    assert!(
        request.url.starts_with("https://slack.com/"),
        "{}",
        request.url
    );
}

/// **Two ports, one tenant.** Nothing in the types stops a host from pairing tenant A's credentials
/// with tenant B's connection settings, and the result of that mistake is one tenant's token sent to
/// another tenant's server. Refused where it is cheapest to notice.
#[test]
fn credentials_and_configuration_must_name_the_same_tenant() {
    let entry = catalog::operation(OperationKey::id("zendesk-ticket-show")).expect("it ships");
    let other = Configuration::new(Arc::new(MemoryConfig::new()), "t-somebody-else")
        .expect("a valid tenant id");

    let error = Operation::project(entry, http(), credentials(), other)
        .expect_err("two tenants are not one connector");
    assert!(matches!(error, Error::TenantMismatch { .. }), "{error}");
}

/// **A store whose answer drifts** — a database-backed one, a cache with a TTL, anything with
/// interior mutability. Every `get` for `endpoint.subdomain` answers with a different host, and the
/// reads are counted so the enforcement can be asserted rather than inferred.
#[derive(Default)]
struct Drifting {
    subdomain_reads: AtomicUsize,
}

impl ConfigStore for Drifting {
    fn get(
        &self,
        _tenant: &str,
        _provider: &str,
        _service: &str,
        field: Field<'_>,
    ) -> Option<String> {
        match field {
            Field::Endpoint("subdomain") => Some(format!(
                "host-{}",
                self.subdomain_reads.fetch_add(1, Ordering::SeqCst)
            )),
            _ => None,
        }
    }
}

/// **The time-of-check/time-of-use hole in the port itself (C-198).**
///
/// `permission_subjects` and `execute` used to perform two *independent* `get`s, and the pack calls
/// `http.request`'s `execute` directly — bypassing `Executor::dispatch` — so this crate's own
/// `permission_subjects` is the only place flux's egress allow-list is consulted for the inner call.
/// A store that answers differently on two calls therefore had the gate approve one host and the
/// request reach another, with the audit record naming the host that was never called.
///
/// The fix is enforcement rather than documentation: every value an operation can ask for is
/// resolved once, at `Operation::project`, and the operation holds no handle to the store
/// afterwards. So the second assertion is the stronger of the two — one read is not "the two reads
/// happened to agree", it is "there is no second read to disagree".
#[test]
fn a_store_that_answers_differently_cannot_gate_one_host_and_call_another() {
    let store = Arc::new(Drifting::default());
    let tool = projected(
        "zendesk-ticket-show",
        Configuration::new(store.clone(), TENANT).expect("a valid tenant id"),
    );
    let params = json!({ "ticket_id": 1 });

    let gated = tool.permission_subjects(&params);
    let sent = tool.build_request(&params).expect("the request builds").url;

    assert_eq!(
        gated,
        vec![sent.clone()],
        "the gate was shown a host the request did not reach: `{sent}` went out"
    );
    assert_eq!(
        store.subdomain_reads.load(Ordering::SeqCst),
        1,
        "a mutable store was consulted more than once, so the two answers can still diverge"
    );
}

/// The whole point, stated once over every configured operation in the shipped catalogue: none can
/// put a brace on the wire once its tenant is configured.
#[test]
fn no_templated_connector_reaches_the_wire_with_a_placeholder() {
    let empty = Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a tenant");
    let mut values = MemoryConfig::new();
    for entry in catalog::operations() {
        let probe = Operation::project(entry, http(), credentials(), empty.clone())
            .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));
        for variable in probe.endpoint_variables() {
            values =
                values.with_endpoint(TENANT, entry.provider, entry.service, variable, "a-value");
        }
    }
    let configuration = Configuration::new(Arc::new(values), TENANT).expect("a tenant");

    let mut templated = 0usize;
    for entry in catalog::operations() {
        let operation = Operation::project(entry, http(), credentials(), configuration.clone())
            .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));
        if operation.endpoint_variables().is_empty() {
            continue;
        }
        templated += 1;

        // The fallback path — no parameters at all — because it needs no per-operation argument
        // table and it is the path a malformed call takes.
        for subject in operation.permission_subjects(&json!({})) {
            assert!(
                !subject.contains('{'),
                "`{}` declares `{subject}`, which no host resolves and no allow-list matches",
                entry.id
            );
        }
    }

    // The control. Without it this passes trivially on a catalogue carrying no templated connector
    // at all, which is the state it exists to guard against regressing *from*.
    assert!(
        templated > 0,
        "no shipped operation declares a configuration variable, so this test asserted nothing"
    );
}
