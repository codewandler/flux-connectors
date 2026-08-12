//! The generic declaration for a connection-specific HTTPS origin.

use connector_spec::{provider, Approval, Format};

use crate::origin_corpus;

fn fixture(origin: &str) -> String {
    format!(
        r#"
id = "acme"
base_url = "{{origin}}/api/v4"

[[operations]]
id = "acme-ping"
method = "GET"
direction = "read"
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

/// The compiler half of the shared corpus (C-523): a declaration is accepted exactly where the
/// author already wrote the canonical origin. Every other safe spelling is a value a *connection*
/// may supply, and `connector-pack` reads the same corpus for that half.
#[test]
fn the_loader_accepts_exactly_the_canonical_origins_of_the_shared_corpus() {
    for case in origin_corpus::ORIGIN_CASES {
        assert_eq!(
            Format::Origin.validate(case.input).is_ok(),
            case.is_canonical_declaration(),
            "loader classified {:?} differently from the shared contract",
            case.input
        );
    }
}

/// **A declaration publishes a canonical origin** (C-523). A provider-authored `default`, `example`
/// or choice reaches the manifest, the embedded catalogue and the public catalogue verbatim, so a
/// second safe spelling of one origin would ship as a second origin — and the runtime, which
/// normalizes before it compares, would then disagree with the artifact about which value is the
/// reviewed default. Runtime input may still arrive in any equivalent safe spelling.
#[test]
fn a_declared_origin_must_already_be_in_canonical_form() {
    for non_canonical in [
        "https://gitlab.com:443",
        "HTTPS://gitlab.com",
        "https://GitLab.com",
        "https://gitlab.example:08443",
        "https://[2001:0db8:0000:0000:0000:0000:0000:0001]",
    ] {
        assert!(
            Format::Origin.validate(non_canonical).is_err(),
            "{non_canonical:?} is an equivalent spelling of a canonical origin, not a canonical \
             origin, and a declaration must carry the canonical one"
        );
    }
}

#[test]
fn an_open_origin_without_operator_approval_is_refused() {
    let source = fixture("").replace("approval = \"operator\"\n", "");
    let error = refusal(&source);
    assert!(error.contains("operator"), "{error}");
}
