//! **C-214: a configuration value is checked where it is *substituted*, not only where it is
//! declared.**
//!
//! `connector-spec` has had the predicate since C-187 — `Position::validate_value` — and it has two
//! call sites, both in the loader, both running against a `ConfigField::example` or a parameter
//! *name*. Nothing ran against the value a tenant actually supplies, because this repository never
//! sees one: it arrives through [`ConfigStore`], at the host, at request time. So the guard existed
//! and did not guard.
//!
//! The severe half is the **host** position, and it predates the pin surface C-187 added. A path or
//! query value cannot move the origin — substitution lands after the authority is fixed in the
//! `base` literal — but nine shipped connectors template the *host* itself, and there the `@` in
//! `acme.zendesk.com@evil.example` turns everything before it into userinfo:
//!
//! ```text
//! https://acme.zendesk.com@evil.example.zendesk.com/api/v2/tickets/1
//!         └──────── userinfo ─────┘└─── the host that is actually resolved ───┘
//! ```
//!
//! The value is operator-supplied rather than attacker-supplied, so this is a paste-the-wrong-thing
//! hazard rather than a classic injection. What makes it worth a refusal anyway is where the wrong
//! thing goes: a host the operator never named, carrying that operator's own token, through an
//! egress gate that was shown a subject ending in `zendesk.com`.

use std::collections::BTreeMap;
use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{
    Configuration, Credentials, Egress, MemoryConfig, MemoryStore, Operation, Request,
};
use flux_runtime::Tool;
use serde_json::json;

/// The tenant both ports answer for.
const TENANT: &str = "t-c214";

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

/// A configuration port holding exactly the `(provider, service, variable, value)` rows given.
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

/// The host a URL actually resolves to: the authority, minus any userinfo, minus any port.
///
/// Hand-rolled rather than taken from a URL crate, because the point of the assertion is to read
/// the URL the way a transport does and this crate deliberately links none.
fn resolved_host(url: &str) -> &str {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let authority = after_scheme
        .find(['/', '?', '#'])
        .map_or(after_scheme, |end| &after_scheme[..end]);
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    host.rsplit_once(':')
        .filter(|(_, port)| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
        .map_or(host, |(host, _)| host)
}

/// The request an operation builds, or the refusal it produced — reported as the URL it would have
/// sent and the host that URL actually resolves to, which is the fact the whole story is about.
fn built(operation: &Operation, params: serde_json::Value) -> Result<Request, String> {
    operation
        .build_request(&params)
        .map_err(|error| error.to_string())
}

/// **The failing-first test.** A `subdomain` of `acme.zendesk.com@evil.example` must not produce a
/// request whose authority is `evil.example.zendesk.com`.
///
/// Before this story `zendesk-ticket-show` built that URL and returned it happily, so the assertion
/// is written as "the request was refused, and the refusal says which field and which operation" —
/// the two facts an operator needs in order to go and fix the value they pasted.
#[test]
fn a_host_value_cannot_move_the_origin() {
    let operation = projected(
        "zendesk-ticket-show",
        configured(&[(
            "zendesk",
            "default",
            "subdomain",
            "acme.zendesk.com@evil.example",
        )]),
    );

    let refusal = match built(&operation, json!({"ticket_id": 1})) {
        Ok(request) => panic!(
            "the request was built and it resolves to `{}`, not to a zendesk subdomain: {}",
            resolved_host(&request.url),
            request.url
        ),
        Err(refusal) => refusal,
    };

    assert!(
        refusal.contains("zendesk-ticket-show"),
        "the refusal does not name the operation: {refusal}"
    );
    assert!(
        refusal.contains("subdomain"),
        "the refusal does not name the field: {refusal}"
    );
}

/// A value that is a subdomain and nothing else still works, which is the half that makes the guard
/// a guard rather than a ban. Nine connectors template their host; all of them must keep resolving.
#[test]
fn an_ordinary_subdomain_still_resolves() {
    let operation = projected(
        "zendesk-ticket-show",
        configured(&[("zendesk", "default", "subdomain", "acme")]),
    );

    let request = built(&operation, json!({"ticket_id": 1})).expect("`acme` is a subdomain");
    assert_eq!(resolved_host(&request.url), "acme.zendesk.com");
    assert_eq!(request.url, "https://acme.zendesk.com/api/v2/tickets/1");
}

/// Every other way a host value has been observed to move the authority, refused for the same
/// reason and by the same rule.
///
/// The point of the allow-list is that this list is not the specification — a character the rule
/// does not permit is refused whether or not anyone thought of it here.
#[test]
fn no_host_value_reshapes_the_authority() {
    for value in [
        "acme.zendesk.com@evil.example", // userinfo: the measured case
        "acme@evil.example",             // the same shape, shorter
        "acme:8080",                     // a port the connector never declared
        "acme/../other",                 // ends the authority and walks the path
        "acme%2eevil",                   // a percent-escape that could decode to a delimiter
        "acme evil",                     // whitespace
        "acme\nevil",                    // a control character
        "acmé",                          // non-ASCII, foldable onto another label by IDNA
        "",                              // nothing at all
        " ",                             // whitespace only — C-214's `config.rs:278` half
    ] {
        let operation = projected(
            "zendesk-ticket-show",
            configured(&[("zendesk", "default", "subdomain", value)]),
        );
        let Err(refusal) = built(&operation, json!({"ticket_id": 1})) else {
            panic!("`{value}` was accepted as a zendesk subdomain");
        };
        assert!(
            refusal.contains("zendesk-ticket-show") && refusal.contains("subdomain"),
            "`{value}`: the refusal names neither the operation nor the field: {refusal}"
        );
    }
}

/// **A path pin refuses rather than encodes**, and the story says why: a `zone_id` with a slash in
/// it is an operator's mistake, and silently percent-encoding it produces a 404 they cannot
/// diagnose. Every probe the story measured against the shipped catalogue is here.
#[test]
fn a_path_value_stays_inside_its_segment() {
    let good = projected(
        "cloudflare-dns-record-list",
        configured(&[(
            "cloudflare",
            "default",
            "zone_id",
            "023e105f4ecef8ad9ca31a8372d0c353",
        )]),
    );
    assert_eq!(
        built(&good, json!({}))
            .expect("a zone id is a path segment")
            .url,
        "https://api.cloudflare.com/client/v4/zones/023e105f4ecef8ad9ca31a8372d0c353/dns_records"
    );

    for value in [
        "../../v4/other",
        "x/../../y",
        "abc?evil=1",
        "abc#frag",
        "abc%2Fdef",
        "abc\ndef",
        "..",
        " ",
    ] {
        let operation = projected(
            "cloudflare-dns-record-list",
            configured(&[("cloudflare", "default", "zone_id", value)]),
        );
        let Err(refusal) = built(&operation, json!({})) else {
            panic!("`{value}` was accepted as a path segment");
        };
        assert!(
            refusal.contains("cloudflare-dns-record-list") && refusal.contains("zone_id"),
            "`{value}`: the refusal names neither the operation nor the field: {refusal}"
        );
    }
}

/// **A query pin goes through the encoder that already exists.**
///
/// Two halves, and both matter. Query *structure* is refused — encoding an `&` would send `%26`
/// where the operator plainly meant a separator, which the vendor answers with something confusing
/// rather than with a diagnosis. Everything else is percent-encoded by `auth::query_encode`, which
/// is the identity over RFC 3986's unreserved set — so an ordinary team id travels byte for byte as
/// it did before this story.
#[test]
fn a_query_value_is_encoded_by_the_encoder_that_already_exists() {
    let unchanged = projected(
        "vercel-projects-list",
        configured(&[("vercel", "default", "teamId", "team_abc123")]),
    );
    assert_eq!(
        built(&unchanged, json!({}))
            .expect("a team id is a query value")
            .url,
        "https://api.vercel.com/v10/projects?teamId=team_abc123"
    );

    let encoded = projected(
        "vercel-projects-list",
        configured(&[("vercel", "default", "teamId", "team/abc:123")]),
    );
    assert_eq!(
        built(&encoded, json!({}))
            .expect("a reserved character with one meaning is encoded")
            .url,
        "https://api.vercel.com/v10/projects?teamId=team%2Fabc%3A123"
    );

    for value in [
        "team_a&projectId=evil",
        "a=b",
        "a?b",
        "a#b",
        "a b",
        "a\nb",
        " ",
    ] {
        let operation = projected(
            "vercel-projects-list",
            configured(&[("vercel", "default", "teamId", value)]),
        );
        let Err(refusal) = built(&operation, json!({})) else {
            panic!("`{value}` was accepted as a query value");
        };
        assert!(
            refusal.contains("vercel-projects-list") && refusal.contains("teamId"),
            "`{value}`: the refusal names neither the operation nor the field: {refusal}"
        );
    }
}

/// **A raw newline cannot reach a header pin** — proved against a fixture, because no shipped
/// provider declares a header pin yet (C-164's Algolia will be the first).
///
/// This is the one position here with a classic exploit: a CR/LF in a field value appends a header
/// of the value's choosing to *every* request the service makes.
///
/// # The fixture is a **document** now, and it had to become one (C-538)
///
/// It used to be a real catalogue entry with its emitted Flux replaced by the shape
/// `connector-flux` emits for a header pin. `Operation::build_request` reads the canonical document
/// since C-538, so a doctored *module* no longer changes the request the pack composes — the
/// fixture would have kept passing while exercising nothing at all, which is worse than deleting
/// it. So the same connector shape is written in the artifact the request is now derived from: one
/// operation whose `endpoint` map places `app_id` in a `header` and `teamId` in a `query`.
///
/// **The guard itself did not move and did not change.** `Slot::Header`'s rule is
/// `connector-resolve`'s, character for character the rule `connector-pack` held before, and the
/// seven assertions above this one still run against shipped connectors through
/// `Operation::build_request`.
#[test]
fn a_newline_cannot_reach_a_header_pin() {
    /// The shape a connector declaring a header pin lowers to: the pin is a `{placeholder}` in the
    /// header's value template, and the document's `endpoint` map says where it lands.
    fn fixture() -> String {
        serde_json::to_string(&json!({
            "connector": "vercel",
            "services": [{"name": "default", "base_url": "https://api.vercel.com"}],
            "operations": [{
                "id": "vercel-projects-list",
                "service": "default",
                "expose": true,
                "params": [],
                "endpoint": {"teamId": ["query"], "app_id": ["header"]},
                "request": {
                    "method": "GET",
                    "url": "{base}/v10/projects",
                    "headers": {"X-App-Id": "{app_id}"},
                    "query": [{"name": "teamId", "value": "{teamId}"}],
                },
            }],
        }))
        .expect("the fixture serializes")
    }

    let document = connector_resolve::document::Document::parse(&fixture())
        .expect("the fixture is a canonical document");
    let operation = document
        .operation("vercel-projects-list")
        .expect("the fixture carries it");
    let base = document.base_url("default").expect("the fixture's service");

    let built = |app_id: &str| {
        connector_resolve::build_request(
            operation,
            base,
            &json!({}),
            &BTreeMap::from([
                ("teamId".to_string(), "team_abc123".to_string()),
                ("app_id".to_string(), app_id.to_string()),
            ]),
        )
        .map_err(|error| error.to_string())
    };

    let sent = built("APP123").expect("an application id is a field value");
    assert_eq!(
        sent.headers.get("X-App-Id").map(String::as_str),
        Some("APP123")
    );
    assert!(
        sent.url.ends_with("/v10/projects?teamId=team_abc123"),
        "{}",
        sent.url
    );

    for value in [
        "APP123\r\nX-Injected: 1", // the classic: a header of the value's own choosing
        "APP123\nX-Injected: 1",   // a bare LF, which some stacks accept as a terminator
        "APP123\r",
        "APP\u{0}123", // a NUL, which truncates in a C-string transport
        " APP123",     // leading whitespace, which RFC 9110 §5.5 does not permit
        "APP123 ",
        " ",
    ] {
        let Err(refusal) = built(value) else {
            panic!("`{}` was accepted as a header value", value.escape_debug());
        };
        assert!(
            refusal.contains("vercel-projects-list") && refusal.contains("app_id"),
            "`{}`: the refusal names neither the operation nor the field: {refusal}",
            value.escape_debug()
        );
    }
}

/// **The gate and the wire do not diverge.**
///
/// `Operation::subjects` hands a host's egress allow-list the URL a request would carry, and falls
/// back to substituting the *declared* hosts when a request cannot be built. A refused value is
/// exactly the case that takes the fallback, so filling it in there would offer an allow-list
/// `acme.zendesk.com@evil.example.zendesk.com` to match on while the request itself never happens.
/// The placeholder is left verbatim instead, which nothing matches.
#[test]
fn a_refused_value_does_not_reach_the_egress_subject_either() {
    let operation = projected(
        "zendesk-ticket-show",
        configured(&[(
            "zendesk",
            "default",
            "subdomain",
            "acme.zendesk.com@evil.example",
        )]),
    );

    let subjects = operation.permission_subjects(&json!({"ticket_id": 1}));
    assert!(
        subjects
            .iter()
            .all(|subject| !subject.contains("evil.example")),
        "a refused value reached the egress subject: {subjects:?}"
    );
    assert!(
        subjects
            .iter()
            .any(|subject| subject.contains("{subdomain}")),
        "the unsubstituted placeholder is what fails closed, and it is not there: {subjects:?}"
    );
}

/// Whitespace-only survived the empty-string filter at `crates/connector-pack/src/config.rs:278`
/// and reached the wire as `?teamId=%20`. An all-whitespace configuration value is not a value, and
/// treating it as *absent* is what makes the refusal name the field an operator has to go and fix.
#[test]
fn an_all_whitespace_value_is_no_value_at_all() {
    for blank in [" ", "  ", "\t", "\n", " \t "] {
        let operation = projected(
            "vercel-projects-list",
            configured(&[("vercel", "default", "teamId", blank)]),
        );
        let Err(refusal) = built(&operation, json!({})) else {
            panic!("{:?} was accepted as a team id", blank.escape_debug());
        };
        assert!(
            refusal.contains("endpoint.teamId") && refusal.contains("supplies none"),
            "{:?}: whitespace should read as unconfigured, not as a value: {refusal}",
            blank.escape_debug()
        );
    }
}
