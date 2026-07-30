//! The configuration surface: what a human is asked for, and where each answer goes.
//!
//! Every test goes through `provider::load`, because a configuration field is a statement about a
//! *file* and about the form generated from it. Two properties are under test throughout, and they
//! are complementary:
//!
//! - **A connector asks for everything it needs.** A `{subdomain}` nobody declares is a connector
//!   that cannot be configured and cannot say why.
//! - **A connector asks for nothing it cannot use.** A field binding something that does not exist is
//!   a question whose answer is discarded.
//!
//! The third property — that `secret` agrees with what a field binds — is the one with a security
//! edge, and it has its own section.

use connector_spec::{provider, Connector, Format, Level};

/// A connector with a templated base URL, a basic credential, and one config field per binding form
/// that applies to it. Each test perturbs exactly one thing.
fn fixture(config: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://{{tenant}}.acme.example"

[[auth]]
name = "acme.api_token"
scheme = "basic"
env = ["ACME_API_TOKEN"]
user_env = ["ACME_USER"]

[[operations]]
id = "acme-ping"
method = "GET"
path = "/ping"
risk = "low"
idempotency = "idempotent"

{config}
"#
    )
}

/// Everything the fixture needs to be valid: the tenant is asked for, and so are both credential
/// halves.
const GOOD: &str = r#"
[[config]]
name = "tenant"
label = "Acme tenant"
help = "The part of your Acme URL before `.acme.example`"
example = "widgets"
format = "subdomain"
binds = "endpoint.tenant"

[[config]]
name = "email"
label = "Account email"
help = "The account the token belongs to"
example = "you@widgets.com"
format = "email"
binds = "username.acme.api_token"

[[config]]
name = "api_token"
label = "API token"
help = "From your Acme account settings"
format = "token"
secret = true
binds = "credential.acme.api_token"
"#;

fn load(source: &str) -> Connector {
    provider::load("providers/fixture.toml", source)
        .unwrap_or_else(|error| panic!("the fixture must load:\n{error}"))
        .connector
}

fn refuse(source: &str) -> String {
    let error = provider::load("providers/fixture.toml", source)
        .err()
        .unwrap_or_else(|| panic!("this definition must not load"));
    format!("{error}")
}

#[test]
fn a_complete_configuration_surface_loads_and_derives_its_levels() {
    let connector = load(&fixture(GOOD));
    assert_eq!(connector.config.len(), 3);

    let tenant = connector.config_field("tenant").expect("declared");
    assert_eq!(tenant.label, "Acme tenant");
    assert_eq!(tenant.format, Format::Subdomain);
    assert!(
        tenant.required,
        "required defaults to true — the safe default is to ask"
    );

    // Level is derived, never authored. Nothing in the TOML above says "connection".
    for name in ["tenant", "email", "api_token"] {
        assert_eq!(
            connector.config_field(name).and_then(|f| f.level()),
            Some(Level::Connection),
            "{name} is a per-tenant value; this connector has no operator-level registration"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// A connector asks for everything it needs
// ---------------------------------------------------------------------------------------------

/// The rule that closes the `SCHEMA GAP:` comment four shipped providers carried since C-17.
#[test]
fn a_template_variable_nothing_binds_is_refused() {
    let error = refuse(&fixture(&GOOD.replace(
        r#"binds = "endpoint.tenant""#,
        r#"binds = "credential.acme.api_token""#,
    )));
    assert!(
        error.contains("carries `{tenant}`, which no `[[config]]` field binds"),
        "a connector with no way to learn its own host must be refused:\n{error}"
    );
    assert!(
        error.contains(r#"binds = "endpoint.tenant""#),
        "the error must name the fix:\n{error}"
    );
}

#[test]
fn a_connector_with_no_configuration_at_all_is_refused_when_it_needs_some() {
    let error = refuse(&fixture(""));
    assert!(
        error.contains("no valid destination URL"),
        "silence is not the same as having nothing to ask for:\n{error}"
    );
}

/// The complement: a connector with a literal base URL needs no endpoint field, and declaring none is
/// correct rather than an omission.
#[test]
fn a_connector_with_a_literal_base_url_needs_no_endpoint_field() {
    let source = fixture(GOOD)
        .replace("https://{tenant}.acme.example", "https://api.acme.example")
        .replace(
            r#"
[[config]]
name = "tenant"
label = "Acme tenant"
help = "The part of your Acme URL before `.acme.example`"
example = "widgets"
format = "subdomain"
binds = "endpoint.tenant"
"#,
            "",
        );
    let connector = load(&source);
    assert_eq!(connector.config.len(), 2);
}

// ---------------------------------------------------------------------------------------------
// A connector asks for nothing it cannot use
// ---------------------------------------------------------------------------------------------

#[test]
fn a_field_binding_a_template_variable_that_does_not_exist_is_refused() {
    let error = refuse(&fixture(
        &GOOD.replace("endpoint.tenant", "endpoint.region"),
    ));
    assert!(
        error.contains("binds `{region}`, which no service's `base_url` carries"),
        "the error must say which templates actually exist:\n{error}"
    );
}

#[test]
fn a_field_binding_a_credential_nobody_declares_is_refused() {
    let error = refuse(&fixture(
        &GOOD.replace("credential.acme.api_token", "credential.acme.absent"),
    ));
    assert!(
        error.contains("which no `[[auth]]` block declares"),
        "the error must name the dangling credential:\n{error}"
    );
}

/// Only `basic` sends a username; for every other scheme the whole credential is the secret, so a
/// username field would collect a value with nowhere to go.
#[test]
fn a_username_field_on_a_non_basic_credential_is_refused() {
    let source = fixture(GOOD).replace(r#"scheme = "basic""#, r#"scheme = "bearer""#);
    let error = refuse(&source);
    assert!(
        error.contains("Only `basic` sends a username"),
        "a bearer credential has no username half:\n{error}"
    );
}

#[test]
fn an_oauth_field_without_an_oauth_credential_is_refused() {
    let error = refuse(&fixture(&format!(
        r#"{GOOD}
[[config]]
name = "client_id"
label = "Client ID"
help = "From your Acme app registration"
binds = "oauth.client_id"
"#
    )));
    assert!(
        error.contains("no `[[auth]]` block declares an `[auth.oauth2]` spec"),
        "there is no OAuth flow for a client id to belong to:\n{error}"
    );
}

#[test]
fn a_binding_that_is_not_a_binding_at_all_is_refused() {
    let error = refuse(&fixture(
        &GOOD.replace(r#"binds = "endpoint.tenant""#, r#"binds = "subdomain""#),
    ));
    assert!(
        error.contains("is not a binding"),
        "the error must list the forms that are:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// `secret` must agree with what the field binds
//
// This is the rule with a security edge. flux partitions secret from non-secret BY TYPE — an
// `AuthMethod` versus a `ConfigSpec` — and enforces it host-side, refusing to hand a
// secret-classified env key back through the non-secret `config` capability. A field that disagreed
// would put a contradicting claim in front of that enforcement.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_credential_field_that_claims_not_to_be_secret_is_refused() {
    let error = refuse(&fixture(&GOOD.replace("secret = true", "secret = false")));
    assert!(
        error.contains("That value is a credential"),
        "a token declared non-secret would be logged and echoed back:\n{error}"
    );
}

#[test]
fn a_non_credential_field_that_claims_to_be_secret_is_refused() {
    let error = refuse(&fixture(&GOOD.replace(
        r#"format = "subdomain"
binds = "endpoint.tenant""#,
        r#"format = "subdomain"
secret = true
binds = "endpoint.tenant""#,
    )));
    assert!(
        error.contains("That value is configuration, not a credential"),
        "masking a subdomain hides it from an operator who needs to read it back:\n{error}"
    );
}

/// The Basic username half is config, not a gated secret — the same split `AuthMethod::user_env`
/// already documents, and why an agent email may appear in a log where its token may not.
#[test]
fn a_username_field_is_not_secret() {
    let error = refuse(&fixture(&GOOD.replace(
        r#"format = "email"
binds = "username.acme.api_token""#,
        r#"format = "email"
secret = true
binds = "username.acme.api_token""#,
    )));
    assert!(
        error.contains("configuration, not a credential"),
        "the username half travels the non-gated path:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// A field has to be renderable
// ---------------------------------------------------------------------------------------------

#[test]
fn a_field_without_a_label_is_refused() {
    let error = refuse(&fixture(
        &GOOD.replace(r#"label = "Acme tenant""#, r#"label = """#),
    ));
    assert!(
        error.contains("is an identifier, not a label"),
        "defaulting a label to the field name ships `api_token` as user-facing copy:\n{error}"
    );
}

#[test]
fn a_field_without_help_is_refused() {
    let error = refuse(&fixture(&GOOD.replace(
        r#"help = "The part of your Acme URL before `.acme.example`""#,
        r#"help = """#,
    )));
    assert!(
        error.contains("stops the installation"),
        "a field a user cannot answer is not a field:\n{error}"
    );
}

/// A placeholder that would fail the field's own validation is worse than none, because a user
/// copies it.
#[test]
fn an_example_that_fails_its_own_format_is_refused() {
    let error = refuse(&fixture(&GOOD.replace(
        r#"example = "widgets""#,
        r#"example = "widgets.acme.example""#,
    )));
    assert!(
        error.contains("does not satisfy it"),
        "the example is checked against the format it claims:\n{error}"
    );
    assert!(
        error.contains("because a user copies it"),
        "the error must say why this matters:\n{error}"
    );
}

#[test]
fn config_names_join_the_shared_member_namespace() {
    let error = refuse(&fixture(
        &GOOD.replace(r#"name = "tenant""#, r#"name = "acme-ping""#),
    ));
    assert!(
        error.contains("names both an operation and a configuration field"),
        "all member kinds share one namespace:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// The verification operation — a host's "Test connection" button
// ---------------------------------------------------------------------------------------------

#[test]
fn a_verify_operation_loads_and_resolves() {
    let source = fixture(GOOD).replace(
        r#"vendor = "Acme""#,
        "vendor = \"Acme\"\nverify = \"acme-ping\"",
    );
    let connector = load(&source);
    assert_eq!(connector.verify.as_deref(), Some("acme-ping"));
    assert!(connector.operation("acme-ping").is_some());
}

#[test]
fn a_verify_operation_that_does_not_exist_is_refused() {
    let source = fixture(GOOD).replace(
        r#"vendor = "Acme""#,
        "vendor = \"Acme\"\nverify = \"acme-absent\"",
    );
    let error = refuse(&source);
    assert!(
        error.contains("which no `[[operations]]` block declares"),
        "the error must name the dangling reference:\n{error}"
    );
}

/// A "Test connection" button that could create a ticket is a button nobody dares press — and it
/// runs unattended every time someone opens a settings page.
#[test]
fn a_verify_operation_that_writes_is_refused() {
    let source = fixture(GOOD)
        .replace(
            r#"vendor = "Acme""#,
            "vendor = \"Acme\"\nverify = \"acme-ping\"",
        )
        .replace(r#"risk = "low""#, r#"risk = "high""#)
        .replace(
            r#"idempotency = "idempotent""#,
            r#"idempotency = "non_idempotent""#,
        );
    let error = refuse(&source);
    assert!(
        error.contains("runs unattended"),
        "a connection test must be a read a user would not mind being repeated:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// The shipped providers
// ---------------------------------------------------------------------------------------------

fn shipped(name: &str) -> Connector {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../providers")
        .join(format!("{name}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    provider::load(&format!("providers/{name}.toml"), &source)
        .unwrap_or_else(|e| panic!("providers/{name}.toml must load:\n{e}"))
        .connector
}

/// **C-68's acceptance, mechanised over the whole fleet.** Every shipped provider asks for every
/// value its own base URL needs — which is now impossible to violate, because the loader refuses it,
/// but is worth asserting over the real corpus rather than only over fixtures.
#[test]
fn no_shipped_provider_has_an_unbound_template_variable() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../providers");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("providers/ is readable") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let name = path
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .to_string();
        let connector = shipped(&name);
        for service in connector.service_names() {
            for variable in
                connector_spec::config::template_variables(connector.base_url_of(service))
            {
                assert!(
                    connector
                        .config_of(service)
                        .any(|field| field.binds == format!("endpoint.{variable}")),
                    "providers/{name}.toml leaves `{{{variable}}}` unbound"
                );
            }
        }
        checked += 1;
    }
    // Derived-set discipline (C-54): an empty providers/ must fail loudly rather than pass vacuously.
    assert!(checked > 0, "no providers were checked");
}

/// The four templated providers, named. This is the list the `SCHEMA GAP:` comments used to live in,
/// and it is here so that a fifth templated provider arriving without a config field is a failure
/// with a name attached rather than only a generic one.
#[test]
fn the_four_tenant_providers_ask_for_their_tenant() {
    for (provider, field, format) in [
        ("zendesk", "subdomain", Format::Subdomain),
        ("jira", "site", Format::Subdomain),
        ("shopify", "shop", Format::Subdomain),
        ("freshdesk", "domain", Format::Hostname),
    ] {
        let connector = shipped(provider);
        let declared = connector
            .config_field(field)
            .unwrap_or_else(|| panic!("{provider} must ask for `{field}`"));
        assert_eq!(declared.binds, format!("endpoint.{field}"));
        assert_eq!(
            declared.format, format,
            "{provider}/{field} must declare the shape a form validates against"
        );
        assert!(
            !declared.label.is_empty() && !declared.help.is_empty(),
            "{provider}/{field} must be renderable"
        );
        assert!(
            declared.docs_url.is_some(),
            "{provider}/{field} should tell a user where to find the value"
        );
    }
}

/// Zendesk is the fullest form the fleet has: a tenant, a username half and a secret — three fields
/// spanning three binding forms, all connection level.
#[test]
fn zendesk_declares_a_complete_connect_form() {
    let connector = shipped("zendesk");
    let names: Vec<&str> = connector.config.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["subdomain", "email", "api_token"]);

    let secret: Vec<bool> = connector.config.iter().map(|f| f.secret).collect();
    assert_eq!(
        secret,
        [false, false, true],
        "only the token is masked — the subdomain and the agent email are configuration"
    );

    assert_eq!(connector.verify.as_deref(), Some("zendesk-test"));
}
