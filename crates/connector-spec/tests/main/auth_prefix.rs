//! C-184: a credential may sit inside a header value it does not wholly occupy.
//!
//! Before this, [`AuthScheme::Header`] carried one field — the header *name* — so the only value it
//! could describe was "the resolved secret, alone". Three vendors need a literal scheme word in
//! front of it, and [C-161](../../../docs/stories/C-161-provider-okta.md) measured all three as the
//! **same** shape rather than three:
//!
//! | vendor | header | prefix |
//! |---|---|---|
//! | Okta | `Authorization: SSWS <token>` | `SSWS ` |
//! | Statuspage | `Authorization: OAuth <key>` | `OAuth ` |
//! | PagerDuty | `Authorization: Token token=<key>` | `Token token=` |
//!
//! PagerDuty is the one that looks like a fourth axis and is not. Its credential is not a *field
//! inside* the value in any sense the model has to know about: the value is a fixed literal followed
//! directly by the raw key, so `Token token=` is a prefix that happens to contain `=`. Nothing in
//! the shipped catalogue, and nothing in these three, needs text *after* the credential.
//!
//! **So the axis is `prefix` alone — no `suffix`, and no value template.** The reasoning is recorded
//! in `docs/designs/unified-auth.md` §"The prefix axis, as built"; the short form is that a template
//! can spell requests this repository must not be able to author (a credential substituted twice, or
//! zero times — which sends an unauthenticated request that looks authenticated), while a prefix
//! makes each of those unspellable rather than merely refused.
//!
//! The refusals below are the other half of the contract. A prefix is *connector data* — a scheme
//! word, public API syntax — and the credential value is a runtime secret. The tests assert that the
//! seam cannot be used to carry the second.

use connector_spec::{provider, AuthScheme};

/// A minimal, otherwise-valid provider fixture. Only the `[[auth]]` block's `scheme` line varies, so
/// a failure is about `scheme` and nothing else. Deliberately the same shape as C-161's probe
/// fixture — this file is the answer to the question that one pinned.
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
direction = "read"
path = "/api/v1/users"
risk = "low"
idempotency = "idempotent"
description = "List users in the Okta org, for the prefix fixture only"
"#
    )
}

fn scheme_of(auth_scheme_toml: &str) -> AuthScheme {
    provider::load("providers/okta.toml", &fixture(auth_scheme_toml))
        .expect("the fixture is otherwise valid")
        .connector
        .auth_method("okta.api_token")
        .expect("the fixture declares it")
        .scheme
        .clone()
}

fn refusal(auth_scheme_toml: &str) -> String {
    provider::load("providers/okta.toml", &fixture(auth_scheme_toml))
        .expect_err("this spelling must be refused")
        .to_string()
}

/// **Okta's `SSWS `, the shape the whole story is named for.** A scheme word this model had never
/// been asked for, carried as data rather than as a sixth enum variant.
#[test]
fn a_header_placement_carries_an_arbitrary_scheme_word() {
    assert_eq!(
        scheme_of(r#"scheme = { header = { name = "Authorization", prefix = "SSWS " } }"#),
        AuthScheme::Header {
            name: "Authorization".to_string(),
            prefix: "SSWS ".to_string(),
        }
    );
}

/// **PagerDuty's `Token token=`, which is a prefix and not a fourth axis.** The `=` is what made this
/// look like structured syntax needing its own model; it is a literal like any other, and the
/// credential still ends the value.
#[test]
fn a_prefix_may_carry_punctuation_and_still_be_a_prefix() {
    assert_eq!(
        scheme_of(r#"scheme = { header = { name = "Authorization", prefix = "Token token=" } }"#),
        AuthScheme::Header {
            name: "Authorization".to_string(),
            prefix: "Token token=".to_string(),
        }
    );
}

/// **Statuspage's `OAuth `** — a scheme word that is not OAuth2's bearer usage, and the third vendor
/// this unblocks.
#[test]
fn a_scheme_word_that_is_not_oauth2_is_still_just_a_prefix() {
    assert_eq!(
        scheme_of(r#"scheme = { header = { name = "Authorization", prefix = "OAuth " } }"#),
        AuthScheme::Header {
            name: "Authorization".to_string(),
            prefix: "OAuth ".to_string(),
        }
    );
}

/// **The byte-identity guard, stated at the model rather than at the artifact.**
///
/// Every shipped `header` credential omits `prefix`, and no already-shipped provider's emitted module
/// may move because this field exists. Omitted and explicitly-empty are the same value, and an empty
/// prefix does not serialize at all — so a connector authored before this change hashes into
/// `connectors.lock` exactly as it did.
///
/// The whole-tree version of this claim is `connector-cli`'s `the_committed_tree_is_a_fixed_point_of
/// _a_build`; this is the one that says *why* it holds, at the one line that decides it.
#[test]
fn an_omitted_prefix_is_empty_and_does_not_serialize() {
    let omitted = scheme_of(r#"scheme = { header = { name = "X-Figma-Token" } }"#);
    let explicit = scheme_of(r#"scheme = { header = { name = "X-Figma-Token", prefix = "" } }"#);
    assert_eq!(omitted, explicit);
    assert_eq!(
        omitted,
        AuthScheme::Header {
            name: "X-Figma-Token".to_string(),
            prefix: String::new(),
        }
    );

    let round_tripped = toml::to_string(&omitted).expect("AuthScheme serializes");
    assert_eq!(
        round_tripped.trim(),
        "[header]\nname = \"X-Figma-Token\"",
        "an empty prefix must not reach the encoding, or every shipped provider's lockfile hash moves"
    );
}

/// **A non-empty prefix does round-trip** — the field is real, not write-only.
#[test]
fn a_declared_prefix_round_trips_through_the_encoding() {
    let scheme = scheme_of(r#"scheme = { header = { name = "Authorization", prefix = "SSWS " } }"#);
    let encoded = toml::to_string(&scheme).expect("AuthScheme serializes");
    assert_eq!(
        encoded.trim(),
        "[header]\nname = \"Authorization\"\nprefix = \"SSWS \"",
    );
    let decoded: AuthScheme = toml::from_str(&encoded).expect("and deserializes");
    assert_eq!(decoded, scheme);
}

/// **The constraint that makes this story subtle: the credential value is never authored.**
///
/// A prefix is the seam closest to a credential value that this repository has, so the attempt it
/// must refuse is an author reaching the secret through it. Nothing interpolates a prefix — it is
/// emitted as a literal — so a resolution marker is either a broken request or an attempt to smuggle
/// a value, and both are refused by name.
/// Each spelling asserts the **specific clause** that must fire, not merely that something refused.
/// C-184's review found every refusal test here asserted only `contains("prefix")`, which a guard
/// refusing for the wrong reason would satisfy — so `{{token}}` and `$secret` are chosen because no
/// other clause catches them.
#[test]
fn a_prefix_may_not_spell_a_resolution_marker() {
    for (spelling, marker) in [
        (
            r#"scheme = { header = { name = "Authorization", prefix = "SSWS ${OKTA_API_TOKEN} " } }"#,
            "${",
        ),
        (
            r#"scheme = { header = { name = "Authorization", prefix = "SSWS {{token}} " } }"#,
            "{{",
        ),
        (
            r#"scheme = { header = { name = "Authorization", prefix = "SSWS $secret " } }"#,
            "$secret",
        ),
    ] {
        let message = refusal(spelling);
        assert!(
            message.contains("prefix") && message.contains("okta.api_token"),
            "expected a refusal naming the prefix and its credential, got: {message}"
        );
        assert!(
            message.contains(marker),
            "the refusal must name the marker {marker:?} it fired on, got: {message}"
        );
    }
}

/// A prefix naming the credential, or the environment variable that resolves it, is the same attempt
/// spelled without a marker: nothing resolves either, so the only way to make it *work* is to paste
/// the value in.
///
/// **Case-folded, and across every declared credential** — both corrections from C-184's review. The
/// sibling guard `credential_shaped_value` has always iterated `connector.auth`, and a prefix naming
/// another credential's variable is the same mistake spelled sideways.
#[test]
fn a_prefix_may_not_name_the_credential_or_its_env_var() {
    for spelling in [
        r#"scheme = { header = { name = "Authorization", prefix = "SSWS okta.api_token " } }"#,
        r#"scheme = { header = { name = "Authorization", prefix = "SSWS OKTA_API_TOKEN " } }"#,
        // Folded: the same two, spelled in the other case.
        r#"scheme = { header = { name = "Authorization", prefix = "SSWS OKTA.API_TOKEN " } }"#,
        r#"scheme = { header = { name = "Authorization", prefix = "SSWS okta_api_token " } }"#,
    ] {
        let message = refusal(spelling);
        assert!(
            message.contains("prefix") && message.contains("okta.api_token"),
            "expected a refusal naming the prefix and the credential, got: {message}"
        );
    }
}

/// **The separator rule, and the pasted credential it exists to refuse.**
///
/// C-184's review found that a prefix of `Bearer sk-live-…` loaded, and would reach
/// `providers/*.toml`, the generated Rust catalogue and the published catalogue verbatim. It could
/// never produce a *working* request — the host still appends the real credential — but it is one
/// keystroke from where someone types a scheme word next to a token.
///
/// The refusal is structural rather than a blocklist: the host appends the credential with nothing
/// in between, so a prefix ending in an alphanumeric would send the two glued together. That is
/// never what a vendor wants, whatever the scheme word is.
#[test]
fn a_prefix_may_not_end_in_an_alphanumeric_character() {
    for spelling in [
        // A pasted credential, in three vendors' spellings. Note a `CREDENTIAL_VALUE_PREFIXES`
        // check would catch only the first — it is matched with `starts_with` over `"bearer "`,
        // `"token "` and friends, and knows nothing of `SSWS` or `OAuth`.
        r#"scheme = { header = { name = "Authorization", prefix = "Bearer sk-live-51H8ZaBcDeFgHiJkLmNoPqRs" } }"#,
        r#"scheme = { header = { name = "Authorization", prefix = "SSWS 00aB3xYzQq7LmN0pR8sT1uV2wX3yZ4" } }"#,
        r#"scheme = { header = { name = "Authorization", prefix = "OAuth 7f3a9c2e5b8d1064" } }"#,
    ] {
        let message = refusal(spelling);
        assert!(
            message.contains("prefix") && message.contains("alphanumeric"),
            "expected the separator refusal, got: {message}"
        );
    }
}

/// **The missing trailing space, which was previously uncatchable — and is the same rule.**
///
/// `crates/connector-flux/tests/okta_connector.rs` documented `prefix = "SSWS"` as a mistake
/// "nothing can catch for you". The separator rule catches it: `SSWS` + `<token>` is `SSWS<token>`,
/// which Okta rejects, and the reason is the same one that refuses a pasted credential.
#[test]
fn a_prefix_missing_its_trailing_separator_is_refused() {
    for spelling in [
        r#"scheme = { header = { name = "Authorization", prefix = "SSWS" } }"#,
        r#"scheme = { header = { name = "Authorization", prefix = "OAuth" } }"#,
    ] {
        let message = refusal(spelling);
        assert!(
            message.contains("prefix") && message.contains("alphanumeric"),
            "expected the separator refusal, got: {message}"
        );
    }
}

/// **The whitespace-corruption class, found by C-184's own review** — and the reason it needed a
/// rule of its own rather than being covered by the two above.
///
/// The separator rule refuses a prefix with *no* trailing separator; the whitespace-only rule
/// refuses one that is *nothing but* separator. Between them sat `"SSWS  "` and `" SSWS "`, which
/// loaded, sent a header the vendor answers `401` to, and were caught by **nothing** — a connector's
/// own suite asserts its prefix against a constant in the same file, so an author editing both
/// together leaves every test green. That is why this is checked at the loader and not left to the
/// per-connector tests.
#[test]
fn a_prefix_may_not_carry_leading_or_doubled_whitespace() {
    for (spelling, clause) in [
        (
            r#"scheme = { header = { name = "Authorization", prefix = " SSWS " } }"#,
            "beginning with whitespace",
        ),
        (
            r#"scheme = { header = { name = "Authorization", prefix = "SSWS  " } }"#,
            "two consecutive whitespace",
        ),
        (
            r#"scheme = { header = { name = "Authorization", prefix = "Token  token=" } }"#,
            "two consecutive whitespace",
        ),
        (
            "scheme = { header = { name = \"Authorization\", prefix = \"SSWS \\t\" } }",
            "two consecutive whitespace",
        ),
    ] {
        let message = refusal(spelling);
        assert!(
            message.contains("prefix") && message.contains(clause),
            "expected the refusal to name {clause:?}, got: {message}"
        );
    }
}

/// The rule above is about **whitespace**, not about repeated punctuation, and this pins that line.
///
/// `"Token token=="` is wrong for PagerDuty, but nothing in this model can say `==` is wrong in
/// general — a vendor is entitled to its own syntax, and a checker that guesses at it starts
/// refusing correct connectors. Whitespace is different: a doubled space is an HTTP hygiene fault
/// for every vendor there is.
#[test]
fn repeated_punctuation_is_the_vendors_business_and_still_loads() {
    assert_eq!(
        scheme_of(r#"scheme = { header = { name = "Authorization", prefix = "Token token==" } }"#),
        AuthScheme::Header {
            name: "Authorization".to_string(),
            prefix: "Token token==".to_string(),
        }
    );
}

/// A prefix of only whitespace carries no scheme word and puts leading space in a header value,
/// which `field-content` disallows at the edges. Omitting `prefix` is how a raw-value header is
/// spelled.
#[test]
fn a_whitespace_only_prefix_is_refused() {
    for spelling in [
        "scheme = { header = { name = \"Authorization\", prefix = \" \" } }",
        "scheme = { header = { name = \"Authorization\", prefix = \"\\t\" } }",
    ] {
        let message = refusal(spelling);
        assert!(
            message.contains("prefix") && message.contains("whitespace"),
            "expected the whitespace refusal, got: {message}"
        );
    }
}

/// **Header injection.** A prefix reaches a header value verbatim, so a newline in one would end the
/// header and begin another — a request neither the module nor the connector author described. The
/// header *name* has been grammar-checked since C-3; this is the value half of the same rule.
#[test]
fn a_prefix_may_not_break_out_of_the_header_value() {
    for spelling in [
        r#"scheme = { header = { name = "Authorization", prefix = "SSWS \r\nX-Evil: 1 " } }"#,
        r#"scheme = { header = { name = "Authorization", prefix = "SSWS \n" } }"#,
        "scheme = { header = { name = \"Authorization\", prefix = \"SSWS \\u0000\" } }",
    ] {
        let message = refusal(spelling);
        assert!(
            message.contains("prefix"),
            "expected a refusal naming the prefix, got: {message}"
        );
    }
}

/// **A prefix is not a place to put a whole credential, and a suffix does not exist.**
///
/// The decision this story recorded is that the credential always ends the value. `suffix` is
/// therefore not a field, and naming one is refused the way any unknown key is — which is also the
/// assertion that catches a future author reaching for the axis that was deliberately not built.
#[test]
fn there_is_no_suffix_axis() {
    let message = refusal(
        r#"scheme = { header = { name = "Authorization", prefix = "Token token=\"", suffix = "\"" } }"#,
    );
    assert!(
        message.to_lowercase().contains("unexpected keys") && message.contains("suffix"),
        "expected an unexpected-key error naming `suffix`, got: {message}"
    );
}

/// The two preset schemes are prefixes that already have a name, so they have nowhere to carry a
/// second one — `bearer` with a prefix would compose `Bearer SSWS <token>`. They are unit variants,
/// which makes that unspellable rather than merely refused; this test says so out loud, because "the
/// type makes it impossible" is a claim worth pinning.
#[test]
fn the_preset_schemes_carry_no_prefix_of_their_own() {
    let message = refusal(r#"scheme = { bearer = { prefix = "SSWS " } }"#);
    assert!(!message.is_empty(), "a bearer with a prefix must not load");
    assert_eq!(scheme_of(r#"scheme = "bearer""#), AuthScheme::Bearer);
}
