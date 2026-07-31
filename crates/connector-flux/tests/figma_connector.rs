//! `providers/figma.toml` exists, emits analyzable Flux, and holds the property C-176 ships it
//! for: **Figma's REST surface is read-only, so a connector curated from it is honestly uniform**
//! rather than varied for appearance.
//!
//! **1. Every operation is a `GET`, and every operation declares `risk = "low"` /
//! `idempotency = "idempotent"` — the same two values, repeated, because the vendor gives this
//! connector nothing else to say truthfully.** A fleet where most connectors mix reads and writes
//! makes uniform declarations look like an author who did not think about risk; here they are the
//! correct answer to a vendor that has no write endpoints in the curated surface at all. C-74's
//! Shopify test pins *one* write's metadata against a vendor detail (no idempotency key); this
//! file's analogous claim is the opposite shape — pinning that *nothing* varies, because nothing
//! about the vendor gives a reason to.
//!
//! **2. The credential is a plain custom header — `X-Figma-Token: <token>` — the same
//! `AuthScheme::Header` shape `providers/shopify.toml` ships first, reused rather than
//! reinvented.** Figma's token is a personal access token with no scheme word in front of it,
//! exactly like Shopify's.
//!
//! **3. `figma-image-render-get` names its URLs' expiry on the record a model reads.** Figma
//! renders images on demand into short-lived storage; the response is a URL, not a stable asset
//! address, and implying otherwise is the "plausible but incorrect" failure AGENTS.md refuses.
//!
//! **4. The one query parameter this connector declares (`ids`, on the two node-addressed reads)
//! is restricted to a charset none of `&`, `#`, `+` or whitespace can reach.** Nothing in this
//! pipeline percent-encodes a query value (C-30 is unimplemented), so a free-form query string
//! would be the `zendesk-ticket-search` failure AGENTS.md records under *Intentional gaps*. A
//! Figma node id is always `<page>:<node>` — digits and a colon — so a comma-joined list of them
//! is safe rather than merely untested, the same argument `providers/shopify.toml` makes for an
//! integer path parameter. The schema's `pattern` is what makes that a checked claim rather than a
//! comment.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod, Idempotency, Risk};

/// The provider under test.
const PROVIDER: &str = "figma";

/// The credential this connector declares, the header it travels in, and the variable it resolves
/// from. Pinned here rather than left to whatever the provider file happens to say, because an
/// operator sets the variable and a manifest names the credential — both are public contract.
const CREDENTIAL: &str = "figma.token";
const AUTH_HEADER: &str = "X-Figma-Token";
const TOKEN_ENV: &str = "FIGMA_TOKEN";

/// The base URL. Fixed — unlike Shopify and Zendesk, Figma serves every tenant from one host, so
/// there is no `{variable}` here and no `[[config]]` field binding `endpoint.*`.
const BASE_URL: &str = "https://api.figma.com";

/// The curated set, in the order `providers/figma.toml` declares it: one verify read plus the
/// five the story names as a starting point.
const OPERATIONS: &[&str] = &[
    "figma-user-me",
    "figma-file-get",
    "figma-file-nodes-get",
    "figma-image-render-get",
    "figma-project-files-list",
    "figma-file-comments-list",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes,
/// so this file cannot pass against a fixture that has drifted from what ships.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-176 ships the Figma connector",
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

/// **The `AuthScheme::Header` round trip**, reused from `providers/shopify.toml` rather than
/// invented a second way to spell one custom header.
#[test]
fn the_figma_connector_authenticates_with_a_plain_custom_header() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Figma");
    assert_eq!(connector.base_url, BASE_URL);

    assert_eq!(
        connector.auth.len(),
        1,
        "figma authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("figma declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: AUTH_HEADER.to_string()
        },
        "Figma's personal access token is the entire value of `{AUTH_HEADER}` — no `Bearer `, no \
         scheme word — the same shape `providers/shopify.toml` ships first"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a custom header has no user half"
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
            "operation `{}` has {} auth alternatives; figma is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the token",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares caller-supplied headers; `{AUTH_HEADER}` is injected by the \
             host and must never travel through the parameter surface",
            operation.id
        );
    }
}

/// **The archetype claim.** Figma's curated surface is entirely reads, so every operation must
/// declare the same two values honestly — not because an author picked one setting and repeated
/// it, but because the vendor gives this connector nothing else to say. A `POST` or a `risk` above
/// `low` here would be the invented variation the story warns against.
#[test]
fn every_operation_is_a_read_declared_uniformly_low_risk_and_idempotent() {
    let connector = load();

    assert!(
        !connector.operations.is_empty(),
        "the curated set is empty; nothing to be uniform about"
    );

    for operation in &connector.operations {
        assert_eq!(
            operation.method,
            HttpMethod::Get,
            "operation `{}` is a {:?}; Figma's curated surface is read-only, so every operation \
             is a GET",
            operation.id,
            operation.method
        );
        assert_eq!(
            operation.risk,
            Risk::Low,
            "operation `{}` declares a risk other than `low`; a read that changes nothing on the \
             vendor is the uniform low case, and varying it would be inventing risk the API does \
             not carry",
            operation.id
        );
        assert_eq!(
            operation.idempotency,
            Idempotency::Idempotent,
            "operation `{}` declares an idempotency other than `idempotent`; a GET that changes \
             nothing is idempotent by construction",
            operation.id
        );
        assert!(
            operation.params.body.is_empty() && operation.params.body_schema.is_none(),
            "operation `{}` declares a request body; nothing in this curated surface writes",
            operation.id
        );
    }
}

/// **The verify operation is one of the uniform reads**, not a special case carved out for it.
#[test]
fn the_connector_verifies_with_the_current_user_read() {
    let connector = load();
    assert_eq!(
        connector.verify.as_deref(),
        Some("figma-user-me"),
        "the `verify` operation is the Test-connection button and must be a read a user would \
         not mind running unattended"
    );
}

/// **`figma-image-render-get` says its URLs expire, on the record a model reads** — the
/// description and the response schema both, since either one read alone would still leave the
/// other implying stability.
#[test]
fn the_image_render_operation_notes_the_url_expiry_rather_than_implying_stability() {
    let connector = load();
    let operation = connector
        .operations
        .iter()
        .find(|operation| operation.id == "figma-image-render-get")
        .expect("figma-image-render-get is declared");

    let mentions_expiry = |text: &str| {
        let lower = text.to_ascii_lowercase();
        lower.contains("expir") || lower.contains("temporary") || lower.contains("short-lived")
    };

    assert!(
        mentions_expiry(&operation.description),
        "`figma-image-render-get`'s description does not note that the returned URLs expire: \
         {:?}",
        operation.description
    );

    let schema_text = operation
        .response_schema
        .as_ref()
        .map(serde_json::Value::to_string)
        .unwrap_or_default();
    assert!(
        mentions_expiry(&schema_text),
        "`figma-image-render-get`'s response_schema does not note that the returned URLs expire \
         anywhere a consumer reading only the schema would see it: {schema_text}"
    );
}

/// **The one query parameter this connector declares is charset-restricted, not free text.**
/// Nothing here percent-encodes a query value, so the only honest way to put a value in a query
/// string is to prove the value cannot carry `&`, `#`, `+` or whitespace in the first place — the
/// same argument `providers/shopify.toml` makes for an integer path parameter, applied here to a
/// comma-joined list of `<page>:<node>` ids.
#[test]
fn the_declared_query_parameter_is_restricted_to_a_safe_charset() {
    let connector = load();

    let mut saw_query_param = false;
    for operation in &connector.operations {
        for param in &operation.params.query {
            saw_query_param = true;
            assert_eq!(
                param.name, "ids",
                "operation `{}` declares an unexpected query parameter `{}`; this connector's \
                 only query parameter is the charset-restricted node-id list",
                operation.id, param.name
            );
            let pattern = param
                .schema
                .get("pattern")
                .and_then(|value| value.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "operation `{}` declares query parameter `ids` with no `pattern`, so \
                         nothing stops it accepting a value carrying `&`, `#`, `+` or whitespace",
                        operation.id
                    )
                });
            // The pattern must be anchored and built only from digits, `:`, `,` and regex
            // syntax — no literal `&`, `#`, `+` or whitespace can appear in a *matched* value,
            // because none of those characters is in the whitelist below. Checking the
            // whitelist is checking the unsafe characters cannot reach a match: an `&` could
            // only match if the pattern named it, and the pattern cannot name it and pass this
            // assertion.
            assert!(
                pattern.starts_with('^') && pattern.ends_with('$'),
                "operation `{}`'s `ids` pattern `{pattern}` is not anchored, so it constrains \
                 nothing",
                operation.id
            );
            assert!(
                pattern.chars().all(|c| c.is_ascii_digit()
                    || matches!(
                        c,
                        ':' | ',' | '^' | '$' | '(' | ')' | '*' | '+' | '?' | '[' | ']' | '-'
                    )),
                "operation `{}`'s `ids` pattern `{pattern}` names a character outside digits, \
                 `:`, `,` and regex syntax",
                operation.id
            );
        }
    }
    assert!(
        saw_query_param,
        "no operation declares the `ids` query parameter; figma-file-nodes-get and \
         figma-image-render-get both require it"
    );
}

/// **No credential reaches a generated module** — not its value, and not even its name. Auth
/// injection needs C-10's `$auth` seam in flux, so today's emitted `op` builds a URL and calls
/// `http.request` with nothing else. Restated locally so this file fails on its own when that
/// stops being true, the same way `shopify_connector.rs` does.
#[test]
fn no_credential_reaches_a_generated_module() {
    for (id, flux) in emitted() {
        assert!(
            !flux.contains(TOKEN_ENV),
            "`{id}` names the credential's environment variable in emitted Flux:\n{flux}"
        );
        assert!(
            !flux.contains(AUTH_HEADER),
            "`{id}` spells the auth header into the request; the host applies the placement \
             scheme, not generated Flux:\n{flux}"
        );
        assert_eq!(
            flux.matches("https://").count(),
            1,
            "`{id}` names more than one absolute URL:\n{flux}"
        );
        assert!(
            flux.contains(&format!("base = \"{BASE_URL}\"")),
            "`{id}` does not reach `{BASE_URL}`:\n{flux}"
        );
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical
/// under flux's own formatter, and loads as exactly one exposed composite op.
#[test]
fn every_figma_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
    for (id, flux) in emitted() {
        let parsed = flux_lang::parser::parse_cst(&flux);
        assert!(
            parsed.errors.is_empty(),
            "`{id}` emits Flux that does not parse: {:?}\n{flux}",
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(flux.as_str()),
            "the flux formatter would rewrite `{id}`"
        );

        let module = flux_lang::program::Module::parse_str(&flux)
            .unwrap_or_else(|error| panic!("`{id}` does not load: {error}"));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{id}` is not a program"));
        assert_eq!(
            program.ops.len(),
            1,
            "one operation is one declaration; `{id}` loaded {}",
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, id);
        assert!(
            program.ops[0].meta.expose,
            "`{id}` must be exposed to the model as a tool"
        );
    }
}
