//! The provider definitions this repository actually ships, loaded through the real loader.
//!
//! The fixtures in `provider_toml.rs` prove the *loader* works on hand-written excerpts. This file
//! proves the opposite direction: that `providers/*.toml` as committed is something the loader
//! accepts. Those are different claims, and only this one fails when someone edits a shipped
//! provider file into a shape the schema rejects — which is the failure a reviewer would otherwise
//! not see until a build.
//!
//! It reads from the repository root rather than an inline constant on purpose: a copy of the file
//! embedded here would be the thing under test drifting away from the thing that ships.

use std::path::{Path, PathBuf};

use connector_spec::{provider, AuthScheme};

/// The three providers C-17 ships, in the order the story names them.
const SHIPPED: &[&str] = &["zendesk", "freshdesk", "babelforce"];

/// `<repo root>/providers`, derived from this crate's manifest directory so the test is independent
/// of the working directory a runner happens to use.
fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn load(name: &str) -> provider::LoadedProvider {
    let path = providers_dir().join(format!("{name}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    provider::load(&format!("providers/{name}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{name}.toml does not load: {error}"))
}

/// The story's load gate: all three shipped definitions parse and validate.
#[test]
fn every_shipped_provider_loads() {
    for name in SHIPPED {
        let loaded = load(name);
        assert_eq!(
            loaded.connector.id, *name,
            "providers/{name}.toml declares id `{}`, but the file name is what names the generated \
             `<id>.flux`",
            loaded.connector.id
        );
        assert!(
            !loaded.connector.operations.is_empty(),
            "providers/{name}.toml declares no operations, so it compiles to an empty module"
        );
    }
}

/// Selection is curated, not exhaustive. Babelforce's manager document carries 163 operations
/// (`docs/designs/provider-operation-inventory.md` §5.2); 163 generated ops would be 163 LLM tools,
/// most of them destructive admin CRUD. The upper bound is what this asserts — the exact counts the
/// inventory selected are asserted next to it.
#[test]
fn operation_selection_stays_curated() {
    let expected = [("zendesk", 7), ("freshdesk", 9), ("babelforce", 9)];
    for (name, count) in expected {
        let loaded = load(name);
        assert_eq!(
            loaded.connector.operations.len(),
            count,
            "providers/{name}.toml selects {} operations; the inventory selects {count}",
            loaded.connector.operations.len()
        );
    }
}

/// Every operation id has to be spellable as a Flux declaration name. flux-lang admits ASCII
/// alphanumerics, `_` and `-` only, so a dotted id such as `zendesk.ticket.show` is undeclarable
/// (C-8, `crates/connector-flux/src/op.rs:108`) and would emit text that does not parse.
#[test]
fn operation_ids_are_declarable_in_flux() {
    for name in SHIPPED {
        let loaded = load(name);
        for operation in &loaded.connector.operations {
            assert!(
                !operation.id.is_empty()
                    && operation
                        .id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "operation `{}` in providers/{name}.toml is not a spellable Flux declaration name",
                operation.id
            );
        }
    }
}

/// Babelforce ships SSO-issued Bearer. The legacy `X-Auth-Access-Id` / `X-Auth-Access-Token` pair is
/// deprecated and is being removed from the API (`provider-operation-inventory.md` §5.1.3); it must
/// not be modelled or emitted, and its absence is the point rather than an oversight.
///
/// The check is structural rather than a text scan of the file: the file *does* name those headers,
/// in the comment that tells a future reader not to add them back, and a grep could not tell that
/// warning apart from a declaration. What matters is that nothing reaches the IR — no second
/// credential, no non-bearer scheme, no caller-supplied auth header smuggled in as a parameter.
#[test]
fn babelforce_is_bearer_only_and_never_the_deprecated_header_pair() {
    let loaded = load("babelforce");
    let connector = &loaded.connector;

    assert_eq!(
        connector.auth.len(),
        1,
        "babelforce declares {} credentials; the excluded header pair is the only reason it would \
         ever be more than one",
        connector.auth.len()
    );
    let method = &connector.auth[0];
    assert_eq!(method.name, "babelforce.access_token");
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "babelforce's credential is not a bearer; the deprecated pair is `header`-schemed"
    );
    assert_eq!(method.env, ["BABELFORCE_ACCESS_TOKEN"]);

    // Every operation resolves to the one bearer, whether it declares auth or inherits the default.
    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; babelforce is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains("babelforce.access_token") && effective[0].len() == 1,
            "operation `{}` names a credential other than the bearer",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares caller-supplied headers; auth headers are injected by the \
             host and must never travel through the parameter surface",
            operation.id
        );
    }
}

/// Zendesk's Basic user half is `<email>/token` — an env value **plus a literal suffix**. The
/// suffix has to be declared, not baked into `ZENDESK_USER`: a pre-composed credential stores a
/// value that is not the thing it is named after and that nothing can validate
/// (`docs/designs/auth-seam.md` §7.5).
#[test]
fn zendesk_declares_the_token_suffix_rather_than_pre_composing_it() {
    let loaded = load("zendesk");
    let method = loaded
        .connector
        .auth_method("zendesk.api_token")
        .expect("zendesk declares `zendesk.api_token`");

    assert_eq!(method.scheme, AuthScheme::Basic);
    assert_eq!(method.env, ["ZENDESK_API_TOKEN"]);
    assert_eq!(method.user_env, ["ZENDESK_USER"]);
    assert_eq!(method.user_suffix.as_deref(), Some("/token"));
}

/// No credential value may appear in a provider file — environment variable *names* only
/// (AGENTS.md). A cheap structural check: nothing that looks like an assignment of a secret.
#[test]
fn no_provider_file_carries_a_credential_value() {
    for name in SHIPPED {
        let loaded = load(name);
        for method in &loaded.connector.auth {
            for key in method.env.iter().chain(&method.user_env) {
                assert!(
                    key.chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
                    "providers/{name}.toml: `{key}` is not an environment variable name"
                );
            }
        }
    }
}
