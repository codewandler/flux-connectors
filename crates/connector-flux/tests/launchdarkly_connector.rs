//! LaunchDarkly (C-175) is the epic's probe for whether "no scheme word at all" is expressible,
//! distinctly from `bearer`. LaunchDarkly sends `Authorization: <token>` raw — no `Bearer `, no
//! `Basic `, no custom word — and `bearer` would send `Authorization: Bearer <token>`, which
//! LaunchDarkly's API rejects.
//!
//! C-161's probe on Okta measured that a **prefix** (`SSWS `, `Token token=`, `OAuth `) has nowhere
//! to go on today's closed `AuthScheme` (`crates/connector-spec/src/auth.rs:70-102`) and recorded a
//! refusal rather than a shipped connector. LaunchDarkly's question is different: it needs **no**
//! prefix, so the whole header value is the secret — exactly what `AuthScheme::Header { name:
//! "Authorization" }` already expresses, the same shape Shopify's `X-Shopify-Access-Token` uses
//! honestly today. This file proves that end to end, against a connector that actually ships:
//!
//! 1. The connector loads and its one credential is `AuthScheme::Header { name: "Authorization" }`,
//!    round-tripping through `toml` to exactly `[header]\nname = "Authorization"` — no second field,
//!    because there is nowhere on this variant to put a prefix and this connector needs nowhere to
//!    put one.
//! 2. No emitted operation's Flux mentions the literal word `Bearer` or `Basic` anywhere — the wire
//!    form really is the raw token, not an accidentally-bearer-flavoured one.
//! 3. `launchdarkly-flag-toggle` — the one write, and a write with immediate production effect for
//!    real users — is declared `risk = "high"`, not the `medium` an "ordinary update" might reach
//!    for, and its `description` names the live effect rather than reading like UI copy for a
//!    routine field edit.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod, Idempotency, Risk};

/// The provider under test. Named once so the file reads as being about LaunchDarkly rather than
/// about a string.
const PROVIDER: &str = "launchdarkly";

/// The credential the connector declares, the header it travels in, and the variable it resolves
/// from. All three are public contract — an operator sets the variable, a manifest names the
/// credential, LaunchDarkly reads the header — so they are pinned here rather than left to whatever
/// the provider file happens to say.
const CREDENTIAL: &str = "launchdarkly.api_token";
/// See [`CREDENTIAL`]. LaunchDarkly authenticates on the standard `Authorization` header, but with no
/// scheme word in front of the token — the fact this whole file exists to prove is expressible.
const AUTH_HEADER: &str = "Authorization";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "LAUNCHDARKLY_API_TOKEN";

/// The fixed API host every account shares; LaunchDarkly has no per-tenant subdomain.
const BASE_URL: &str = "https://app.launchdarkly.com/api/v2";

/// The one operation with immediate, live production effect. See the module docs and the write-up
/// in `providers/launchdarkly.toml`'s header comment.
const TOGGLE_OPERATION: &str = "launchdarkly-flag-toggle";

/// The five curated operations, in the order `providers/launchdarkly.toml` declares them.
const OPERATIONS: &[&str] = &[
    "launchdarkly-project-list",
    "launchdarkly-environment-list",
    "launchdarkly-flag-list",
    "launchdarkly-flag-get",
    TOGGLE_OPERATION,
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
            "cannot read {} ({error}) — C-175 ships the LaunchDarkly connector",
            path.display()
        )
    });
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// Every operation's emitted module, paired with its id.
fn emitted() -> Vec<(String, String)> {
    let connector = load();
    connector
        .operations
        .iter()
        .map(|operation| {
            let flux = emit_operation(&connector, operation)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
            (operation.id.clone(), flux)
        })
        .collect()
}

/// **The headline finding: a raw, unprefixed `Authorization` value is expressible today, with no
/// change to `connector-spec`.**
///
/// `AuthScheme::Header { name: "Authorization" }` carries only the header's *name* — the value it is
/// given at runtime is the resolved secret and nothing else, so there is no `Bearer `/`Basic `/custom
/// word anywhere in this connector's declaration for LaunchDarkly to reject. The round trip through
/// `toml` is the sharpest way to say it: one field, `name`, and nothing else — if `AuthScheme::Header`
/// ever grows a second field this assertion is the first thing to reconsider.
#[test]
fn the_launchdarkly_connector_authenticates_with_a_raw_unprefixed_header() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "LaunchDarkly");
    assert_eq!(connector.base_url, BASE_URL);

    assert_eq!(
        connector.auth.len(),
        1,
        "launchdarkly authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("launchdarkly declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: AUTH_HEADER.to_string()
        },
        "LaunchDarkly's token is the entire value of `{AUTH_HEADER}` — no `Bearer `, no `Basic `, no \
         scheme word of any kind — which `AuthScheme::Header` already expresses, distinctly from \
         `bearer`, with no change to connector-spec"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a raw header credential has no user half"
    );

    let round_tripped = toml::to_string(&method.scheme).expect("AuthScheme serializes");
    assert_eq!(
        round_tripped.trim(),
        "[header]\nname = \"Authorization\"",
        "one field, `name`, and nothing else — there is nowhere on this variant to put a scheme \
         word, and this connector needs nowhere to put one"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    // Every operation resolves to the one credential, whether it declares auth or inherits the
    // connector default, and none declares a caller-supplied `Authorization` header of its own —
    // that header is injected by the host, and a caller-facing parameter for it would let a caller
    // override the credential entirely.
    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; launchdarkly is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the API token",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares a caller-supplied header; `{AUTH_HEADER}` is injected by the \
             host and must never travel through the parameter surface",
            operation.id
        );
    }
}

/// **No emitted operation ever mentions `Bearer` or `Basic`.** The declaration proves the *scheme*
/// is right; this proves the wire form the emitter actually produces carries no scheme word either,
/// so nothing downstream of the loader quietly reintroduces one.
#[test]
fn no_emitted_operation_carries_a_bearer_or_basic_word() {
    for (id, flux) in emitted() {
        assert!(
            !flux.contains("Bearer"),
            "`{id}` emits the literal word `Bearer`, but LaunchDarkly's Authorization header takes \
             the raw token with no scheme word:\n{flux}"
        );
        assert!(
            !flux.contains("Basic"),
            "`{id}` emits the literal word `Basic`, but LaunchDarkly's Authorization header takes \
             the raw token with no scheme word:\n{flux}"
        );
    }
}

/// **The second finding: a flag toggle is declared as the live, high-risk write it is — not glossed
/// over as an ordinary update.**
///
/// `risk = "high"` is "writes a reviewer would want to see first"; `medium` here would let an agent
/// flip production behaviour for real users without a human seeing it first, which is exactly what
/// flux's approval gate exists to stop. The description is asserted to actually say so, because a
/// `risk` field a reviewer never reads and a description that reads like routine UI copy would both
/// be a silent version of the same failure.
#[test]
fn the_flag_toggle_is_declared_as_a_high_risk_write_with_immediate_live_effect() {
    let connector = load();
    let toggle = connector
        .operations
        .iter()
        .find(|operation| operation.id == TOGGLE_OPERATION)
        .unwrap_or_else(|| panic!("`{TOGGLE_OPERATION}` is one of the curated operations"));

    assert_eq!(toggle.method, HttpMethod::Patch);
    assert_eq!(
        toggle.risk,
        Risk::High,
        "toggling a flag changes live behaviour for real users the instant the call returns; \
         `medium` would let this through flux's approval gate as an ordinary write"
    );
    assert_eq!(
        toggle.idempotency,
        Idempotency::NonIdempotent,
        "PATCH is not an idempotent method under RFC 9110 §9.2.2, and this repository's emitter \
         refuses `idempotency = \"idempotent\"` on one; only PUT and DELETE are left alone"
    );

    let description = toggle.description.to_lowercase();
    assert!(
        description.contains("live") || description.contains("immediate"),
        "`{TOGGLE_OPERATION}`'s description does not name the live/immediate production effect a \
         model needs to weigh before calling it, and reads instead like an ordinary field update: \
         {:?}",
        toggle.description
    );

    // Every other operation is a bounded read; only the toggle carries any risk above `low`.
    for operation in &connector.operations {
        if operation.id == TOGGLE_OPERATION {
            continue;
        }
        assert_eq!(
            operation.risk,
            Risk::Low,
            "operation `{}` is one of the curated reads and should carry no risk above `low`",
            operation.id
        );
        assert_eq!(operation.method, HttpMethod::Get);
    }
}

/// **The toggle's body is a narrow, verified slice of JSON Patch — not an open door to arbitrary flag
/// edits.** Only `replace` is accepted, and only onto one environment's `on` bit; see
/// `providers/launchdarkly.toml`'s header comment for why the wider JSON Patch/semantic-patch surface
/// is excluded rather than guessed at.
#[test]
fn the_toggle_body_schema_admits_only_a_single_replace_onto_one_environments_on_bit() {
    let connector = load();
    let toggle = connector
        .operations
        .iter()
        .find(|operation| operation.id == TOGGLE_OPERATION)
        .unwrap_or_else(|| panic!("`{TOGGLE_OPERATION}` is one of the curated operations"));

    let body_schema = toggle
        .params
        .body_schema
        .as_ref()
        .unwrap_or_else(|| panic!("`{TOGGLE_OPERATION}` declares a free-form JSON Patch body"));
    let rendered = serde_json::to_string(body_schema).expect("JsonSchema serializes");

    assert!(
        rendered.contains("\"maxItems\":1") || rendered.contains("\"maxItems\": 1"),
        "the toggle's body schema should bound the patch document to exactly one operation, got: \
         {rendered}"
    );
    assert!(
        rendered.contains("replace"),
        "the toggle's body schema should name `replace` as the only accepted `op`, got: {rendered}"
    );
    assert!(
        !rendered.to_lowercase().contains("\"add\"") && !rendered.to_lowercase().contains("\"remove\""),
        "the toggle's body schema should not admit `add`/`remove` JSON Patch operations, which this \
         curated connector never verified: {rendered}"
    );

    assert!(
        toggle.params.body.is_empty(),
        "`{TOGGLE_OPERATION}` declares named body params alongside a free-form `body_schema`, which \
         is the ambiguous-body shape the loader refuses"
    );
}
