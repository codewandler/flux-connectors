//! The DocuSign connector, and the archetype it exists to prove out: **a two-level configured
//! host, expressed as two independently-bound `{variable}`s in one `base_url` template.**
//!
//! [C-163](../../../docs/stories/C-163-provider-salesforce.md) (Salesforce) established that
//! `ConfigField::binds` reaching `endpoint.<variable>` — a `{variable}` in the connector's
//! `base_url` — is already the shape the configuration contract supports, and that C-169/C-170
//! (filed as [C-187](../../../docs/stories/C-187-config-cannot-pin-a-request-component.md))
//! measured the boundary of that shape: `binds` reaches `base_url` and **nothing else**. It cannot
//! pin a path segment or a query parameter.
//!
//! DocuSign's own operations sit under a per-account host *and* an account id
//! (`https://{host}/restapi/v2.1/accounts/{account_id}/...`), which reads at first like exactly the
//! path-segment case C-187 says is out of reach. It is not, because `account_id` never has to be a
//! path segment here: `crates/connector-spec/src/config.rs::template_variables` extracts **every**
//! `{...}` placeholder in a `base_url`, not just one
//! (`crates/connector-spec/src/config.rs:348-362`), and
//! `crates/connector-spec/src/provider.rs::validate_binding`/`validate_every_template_variable_is_asked_for`
//! (`provider.rs:557-573`, `provider.rs:638-660`) check and require a bound `[[config]]` field **per
//! variable**, with no cap on how many a `base_url` carries. So both the host and the account id
//! fold into `base_url` as two named placeholders, each with its own field — the same mechanism
//! Salesforce uses once, used here twice. Nothing about this connector needed a path-segment
//! binding, and nothing here needed C-187 to land.
//!
//! [`the_base_url_carries_two_independently_bound_variables`] is the load-bearing proof. The
//! remaining tests cover this connector's other hazards: no personal data (a signer's name or
//! email) is ever given as a literal example anywhere the connector declares itself; the one query
//! parameter this connector offers is constrained tightly enough that the missing percent-encoder
//! (C-30) cannot corrupt it; and `templateRoles` — an array of recipient objects — is a single
//! caller-supplied value at the body root, not a `wire` path an array would have to be decomposed
//! across, which is the narrower shape C-179 confirmed `body_tree` already supports (C-185).

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::config::template_variables;
use connector_spec::{AuthScheme, Binding, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

const PROVIDER: &str = "docusign";

const CREDENTIAL: &str = "docusign.access_token";
const TOKEN_ENV: &str = "DOCUSIGN_ACCESS_TOKEN";

/// The curated operations, in the order `providers/docusign.toml` declares them.
const OPERATIONS: &[&str] = &[
    "docusign-verify",
    "docusign-envelope-list",
    "docusign-envelope-get",
    "docusign-envelope-recipients-get",
    "docusign-envelope-create-from-template",
    "docusign-envelope-void",
];

/// Words that must never appear literally in this connector's own declaration or in any module it
/// emits: a fabricated signer name or email address. The story's directive is "author none", so the
/// check is on the source file and the TOML text, not merely on emitted Flux.
const FORBIDDEN_PII_EXAMPLES: &[&str] = &[
    "@example.com",
    "jane.doe",
    "john.doe",
    "jane@",
    "john@",
    "signer@",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn provider_toml_text() -> String {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    let source = provider_toml_text();
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

fn emitted(connector: &Connector) -> Vec<(String, String)> {
    connector
        .operations
        .iter()
        .map(|operation| {
            let text = emit_operation(connector, operation)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
            (operation.id.clone(), text)
        })
        .collect()
}

/// The connector exists, loads, and is the one the story specifies: a bearer access token, with the
/// curated operation set.
#[test]
fn the_docusign_connector_loads_and_authenticates_with_a_bearer_access_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "DocuSign");

    assert_eq!(
        connector.auth.len(),
        1,
        "docusign authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("docusign declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "an OAuth2 access token is sent as `Authorization: Bearer <token>`"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.oauth2.is_none(),
        "the token arrives already minted through the environment; this connector runs no OAuth2 \
         grant itself (docs/designs/auth-seam.md)"
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
            "operation `{}` has {} auth alternatives; docusign is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the access token",
            operation.id
        );
    }
}

/// **The load-bearing assertion.** `base_url` carries exactly two template variables — the account
/// host and the account id — and each is bound by its own `[[config]]` field, exactly the way
/// Salesforce's single `{instance}` is bound, just twice. Neither variable is a path segment, so
/// C-187's boundary (`binds` reaches `base_url` and nothing else) is never in tension with this
/// connector.
#[test]
fn the_base_url_carries_two_independently_bound_variables() {
    let connector = load();

    let variables = template_variables(&connector.base_url);
    assert_eq!(
        variables,
        ["account_host", "account_id"],
        "docusign's base URL must carry exactly the two template variables the two-level host \
         needs"
    );

    for variable in ["account_host", "account_id"] {
        let field = connector
            .config
            .iter()
            .find(|field| field.binds == format!("endpoint.{variable}"))
            .unwrap_or_else(|| {
                panic!(
                    "no `[[config]]` field binds `endpoint.{variable}`; the base URL template is \
                     unbound"
                )
            });
        assert_eq!(field.binding(), Some(Binding::Endpoint { variable }));
        assert!(
            !field.label.is_empty(),
            "config field `{variable}` must be renderable"
        );
        assert!(
            !field.help.is_empty(),
            "config field `{variable}` must be renderable"
        );
        assert!(
            !field.secret,
            "an `endpoint` binding is never a secret — `Binding::is_secret` says so, and `{variable}` \
             must agree"
        );
    }

    let credential_field = connector
        .config
        .iter()
        .find(|field| field.binds == format!("credential.{CREDENTIAL}"))
        .unwrap_or_else(|| panic!("no `[[config]]` field binds `credential.{CREDENTIAL}`"));
    assert!(
        credential_field.secret,
        "the access token binds a credential and must be declared secret"
    );
    assert!(
        credential_field.example.is_none(),
        "no realistic example on a secret field"
    );

    assert_eq!(
        connector.config.len(),
        3,
        "docusign asks for exactly three things: the account host, the account id, and the access \
         token"
    );

    // No operation's own `path` re-declares either variable — they live in `base_url` alone, never
    // duplicated onto a `[[operations.params.path]]` entry as a second, contradictable source.
    for operation in &connector.operations {
        assert!(
            !operation.path.contains("{account_host}") && !operation.path.contains("{account_id}"),
            "operation `{}` re-declares a base-URL variable in its own path: {:?}",
            operation.id,
            operation.path
        );
    }

    // The templates reach the emitted module verbatim, unresolved.
    for (id, text) in emitted(&connector) {
        assert!(
            text.contains(r#"base = "https://{account_host}/restapi/v2.1/accounts/{account_id}""#),
            "`{id}` does not carry the unbound two-level template:\n{text}"
        );
    }
}

/// Only one operation reaches a query string, and its one query parameter is constrained tightly
/// enough that the missing percent-encoder (C-30, `zendesk-ticket-search`'s own defect) cannot
/// corrupt the request — the pattern admits no `&`, `#`, space, or `=`.
#[test]
fn the_one_query_parameter_this_connector_offers_cannot_be_corrupted_by_the_missing_encoder() {
    let connector = load();

    let with_query: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| !operation.params.query.is_empty())
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        with_query,
        ["docusign-envelope-list"],
        "exactly one operation should declare a query parameter"
    );

    let list = connector
        .operations
        .iter()
        .find(|operation| operation.id == "docusign-envelope-list")
        .expect("docusign-envelope-list is declared");
    assert_eq!(list.params.query.len(), 1);
    let from_date = &list.params.query[0];
    assert_eq!(from_date.name, "from_date");
    assert!(
        from_date.required,
        "DocuSign 400s this resource with no date filter at all"
    );

    let pattern = from_date
        .schema
        .get("pattern")
        .and_then(|value| value.as_str())
        .unwrap_or_else(|| {
            panic!("`from_date` declares no `pattern`, so nothing bounds its characters")
        });
    for corrosive in ['&', '#', '=', ' ', '+', '%'] {
        assert!(
            !regex_like_admits(pattern, corrosive),
            "`from_date`'s pattern {pattern:?} must not admit {corrosive:?}, which would corrupt \
             the query string with no percent-encoder to protect it (C-30)"
        );
    }
}

/// A crude but sufficient check: the anchored date pattern this connector declares is a fixed
/// character class (`\d`, `-`) between `^` and `$`, so any character not in `0123456789-` is
/// inadmissible. Not a general regex engine — this repository takes no such dependency — just
/// enough to keep the test honest about what it is asserting.
fn regex_like_admits(pattern: &str, ch: char) -> bool {
    pattern.starts_with('^')
        && pattern.ends_with('$')
        && !"0123456789-".contains(ch)
        && pattern.contains(ch)
}

/// `templateRoles` is a single caller-supplied value at the body root — an array of recipient
/// objects — not a `wire` path that would have to be decomposed across nested segments. That
/// narrower shape is exactly what C-179 confirmed `body_tree` already supports (Front's `tag_ids`),
/// and it is the boundary C-185 draws: an envelope whose recipients are only reachable by
/// decomposing an array element-by-element across separate named leaves is still refused, but a
/// whole array handed over as one parameter is not that case.
#[test]
fn template_roles_is_one_body_root_value_not_a_decomposed_wire_path() {
    let connector = load();

    let create = connector
        .operations
        .iter()
        .find(|operation| operation.id == "docusign-envelope-create-from-template")
        .expect("docusign-envelope-create-from-template is declared");

    let template_roles = create
        .params
        .body
        .iter()
        .find(|param| param.name == "templateRoles")
        .expect("`templateRoles` is declared on the create-from-template operation");

    assert!(
        template_roles.wire.is_none(),
        "`templateRoles` must sit at the body root, not under a dotted `wire` path"
    );
    assert_eq!(
        template_roles.schema.get("type").and_then(|v| v.as_str()),
        Some("array"),
        "`templateRoles` is DocuSign's own array-of-roles shape"
    );
    assert_eq!(
        template_roles
            .schema
            .get("items")
            .and_then(|items| items.get("type"))
            .and_then(|v| v.as_str()),
        Some("object"),
        "each role is an object — email, name, roleName"
    );

    // It emits cleanly as one Flux argument, the same way Front's flat `tag_ids: List<String>`
    // does, proving the mechanism rather than trusting the schema alone.
    let text = emit_operation(&connector, create)
        .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", create.id));
    assert!(
        text.contains("templateRoles"),
        "the emitted op must declare a `templateRoles` argument:\n{text}"
    );
}

/// This connector's whole point is that voiding is destructive and creating dispatches a real
/// signature request — the story's own framing, restated here as a machine check on `risk` rather
/// than trusted from prose alone.
#[test]
fn void_is_destructive_and_create_is_high_risk_and_non_idempotent() {
    let connector = load();

    let void = connector
        .operations
        .iter()
        .find(|operation| operation.id == "docusign-envelope-void")
        .expect("docusign-envelope-void is declared");
    assert_eq!(void.method, HttpMethod::Put);
    assert_eq!(
        void.risk,
        Risk::Destructive,
        "voiding an envelope permanently cancels it for every recipient"
    );

    let create = connector
        .operations
        .iter()
        .find(|operation| operation.id == "docusign-envelope-create-from-template")
        .expect("docusign-envelope-create-from-template is declared");
    assert_eq!(create.method, HttpMethod::Post);
    assert_eq!(
        create.risk,
        Risk::High,
        "sending an envelope has real-world effect on a real person"
    );
    assert_eq!(
        create.idempotency,
        Idempotency::NonIdempotent,
        "calling create-from-template twice creates two envelopes, not one"
    );

    // Every read is low risk and idempotent; every write is at least medium risk and never claims
    // low risk. No operation here is a PATCH, so C-186's POST/PATCH idempotent-refusal is never in
    // tension with an honest claim this connector wants to make (see the file's own header comment
    // and `## Progress` in the story for why).
    for operation in &connector.operations {
        match operation.method {
            HttpMethod::Get => {
                assert_eq!(
                    operation.risk,
                    Risk::Low,
                    "operation `{}` is a read",
                    operation.id
                );
                assert_eq!(
                    operation.idempotency,
                    Idempotency::Idempotent,
                    "operation `{}` is a GET, which is repeatable",
                    operation.id
                );
            }
            HttpMethod::Post | HttpMethod::Put => {
                assert_ne!(
                    operation.risk,
                    Risk::Low,
                    "operation `{}` is a write declared low risk",
                    operation.id
                );
            }
            other => panic!(
                "operation `{}` uses method {other:?} this connector does not curate",
                operation.id
            ),
        }
    }
}

/// **Signer names and email addresses are personal data. Author none.** No literal example
/// resembling one appears anywhere this connector speaks for itself: not the provider TOML's own
/// text, and not any emitted module.
#[test]
fn no_example_signer_name_or_email_appears_anywhere() {
    let toml_text = provider_toml_text();
    for needle in FORBIDDEN_PII_EXAMPLES {
        assert!(
            !toml_text.to_lowercase().contains(&needle.to_lowercase()),
            "providers/docusign.toml contains {needle:?}, which reads as an example signer identity"
        );
    }

    let connector = load();
    for (id, text) in emitted(&connector) {
        for needle in FORBIDDEN_PII_EXAMPLES {
            assert!(
                !text.to_lowercase().contains(&needle.to_lowercase()),
                "`{id}` carries {needle:?}, which reads as an example signer identity:\n{text}"
            );
        }
    }

    // Every field carrying a real signer's name or email is described as personal data, wherever it
    // is declared: the create operation's own recipient input (`templateRoles` items), and the
    // recipients-get response schema.
    let create = connector
        .operations
        .iter()
        .find(|operation| operation.id == "docusign-envelope-create-from-template")
        .expect("docusign-envelope-create-from-template is declared");
    let template_roles = create
        .params
        .body
        .iter()
        .find(|param| param.name == "templateRoles")
        .expect("`templateRoles` is declared");
    for field in ["email", "name"] {
        let description = template_roles
            .schema
            .get("items")
            .and_then(|items| items.get("properties"))
            .and_then(|props| props.get(field))
            .and_then(|prop| prop.get("description"))
            .and_then(|d| d.as_str())
            .unwrap_or_else(|| panic!("`templateRoles` items' `{field}` carries no description"));
        assert!(
            description.to_lowercase().contains("personal data"),
            "`templateRoles` items' `{field}` must be marked personal data, got {description:?}"
        );
    }

    let recipients_get = connector
        .operations
        .iter()
        .find(|operation| operation.id == "docusign-envelope-recipients-get")
        .expect("docusign-envelope-recipients-get is declared");
    let response = recipients_get
        .response_schema
        .as_ref()
        .expect("docusign-envelope-recipients-get declares a response_schema");
    for field in ["email", "name"] {
        let description = response
            .get("properties")
            .and_then(|props| props.get("signers"))
            .and_then(|signers| signers.get("items"))
            .and_then(|items| items.get("properties"))
            .and_then(|props| props.get(field))
            .and_then(|prop| prop.get("description"))
            .and_then(|d| d.as_str())
            .unwrap_or_else(|| panic!("`signers[].{field}` carries no description"));
        assert!(
            description.to_lowercase().contains("personal data"),
            "`signers[].{field}` must be marked personal data, got {description:?}"
        );
    }
}

/// No credential, and no credential's variable name, reaches a generated module.
#[test]
fn no_docusign_module_carries_a_credential_or_its_variable_name() {
    let connector = load();

    for (id, text) in emitted(&connector) {
        for forbidden in [TOKEN_ENV, CREDENTIAL, "$secret", "Authorization", "Bearer"] {
            assert!(
                !text.contains(forbidden),
                "`{id}` names `{forbidden}` in generated Flux; a generated module carries no \
                 credential and no credential reference (C-10, AGENTS.md):\n{text}"
            );
        }
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical,
/// and loads as exactly one exposed composite op.
#[test]
fn every_docusign_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

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
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, operation.id);
        assert!(
            program.ops[0].meta.expose,
            "`{}` must be exposed to the model as a tool",
            operation.id
        );
    }
}
