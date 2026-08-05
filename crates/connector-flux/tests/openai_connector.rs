//! The OpenAI connector's own contract (C-51) — the claims that are true of *this* provider and of
//! no other, held against `providers/openai.toml` as committed.
//!
//! `shipped_modules.rs` next door asserts the properties every provider shares: each operation
//! emits, parses, is canonical, and reloads. This file exists because OpenAI's reason for shipping
//! is a **narrower** claim than that, and one the shared tests cannot make.
//!
//! # The headline invariant: only integer limits enter the query string
//!
//! C-30 is not implemented. `connector-flux` interpolates a query value into the URL verbatim —
//! nothing percent-encodes it (`crates/connector-flux/src/op.rs`, module docs) — so a string-ish
//! query value carrying `&`, `#` or `+` corrupts the request and can *inject a parameter*.
//! `zendesk-ticket-search` is the standing proof: `providers/zendesk.toml` declares it
//! KNOWN NON-FUNCTIONAL for exactly this reason, and `AGENTS.md` lists it under intentional gaps.
//!
//! OpenAI's established surface avoided the gap entirely. C-472 adds three first-party collection
//! reads whose only query is an integer `limit`; its decimal rendering cannot introduce a second
//! query pair. String cursors, ordering, expansion and streaming controls remain omitted. That is a
//! **property to enforce, not a coincidence to observe**: a later string query must fail here rather
//! than arrive as a latent injection. [`every_openai_query_is_an_integer_limit`] is deliberately
//! stated over the whole connector rather than per operation — "this one is fine" is not the claim.
//!
//! # Why the assertions are stated over the loaded IR and the emitted text
//!
//! Reading the TOML with a regex would pass on a file the loader rejects, and asserting on the
//! committed `connectors/openai.flux` would pass on bytes the emitter no longer produces. So the
//! file is loaded through the real loader and the Flux is re-emitted here, exactly as
//! `shipped_modules.rs` does. Staleness of the committed artifacts is `connector-cli`'s
//! `catalog_artifacts.rs` and `site_catalog.rs`; that is a different failure and has its own test.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, Idempotency, OperationDirection, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider id, and therefore the file name, the module name and every op id's prefix.
const PROVIDER: &str = "openai";

/// OpenAI serves every selected operation from one tenant-independent host, so `base_url` carries no
/// template variable and the connector's whole egress surface is this single name.
const BASE_URL: &str = "https://api.openai.com";

/// The one environment variable the connector resolves its secret from. A **name**, never a value —
/// AGENTS.md's hard invariant, re-asserted from the emitted text in
/// [`the_emitted_flux_carries_no_credential_at_all`].
const SECRET_ENV: &str = "OPENAI_API_KEY";

/// The closed C-472 set allowed to expose OpenAI's integer `limit` query.
const INTEGER_LIMIT_OPERATIONS: [&str; 3] = [
    "openai-response-input-item-list",
    "openai-file-list",
    "openai-batch-list",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// `providers/openai.toml`, through the real loader.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-51 ships this file",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// **The connector exists and is well-formed.** Everything below reads the loaded IR, so a failure
/// here would otherwise surface four times over as something less specific.
#[test]
fn the_openai_connector_loads() {
    let connector = load();
    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "OpenAI");
    assert_eq!(connector.base_url, BASE_URL);
    assert!(
        !connector.operations.is_empty(),
        "a connector that declares no operation describes nothing"
    );
    for operation in &connector.operations {
        assert!(
            operation.id.starts_with("openai-"),
            "`{}` does not name its provider; the op id is the public name a model calls",
            operation.id
        );
    }
}

/// **The headline invariant.** See the module documentation: C-30 is not implemented, so every
/// caller-visible query must have a representation that cannot change the query shape. C-472 keeps
/// only integer `limit`; stated over every operation, including the ones a later author adds.
#[test]
fn every_openai_query_is_an_integer_limit() {
    let connector = load();
    for operation in &connector.operations {
        if INTEGER_LIMIT_OPERATIONS.contains(&operation.id.as_str()) {
            assert_eq!(
                operation.params.query.len(),
                1,
                "`{}` must expose exactly its reviewed C-472 integer limit",
                operation.id
            );
            let param = &operation.params.query[0];
            assert_eq!(param.name, "limit");
            assert_eq!(
                param.schema["type"],
                serde_json::json!("integer"),
                "`{}` has a non-integer limit, whose rendering could change the query shape",
                operation.id
            );
        } else {
            let declared: Vec<&str> = operation
                .params
                .query
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect();
            assert!(
                declared.is_empty(),
                "`{}` exposes unreviewed queries {declared:?}; only the three exact C-472 list \
                 operations may carry integer `limit` until C-30 lands",
                operation.id
            );
        }
    }
}

/// **Bearer over one env var, for every operation.** The auth model is the reason this connector is
/// cheap: one credential, one scheme, no user half, no acquisition step.
#[test]
fn every_operation_authenticates_as_bearer_over_one_env_var() {
    let connector = load();

    let [method] = connector.auth.as_slice() else {
        panic!(
            "OpenAI declares exactly one credential; found {}",
            connector.auth.len()
        );
    };
    assert_eq!(method.scheme, AuthScheme::Bearer);
    assert_eq!(method.env, vec![SECRET_ENV.to_string()]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` belong to the `basic` scheme only"
    );
    assert!(
        method.oauth2.is_none(),
        "the key is minted in OpenAI's dashboard; flux runs no grant for it"
    );

    // `effective_auth`, not `Operation::auth`: an operation that declares nothing inherits
    // `default_auth`, and one declaring an explicit `[]` inherits nothing. Reading the field
    // directly would let an accidentally-unauthenticated operation pass.
    for operation in &connector.operations {
        let alternatives = connector.effective_auth(operation);
        assert_eq!(
            alternatives.len(),
            1,
            "`{}` has no single credential mechanism",
            operation.id
        );
        assert!(
            alternatives[0].contains(&method.name),
            "`{}` does not require {:?}",
            operation.id,
            method.name
        );
    }
}

/// **An operation an LLM can call that spends money is not `low` risk or idempotent.**
///
/// `connector-flux`'s `check_write_metadata` already refuses both for any authored write, so
/// this is not the emitter's gate restated: it is the *reason* stated where a reader looking at
/// OpenAI will find it. Inference is billed per token, and `risk` is what flux's approval gate
/// reads before letting a model run the call unattended.
#[test]
fn the_cost_bearing_operations_declare_what_they_cost() {
    let connector = load();
    for operation in &connector.operations {
        let mutates = operation.direction == OperationDirection::Write;
        if !mutates {
            continue;
        }
        assert_ne!(
            operation.risk,
            Risk::Low,
            "`{}` bills the account per call; `low` is what flux's approval gate waves through",
            operation.id
        );
        assert_ne!(
            operation.idempotency,
            Idempotency::Idempotent,
            "`{}` is an authored write: declaring it idempotent lets Flux skip or retry it unsafely",
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
fn every_openai_operation_emits_an_analyzable_module() {
    let connector = load();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` is not emittable: {error}", operation.id));

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

/// **The host is exactly `api.openai.com`, and the request goes nowhere else.**
///
/// `base_url` carries no `{template}`, which is what makes this connector's egress surface a single
/// literal name a reviewer can check — unlike zendesk's `{subdomain}` and freshdesk's `{domain}`,
/// which are unbound (C-17). Asserted against the emitted `$base`, because that is the string the
/// request is actually built from.
#[test]
fn every_request_targets_api_openai_com_and_nothing_wider() {
    let connector = load();
    assert!(
        !connector.base_url.contains('{'),
        "`base_url` must be a bound literal, not a template: {:?}",
        connector.base_url
    );

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).expect("openai operations emit");
        assert!(
            emitted.contains(&format!(r#"base = "{BASE_URL}""#)),
            "`{}` does not bind the OpenAI base URL:\n{emitted}",
            operation.id
        );
        assert!(
            operation.path.starts_with("/v1/"),
            "`{}` has path {:?}; every selected OpenAI endpoint is under `/v1/`",
            operation.id,
            operation.path
        );
    }
}

/// **No credential value, and not even a credential *name*, reaches the generated Flux**
/// (AGENTS.md). The connector carries the env-var name so a host can resolve it; the emitted module
/// carries neither that name nor anything shaped like a key, because auth injection is C-10 and is
/// deliberately absent rather than stubbed.
#[test]
fn the_emitted_flux_carries_no_credential_at_all() {
    let connector = load();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).expect("openai operations emit");
        assert!(
            !emitted.contains(SECRET_ENV),
            "`{}` names {SECRET_ENV} in generated Flux:\n{emitted}",
            operation.id
        );
        // OpenAI's keys are `sk-`-prefixed. A literal one in a generated artifact is the failure
        // this invariant exists to prevent, so it is checked for by shape and not only by name.
        assert!(
            !emitted.contains("sk-"),
            "`{}` embeds something shaped like an OpenAI key:\n{emitted}",
            operation.id
        );
    }
}
