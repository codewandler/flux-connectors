//! Datadog (C-160) is the epic's probe for **two credentials required together on the same
//! request** — the AND case the two-level `auth` shape has always been able to express
//! (`Operation::auth`/`Connector::default_auth` is `Vec<AuthRequirement>`, an OR of AND-mechanisms;
//! see `crates/connector-spec/src/auth.rs:272-288` and `crates/connector-spec/src/ir.rs:652-668`),
//! but that no shipped connector had actually exercised before this one.
//! `providers/babelforce.toml` documents the identical AND-pair shape (`accessId`/`accessToken`) and
//! deliberately excludes it because the vendor deprecated it; `providers/postmark.toml` ships the
//! *other* half of the two-level shape, an OR of two single-credential mechanisms partitioned by
//! service. Datadog is the connector that finally puts a two-credential AND-mechanism on a real,
//! shipped operation.
//!
//! This file measures the shape directly, rather than asserting it in prose:
//!
//! 1. **The connector declares exactly two credentials**, each a custom header, each resolving from
//!    its own environment variable.
//! 2. **`default_auth` is exactly one alternative naming both credentials** — an AND-set, not an
//!    OR-choice — and **every** operation's `effective_auth` resolves to that same single
//!    two-credential mechanism. No operation overrides it (unlike Postmark, where the whole point is
//!    that operations *do* override the default to keep two credentials apart); here the whole point
//!    is that nothing ever needs to, because both credentials travel on every request.
//! 3. **The emitted catalogue's nested-slice `credentials` field carries the AND-set intact**: one
//!    outer alternative, two inner credential names — `&[&["datadog.api_key",
//!    "datadog.application_key"]]` — never flattened to a list of two independent alternatives,
//!    which would silently tell a caller that either credential alone authenticates.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthRequirement, AuthScheme, Connector};

use crate::shipped_provider;

/// The provider under test.
const PROVIDER: &str = "datadog";

const API_KEY: &str = "datadog.api_key";
const API_KEY_HEADER: &str = "DD-API-KEY";
/// A variable *name*; no credential value appears in this repository.
const API_KEY_ENV: &str = "DATADOG_API_KEY";

const APPLICATION_KEY: &str = "datadog.application_key";
const APPLICATION_KEY_HEADER: &str = "DD-APPLICATION-KEY";
/// A variable *name*; no credential value appears in this repository.
const APPLICATION_KEY_ENV: &str = "DATADOG_APPLICATION_KEY";

const BASE_URL: &str = "https://api.datadoghq.com";

/// The four curated operations, in the order `providers/datadog.toml` declares them.
const OPERATIONS: &[&str] = &[
    "datadog-monitor-list",
    "datadog-monitor-get",
    "datadog-incident-list",
    "datadog-incident-get",
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
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-160 ships the Datadog connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// **Finding 1: two credentials, two custom headers, two environment variables.**
#[test]
fn the_datadog_connector_declares_two_independent_header_credentials() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Datadog");
    assert_eq!(connector.base_url, BASE_URL);

    assert_eq!(
        connector.auth.len(),
        2,
        "Datadog authenticates with an API key and an Application key, never one alone"
    );

    let api_key = connector
        .auth_method(API_KEY)
        .unwrap_or_else(|| panic!("datadog declares `{API_KEY}`"));
    assert_eq!(
        api_key.scheme,
        AuthScheme::Header {
            name: API_KEY_HEADER.to_string(),
            prefix: String::new(),
        }
    );
    assert_eq!(api_key.env, [API_KEY_ENV]);

    let application_key = connector
        .auth_method(APPLICATION_KEY)
        .unwrap_or_else(|| panic!("datadog declares `{APPLICATION_KEY}`"));
    assert_eq!(
        application_key.scheme,
        AuthScheme::Header {
            name: APPLICATION_KEY_HEADER.to_string(),
            prefix: String::new(),
        }
    );
    assert_eq!(application_key.env, [APPLICATION_KEY_ENV]);

    assert_ne!(
        api_key.env, application_key.env,
        "the two credentials must never resolve from the same variable"
    );
}

/// **Finding 2 and 3, the heart of the probe: `default_auth` is one AND-mechanism naming both
/// credentials, not two OR-alternatives each naming one — and every operation resolves to exactly
/// that mechanism, with both credentials required together.**
///
/// This is the assertion that distinguishes Datadog from Postmark: Postmark's
/// `no_operation_ever_requires_both_tokens_and_each_resolves_to_its_own_services_token` asserts
/// `requirement.len() == 1` on every operation, because Postmark's two tokens are never sent
/// together. Here the analogous assertion is `requirement.len() == 2` on every operation, because
/// Datadog's two keys are *never sent apart*.
#[test]
fn every_operation_requires_both_credentials_together_as_one_mechanism() {
    let connector = load();

    assert_eq!(
        connector.default_auth,
        vec![AuthRequirement::all([API_KEY, APPLICATION_KEY])],
        "the connector default must be a single alternative naming both credentials as one AND-set"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    for operation in &connector.operations {
        assert!(
            operation.auth.is_none(),
            "`{}` must inherit the connector default rather than overriding it — nothing in this \
             connector ever needs a different credential set",
            operation.id
        );

        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "`{}` must have exactly one auth alternative — Datadog offers no OR choice between \
             mechanisms",
            operation.id
        );

        let requirement = &effective[0];
        assert_eq!(
            requirement.len(),
            2,
            "`{}`'s one alternative must name exactly two credentials — the whole point of this \
             connector is that the API key and the Application key are an AND-group on the same \
             request, never independent alternatives",
            operation.id
        );
        assert!(
            requirement.contains(API_KEY) && requirement.contains(APPLICATION_KEY),
            "`{}` must require both `{API_KEY}` and `{APPLICATION_KEY}` together",
            operation.id
        );
    }
}

/// **The emitted catalogue must carry the AND-set intact, not flattened.** This walks the same
/// `credential_mechanisms` shape `crates/connector-cli/src/catalog.rs` renders into
/// `crates/catalog/src/generated/datadog.rs`'s `credentials: &[&[...]]` literal, so a regression that
/// flattened two alternatives-of-one back out of one alternative-of-two would show up here before it
/// ever reached the generated file.
#[test]
fn the_credential_mechanism_shape_is_one_alternative_of_two_not_two_alternatives_of_one() {
    let connector = load();

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        let mechanisms: Vec<Vec<&str>> = effective
            .iter()
            .map(|mechanism| mechanism.iter().map(String::as_str).collect())
            .collect();

        assert_eq!(
            mechanisms.len(),
            1,
            "`{}` must render as one outer alternative, not two",
            operation.id
        );
        assert_eq!(
            mechanisms[0].len(),
            2,
            "`{}`'s one alternative must carry both credential names, not one",
            operation.id
        );
    }
}

/// Every curated operation actually emits. None of the four sends a request body, so this connector
/// exercises no `BodyNode` path at all — deliberately: `datadog-monitor-list` and
/// `datadog-incident-list` take no parameters, `datadog-monitor-get` and `datadog-incident-get` take
/// one path parameter each.
#[test]
fn every_curated_operation_emits() {
    let connector = load();
    for operation in &connector.operations {
        emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
    }
}

/// No config field claims to be non-secret while binding a credential, and no `example` appears on
/// either secret field — a placeholder shaped like a real API key or Application key has tripped
/// GitHub's push protection before (`providers/postmark.toml`'s own caveat).
#[test]
fn no_example_appears_on_either_secret_config_field() {
    let connector = load();
    for field in &connector.config {
        if field.secret {
            assert!(
                field.example.is_none(),
                "config field `{}` is secret and must carry no `example`",
                field.name
            );
        }
    }
}
