//! The generic declaration for a connection-specific HTTPS origin.

use connector_spec::{provider, Approval, Format};

#[path = "fixtures/origin_grammar.rs"]
mod origin_grammar;

fn fixture(origin: &str) -> String {
    format!(
        r#"
id = "acme"
base_url = "{{origin}}/api/v4"

[[operations]]
id = "acme-ping"
method = "GET"
path = "/ping"
risk = "low"
idempotency = "idempotent"

[[config]]
name = "origin"
label = "Acme origin"
help = "The HTTPS origin of the operator-approved Acme installation"
example = "https://acme.example"
format = "origin"
required = false
default = "https://acme.example"
approval = "operator"
binds = "endpoint.origin"
{origin}
"#
    )
}

fn load(source: &str) -> connector_spec::Connector {
    provider::load("providers/acme.toml", source)
        .unwrap_or_else(|error| panic!("fixture must load:\n{error}"))
        .connector
}

fn refusal(source: &str) -> String {
    provider::load("providers/acme.toml", source)
        .expect_err("fixture must be refused")
        .to_string()
}

#[test]
fn an_operator_pinned_origin_is_a_value_free_generic_config_declaration() {
    let connector = load(&fixture(""));
    let field = connector.config_field("origin").expect("origin field");
    assert_eq!(field.format, Format::Origin);
    assert_eq!(field.approval, Approval::Operator);
    assert_eq!(field.default.as_deref(), Some("https://acme.example"));
}

#[test]
fn an_origin_accepts_only_an_absolute_https_origin_without_url_tail() {
    for bad in [
        "http://acme.example",
        "https://user@acme.example",
        "https://acme.example/path",
        "https://acme.example?query=1",
        "https://acme.example#fragment",
        "https://acme.example/api/v4",
    ] {
        let source = fixture("\n# force the invalid value through the declared default\n").replace(
            "https://acme.example\"\napproval",
            &format!("{bad}\"\napproval"),
        );
        let error = refusal(&source);
        assert!(error.contains("origin"), "{bad:?}: {error}");
    }
}

#[test]
fn the_loader_accepts_exactly_the_shared_origin_grammar_cases() {
    for case in origin_grammar::ORIGIN_CASES {
        assert_eq!(
            Format::Origin.validate(case.value).is_ok(),
            case.accepted,
            "loader classified {:?} differently from the shared contract",
            case.value
        );
    }
}

#[test]
fn an_open_origin_without_operator_approval_is_refused() {
    let source = fixture("").replace("approval = \"operator\"\n", "");
    let error = refusal(&source);
    assert!(error.contains("operator"), "{error}");
}
