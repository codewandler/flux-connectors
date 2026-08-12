//! Contentful (C-177) is the epic's probe for **a service that owns its base URL *and* its
//! credential, with more than one template variable in that URL.**
//!
//! Two hosts, two tokens, one vendor: the Content Delivery API (`cdn.contentful.com`, read-only,
//! published content) and the Content Management API (`api.contentful.com`, create and publish
//! drafts) are different authorities with different bearer tokens. `providers/postmark.toml`
//! already measured *that* shape — two credentials, partitioned by service, enforced by
//! `Operation::auth` overriding `default_auth` (`ir.rs:652-669`), with `credential_ref_for`
//! (`ir.rs:1166-1178`) still eliding every credential under the reserved default service regardless.
//! This file does not re-measure that question; it measures the one C-177 actually adds.
//!
//! **What is new here:** every Contentful call is scoped to a space *and* an environment —
//! `/spaces/{space_id}/environments/{environment_id}/...` — and both are properties of the
//! *installation*, not of an individual call: a token is provisioned against one known space, the
//! same way a Zendesk token is provisioned against one known subdomain
//! (`providers/zendesk.toml`'s `{subdomain}`). So both ride in the **service's own `base_url`
//! template** (`AGENTS.md`'s service contract: "a service owns its base URL... each emitted manifest
//! carries its own service's `base_url`"), not as per-operation path parameters the way Airtable's
//! `baseId`/`tableId` are (`providers/airtable.toml`) — Airtable's pair genuinely differs from one
//! call to the next; Contentful's space/environment pair does not. This is the first provider whose
//! service `base_url` carries **two** template variables rather than one, and the first where **two**
//! distinct services each need their own copy of both — `crates/connector-spec/src/config.rs`'s
//! `template_variables` and `validate_every_template_variable_is_asked_for` are generic over the
//! count, and this measures that genericity rather than assuming it.
//!
//! This is also what makes `verify` honest: `contentful-entries-list` declares **no path parameter at
//! all** — `space_id`/`environment_id` are already resolved from configuration before the call is
//! built, and it is a read a settings page can press without knowing an entry or asset id in advance.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::Connector;

use crate::shipped_provider;

const PROVIDER: &str = "contentful";

const DELIVERY_SERVICE: &str = "delivery";
const MANAGEMENT_SERVICE: &str = "management";

const DELIVERY_TOKEN: &str = "contentful.delivery_token";
const MANAGEMENT_TOKEN: &str = "contentful.management_token";

const DELIVERY_BASE_URL: &str =
    "https://cdn.contentful.com/spaces/{space_id}/environments/{environment_id}";
const MANAGEMENT_BASE_URL: &str =
    "https://api.contentful.com/spaces/{space_id}/environments/{environment_id}";

/// The three Delivery-token reads, in declaration order.
const DELIVERY_OPERATIONS: &[&str] = &[
    "contentful-entry-get",
    "contentful-entries-list",
    "contentful-asset-get",
];

/// The two Management-token writes, in declaration order.
const MANAGEMENT_OPERATIONS: &[&str] = &["contentful-entry-create", "contentful-entry-publish"];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-177 ships the Contentful connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// **Finding 1: each service owns a genuinely different base URL, and each carries two template
/// variables — the stress no earlier provider's single-variable `{subdomain}`/`{shop}`/`{site}`
/// exercised.**
#[test]
fn each_service_owns_its_own_two_variable_base_url() {
    let connector = load();

    assert_eq!(connector.base_url_of(DELIVERY_SERVICE), DELIVERY_BASE_URL);
    assert_eq!(
        connector.base_url_of(MANAGEMENT_SERVICE),
        MANAGEMENT_BASE_URL
    );
    assert_ne!(
        connector.base_url_of(DELIVERY_SERVICE),
        connector.base_url_of(MANAGEMENT_SERVICE),
        "delivery and management are different authorities with different hosts, not one host \
         shared by both services"
    );

    // Every config field that binds `endpoint.space_id` / `endpoint.environment_id` is scoped to
    // the service whose template it closes — one pair of fields per service, not one pair shared
    // across both, because `ConfigField::service` is "exactly one, always concrete." The names
    // themselves differ per service (`delivery_space_id` vs `management_space_id`) because a
    // `ConfigField::name` is unique across the *whole connector*, not per service
    // (`validate_config`) — measured directly here rather than assumed.
    for (service, space_field, environment_field) in [
        (
            DELIVERY_SERVICE,
            "delivery_space_id",
            "delivery_environment_id",
        ),
        (
            MANAGEMENT_SERVICE,
            "management_space_id",
            "management_environment_id",
        ),
    ] {
        let fields: Vec<&connector_spec::ConfigField> = connector.config_of(service).collect();
        let space = fields
            .iter()
            .find(|field| field.name == space_field)
            .unwrap_or_else(|| panic!("service {service:?} must declare `{space_field}`"));
        assert_eq!(space.binds, "endpoint.space_id");
        let environment = fields
            .iter()
            .find(|field| field.name == environment_field)
            .unwrap_or_else(|| panic!("service {service:?} must declare `{environment_field}`"));
        assert_eq!(environment.binds, "endpoint.environment_id");
    }
}

/// **Finding 2: the two tokens are never sent together — every operation's effective auth resolves
/// to exactly its own service's credential**, the same per-operation `auth`-overriding-`default_auth`
/// mechanism `providers/postmark.toml` already shipped (`ir.rs:652-669`). Not re-derived here, only
/// exercised on this connector's own two services.
#[test]
fn every_operation_resolves_to_its_own_services_token_only() {
    let connector = load();

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    let mut expected: Vec<&str> = DELIVERY_OPERATIONS.to_vec();
    expected.extend_from_slice(MANAGEMENT_OPERATIONS);
    assert_eq!(declared, expected);

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "`{}` must have exactly one auth alternative",
            operation.id
        );
        let requirement = &effective[0];
        assert_eq!(
            requirement.len(),
            1,
            "`{}` must name one credential",
            operation.id
        );

        let expected_credential = if operation.service == MANAGEMENT_SERVICE {
            MANAGEMENT_TOKEN
        } else {
            assert_eq!(operation.service, DELIVERY_SERVICE);
            DELIVERY_TOKEN
        };
        assert!(
            requirement.contains(expected_credential),
            "`{}` (service {:?}) must resolve to `{expected_credential}`",
            operation.id,
            operation.service
        );
    }
}

/// **Finding 3: `verify` runs with no per-call argument at all.** `space_id`/`environment_id` are
/// resolved from configuration before the request is built, so `contentful-entries-list` needs
/// nothing a settings page would have to invent — no entry id, no asset id.
#[test]
fn verify_is_argument_free() {
    let connector = load();

    let verify_id = connector
        .verify
        .as_deref()
        .expect("contentful declares a `verify` operation");
    assert_eq!(verify_id, "contentful-entries-list");

    let verify = connector
        .operation(verify_id)
        .expect("`verify` names a declared operation");
    assert!(
        verify.params.path.is_empty(),
        "`{verify_id}` must take no path parameter — space/environment are already resolved from \
         configuration, and a `verify` op that needed an entry or asset id could not run unattended"
    );
    assert!(
        verify.params.iter().all(|param| !param.required),
        "`{verify_id}` must have no *required* parameter at all, so it can run with nothing beyond \
         what configuration already resolved"
    );
    assert_eq!(verify.service, DELIVERY_SERVICE);
    assert_ne!(
        verify.risk,
        connector_spec::Risk::High,
        "a verify op must not be high risk"
    );
    assert_ne!(
        verify.risk,
        connector_spec::Risk::Destructive,
        "a verify op must not be destructive"
    );
}

/// Every curated operation actually emits — including `contentful-entry-create`, whose body is a
/// free-form `body_schema` (`{"fields": {...}}`, keyed by the space's own content-model field ids)
/// rather than named `params.body` fields. Contentful's field object is dynamic per content type, the
/// same reason `providers/airtable.toml`'s `fields` and `providers/babelforce.toml`'s session
/// variables are declared with `body_schema` rather than composed through `BodyNode` — so this
/// connector never needs `BodyNode`'s missing array variant (C-168/C-185) at all: the whole body is
/// one opaque schema, not a tree of named `wire` paths.
#[test]
fn every_curated_operation_emits() {
    let connector = load();
    for operation in &connector.operations {
        emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
    }
}

/// No state-changing operation declares `risk = "low"`, and no `POST`/`PATCH` declares
/// `idempotency = "idempotent"` (RFC 9110 §9.2.2, C-186) — `contentful-entry-create` is a `POST` and
/// must be `non_idempotent`; `contentful-entry-publish` is a `PUT`, which the emitter leaves alone.
#[test]
fn writes_declare_real_risk_and_the_post_is_not_idempotent() {
    let connector = load();

    let create = connector
        .operation("contentful-entry-create")
        .expect("contentful-entry-create is declared");
    assert_eq!(create.method, connector_spec::HttpMethod::Post);
    assert_ne!(create.risk, connector_spec::Risk::Low);
    assert_eq!(
        create.idempotency,
        connector_spec::Idempotency::NonIdempotent
    );

    let publish = connector
        .operation("contentful-entry-publish")
        .expect("contentful-entry-publish is declared");
    assert_eq!(publish.method, connector_spec::HttpMethod::Put);
    assert_ne!(publish.risk, connector_spec::Risk::Low);
}
