//! Okta (C-161) is the epic's probe for an Authorization scheme word this model has never been
//! asked for. Fifteen shipped providers are `bearer` and two are `basic`; Okta authenticates with
//! `Authorization: SSWS <apiToken>` — a **custom scheme word**, not a placement this model has met.
//!
//! This is not a per-provider contract test in the shape every sibling in this directory is: there
//! is no `providers/okta.toml` to load, because the connector cannot authenticate honestly with
//! today's [`AuthScheme`]. That is the answer the probe was chosen to produce — see the story's
//! `## Progress` for the full account — and this file is the test that pins it down rather than
//! leaving it as prose:
//!
//! 1. [`AuthScheme`] is a closed, five-member enum. Naming Okta's own scheme word (`ssws`) directly
//!    is refused at deserialization — it is not `bearer`, `basic`, `header`, `query` or `signing`.
//! 2. The one variant shaped like it *could* carry an arbitrary word — `Header` — has no field to
//!    carry one on. `docs/designs/unified-auth.md` proposed exactly that field (a `prefix` on
//!    header placement, "the single highest-value element of this whole design") and it was never
//!    implemented: `AuthScheme::Header` declares `name` and nothing else
//!    (`crates/connector-spec/src/auth.rs:78-82`), and `#[serde(deny_unknown_fields)]` on the enum
//!    (`:70-71`) refuses an unrecognized `prefix` key rather than silently dropping it.
//! 3. A bare `header` placement aimed at `Authorization` *does* load — legally, because
//!    `AuthScheme::Header` does not know or care what header name it is given — but its whole value
//!    is the resolved secret and nothing else, so the wire form would be `Authorization: <token>`
//!    with the literal word `SSWS` simply missing. Reaching `Authorization: SSWS <token>` from here
//!    would mean baking `"SSWS "` into the *credential value itself*, which is exactly the
//!    credential-value rule AGENTS.md refuses ("no credential value enters provider TOML, generated
//!    Flux, a manifest, the public catalogue, or the lockfile").
//!
//! So the refusal is at the model, not at Okta's API: the enum has no seam for an arbitrary prefix,
//! and inventing one is a change to `connector-spec` this story deliberately does not make — see the
//! story's `## Progress` for why, and for the four later stories (C-162, C-175, C-178, C-181) that
//! were waiting on this exact answer.

use connector_spec::{provider, AuthScheme};
use std::path::Path;

/// A minimal, otherwise-valid provider fixture. Only the `[[auth]]` block's `scheme` line varies
/// across the cases below — everything else is held constant so a failure is about `scheme` and
/// nothing else.
fn fixture(auth_scheme_toml: &str) -> String {
    format!(
        r#"
id = "okta"
vendor = "Okta"
base_url = "https://acme.okta.com"

[[auth]]
name = "okta.api_token"
env = ["OKTA_API_TOKEN"]
{auth_scheme_toml}

[[operations]]
id = "okta-user-list"
method = "GET"
path = "/api/v1/users"
risk = "low"
idempotency = "idempotent"
description = "List users in the Okta org, for the probe fixture only"
"#
    )
}

/// **`AuthScheme` is a closed, five-member enum and `ssws` is not one of them.**
///
/// `crates/connector-spec/src/auth.rs:70-102` declares
/// `#[serde(rename_all = "snake_case", deny_unknown_fields)] pub enum AuthScheme { Bearer, Basic,
/// Header { name }, Query { name }, Signing }` — five variants naming a *placement*, none of which
/// is a customizable prefix word. Naming Okta's own scheme word directly is refused at
/// deserialization rather than silently coerced to a preset.
#[test]
fn an_arbitrary_scheme_word_is_not_a_variant_of_auth_scheme() {
    let source = fixture("scheme = \"ssws\"");
    let error = provider::load("providers/okta.toml", &source)
        .expect_err("`ssws` is not one of AuthScheme's five variants and must be refused");
    let message = error.to_string();
    assert!(
        message.to_lowercase().contains("unknown variant"),
        "expected an unknown-variant error naming the closed enum, got: {message}"
    );
}

/// **The `header` scheme has no prefix axis to carry `SSWS ` on.**
///
/// `AuthScheme::Header` (`crates/connector-spec/src/auth.rs:78-82`) declares one field, `name` — the
/// header key — and nothing else. `docs/designs/unified-auth.md:75-77` proposed a `prefix` field on
/// header placement as "the single highest-value element of this whole design", so that `Bearer `,
/// `Basic `, `Token ` and Okta's `SSWS ` would all be one code path; it was never implemented in
/// this enum. Naming a `prefix` key is refused the same way any unknown field is, by
/// `#[serde(deny_unknown_fields)]`.
#[test]
fn the_header_scheme_carries_no_prefix_to_smuggle_ssws_onto() {
    let source = fixture("scheme = { header = { name = \"Authorization\", prefix = \"SSWS \" } }");
    let error = provider::load("providers/okta.toml", &source)
        .expect_err("`prefix` is not a field of AuthScheme::Header and must be refused");
    let message = error.to_string();
    assert!(
        message.to_lowercase().contains("unexpected keys")
            && message.to_lowercase().contains("prefix"),
        "expected an unexpected-key error naming `prefix`, got: {message}"
    );
}

/// **A bare `header` placement on `Authorization` loads, and that is exactly the trap.**
///
/// It is legal — `AuthScheme::Header` does not know or care what header name it is given, including
/// `Authorization` — but its whole value is the resolved secret and nothing else. For Okta that
/// means the wire form would be `Authorization: <token>` with the literal word `SSWS` simply
/// missing, which fails at the vendor rather than being expressed honestly. Reaching
/// `Authorization: SSWS <token>` from here would mean baking `"SSWS "` into the credential value
/// itself, which this repository must never author (AGENTS.md, "no credential value").
///
/// The round-trip back through `toml` is the sharpest way to say it: one field, `name`, and no
/// second field this connector could have put `SSWS ` in.
#[test]
fn the_header_scheme_would_load_but_cannot_honestly_spell_okta_s_prefix() {
    let source = fixture("scheme = { header = { name = \"Authorization\" } }");
    let connector = provider::load("providers/okta.toml", &source)
        .expect("a bare `header` placement on `Authorization` is legal — that is exactly the trap")
        .connector;
    let method = connector
        .auth_method("okta.api_token")
        .expect("the fixture declares it");
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: "Authorization".to_string()
        }
    );

    let round_tripped = toml::to_string(&method.scheme).expect("AuthScheme serializes");
    assert_eq!(
        round_tripped.trim(),
        "[header]\nname = \"Authorization\"",
        "if AuthScheme::Header ever gains a second field this assertion is the first thing to \
         reconsider — today it proves there is nowhere on this variant to put a scheme word"
    );
}

/// **The recorded outcome: no dishonest connector was shipped for this probe.**
///
/// `providers/okta.toml` does not exist. See the story's `## Progress` for the full account. If a
/// future story adds one under this connector's charter, it must do so only once `AuthScheme` (or
/// an equivalent seam) can carry an arbitrary prefix — shipping today would mean either the enum
/// gained a variant this test would need to be rewritten around, or the connector authenticates
/// dishonestly.
#[test]
fn no_provider_toml_was_shipped_for_this_probe() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers/okta.toml");
    assert!(
        !path.exists(),
        "providers/okta.toml exists, but C-161 concluded the connector cannot authenticate \
         honestly with today's closed AuthScheme — if this now exists, the story's refusal has \
         been overturned and this test (and the story's `## Progress`) must be updated to say how"
    );
}
