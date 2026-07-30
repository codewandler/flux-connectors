//! The HubSpot connector's own contract (C-72) — the claims that are true of *this* provider and of
//! no other, held against `providers/hubspot.toml` as committed.
//!
//! `shipped_modules.rs` next door asserts the properties every provider shares: each operation
//! emits, parses, is canonical and reloads. This file exists because two of HubSpot's properties are
//! **correctness** claims the shared gates cannot make, and both fail *silently* if they regress.
//!
//! # 1. Writes are wrapped in a `properties` envelope
//!
//! HubSpot's CRM v3 write body is `{"properties": {"email": …}}`, never a flat `{"email": …}`. The
//! flat form is **accepted, ignored and answered 2xx** — it stores nothing. That is the same class of
//! failure `providers/zendesk.toml` shipped before [`connector_spec::Param::wire`] existed (a flat
//! ticket body Zendesk answers 200 to), and it is the worst possible one: the caller is told the write
//! succeeded and the CRM record is unchanged. So the envelope is asserted field by field over the
//! emitted payload rather than approximated by "the module mentions `email`".
//!
//! # 2. Nothing reaches the query string, of any type
//!
//! C-30 is not implemented. `connector-flux` interpolates a query value into the URL verbatim and
//! percent-encodes nothing (`crates/connector-flux/src/op.rs`), so a string-ish value carrying `&`,
//! `#` or `+` corrupts the request and can *inject a parameter* — `zendesk-ticket-search` is the
//! standing proof, and AGENTS.md lists it under intentional gaps.
//!
//! The claim asserted here is the strong one — **no query parameter of any type** — and it is
//! deliberately stronger than "no string-ish query parameter". HubSpot's `archived` filter is a
//! boolean, which `connector-cli`'s `status.rs` would classify as safely interpolated and *not* flag;
//! the reason it is still excluded is that "the query surface is empty" is a property that survives
//! editing, whereas "the query values happen to be numeric" is one that invites the next author to
//! add a `properties=` projection next to them. The same reasoning shapes `providers/openai.toml`
//! and `providers/slack.toml`.
//!
//! It is asserted twice, over the IR and over the emitted text, because those are different claims.
//! The emitter re-binds `$url` once per *optional* query parameter inside a `when` guard and puts the
//! `?` on a separate `$sep` binding, so checking only the first `$url` line would prove nothing —
//! hence the assertion that each module binds `$url` exactly once.
//!
//! # Why the assertions are stated over the loaded IR and freshly emitted text
//!
//! Reading the TOML with a regex would pass on a file the loader rejects, and asserting on the
//! committed `connectors/hubspot.flux` would pass on bytes the emitter no longer produces. So the
//! file is loaded through the real loader and the Flux is re-emitted here, exactly as
//! `shipped_modules.rs` does. Staleness of the committed artifacts is `connector-cli`'s
//! `catalog_artifacts.rs` and `site_catalog.rs`; that is a different failure with its own test.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod, Idempotency, Risk};

/// The provider id, and therefore the file name, the module name and every op id's prefix.
const PROVIDER: &str = "hubspot";

/// HubSpot serves every portal from one host, so `base_url` carries no tenant template — unlike
/// zendesk's `{subdomain}` and freshdesk's `{domain}`, which are unbound (C-17). This connector's
/// whole egress surface is therefore this single literal name.
const BASE_URL: &str = "https://api.hubapi.com";

/// The one environment variable the connector resolves its secret from. A **name**, never a value —
/// AGENTS.md's hard invariant, re-asserted from the emitted text in
/// [`the_emitted_flux_carries_no_credential_at_all`].
const SECRET_ENV: &str = "HUBSPOT_ACCESS_TOKEN";

/// The credential the connector declares. Public contract: an operator sets the variable above and a
/// manifest names this, so both are pinned here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "hubspot.access_token";

/// The five curated operations, in the order `providers/hubspot.toml` declares them.
const OPERATIONS: &[&str] = &[
    "hubspot-contact-get",
    "hubspot-contact-create",
    "hubspot-contact-update",
    "hubspot-company-get",
    "hubspot-deal-get",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// `providers/hubspot.toml`, through the real loader.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-72 ships this file",
            path.display()
        )
    });
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// One operation's freshly emitted Flux.
fn emit(connector: &Connector, id: &str) -> String {
    let operation = connector
        .operation(id)
        .unwrap_or_else(|| panic!("providers/{PROVIDER}.toml declares `{id}`"));
    emit_operation(connector, operation)
        .unwrap_or_else(|error| panic!("`{id}` is not emittable: {error}"))
}

/// Whether the method changes state, and therefore whether the write rules below apply.
fn mutates(method: HttpMethod) -> bool {
    matches!(
        method,
        HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete
    )
}

/// **The connector exists, loads, and authenticates with one bearer private-app token.**
///
/// Everything below reads the loaded IR, so a failure here would otherwise surface five times over
/// as something less specific.
#[test]
fn the_hubspot_connector_loads_and_authenticates_with_a_bearer_private_app_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "HubSpot");
    assert_eq!(connector.base_url, BASE_URL);

    let [method] = connector.auth.as_slice() else {
        panic!(
            "HubSpot declares exactly one credential; found {}. The deprecated `hapikey` is the only \
             reason it would ever be more than one, and it must not be modelled",
            connector.auth.len()
        );
    };
    assert_eq!(method.name, CREDENTIAL);
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "a private-app access token travels as `Authorization: Bearer <token>`"
    );
    assert_eq!(method.env, [SECRET_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a bearer has no user half"
    );
    assert!(
        method.oauth2.is_none(),
        "a private-app token is minted in HubSpot's UI; flux runs no grant for it"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    // `effective_auth`, not `Operation::auth`: an operation that declares nothing inherits
    // `default_auth`, and one declaring an explicit `[]` inherits nothing. Reading the field
    // directly would let an accidentally-unauthenticated operation pass.
    for operation in &connector.operations {
        let alternatives = connector.effective_auth(operation);
        assert_eq!(
            alternatives.len(),
            1,
            "`{}` has {} auth alternatives; HubSpot is single-mechanism",
            operation.id,
            alternatives.len()
        );
        assert!(
            alternatives[0].contains(CREDENTIAL) && alternatives[0].len() == 1,
            "`{}` names a credential other than the private-app token",
            operation.id
        );
    }
}

/// **The `hapikey` query parameter is not modelled** — and its absence is the point rather than an
/// oversight.
///
/// HubSpot's legacy API key was sunset in 2022, and it is excluded for two independent reasons: it
/// puts a live secret in a query string, which is a credential leak into logs and referrers *and* an
/// unencodable value (C-30). This is the same class of decision babelforce records for its deprecated
/// `X-Auth-Access-Id`/`X-Auth-Access-Token` pair, and it is checked the same way — **structurally,
/// not by grepping the file**. `providers/hubspot.toml` does name `hapikey`, in the comment telling a
/// future reader not to add it back, and a text scan could not tell that warning apart from a
/// declaration. What matters is that nothing reaches the IR: no second credential, and no
/// caller-supplied parameter in any request position that could carry one.
#[test]
fn the_deprecated_hapikey_credential_never_reaches_the_ir_or_the_wire() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.params.header.is_empty(),
            "`{}` declares caller-supplied headers; the Authorization header is injected by the \
             host and must never travel through the parameter surface",
            operation.id
        );
        for param in operation.params.iter() {
            let wire = param.wire.as_deref().unwrap_or(&param.name);
            assert!(
                !param.name.eq_ignore_ascii_case("hapikey")
                    && !wire.eq_ignore_ascii_case("hapikey"),
                "`{}` declares `{}`, HubSpot's sunset API key. It is deprecated, and as a query \
                 value it would put a live secret in a URL — a credential leak and an unencodable \
                 value (C-30) at once",
                operation.id,
                param.name
            );
        }

        let emitted = emit(&connector, &operation.id);
        assert!(
            !emitted.to_lowercase().contains("hapikey"),
            "`{}` emits `hapikey`:\n{emitted}",
            operation.id
        );
    }
}

/// **The headline invariant: no query parameter of any type.** See the module documentation — the
/// strong form is the one that stays true under editing, and a boolean `archived` filter would pass
/// the weak one.
#[test]
fn no_hubspot_operation_declares_a_query_parameter_of_any_type() {
    let connector = load();

    for operation in &connector.operations {
        let declared: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        assert!(
            declared.is_empty(),
            "`{}` declares query parameters {declared:?}. C-30 is not implemented, so a query value \
             reaches the wire unencoded and `&`, `#` or `+` inside it can inject a parameter \
             (`zendesk-ticket-search` is the standing proof). Leave the operation out and name it in \
             the provider file's Notes instead, or close C-30 first",
            operation.id
        );
    }
}

/// The same claim over the **emitted text**, which is what flux actually loads.
///
/// Not a restatement of the test above: the IR and the emitted request can disagree, and the emitter
/// assembles a query string from `params.query` alone. A required query parameter lands in the first
/// `$url` template; an *optional* one re-binds `$url` inside a `when` guard with the `?`/`&` carried
/// on `$sep`. So the URL is pinned three ways — one `$url` binding, no `$sep`, no `?` anywhere.
#[test]
fn no_hubspot_module_assembles_a_query_string() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit(&connector, &operation.id);
        assert_eq!(
            emitted.matches("$url =").count(),
            1,
            "`{}` binds `$url` more than once, which is how the emitter appends an optional query \
             parameter inside a `when` guard:\n{emitted}",
            operation.id
        );
        assert!(
            !emitted.contains("$sep"),
            "`{}` emits query-string separator machinery, so a value is reaching the URL \
             unencoded:\n{emitted}",
            operation.id
        );
        assert!(
            !emitted.contains('?'),
            "`{}` emits a `?` in its request URL:\n{emitted}",
            operation.id
        );
    }
}

/// **Every write wraps its fields in HubSpot's `properties` envelope.** The silent failure, pinned:
/// a flat body is accepted, ignored and answered 2xx, so nothing but this assertion distinguishes a
/// write that stores the caller's data from one that stores nothing.
///
/// Stated as the exact emitted payload rather than as a property of the `wire` strings, because the
/// `wire` path is the input and the record is the thing HubSpot reads.
#[test]
fn every_hubspot_write_wraps_its_fields_in_the_properties_envelope() {
    let connector = load();

    let create = emit(&connector, "hubspot-contact-create");
    assert!(
        create.contains("$payload = { properties: { email: $email } }"),
        "`hubspot-contact-create` must nest `email` under `properties`; a flat body is accepted, \
         ignored and answered 2xx:\n{create}"
    );

    let update = emit(&connector, "hubspot-contact-update");
    assert!(
        update
            .contains("$payload = { properties: { firstname: $firstname, lastname: $lastname } }"),
        "`hubspot-contact-update` must nest its properties under `properties`:\n{update}"
    );

    // The negative half. A body field at the root of the payload is the exact shape HubSpot swallows,
    // so no write may emit one — checked over every write rather than only the two above.
    for operation in &connector.operations {
        if !mutates(operation.method) {
            continue;
        }
        let emitted = emit(&connector, &operation.id);
        assert!(
            emitted.contains("$payload = { properties: {"),
            "`{}` writes something other than a `properties` envelope:\n{emitted}",
            operation.id
        );
        for param in &operation.params.body {
            let wire = param.wire.as_deref().unwrap_or(&param.name);
            assert!(
                wire.starts_with("properties."),
                "`{}` declares body field `{}` at wire path `{wire}`, outside the `properties` \
                 envelope",
                operation.id,
                param.name
            );
        }
    }
}

/// **No optional request-body field, until C-56 lands.**
///
/// An omitted optional body field is not omitted: it travels as an explicit `null`, because the
/// emitter binds every declared field into the payload record. HubSpot's property model gives `null`
/// its own meaning, so an unsupplied field is a value the vendor acts on rather than one it ignores.
/// Declaring the field required is the fail-closed direction — the caller states what it means.
///
/// `body_schema` is checked too: a free-form body would satisfy the letter of this test while
/// handing a model an untyped object and sidestepping the `properties` envelope entirely.
#[test]
fn no_hubspot_operation_declares_an_optional_request_body_field() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.params.body_schema.is_none(),
            "`{}` declares a free-form body; HubSpot's envelope is a known shape and must be \
             declared through `wire` paths, not delegated to the caller",
            operation.id
        );
        for param in &operation.params.body {
            assert!(
                param.required,
                "`{}` declares optional body field `{}`. Until C-56 lands an omitted optional field \
                 travels as an explicit `null`, which is a value HubSpot acts on. Declare it \
                 required, or leave it out and name it in the provider file",
                operation.id,
                param.name
            );
        }
    }
}

/// **Every write says what it changes and who sees it, and no write claims to be idempotent.**
///
/// `connector-flux`'s `check_write_metadata` already refuses `risk = "low"` on a state-changing
/// method and `idempotency = "idempotent"` on a `POST`/`PATCH`, so this is not the emitter's gate
/// restated: it is the *reason* stated where a reader looking at HubSpot will find it. A CRM write is
/// seen by every sales rep in the portal and can enrol the record in a workflow that emails a real
/// person, and `risk` is what flux's approval gate reads before running the call unattended. HubSpot
/// documents no idempotency key on these endpoints, so `conditional` would be a claim with nothing
/// behind it.
#[test]
fn every_hubspot_write_declares_what_it_changes() {
    let connector = load();

    for operation in &connector.operations {
        if !mutates(operation.method) {
            assert_eq!(
                operation.risk,
                Risk::Low,
                "`{}` is a read; anything above `low` would make the approval gate prompt for a \
                 lookup",
                operation.id
            );
            assert_eq!(operation.idempotency, Idempotency::Idempotent);
            continue;
        }
        assert_eq!(
            operation.risk,
            Risk::High,
            "`{}` writes a CRM record every rep in the portal sees and a workflow can act on; \
             `high` is the tier a reviewer would want to see first",
            operation.id
        );
        assert_eq!(
            operation.idempotency,
            Idempotency::NonIdempotent,
            "`{}` is not documented as idempotent, and a replay is observable — HubSpot appends to \
             the property's history on every write",
            operation.id
        );
    }
}

/// **The host is exactly `api.hubapi.com`, and the request goes nowhere else.**
///
/// `base_url` carries no `{template}`, which is what makes this connector's egress surface a single
/// literal name a reviewer can check, and it is never widened to a pattern: `http_hosts` is derived
/// from this one value (`crates/connector-cli/src/catalog.rs`, `host_of`). Asserted against the
/// emitted `$base`, because that is the string the request is actually built from.
#[test]
fn every_request_targets_api_hubapi_com_and_nothing_wider() {
    let connector = load();

    assert!(
        !connector.base_url.contains('{'),
        "`base_url` must be a bound literal, not a template: {:?}",
        connector.base_url
    );
    assert!(
        !connector.base_url.contains('*'),
        "`base_url` must name one host, never a wildcard: {:?}",
        connector.base_url
    );

    for operation in &connector.operations {
        let emitted = emit(&connector, &operation.id);
        assert!(
            emitted.contains(&format!(r#"$base = "{BASE_URL}""#)),
            "`{}` does not bind the HubSpot base URL:\n{emitted}",
            operation.id
        );
        assert!(
            operation.path.starts_with("/crm/v3/objects/"),
            "`{}` has path {:?}; every selected HubSpot endpoint is a CRM v3 object endpoint",
            operation.id,
            operation.path
        );
    }
}

/// **No credential value, and not even a credential *name*, reaches the generated Flux**
/// (AGENTS.md). The connector carries the env-var name so a host can resolve it; the emitted module
/// carries neither that name nor anything shaped like a token, because auth injection is C-10 and is
/// deliberately absent rather than stubbed.
#[test]
fn the_emitted_flux_carries_no_credential_at_all() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit(&connector, &operation.id);
        assert!(
            !emitted.contains(SECRET_ENV),
            "`{}` names {SECRET_ENV} in generated Flux:\n{emitted}",
            operation.id
        );
        // HubSpot private-app tokens are `pat-`-prefixed. A literal one in a generated artifact is
        // the failure this invariant exists to prevent, so it is checked for by shape as well as by
        // name.
        assert!(
            !emitted.contains("pat-"),
            "`{}` embeds something shaped like a HubSpot private-app token:\n{emitted}",
            operation.id
        );
    }
}

/// **The C-11 gate, per operation.** Each one emits, parses with no diagnostics, is already a fixed
/// point of flux's own formatter, and reloads through flux-lang's module loader as exactly one
/// exposed composite op.
///
/// "Analyzable" is the load half: a module that parsed but did not load would publish no ops at all,
/// so a consumer handing it to flux would get silence rather than an error.
#[test]
fn every_hubspot_operation_emits_an_analyzable_module() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit(&connector, &operation.id);

        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "`{}` emits Flux that does not parse: {:?}\n{emitted}",
            operation.id,
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str()),
            "the flux formatter would rewrite `{}`",
            operation.id
        );

        let module = flux_lang::program::Module::parse_str(&emitted)
            .unwrap_or_else(|error| panic!("`{}` does not load: {error}", operation.id));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{}` is not a program", operation.id));
        assert_eq!(program.ops.len(), 1, "one operation is one declaration");
        assert_eq!(program.ops[0].name, operation.id);
        assert!(
            program.ops[0].meta.expose,
            "`{}` must be exposed to the model as a tool",
            operation.id
        );
    }
}
