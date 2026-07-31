//! New Relic (C-220) is the epic's probe for a configuration value drawn from a **closed set** —
//! a field where exactly two answers work and every other answer is a plausible, well-formed,
//! wrong one.
//!
//! Nine shipped connectors template their host, and every one of them templates a label the
//! *operator* owns: `{subdomain}.zendesk.com`, `{site}.atlassian.net`, `{shop}.myshopify.com`,
//! `{instance}.my.salesforce.com`, `{domain}` for freshdesk and okta. New Relic is the other
//! shape. It serves one API from two **vendor-owned** hosts — `api.newrelic.com` for a US account
//! and `api.eu.newrelic.com` for an EU one — the operator picks between them, and **the credential
//! does not say which one it belongs to**. Presenting a US key to the EU host returns `401`, which
//! is indistinguishable from a bad key: the failure names the credential and not the routing.
//!
//! Four findings, each pinned below:
//!
//! 1. **The host is bound to operator configuration, and neither region is baked in.** The US host
//!    is not a default here. A connector that hard-coded it would work for most accounts and fail
//!    an EU one with an authentication error, which is precisely the misdiagnosis this connector is
//!    shaped to avoid — `providers/intercom.toml` records the same vendor shape as an unclosed gap
//!    ("SCHEMA GAP: the regional hosts are not selectable"), and this is the first connector in the
//!    catalogue to actually bind one.
//! 2. **The closed set is not expressible, and this file measures the gap rather than describing
//!    it.** `ConfigField` has no way to say "one of these two". `Format` is a closed enum of
//!    *shapes* (`crates/connector-spec/src/config.rs:104-123`), not of *values*, and the nearest
//!    shape — `hostname` — admits every syntactically valid host on the internet. Both halves are
//!    asserted: the loader accepts a host with no relationship to New Relic, and the obvious
//!    declaration an author would reach for is refused as an unknown field. That is the finding
//!    C-220 exists to produce, and it is filed rather than worked around with `help` text.
//! 3. **Only the placeholder is templated, so the value cannot reshape the request beyond its own
//!    host.** `base_url` is `https://{host}/v2`: the version segment and every operation path are
//!    literal, so a wrong answer misroutes the request and cannot restructure it.
//! 4. **Nothing reaches the query string, and NRQL is not an operation.** The emitter interpolates
//!    query values verbatim (`crates/connector-flux/src/op.rs:138-143`, `AGENTS.md`'s
//!    `zendesk-ticket-search` gap), and New Relic's query surface is a whole query *language* —
//!    an unbounded operation dressed as a parameter. Both are excluded, and this file asserts the
//!    exclusion instead of trusting the curation to stay curated.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{
    config::{template_variables, Format, Level},
    provider, AuthScheme, Binding, Connector, HttpMethod, Idempotency, Risk,
};

/// The provider under test.
const PROVIDER: &str = "newrelic";

/// The one credential. New Relic's REST v2 authenticates with a User key in its own header.
const KEY: &str = "newrelic.api_key";
const KEY_HEADER: &str = "X-Api-Key";
/// A variable *name*; no credential value appears in this repository.
const KEY_ENV: &str = "NEW_RELIC_API_KEY";

/// The templated base URL. Only the host varies; the version segment is literal.
const BASE_URL: &str = "https://{host}/v2";
/// The `{placeholder}` the operator's answer fills, and the `[[config]]` field that fills it.
const HOST_VARIABLE: &str = "host";
const HOST_FIELD: &str = "host";

/// **The closed set the IR cannot express.** Exactly these two answers work.
const US_HOST: &str = "api.newrelic.com";
const EU_HOST: &str = "api.eu.newrelic.com";

/// The verification read — argument-free, so a settings page can run it unattended.
const VERIFY: &str = "newrelic-application-list";

/// The six curated operations, in the order `providers/newrelic.toml` declares them.
const OPERATIONS: &[&str] = &[
    "newrelic-application-list",
    "newrelic-application-get",
    "newrelic-alert-policy-list",
    "newrelic-alert-violation-list",
    "newrelic-deployment-list",
    "newrelic-deployment-create",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn source() -> String {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-220 ships the New Relic connector",
            path.display()
        )
    })
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    provider::load(&format!("providers/{PROVIDER}.toml"), &source())
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// **Finding 1: the region host is operator-supplied, and neither region is a default.**
///
/// `base_url` carries exactly one `{placeholder}` and a `[[config]]` field binds it. The field is
/// connection level (one per tenant — two customers of one deployment may sit in two regions),
/// non-secret (it is a hostname the operator reads back in a settings page), and required, because
/// there is no answer this connector could substitute if it were omitted.
#[test]
fn the_region_host_is_operator_bound_and_neither_region_is_baked_in() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "New Relic");
    assert_eq!(connector.authority.as_deref(), Some("com.newrelic.api"));
    assert_eq!(
        connector.base_url, BASE_URL,
        "the host is a placeholder; the version segment is not. Hard-coding `{US_HOST}` would \
         serve most accounts and fail every EU one with a 401 that names the credential"
    );
    assert_eq!(
        template_variables(&connector.base_url),
        [HOST_VARIABLE],
        "exactly one variable, so exactly one thing an operator can get wrong"
    );

    let field = connector
        .config
        .iter()
        .find(|field| field.name == HOST_FIELD)
        .unwrap_or_else(|| panic!("newrelic declares the `{HOST_FIELD}` config field"));
    assert_eq!(
        field.binding(),
        Some(Binding::Endpoint {
            variable: HOST_VARIABLE
        }),
        "the field fills the base URL's placeholder and nothing else"
    );
    assert_eq!(
        field.level(),
        Some(Level::Connection),
        "two tenants of one deployment may live in two regions, so this is not set once per vendor"
    );
    assert!(
        !field.secret,
        "a hostname is configuration an operator reads back, not a credential"
    );
    assert!(
        field.required,
        "there is no host this connector could assume"
    );
    assert_eq!(field.format, Format::Hostname);
    assert!(
        !field.label.is_empty() && !field.help.is_empty(),
        "`{HOST_FIELD}` is renderable"
    );
    assert!(
        field.help.contains(US_HOST) && field.help.contains(EU_HOST),
        "the two valid answers are unspellable in the declaration, so the help text is the only \
         place they are written down — and that is the whole finding. Got: {}",
        field.help
    );
    assert!(
        field.docs_url.is_some(),
        "an operator who does not know their region needs the vendor's own page, not ours"
    );
}

/// **Finding 2, and the story's whole purpose: the closed set of two hosts cannot be declared.**
///
/// New Relic's host is not free text that happens to have two common answers — it has *exactly*
/// two answers, and every third one is wrong. Nothing in the IR says so:
///
/// - [`Format`] is a closed enum of value *shapes*, not of values. `hostname` is the nearest, and
///   it accepts any syntactically valid host, so the loader is content with a host that has
///   nothing to do with New Relic. Measured below on the shipped file itself, via the one lever
///   this crate gives a test: the loader validates `example` against `format`, so an `example`
///   naming an unrelated host is a load the connector should refuse and does not.
/// - The declaration an author reaches for next — a list of permitted values on the field — is not
///   a field. `ConfigField` is `#[serde(deny_unknown_fields)]`, so adding one is a load error
///   rather than a key that is quietly accepted and ignored. That refusal is what makes this a gap
///   in the model rather than an omission in this file.
///
/// **The consequence, stated plainly:** an operator who picks the wrong host gets a `401` on every
/// call, and that `401` is indistinguishable from a bad key. Nothing in this repository can catch
/// it, nothing in a form can catch it, and the only mitigation shipped here is prose in `help`. It
/// is filed rather than papered over — see the story's Progress note.
#[test]
fn the_closed_set_of_two_hosts_is_not_expressible_and_the_field_admits_any_host() {
    // Both documented answers satisfy the field. Neither is privileged by the declaration.
    for host in [US_HOST, EU_HOST] {
        assert_eq!(
            Format::Hostname.validate(host),
            Ok(()),
            "{host} is one of the two answers this connector accepts"
        );
    }

    // And so does a host with no relationship to the vendor at all. This is the gap: a value that
    // is well-formed, accepted, and wrong.
    let unrelated = "api.not-new-relic.example";
    assert_eq!(
        Format::Hostname.validate(unrelated),
        Ok(()),
        "`hostname` constrains the shape of a host, never which host — there is no format, and no \
         other field, that says `one of these two`"
    );

    let shipped = source();
    let example = format!("example = \"{US_HOST}\"");
    assert!(
        shipped.contains(&example),
        "the shipped file must carry `{example}` for the substitutions below to prove anything"
    );

    // The loader checks `example` against `format` (`Format::validate`, applied at load), so this
    // is the one place a test can watch the field's own validation run against a value.
    for accepted in [EU_HOST, unrelated] {
        let mutated = shipped.replace(&example, &format!("example = \"{accepted}\""));
        assert_ne!(mutated, shipped, "the substitution must actually apply");
        assert!(
            provider::load(&format!("providers/{PROVIDER}.toml"), &mutated).is_ok(),
            "the connector loads with `{accepted}` as the host operators are shown. For {EU_HOST} \
             that is correct and necessary; for {unrelated} it is the defect, and the two are \
             indistinguishable to every check this repository runs"
        );
    }

    // The declaration that would close it does not exist. `deny_unknown_fields` turns the guess
    // into a load error, which is the honest answer — an accepted-and-ignored key would be worse.
    let with_a_closed_set = shipped.replace(
        &example,
        &format!("values = [\"{US_HOST}\", \"{EU_HOST}\"]\n{example}"),
    );
    let error = provider::load(&format!("providers/{PROVIDER}.toml"), &with_a_closed_set)
        .expect_err("`ConfigField` has no way to enumerate the values a field permits");
    assert!(
        error.to_string().contains("values"),
        "expected an unknown-field refusal naming `values`, got: {error}"
    );
}

/// **Finding 3: one credential, in New Relic's own header, on every operation.**
///
/// REST v2 takes a User key as `X-Api-Key`, with no prefix — the whole header value is the secret.
/// There is one mechanism, so there is one alternative; a second entry would tell a host that
/// either half authenticates a request.
#[test]
fn every_operation_authenticates_with_the_user_key_header() {
    let connector = load();

    assert_eq!(connector.auth.len(), 1);
    let key = connector
        .auth_method(KEY)
        .unwrap_or_else(|| panic!("newrelic declares `{KEY}`"));
    assert_eq!(
        key.scheme,
        AuthScheme::Header {
            name: KEY_HEADER.to_string(),
            prefix: String::new(),
        },
        "New Relic's REST v2 key travels in its own header, and the header value is the key alone"
    );
    assert_eq!(key.env, [KEY_ENV]);

    assert_eq!(connector.default_auth.len(), 1);
    let mechanism: Vec<&str> = connector.default_auth[0]
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(mechanism, [KEY]);

    for id in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("newrelic declares `{id}`"));
        let effective: Vec<Vec<&str>> = connector
            .effective_auth(operation)
            .iter()
            .map(|requirement| requirement.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(
            effective,
            vec![vec![KEY]],
            "{id} carries the account key; none overrides the default"
        );
    }

    // The credential is a credential and the host is not. The two travel together on every request
    // and only one of them is gated — which is exactly why a wrong host reads as a bad key.
    let host = connector
        .config
        .iter()
        .find(|field| field.name == HOST_FIELD)
        .expect("the host field");
    assert_eq!(
        host.pin(),
        None,
        "the host fills a base URL, not a request position"
    );
    assert!(!Binding::Endpoint {
        variable: HOST_VARIABLE
    }
    .is_secret());
}

/// **Finding 4: the curated surface, and what it deliberately leaves out.**
///
/// Six operations: five reads and one write. The write is a deployment marker, which is
/// `non_idempotent` because New Relic documents no idempotency key on it — this repository's
/// convention (first written for `providers/asana.toml`) is that a write earns `idempotent` only
/// from a vendor-documented guarantee, never from an accident of implementation. Calling it twice
/// records two deployments.
///
/// Two exclusions are asserted rather than described, because both would be easy to add later
/// without noticing what they cost:
///
/// - **No query parameter anywhere.** Every filter New Relic documents on these endpoints —
///   `filter[name]`, `only_open`, `start_date`/`end_date` — would be interpolated verbatim into a
///   URL by an emitter that percent-encodes nothing. They belong to whichever story closes the
///   query-encoding gap (`docs/designs/query-encoding-flux-stories.md`), not to this one.
/// - **No NRQL.** A free-form query string is not a curated operation: it would make this
///   connector's surface unbounded, and the risk and effects of a call would depend on text a
///   caller supplied rather than on anything declared here.
#[test]
fn the_curated_set_is_five_reads_and_one_deployment_marker() {
    let connector = load();

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        declared, OPERATIONS,
        "the curated set, in declaration order"
    );

    for operation in &connector.operations {
        assert!(
            !operation.description.is_empty(),
            "{} carries a description — the text a model receives as its tool contract",
            operation.id
        );
        assert!(
            operation.params.query.is_empty(),
            "{} declares a query parameter, and every query value this emitter writes is \
             interpolated verbatim",
            operation.id
        );

        let flux = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{} does not emit: {error}", operation.id));
        assert!(
            !flux.contains('?'),
            "{} emits a `?` into its URL:\n{flux}",
            operation.id
        );
        assert!(
            !flux.contains("nrql") && !flux.contains("NRQL"),
            "{} reaches New Relic's query language, which is an unbounded surface:\n{flux}",
            operation.id
        );
    }

    let reads: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| operation.method == HttpMethod::Get)
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        reads,
        &OPERATIONS[..5],
        "five reads, and they are the first five declared"
    );
    for id in &OPERATIONS[..5] {
        let operation = connector.operation(id).expect("a declared read");
        assert_eq!(operation.risk, Risk::Low, "{id} only reads");
        assert_eq!(operation.idempotency, Idempotency::Idempotent, "{id}");
    }

    let write = connector
        .operation("newrelic-deployment-create")
        .expect("the one write");
    assert_eq!(write.method, HttpMethod::Post);
    assert_eq!(
        write.risk,
        Risk::Medium,
        "a deployment marker is visible to everyone on the account and cannot be removed through \
         this connector"
    );
    assert_eq!(
        write.idempotency,
        Idempotency::NonIdempotent,
        "New Relic documents no idempotency key, so calling it twice records two deployments"
    );

    // The write's body is nested under `deployment`, which is a `wire` path rather than a
    // caller-facing name — the shape `providers/pagerduty.toml` and `providers/asana.toml` ship.
    let body: Vec<(&str, &str)> = write
        .params
        .body
        .iter()
        .map(|param| {
            (
                param.name.as_str(),
                param.wire.as_deref().unwrap_or(param.name.as_str()),
            )
        })
        .collect();
    assert_eq!(
        body,
        [
            ("revision", "deployment.revision"),
            ("changelog", "deployment.changelog"),
            ("description", "deployment.description"),
            ("user", "deployment.user"),
        ],
        "caller-facing name on the left, the spelling New Relic sees on the right"
    );
    let flux = emit_operation(&connector, write).expect("the deployment write emits");
    assert!(
        flux.contains("content_type = \"application/json\""),
        "the deployment marker sends a JSON body:\n{flux}"
    );

    // Nothing here pins a request component: the region is the only operator-supplied value, and
    // it reaches a base URL. A New Relic account id is not a path pin — every path below is
    // literal or takes an application id the caller chooses per call.
    for field in &connector.config {
        assert!(
            !matches!(field.binding(), Some(Binding::Request { .. })),
            "`{}` pins a request position; this connector's only configured value is its host",
            field.name
        );
    }
}

/// **`verify` is a read that runs unattended.**
///
/// A "Test connection" button is pressed whenever someone opens a settings page, so it must be a
/// read (the loader checks the declared risk) *and* take no argument, which the loader does not
/// check and a connector can still get wrong.
///
/// It carries a second job here that no other connector's `verify` has: it is the **only** signal
/// that distinguishes a wrong region from a wrong key, and it cannot do it. `GET /v2/applications`
/// answers `401` in both cases. The story records that; this test records that the read at least
/// exists and runs.
#[test]
fn verify_is_an_argument_free_read() {
    let connector = load();

    assert_eq!(connector.verify.as_deref(), Some(VERIFY));
    let operation = connector
        .operation(VERIFY)
        .expect("verify names an operation");

    assert_eq!(operation.method, HttpMethod::Get);
    assert_eq!(operation.risk, Risk::Low);
    assert_eq!(operation.idempotency, Idempotency::Idempotent);
    assert!(
        operation.params.path.is_empty()
            && operation.params.query.is_empty()
            && operation.params.body.is_empty()
            && operation.params.header.is_empty()
            && operation.params.body_schema.is_none(),
        "a connection test that needs an argument cannot run unattended"
    );
}
