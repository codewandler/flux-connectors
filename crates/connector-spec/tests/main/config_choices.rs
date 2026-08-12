//! A configuration field with a **closed set of values** — C-225.
//!
//! `format` answers *what shape is this value*; `choices` answers *which values are legal*. They are
//! different questions, and the tests here are arranged around keeping them different: a closed set
//! is a **narrowing** of a formatted field, so every permitted value still has to satisfy the
//! field's own `format`, and dropping `format` because a set is declared would throw away the
//! validation a renderer applies to the fallback input and to the example.
//!
//! The vendor behind it is New Relic: two API hosts, US and EU, nothing pre-auth discloses which,
//! and a wrong answer returns `401` on every call — indistinguishable from a bad key. Intercom
//! recorded the same wall for its three regional hosts before New Relic existed.

use connector_spec::{provider, Connector};

use crate::shipped_provider;

/// A connector whose base URL is a whole vendor host, which is the shape a closed set is for.
fn fixture(config: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://{{host}}/v2"

[[auth]]
name = "acme.api_key"
scheme = {{ header = {{ name = "X-Api-Key" }} }}
env = ["ACME_API_KEY"]

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

/// The `host` field with its two regions declared, plus the credential the fixture needs.
const CLOSED: &str = r#"
[[config]]
name = "host"
label = "Acme API host"
help = "Which region this account lives in"
example = "api.acme.example"
format = "hostname"
choices = [
  { value = "api.acme.example", label = "United States" },
  { value = "api.eu.acme.example", label = "European Union" },
]
binds = "endpoint.host"

[[config]]
name = "api_key"
label = "Acme API key"
help = "From your Acme account settings"
format = "token"
secret = true
binds = "credential.acme.api_key"
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

/// **The failing-first test.** A field declares an enumerated set of values, each with the text a
/// renderer shows for it, and a value outside the set is refused where it is supplied — by a
/// message that names the field and lists what is permitted.
///
/// Before C-225 the declaration was not merely unused: `ConfigField` is `deny_unknown_fields`, so
/// `choices` was a load error, and the two answers a New Relic operator has to choose between
/// existed only as prose in the field's `help`.
#[test]
fn a_config_field_declares_a_closed_set_of_values_and_a_value_outside_it_is_refused() {
    let connector = load(&fixture(CLOSED));
    let host = connector.config_field("host").expect("declared");

    // The set reached the IR, in the declared order, and every value carries a human label — a set
    // of raw values is a dropdown nobody can read.
    assert_eq!(
        host.choices
            .iter()
            .map(|choice| (choice.value.as_str(), choice.label.as_str()))
            .collect::<Vec<_>>(),
        [
            ("api.acme.example", "United States"),
            ("api.eu.acme.example", "European Union"),
        ],
        "the permitted values and the text a renderer shows for each"
    );
    assert!(
        host.is_closed(),
        "a field with choices is closed; a renderer offers a choice rather than a text box"
    );

    // Membership is checked where the value is supplied, and the refusal is diagnostic rather than
    // the bare "invalid" this story exists to remove.
    for permitted in ["api.acme.example", "api.eu.acme.example"] {
        assert_eq!(host.permits(permitted), Ok(()), "{permitted}");
    }
    let refusal = host
        .permits("api.not-acme.example")
        .expect_err("a host with no relationship to the vendor is not one of the two answers");
    assert!(
        refusal.contains("host"),
        "the refusal must name the field it is about: {refusal}"
    );
    for permitted in ["api.acme.example", "United States", "api.eu.acme.example"] {
        assert!(
            refusal.contains(permitted),
            "the refusal must list what is permitted, or it reproduces the diagnosis problem: \
             {refusal}"
        );
    }

    // An open field is unchanged: no choices, and nothing to be outside of.
    let key = connector.config_field("api_key").expect("declared");
    assert!(!key.is_closed());
    assert_eq!(key.permits("anything at all"), Ok(()));
}

/// **A closed set does not replace `format`; it narrows it.** They answer different questions —
/// shape versus membership — and a value that is a member has to be a well-formed one, or the
/// fallback input a renderer builds from `format` would accept text the set never could.
#[test]
fn every_permitted_value_still_satisfies_the_fields_format() {
    let error = refuse(&fixture(&CLOSED.replace(
        r#"{ value = "api.eu.acme.example", label = "European Union" },"#,
        r#"{ value = "eu", label = "European Union" },"#,
    )));
    assert!(
        error.contains("choice") && error.contains("hostname"),
        "a choice that is not a hostname must be refused by the field's own format:\n{error}"
    );
}

/// The example is a placeholder a user copies, so on a closed field it has to be one of the answers.
/// Same class of defect as an example that fails its own `format`, which the loader already refuses.
#[test]
fn an_example_outside_the_closed_set_is_refused() {
    let error = refuse(&fixture(&CLOSED.replace(
        r#"example = "api.acme.example""#,
        r#"example = "api.not-acme.example""#,
    )));
    assert!(
        error.contains("example") && error.contains("api.acme.example"),
        "an example nobody may enter is a placeholder that misleads:\n{error}"
    );
}

/// A set of one is a **constant**: the field asks a question with one answer, which belongs in the
/// base URL rather than in front of a human.
///
/// An empty `choices = []` is deliberately *not* a separate refusal — `choices` is a `Vec` with
/// serde's `default`, so an empty list and an absent key are the same IR, and inventing a
/// distinction between them would mean an `Option<Vec<_>>` in the public surface to carry a
/// diagnostic nobody has needed. It reads as an open field, which is what it is.
#[test]
fn a_set_with_one_value_is_refused_and_an_empty_one_is_an_open_field() {
    let error = refuse(&fixture(&CLOSED.replace(
        r#"  { value = "api.eu.acme.example", label = "European Union" },
"#,
        "",
    )));
    assert!(
        error.contains("one value"),
        "a single-value set is a constant, not a choice:\n{error}"
    );

    let empty = load(&fixture(&CLOSED.replace(
        r#"choices = [
  { value = "api.acme.example", label = "United States" },
  { value = "api.eu.acme.example", label = "European Union" },
]"#,
        "choices = []",
    )));
    let host = empty.config_field("host").expect("declared");
    assert!(
        !host.is_closed(),
        "an empty list is indistinguishable from no list, and both mean an open field"
    );
}

/// Every entry is renderable and distinguishable: a blank label is an unreadable row, a repeated
/// value is a set with one member wearing two names, and a repeated label is two rows a user cannot
/// tell apart.
#[test]
fn a_choice_must_be_renderable_and_distinct() {
    let blank = refuse(&fixture(
        &CLOSED.replace(r#"label = "United States""#, r#"label = """#),
    ));
    assert!(
        blank.contains("empty `label`"),
        "a choice with no label is a dropdown row with nothing in it:\n{blank}"
    );

    let repeated = refuse(&fixture(&CLOSED.replace(
        r#"{ value = "api.eu.acme.example", label = "European Union" },"#,
        r#"{ value = "api.acme.example", label = "European Union" },"#,
    )));
    assert!(
        repeated.contains("more than once"),
        "one value under two labels is a set that cannot be selected from:\n{repeated}"
    );

    let ambiguous = refuse(&fixture(
        &CLOSED.replace(r#"label = "European Union""#, r#"label = "United States""#),
    ));
    assert!(
        ambiguous.contains("more than once"),
        "two rows a user cannot tell apart:\n{ambiguous}"
    );
}

/// **A secret declares no closed set.** The values would be credentials, written into a committed
/// file — the same defect `secret` + `example` is refused for, and a stronger form of it, because a
/// set is exhaustive.
#[test]
fn a_secret_field_cannot_declare_a_closed_set() {
    let error = refuse(&fixture(&CLOSED.replace(
        r#"secret = true
binds = "credential.acme.api_key""#,
        r#"secret = true
choices = [
  { value = "NRAK-AAAA", label = "First" },
  { value = "NRAK-BBBB", label = "Second" },
]
binds = "credential.acme.api_key""#,
    )));
    assert!(
        error.contains("secret") && error.contains("choices"),
        "an enumeration of secret values is a list of credentials in a committed file:\n{error}"
    );
}

/// **A closed set composes with a pinned request position rather than sitting beside it.** A pinned
/// value is substituted into a URL a host composes, so every permitted value has to survive that
/// substitution — exactly the rule the field's `example` already answers to.
#[test]
fn a_pinned_field_checks_every_choice_against_its_request_position() {
    let pinned = r#"
[[config]]
name = "host"
label = "Acme API host"
help = "Which region this account lives in"
format = "hostname"
choices = [
  { value = "api.acme.example", label = "United States" },
  { value = "api.eu.acme.example", label = "European Union" },
]
binds = "endpoint.host"

[[config]]
name = "region"
label = "Acme region"
help = "The region every request is scoped to"
choices = [
  { value = "us", label = "United States" },
  { value = "eu/../admin", label = "European Union" },
]
binds = "path.region"

[[config]]
name = "api_key"
label = "Acme API key"
help = "From your Acme account settings"
format = "token"
secret = true
binds = "credential.acme.api_key"
"#;
    let source = fixture(pinned).replace(r#"path = "/ping""#, r#"path = "/{region}/ping""#);
    let error = refuse(&source);
    assert!(
        error.contains("choice") && error.contains("path segment"),
        "a permitted value that escapes its path segment is a permitted way to address another \
         resource:\n{error}"
    );
}

/// The shipped catalogue: the two connectors the story names declare their regions, and every
/// closed set in `providers/` is one the loader has checked.
#[test]
fn the_shipped_connectors_that_have_regions_declare_them() {
    for (provider, field, values) in [
        (
            "newrelic",
            "host",
            &["api.newrelic.com", "api.eu.newrelic.com"][..],
        ),
        (
            "intercom",
            "host",
            &[
                "api.intercom.io",
                "api.eu.intercom.io",
                "api.au.intercom.io",
            ][..],
        ),
    ] {
        let connector = shipped_provider::load(provider).connector;
        let declared = connector
            .config_field(field)
            .unwrap_or_else(|| panic!("{provider} declares `{field}`"));
        assert_eq!(
            declared
                .choices
                .iter()
                .map(|choice| choice.value.as_str())
                .collect::<Vec<_>>(),
            values,
            "{provider}'s regional hosts are selectable, in the vendor's own order"
        );
        assert!(
            declared
                .choices
                .iter()
                .all(|choice| !choice.label.is_empty()),
            "{provider}'s regions each read as a place, not as a hostname"
        );
    }
}
