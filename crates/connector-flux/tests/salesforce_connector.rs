//! The Salesforce connector, and the archetype it exists to prove out: **a configured host.**
//!
//! Every provider before this one had a base URL that was a constant, or at worst a `{placeholder}`
//! left deliberately unbound (zendesk's `{subdomain}`, jira's `{site}`) because nothing declared
//! where the value came from. Salesforce's is the same shape — `https://{instance}.my.salesforce.com`,
//! an org's own "My Domain" name, discovered at login — but this file is the one that closes the
//! loop: `crates/connector-spec/src/config.rs`'s `Binding::Endpoint { variable }` reaches exactly
//! `endpoint.<name>`, and a `[[config]]` field naming it is what makes the base URL a value an
//! operator supplies rather than a gap this repository records in a comment.
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims specific to Salesforce:
//!
//! - **The `{instance}` template is bound by a `[[config]]` field, not merely present.** C-163's
//!   hazard was checking `ConfigField::binds` before promising this, and C-169/C-170 (filed as
//!   C-187) already measured that `binds` reaches `base_url` and nothing else — a path segment or a
//!   query parameter is out of reach. A base URL template is exactly the case it *can* express, and
//!   [`the_instance_template_is_bound_by_a_config_field`] is the proof.
//! - **No operation reaches a query string.** Salesforce's SOQL query resource
//!   (`GET /services/data/.../query?q=<SOQL>`) is excluded: a SOQL expression is full of spaces,
//!   commas and quotes, and the emitter percent-encodes no query value at all
//!   (`crates/connector-flux/src/op.rs`, C-30) — the defect `zendesk-ticket-search` carries. That
//!   exclusion is asserted the same way jira's and github's absent query surfaces are.
//! - **No credential reaches emitted Flux**, and no operation claims a POST or PATCH is idempotent —
//!   `check_write_metadata` refuses that by method (C-186), and `salesforce-record-update`'s comment
//!   in the provider file records the honest claim the field cannot carry.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::config::template_variables;
use connector_spec::{AuthScheme, Binding, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider under test. Named once so the file reads as being about Salesforce rather than a
/// string.
const PROVIDER: &str = "salesforce";

/// The credential the connector declares, and the environment variable it resolves from. Public
/// contract — an operator sets the variable and a manifest names the credential — so it is pinned
/// here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "salesforce.access_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "SALESFORCE_ACCESS_TOKEN";

/// The curated operations, in the order `providers/salesforce.toml` declares them.
const OPERATIONS: &[&str] = &[
    "salesforce-whoami",
    "salesforce-record-get",
    "salesforce-record-create",
    "salesforce-record-update",
    "salesforce-sobject-describe",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// Every operation's emitted module text, paired with its operation id.
fn emitted(connector: &Connector) -> Vec<(String, String)> {
    connector
        .operations
        .iter()
        .map(|operation| {
            let text = emit_operation(connector, operation)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
            (operation.id.clone(), text)
        })
        .collect()
}

/// The connector exists, loads, and is the one the story specifies: a bearer access token over a
/// per-org host, with the curated operation set.
#[test]
fn the_salesforce_connector_loads_and_authenticates_with_a_bearer_access_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Salesforce");
    assert_eq!(connector.base_url, "https://{instance}.my.salesforce.com");

    assert_eq!(
        connector.auth.len(),
        1,
        "salesforce authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("salesforce declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "an OAuth2 access token is sent as `Authorization: Bearer <token>`"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.oauth2.is_none(),
        "the token arrives already minted through the environment; this connector runs no OAuth2 \
         grant itself (docs/designs/auth-seam.md), the same provenance every other bearer-scheme \
         connector here has"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; salesforce is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the access token",
            operation.id
        );
    }
}

/// **The load-bearing assertion: the configured host is bound, not merely present.**
///
/// A `{placeholder}` in `base_url` with nothing binding it is exactly the state zendesk's
/// `{subdomain}` and jira's `{site}` were shipped in before their own `[[config]]` sections existed.
/// `ConfigField::binds` reaching `endpoint.<variable>` is the mechanism `crates/connector-spec/src/
/// config.rs` already provides, and C-169/C-170 (filed as C-187) measured that it reaches `base_url`
/// and nothing else — a base URL template is precisely the case it can express, and this is the
/// connector that uses it rather than recording the gap a fourth time.
#[test]
fn the_instance_template_is_bound_by_a_config_field() {
    let connector = load();

    let variables = template_variables(&connector.base_url);
    assert_eq!(
        variables,
        ["instance"],
        "salesforce's base URL must carry exactly one template variable, the org's My Domain name"
    );

    let field = connector
        .config
        .iter()
        .find(|field| field.binds == "endpoint.instance")
        .unwrap_or_else(|| {
            panic!(
                "no `[[config]]` field binds `endpoint.instance`; the base URL template is \
                 unbound and this connector has not answered the question it was chosen for"
            )
        });
    assert_eq!(
        field.binding(),
        Some(Binding::Endpoint {
            variable: "instance"
        })
    );
    assert!(!field.label.is_empty(), "a config field must be renderable");
    assert!(!field.help.is_empty(), "a config field must be renderable");
    assert!(
        !field.secret,
        "the org's My Domain name is not a secret; a subdomain is public by construction"
    );

    // The other config field is the credential itself, and `secret` must agree with `binds` — the
    // configuration contract's rule, enforced at the loader and restated here as the connector's own
    // claim about itself.
    let credential_field = connector
        .config
        .iter()
        .find(|field| field.binds == format!("credential.{CREDENTIAL}"))
        .unwrap_or_else(|| panic!("no `[[config]]` field binds `credential.{CREDENTIAL}`"));
    assert!(
        credential_field.secret,
        "the access token binds a credential and must be declared secret"
    );
    assert!(
        credential_field.example.is_none(),
        "no realistic example on a secret field"
    );

    assert_eq!(
        connector.config.len(),
        2,
        "salesforce asks for exactly two things: the org host and the access token"
    );

    // The template reaches the emitted module verbatim, unresolved — the connector cannot invent an
    // org, and a build must not silently substitute one.
    for (id, text) in emitted(&connector) {
        assert!(
            text.contains(r#"base = "https://{instance}.my.salesforce.com""#),
            "`{id}` does not carry the unbound tenant template:\n{text}"
        );
    }
}

/// **No operation reaches a query string.** Salesforce's SOQL query resource is the excluded one —
/// see the module docs — and every curated operation is addressed by path and body alone.
#[test]
fn no_salesforce_operation_declares_a_query_parameter_or_assembles_one() {
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
            "operation `{}` declares query parameters {declared:?}; a SOQL-shaped or free-text \
             query value is interpolated into the URL with no percent-encoding (C-30), the defect \
             `zendesk-ticket-search` carries",
            operation.id
        );
        assert!(
            !operation.id.contains("query"),
            "operation `{}` looks like the excluded SOQL query resource",
            operation.id
        );
    }

    for (id, text) in emitted(&connector) {
        assert!(
            !text.contains('?'),
            "`{id}` emits a `?` in its request URL:\n{text}"
        );
    }
}

/// Write metadata says what each write changes and never claims a POST or PATCH is idempotent —
/// `check_write_metadata` refuses that by method (C-186), and the reads are genuinely idempotent.
#[test]
fn every_salesforce_write_declares_non_idempotence_and_no_write_is_low_risk() {
    let connector = load();

    for operation in &connector.operations {
        match operation.method {
            HttpMethod::Get => {
                assert_eq!(
                    operation.risk,
                    Risk::Low,
                    "operation `{}` is a read",
                    operation.id
                );
                assert_eq!(
                    operation.idempotency,
                    Idempotency::Idempotent,
                    "operation `{}` is a GET, which is repeatable",
                    operation.id
                );
            }
            HttpMethod::Post | HttpMethod::Patch => {
                assert_ne!(
                    operation.risk,
                    Risk::Low,
                    "operation `{}` is a write declared low risk",
                    operation.id
                );
                assert_ne!(
                    operation.idempotency,
                    Idempotency::Idempotent,
                    "operation `{}` claims idempotence on a method the emitter refuses it on \
                     (C-186)",
                    operation.id
                );
            }
            other => panic!(
                "operation `{}` uses method {other:?} this connector does not curate",
                operation.id
            ),
        }
    }
}

/// No credential, and no credential's variable name, reaches a generated module.
///
/// Auth injection is C-10 and is deliberately absent from emitted Flux rather than stubbed, so the
/// strongest available statement is that nothing credential-shaped is in the text at all.
#[test]
fn no_salesforce_module_carries_a_credential_or_its_variable_name() {
    let connector = load();

    for (id, text) in emitted(&connector) {
        for forbidden in [TOKEN_ENV, CREDENTIAL, "$secret", "Authorization", "Bearer"] {
            assert!(
                !text.contains(forbidden),
                "`{id}` names `{forbidden}` in generated Flux; a generated module carries no \
                 credential and no credential reference (C-10, AGENTS.md):\n{text}"
            );
        }
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical,
/// and **loads** as exactly one exposed composite op.
#[test]
fn every_salesforce_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

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
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, operation.id);
        assert!(
            program.ops[0].meta.expose,
            "`{}` must be exposed to the model as a tool",
            operation.id
        );
    }
}
