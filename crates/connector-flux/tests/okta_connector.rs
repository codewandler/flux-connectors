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
//! 2. The one variant shaped like it *could* carry an arbitrary word — `Header` — had no field to
//!    carry one on. `docs/designs/unified-auth.md` proposed exactly that field (a `prefix` on
//!    header placement, "the single highest-value element of this whole design") and it was never
//!    implemented.
//! 3. A bare `header` placement aimed at `Authorization` *does* load — legally, because
//!    `AuthScheme::Header` does not know or care what header name it is given — but its whole value
//!    is the resolved secret and nothing else, so the wire form would be `Authorization: <token>`
//!    with the literal word `SSWS` simply missing. Reaching `Authorization: SSWS <token>` from here
//!    would mean baking `"SSWS "` into the *credential value itself*, which is exactly the
//!    credential-value rule AGENTS.md refuses ("no credential value enters provider TOML, generated
//!    Flux, a manifest, the public catalogue, or the lockfile").
//!
//! # C-184 answered finding 2, and this file was updated deliberately
//!
//! [C-184](../../../docs/stories/C-184-auth-scheme-prefix-axis.md) built the prefix axis this probe
//! showed was missing, so the second test below now asserts the **opposite** of what it asserted when
//! it was written — that was the plan, and C-161's own doc comment said the day would come. The
//! finding it recorded is unchanged and still worth reading: it is *why* the axis exists, and it is
//! what stopped a dishonest Okta connector from shipping in the meantime.
//!
//! Findings 1 and 3 still hold exactly as written, and are the reason the axis is a `prefix` on
//! `Header` rather than a sixth variant: `ssws` is still not a scheme, because a vendor's scheme word
//! is data.
//!
//! **What is still true: no `providers/okta.toml` ships.** C-184 unblocked the connector; it did not
//! write it. C-161 remains the story that does.

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

/// **The `header` scheme now carries `SSWS `, which is what C-161 asked for and C-184 built.**
///
/// This test is the inverted one. When C-161 wrote it, it asserted that `prefix` was refused as an
/// unknown key — the measurement that produced C-184. `AuthScheme::Header` now declares
/// `{ name, prefix }`, `docs/designs/unified-auth.md` §"The prefix axis, as built" records why the
/// axis is a prefix and not a template, and Okta's scheme word is expressible without any credential
/// value being authored.
///
/// The prefix is `"SSWS "` — **with the trailing space**, because the space is part of the literal
/// and not a separator the host inserts. A prefix of `"SSWS"` would compose `Authorization:
/// SSWS<token>`, which Okta rejects.
///
/// This comment used to end "nothing can catch that for the author". That is **no longer true**: the
/// loader now refuses a prefix ending in an alphanumeric, because the host appends the credential
/// directly and the two would travel glued together. See `validate_auth_prefix` and
/// `crates/connector-spec/tests/auth_prefix.rs::a_prefix_missing_its_trailing_separator_is_refused`.
#[test]
fn the_header_scheme_carries_the_ssws_prefix_it_once_could_not() {
    let source = fixture("scheme = { header = { name = \"Authorization\", prefix = \"SSWS \" } }");
    let connector = provider::load("providers/okta.toml", &source)
        .expect("C-184 built the prefix axis; Okta's scheme word is now expressible")
        .connector;
    let method = connector
        .auth_method("okta.api_token")
        .expect("the fixture declares it");
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: "Authorization".to_string(),
            prefix: "SSWS ".to_string(),
        }
    );

    // The credential-value rule, on the seam that sits closest to breaking it: the loaded connector
    // holds the vendor's public scheme word and the *name* of an environment variable, and nothing
    // that resolves to a secret.
    let encoded = toml::to_string(&method.scheme).expect("AuthScheme serializes");
    assert_eq!(
        encoded.trim(),
        "[header]\nname = \"Authorization\"\nprefix = \"SSWS \"",
    );
    assert_eq!(method.env, vec!["OKTA_API_TOKEN".to_string()]);
}

/// **A bare `header` placement on `Authorization` still loads, and it is still the trap.**
///
/// C-184 did not close this one, and could not: omitting `prefix` is *correct* for LaunchDarkly and
/// ClickUp, whose whole Authorization value is the token, so a connector that omits it is
/// indistinguishable at the model from one that forgot. Applied to Okta the wire form is
/// `Authorization: <token>` with the literal word `SSWS` simply missing — a request the vendor
/// rejects, and the reason C-161 called this the trap rather than the gap.
///
/// What changed is the *escape*: reaching `Authorization: SSWS <token>` no longer requires baking
/// `"SSWS "` into the credential value, which this repository must never author (AGENTS.md, "no
/// credential value"). It requires declaring the prefix, as the test above does.
#[test]
fn a_bare_header_placement_still_omits_the_scheme_word_it_does_not_declare() {
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
            name: "Authorization".to_string(),
            prefix: String::new(),
        }
    );

    let round_tripped = toml::to_string(&method.scheme).expect("AuthScheme serializes");
    assert_eq!(
        round_tripped.trim(),
        "[header]\nname = \"Authorization\"",
        "an empty prefix must not reach the encoding — 23 providers' committed artifacts depend on \
         a connector authored before C-184 serializing exactly as it did"
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
