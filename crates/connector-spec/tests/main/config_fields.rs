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

use connector_spec::{provider, Binding, Connector, Format, Level, Pin, Position};

use crate::shipped_provider;

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
direction = "read"
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
fn the_username_placeholder_prefix_is_reserved_from_endpoint_fields() {
    let source = fixture(GOOD)
        .replace("{tenant}", "{username.tenant}")
        .replace("endpoint.tenant", "endpoint.username.tenant");
    let error = refuse(&source);
    assert!(
        error.contains("reserved `username.` placeholder prefix"),
        "an endpoint must not impersonate the qualified Basic-username address:\n{error}"
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

/// **A secret field declares no `example` at all** — C-231.
///
/// Not a documentation preference: a token-shaped placeholder has tripped GitHub's push protection
/// and blocked a release in this repository before. The cost is asymmetric — a placeholder that
/// merely *looks* like a token blocks a push, and one that *is* a token is a disclosed credential.
#[test]
fn a_secret_field_that_declares_an_example_is_refused() {
    let error = refuse(&fixture(&GOOD.replace(
        r#"format = "token"
secret = true"#,
        r#"example = "ACME-A1B2C3D4E5"
format = "token"
secret = true"#,
    )));
    assert!(
        error.contains("declares `secret = true` and an `example`"),
        "a secret field must carry no placeholder:\n{error}"
    );
    assert!(
        error.contains("push protection"),
        "the error must say why this matters:\n{error}"
    );
}

/// The rule is about **secrets**, not about examples. A non-secret field's placeholder is a
/// documentation question and stays welcome — the scope line C-231 draws explicitly.
#[test]
fn a_non_secret_field_may_still_declare_an_example() {
    let connector = load(&fixture(GOOD));
    assert_eq!(
        connector
            .config_field("tenant")
            .and_then(|field| field.example.as_deref()),
        Some("widgets"),
        "a subdomain placeholder is documentation, and nothing here should discourage it"
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
        .replace(r#"direction = "read""#, r#"direction = "write""#)
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
    shipped_provider::load_definition(name, &source)
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
                // The head service, or a sibling sharing the same one address (C-529). GitLab's
                // API and OAuth surfaces are one deployment, so one approved `{origin}` fills both.
                assert!(
                    connector.config.iter().any(|field| {
                        let fills = field.service == service
                            || field.also_services.iter().any(|extra| extra == service);
                        fills && field.binds == format!("endpoint.{variable}")
                    }),
                    "providers/{name}.toml leaves `{{{variable}}}` unbound for service {service:?}"
                );
            }
        }
        checked += 1;
    }
    // Derived-set discipline (C-54): an empty providers/ must fail loudly rather than pass vacuously.
    assert!(checked > 0, "no providers were checked");
}

/// **C-231's acceptance, mechanised over the whole fleet.** No shipped provider gives a secret
/// configuration field an `example` — which, like the rule above, the loader now refuses outright,
/// and which is worth asserting over the real corpus anyway.
///
/// The corpus is the point. A per-connector version of this check is what 24 connector tests
/// wrote and 14 providers with a secret field never got, which is why `example = "NRAK-ABCDEFG"` on
/// `providers/newrelic.toml`'s secret `api_key` turned nothing red. `providers/` is read from disk
/// so that a connector landing tomorrow is covered without anyone adding it to a list — the C-81
/// defect one level up.
///
/// Note what a violation looks like now: `shipped()` panics with the loader's own refusal before
/// this loop is reached. That is the rule working, and the message names the file and the field.
#[test]
fn no_shipped_provider_gives_a_secret_field_an_example() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../providers");
    let mut checked = 0;
    let mut secrets = 0;
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
        for field in &shipped(&name).config {
            if !field.secret {
                continue;
            }
            secrets += 1;
            assert!(
                field.example.is_none(),
                "providers/{name}.toml gives secret field `{}` an `example`. A token-shaped \
                 placeholder in a committed file has tripped GitHub push protection and blocked a \
                 release here; a real one would be a disclosed credential",
                field.name
            );
        }
        checked += 1;
    }
    // Derived-set discipline (C-54): neither an empty providers/ nor a catalogue that happens to
    // declare no secret at all may pass this vacuously.
    assert!(checked > 0, "no providers were checked");
    assert!(secrets > 0, "no secret configuration field was checked");
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
    let support: Vec<_> = connector
        .config_of(connector_spec::DEFAULT_SERVICE)
        .collect();
    let names: Vec<&str> = support.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["subdomain", "email", "api_token"]);

    let secret: Vec<bool> = support.iter().map(|f| f.secret).collect();
    assert_eq!(
        secret,
        [false, false, true],
        "only the token is masked — the subdomain and the agent email are configuration"
    );

    assert_eq!(connector.verify.as_deref(), Some("zendesk-test"));
}

// ---------------------------------------------------------------------------------------------
// An operator pins a tenant-scoping value at install time (C-187)
//
// `endpoint.<variable>` reaches a `{placeholder}` in a service's `base_url` and nothing else, so a
// tenant value living anywhere else on the request — Cloudflare's `{zone_id}` path segment,
// Vercel's `?teamId=` — had to ship as a per-call argument a model chooses on every invocation.
// These tests are about the three request positions a pin may reach instead.
// ---------------------------------------------------------------------------------------------

/// A connector whose host is literal and whose every real operation is scoped under one `{zone_id}`
/// path segment — the Cloudflare shape, reduced to what a binding needs.
///
/// One operation (`acme-zone-list`) deliberately carries no `{zone_id}`: it is the call that
/// *discovers* the value, and a pin that required every operation to reference it would make that
/// call unexpressible.
fn scoped_fixture(config: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"

[[auth]]
name = "acme.api_token"
scheme = "bearer"
env = ["ACME_API_TOKEN"]

[[operations]]
id = "acme-zone-list"
method = "GET"
direction = "read"
path = "/zones"
risk = "low"
idempotency = "idempotent"

[[operations]]
id = "acme-record-list"
method = "GET"
direction = "read"
path = "/zones/{{zone_id}}/records"
risk = "low"
idempotency = "idempotent"

[[config]]
name = "api_token"
label = "API token"
help = "From your Acme account settings"
format = "token"
secret = true
binds = "credential.acme.api_token"

{config}
"#
    )
}

/// The pin itself: an operator-supplied zone, bound to the path segment every scoped operation
/// carries.
const ZONE_PIN: &str = r#"
[[config]]
name = "zone_id"
label = "Acme zone"
help = "The zone this connector is installed for"
example = "023e105f4ecef8ad9ca31a8372d0c353"
binds = "path.zone_id"
"#;

/// **The failing-first test.** A `[[config]]` field binding a path segment does not load today:
/// `parse_binding` reaches `endpoint.`, `credential.`, `username.` and the two `oauth.` literals,
/// and a path segment is none of them — so an operator cannot say "install this connector for this
/// zone", and the value stays an argument a model chooses on every call.
#[test]
fn a_config_field_can_pin_a_path_segment() {
    let connector = load(&scoped_fixture(ZONE_PIN));

    let zone = connector
        .config_field("zone_id")
        .expect("the fixture declares it");
    assert_eq!(zone.binds, "path.zone_id");

    // Derived, never authored. A zone is per-tenant, so it is connection level for the same reason
    // `{subdomain}` is — the level is a consequence of where the value goes.
    assert_eq!(
        zone.level(),
        Some(Level::Connection),
        "a pinned tenant value is set once per connection, not once per vendor"
    );
    assert!(
        !zone.secret,
        "a pinned value is operator-supplied configuration, not a credential"
    );

    // **The pin is not advisory.** Nothing declares `zone_id` as a caller argument, so a model
    // cannot override the operator's choice of zone.
    assert!(
        connector
            .operations
            .iter()
            .all(|op| op.params.path.iter().all(|p| p.name != "zone_id")),
        "a pinned value must not also be a caller argument"
    );
}

/// The typed form, and the two facts derivation is responsible for. Kept separate from the test
/// above so that the failing-first assertion stays about the *file*, not about the enum's shape.
#[test]
fn a_pin_parses_to_its_position_and_derives_its_level_and_secrecy() {
    let connector = load(&scoped_fixture(ZONE_PIN));
    let zone = connector.config_field("zone_id").expect("declared");

    assert_eq!(
        zone.binding(),
        Some(Binding::Request {
            position: Position::Path,
            name: "zone_id"
        })
    );
    assert_eq!(zone.pin(), Some((Position::Path, "zone_id")));
    assert!(
        zone.required,
        "`required` defaults to true, and a pin may not turn it off"
    );
}

/// A query pin and a header pin, on a connector shaped like the two that measured the gap.
#[test]
fn a_query_parameter_and_a_header_can_be_pinned_too() {
    let config = r#"
[[config]]
name = "account_id"
label = "Account"
help = "The account every call acts on behalf of"
binds = "query.accountId"

[[config]]
name = "application_id"
label = "Application"
help = "The application id this connection uses"
binds = "header.X-Vendor-Application-Id"
"#;
    let connector = load(&scoped_fixture(config));

    assert_eq!(
        connector.config_field("account_id").and_then(|f| f.pin()),
        Some((Position::Query, "accountId"))
    );
    assert_eq!(
        connector
            .config_field("application_id")
            .and_then(|f| f.pin()),
        Some((Position::Header, "X-Vendor-Application-Id"))
    );
    // A non-secret value reaching a header without routing through `[[auth]]` is the whole of
    // C-164's blocker — the fix is not "let a credential be non-secret".
    for name in ["account_id", "application_id"] {
        assert!(!connector.config_field(name).expect("declared").secret);
        assert_eq!(
            connector.config_field(name).and_then(|f| f.level()),
            Some(Level::Connection)
        );
    }
}

// ---------------------------------------------------------------------------------------------
// A pin that does not pin is refused
// ---------------------------------------------------------------------------------------------

/// **The acceptance rule with teeth.** If the operator pins it *and* the caller may pass it, the
/// caller's value wins and the operator's choice of tenant becomes a suggestion.
#[test]
fn a_value_that_is_both_pinned_and_declared_as_a_parameter_is_refused() {
    // The trailing table re-opens the last `[[operations]]` — `acme-record-list`, the scoped one.
    let error = refuse(&scoped_fixture(&format!(
        r#"{ZONE_PIN}
[[operations.params.path]]
name = "zone_id"
description = "The zone, as a caller argument"
required = true
schema = {{ type = "string" }}
"#
    )));
    assert!(
        error.contains("already declares it") && error.contains("is not pinned"),
        "a pin a caller can override is advisory, which is the opposite of a pin:\n{error}"
    );
}

/// The query half of the same rule, which is the one with the sharper failure: Vercel's `teamId`
/// redirects a write to the personal account when it goes unset.
#[test]
fn a_pinned_query_parameter_that_is_also_an_argument_is_refused() {
    let config = r#"
[[config]]
name = "account_id"
label = "Account"
help = "The account every call acts on behalf of"
binds = "query.accountId"
"#;
    let source = scoped_fixture(config).replace(
        r#"[[operations]]
id = "acme-record-list""#,
        r#"[[operations.params.query]]
name = "accountId"
description = "The account"
required = false
schema = { type = "string" }

[[operations]]
id = "acme-record-list""#,
    );
    let error = refuse(&source);
    assert!(
        error.contains("already declares it"),
        "an optional argument beside a pin is exactly the silent-redirect shape:\n{error}"
    );
}

/// A pin is mandatory. A host substitutes a pinned placeholder into an emitted literal and refuses
/// the whole request when it has no value, so `required = false` describes a connector that
/// composes no URL — and, for a query pin, reintroduces the absent-parameter hazard entirely.
#[test]
fn an_optional_pin_is_refused() {
    let error = refuse(&scoped_fixture(&format!("{ZONE_PIN}required = false\n")));
    assert!(
        error.contains("composes no URL"),
        "an optional pin is not a smaller pin, it is a broken connector:\n{error}"
    );
}

/// A pin nothing interpolates is a question whose answer is discarded — the request-position twin of
/// the endpoint rule.
#[test]
fn a_path_pin_no_operation_carries_is_refused() {
    let error = refuse(&scoped_fixture(
        &ZONE_PIN.replace("path.zone_id", "path.region"),
    ));
    assert!(
        error.contains("which no operation of service") && error.contains("{region}"),
        "the error must say the placeholder reaches nothing:\n{error}"
    );
}

/// **The C-197 collapse, made unreachable.** A host keys a configuration value by
/// `(tenant, provider, service, kind, name)` and the module carries one placeholder per pinned
/// value, so two declarations sharing a placeholder are one address — which is how a management write
/// once landed in whichever space the delivery reads had been configured with.
#[test]
fn two_fields_that_would_share_one_placeholder_are_refused() {
    let source = scoped_fixture(&format!(
        r#"{ZONE_PIN}
[[config]]
name = "zone_id_again"
label = "Zone, again"
help = "The same placeholder under a second name"
binds = "path.zone_id"
"#
    ));
    let error = refuse(&source);
    assert!(
        error.contains("one value under one address"),
        "two questions that share an answer are one question:\n{error}"
    );
}

/// **A pinned value must not be able to reshape the request it lands in.** The example is what a
/// user copies, so it is held to the rule the position imposes on the real value.
#[test]
fn a_path_pin_whose_example_escapes_its_segment_is_refused() {
    for escape in ["../admin", "a/b", "%2Fadmin"] {
        let error = refuse(&scoped_fixture(
            &ZONE_PIN.replace("023e105f4ecef8ad9ca31a8372d0c353", escape),
        ));
        assert!(
            error.contains("could not be one"),
            "{escape:?} would not stay inside one path segment:\n{error}"
        );
    }
}

/// A pinned value is configuration: never masked, never redacted, readable back by anyone who can
/// open a settings page. So it may not land where a credential goes — the pin must not become a
/// second, ungated route into `Authorization`.
#[test]
fn a_header_pin_on_an_auth_owned_header_is_refused() {
    let config = r#"
[[config]]
name = "auth_header"
label = "Authorization"
help = "For the refusal test only"
binds = "header.Authorization"
"#;
    let error = refuse(&scoped_fixture(config));
    assert!(
        error.contains("carries a credential"),
        "a pinned value in `Authorization` would be an unredacted credential:\n{error}"
    );
}

/// The `secret`/`binds` agreement covers the new forms too, in the direction that matters here: a
/// pinned value claiming to be a secret would hide it from the operator who has to read it back, and
/// would claim gating this repository does not provide.
#[test]
fn a_pin_that_claims_to_be_secret_is_refused() {
    let error = refuse(&scoped_fixture(&format!("{ZONE_PIN}secret = true\n")));
    assert!(
        error.contains("That value is configuration, not a credential"),
        "a pin is not a credential and must not be declared one:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// C-229: one question whose single answer reaches more than one destination.
//
// C-187 gave a field a request position; it still gave it exactly one. Algolia's application id
// composes the hostname *and* travels as a header on every call, so the vendor's own shape is one
// the section above cannot express. These tests are about a field naming several destinations
// while staying one question with one host-side slot.
// ---------------------------------------------------------------------------------------------

/// The Algolia shape, reduced to what a binding needs: a hostname composed from a tenant scope the
/// vendor *also* requires as a header on every call.
fn two_position_fixture(config: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://{{app_id}}-dsn.acme.example"

[[auth]]
name = "acme.api_key"
scheme = {{ header = {{ name = "X-Acme-Api-Key" }} }}
env = ["ACME_API_KEY"]

[[operations]]
id = "acme-index-list"
method = "GET"
direction = "read"
path = "/1/indexes"
risk = "low"
idempotency = "idempotent"

[[config]]
name = "api_key"
label = "API key"
help = "From your Acme dashboard"
format = "token"
secret = true
binds = "credential.acme.api_key"

{config}
"#
    )
}

/// **The failing-first test.** One field, one `name`, one answer — reaching both the `base_url`
/// placeholder and a request header.
///
/// It cannot be declared today: `binds` is one string naming one destination, and the two shapes
/// that would fake it are each refused or wrong. Two fields with two names load and ship two
/// host-side slots for one answer; two fields with one name are refused by `validate_pin`'s C-197
/// shared-slot pass, and rightly so — *two questions that share an answer are one question*. What
/// is missing is the one question, and `also_binds` is it.
#[test]
fn one_field_declares_two_destinations_and_one_value_reaches_both() {
    let config = r#"
[[config]]
name = "app_id"
label = "Acme application id"
help = "Shown on your Acme dashboard. It is both the host prefix and a header on every call"
example = "B1G2GM9NG0"
binds = "endpoint.app_id"
also_binds = ["header.X-Acme-Application-Id"]
"#;
    let connector = load(&two_position_fixture(config));
    let field = connector
        .config_field("app_id")
        .expect("the fixture declares it");

    // One question. The header destination adds no second field and no second name.
    assert_eq!(
        connector
            .config
            .iter()
            .filter(|other| other.name != "api_key")
            .count(),
        1,
        "a value reaching two positions is still one thing to ask a human for"
    );

    // Both destinations, in declaration order, with `binds` the head.
    assert_eq!(
        field.bindings(),
        Some(vec![
            Binding::Endpoint { variable: "app_id" },
            Binding::Request {
                position: Position::Header,
                name: "X-Acme-Application-Id",
            },
        ])
    );

    // **One host-side slot**, and it is `binds`' own target. That is the answer to the question a
    // multi-destination field forces: `Position::name` is both the placeholder and the wire
    // spelling, so when the two destinations spell the value differently the emitted module carries
    // the *slot's* spelling everywhere and the header's name is only what the vendor sees.
    assert_eq!(field.slot(), Some("app_id"));
    assert_eq!(
        field.pins(),
        vec![Pin {
            position: Position::Header,
            name: "X-Acme-Application-Id",
            variable: "app_id".into(),
        }],
        "the header pin carries the slot's placeholder, not a second one"
    );

    // Derivation is unchanged: every destination agrees about level and secrecy, and the loader
    // refuses a field whose destinations do not.
    assert_eq!(field.level(), Some(Level::Connection));
    assert!(
        !field.secret,
        "neither destination is a credential, so the field is not one"
    );
}

/// A field whose destinations differ only in position, with the slot coming from `binds` — the
/// shape a vendor wanting its tenant scope in a path segment *and* a header would write.
fn two_pins(config: &str) -> String {
    scoped_fixture(config)
}

/// **Only a request position may be a further destination.**
///
/// Every other kind resolves under its own address through a different port — a credential and an
/// OAuth half through the secret side, a Basic username under its own `(kind, name)` — so a single
/// slot cannot serve them. The `endpoint.` case has a spelling that works and it is `binds`, which
/// is what keeps the placeholder rule unconditional.
#[test]
fn a_further_destination_that_is_not_a_request_position_is_refused() {
    for (also, kind) in [
        ("endpoint.tenant", "endpoint"),
        ("credential.acme.api_token", "credential"),
        ("username.acme.api_token", "username"),
        ("oauth.client_id", "oauth"),
    ] {
        let config = format!(
            r#"
[[config]]
name = "zone_id"
label = "Acme zone"
help = "The zone this connector is installed for"
binds = "path.zone_id"
also_binds = ["{also}"]
"#
        );
        let error = refuse(&two_pins(&config));
        assert!(
            error.contains("also_binds") && error.contains(kind),
            "a `{also}` destination cannot share one slot, and the refusal must say which kind it \
             is:\n{error}"
        );
    }
}

/// One value reaches a position once. A repeat is either dropped by the emitter or sent twice, and
/// says nothing the first did not.
#[test]
fn a_destination_named_twice_is_refused() {
    let config = r#"
[[config]]
name = "zone_id"
label = "Acme zone"
help = "The zone this connector is installed for"
binds = "path.zone_id"
also_binds = ["header.X-Acme-Zone", "header.X-Acme-Zone"]
"#;
    let error = refuse(&two_pins(config));
    assert!(
        error.contains("twice"),
        "a destination declared twice must be refused:\n{error}"
    );
}

/// **Every destination is checked, not only the first** — and each pair is chosen so that exactly
/// one of the two destinations can refuse it.
///
/// `zone/admin` is a perfectly good HTTP field value and reshapes a path; `café` is a perfectly good
/// path segment and is not an HTTP field value at all. Neither is caught by the other destination's
/// rule, so a loader checking only `binds` would ship one of them and a loader checking only the
/// last would ship the other.
#[test]
fn an_example_is_checked_against_every_destination_and_not_only_the_first() {
    for (example, expected) in [("zone/admin", "path"), ("café", "header")] {
        let config = format!(
            r#"
[[config]]
name = "zone_id"
label = "Acme zone"
help = "The zone this connector is installed for"
example = "{example}"
binds = "path.zone_id"
also_binds = ["header.X-Acme-Zone"]
"#
        );
        let error = refuse(&two_pins(&config));
        assert!(
            error.contains(expected) && error.contains("`example`"),
            "an example illegal in the {expected} destination must be refused by it:\n{error}"
        );
    }
}

/// The C-225 interaction, spelled out: a closed set is a set of values an operator is *invited* to
/// pick, so every one of them is held to every position the field reaches.
#[test]
fn every_permitted_choice_is_checked_against_every_destination() {
    let config = r#"
[[config]]
name = "zone_id"
label = "Acme zone"
help = "The zone this connector is installed for"
binds = "path.zone_id"
also_binds = ["header.X-Acme-Zone"]

[[config.choices]]
value = "primary"
label = "Primary"

[[config.choices]]
value = "sécondaire"
label = "Secondary"
"#;
    let error = refuse(&two_pins(config));
    assert!(
        error.contains("header") && error.contains("choice"),
        "a permitted value that is not an HTTP field value must be refused by the header \
         destination:\n{error}"
    );
}

/// **A value that composes a host is held to the host rule, which is the strict one.**
///
/// `acme.example@evil.example` passes the path, query and header rules — none of those positions
/// cares about an `@` — and substituted into an authority it sends the request, and the operator's
/// own credential, to a host nobody named. So a field binding `endpoint.` is checked against it,
/// and that is what stops the intersection of a multi-destination field from being the weaker rule.
#[test]
fn a_value_that_composes_a_host_is_refused_when_it_could_move_the_authority() {
    let config = r#"
[[config]]
name = "tenant"
label = "Acme tenant"
help = "The part of your Acme URL before `.acme.example`"
example = "widgets.acme.example@evil.example"
binds = "endpoint.tenant"
"#;
    let error = refuse(&fixture(&format!("{config}{USERNAME_AND_TOKEN}")));
    assert!(
        error.contains("host character"),
        "an example that could move the authority must be refused:\n{error}"
    );
}

/// The two credential halves the `fixture` needs alongside a varying `tenant` field.
const USERNAME_AND_TOKEN: &str = r#"
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

/// **The door C-229 must not reopen, from the other side.** Two fields, two slots, one wire
/// position: the request carries one of two values depending on an order nothing declares.
///
/// The C-197 shared-slot refusal does not catch this — the two slots are genuinely different — so it
/// is its own rule, and it is what a further destination makes newly possible.
#[test]
fn two_fields_writing_one_header_are_refused() {
    let config = r#"
[[config]]
name = "zone_id"
label = "Acme zone"
help = "The zone this connector is installed for"
binds = "path.zone_id"
also_binds = ["header.X-Acme-Scope"]

[[config]]
name = "account_id"
label = "Acme account"
help = "The account this connector is installed for"
binds = "header.X-Acme-Scope"
"#;
    let error = refuse(&two_pins(config));
    assert!(
        error.contains("zone_id") && error.contains("account_id") && error.contains("X-Acme-Scope"),
        "two fields writing one header must be refused naming both:\n{error}"
    );
}

/// **A secret cannot acquire a second destination, whichever way round it is declared.**
///
/// The line this whole binding exists not to cross: a value that reaches a URL or a header the
/// emitted module composes is never masked and reaches no redactor. `secret` must agree with what a
/// field binds, and with `also_binds` it must agree with **every** destination — so a credential
/// that also claimed a header is a contradiction whichever value `secret` takes.
#[test]
fn a_credential_cannot_also_reach_a_request_position() {
    for secret in ["true", "false"] {
        let config = format!(
            r#"
[[config]]
name = "leaky"
label = "Acme token"
help = "For the refusal test only"
format = "token"
secret = {secret}
binds = "credential.acme.api_token"
also_binds = ["header.X-Acme-Token"]
"#
        );
        let error = refuse(&two_pins(&config));
        assert!(
            error.contains("leaky") && error.contains("That value is"),
            "a credential that also lands in a header must be refused by the `secret`/`binds` \
             agreement with `secret = {secret}`, because the two destinations disagree about what \
             the value is:\n{error}"
        );
    }
}
