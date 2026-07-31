//! `providers/anthropic.toml` exists, emits analyzable Flux, claims `llm_catalogue` without
//! reshaping the role, ships no inference operation, and **sends `anthropic-version` on every
//! request as a literal** (C-122).
//!
//! The last claim is the one this file exists for, mirroring
//! `crates/connector-flux/tests/notion_connector.rs`: Anthropic answers every request with no
//! `anthropic-version` header exactly as Notion answers `400 validation_error` to one missing
//! `Notion-Version` — no default, no grace period. Pinning the header with a JSON Schema `const` on
//! a caller-supplied `params.header` emits a required argument a model must guess with the
//! constraint silently dropped (C-55's own finding); `const_headers` is the honest mechanism, and
//! this file proves the *connector* uses it — on the emitted request, not just the IR — so a later
//! edit reverting to a parameter is caught here rather than shipped.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, Connector, HttpMethod, Role};

/// The version this connector is pinned to. Anthropic versions its API by date in this header, and
/// the value is a property of the *connector* — never a caller's to supply.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// `<repo root>/providers/anthropic.toml`, derived from this crate's manifest directory so the test
/// is independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("anthropic.toml")
}

fn anthropic() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-122 ships the Anthropic connector",
            path.display()
        )
    });
    provider::load("providers/anthropic.toml", &source)
        .expect("providers/anthropic.toml does not load")
        .connector
}

/// The declaration line, which is where a caller-supplied argument would show up.
fn signature(emitted: &str) -> &str {
    emitted.lines().next().expect("a declaration line")
}

/// The connector exists, loads through the real loader, and is the one C-122 describes.
#[test]
fn the_anthropic_connector_loads() {
    let connector = anthropic();

    assert_eq!(connector.id, "anthropic");
    assert_eq!(connector.vendor, "Anthropic");
    assert_eq!(
        connector.base_url, "https://api.anthropic.com",
        "one tenant-independent host, never widened"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// **The headline acceptance assertion: the version header reaches every emitted operation.**
///
/// Both halves are checked and they fail differently. A missing literal is a broken request on the
/// first call; a surviving parameter is a required argument a model has to guess, which is the same
/// broken request one call later with a worse tool contract in between.
#[test]
fn the_version_header_reaches_every_emitted_operation() {
    let connector = anthropic();
    let expected_binding = format!(r#"anthropic_version = "{ANTHROPIC_VERSION}""#);

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).unwrap_or_else(|error| {
            panic!("operation `{}` is not emittable: {error}", operation.id)
        });

        assert!(
            emitted.contains(&expected_binding),
            "`{}` does not bind the version as a literal. Anthropic requires `anthropic-version` on \
             every request, so an operation missing it cannot work at all:\n{emitted}",
            operation.id
        );
        assert!(
            emitted.contains(r#""anthropic-version": anthropic_version"#),
            "`{}` binds the version but never sends it — the literal must reach the request under \
             Anthropic's own header spelling:\n{emitted}",
            operation.id
        );
        assert!(
            !signature(&emitted)
                .to_lowercase()
                .contains("anthropic_version"),
            "`{}` declares the version as a caller-supplied argument. It is a constant of the \
             connector, not an input: a model would have to guess it on every call and any caller \
             could set it to anything. Declare it in `const_headers` (C-55):\n{}",
            operation.id,
            signature(&emitted)
        );
    }
}

/// The same claim on the IR, so a provider file that dropped `const_headers` fails here with a
/// message naming the mechanism rather than only as a missing string in emitted text.
#[test]
fn every_operation_carries_the_version_in_const_headers() {
    let connector = anthropic();
    for operation in &connector.operations {
        assert_eq!(
            operation.params.const_headers.get("anthropic-version"),
            Some(&ANTHROPIC_VERSION.to_string()),
            "`{}` does not declare `anthropic-version` in `const_headers`. The provider-level \
             `const_headers` table is distributed onto every operation by the loader, so an \
             operation missing it means the table was removed or overridden",
            operation.id
        );
    }
}

/// The version is never a caller-supplied header parameter — the spelling C-55 refuses.
#[test]
fn no_operation_declares_the_version_as_a_header_parameter() {
    let connector = anthropic();
    for operation in &connector.operations {
        for header in &operation.params.header {
            let name = header.name.to_ascii_lowercase();
            let wire = header
                .wire
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            assert!(
                !name.contains("anthropic-version") && !wire.contains("anthropic-version"),
                "`{}` declares the version as a caller-supplied header parameter `{}`. \
                 `params.header` means caller-supplied; a vendor constant belongs in \
                 `const_headers`",
                operation.id,
                header.name
            );
        }
    }
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op — the C-11 gate, held against anthropic specifically.
#[test]
fn every_anthropic_operation_emits_an_analyzable_module() {
    let connector = anthropic();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).unwrap_or_else(|error| {
            panic!("operation `{}` is not emittable: {error}", operation.id)
        });

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
        assert_eq!(
            program.ops.len(),
            1,
            "one operation is one declaration; `{}` loaded {}",
            operation.id,
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, operation.id);
    }
}

/// The curated set C-122 selected, exactly. Named rather than counted so that adding an operation —
/// in particular an inference one — is a deliberate edit here.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = anthropic();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "anthropic-api-keys-list",
            "anthropic-model-get",
            "anthropic-models-list",
            "anthropic-organization-get",
            "anthropic-workspaces-list",
        ],
        "the curated set changed — this connector ships management surface and model catalogue \
         only, never inference"
    );
}

/// **The charter boundary: no inference operation.** `docs/vision.md`'s non-goal excludes
/// "replacing flux's native model providers"; `POST /v1/messages` belongs to flux's own `anthropic`
/// provider, never to this connector.
#[test]
fn no_operation_is_the_messages_inference_endpoint() {
    let connector = anthropic();
    for operation in &connector.operations {
        let path_lower = operation.path.to_lowercase();
        assert!(
            !path_lower.contains("/messages") && !path_lower.contains("/complete"),
            "`{}` at `{}` looks like an inference endpoint. This connector is management-plane \
             only — inference stays with flux's native anthropic provider (vision.md's non-goal, \
             C-123)",
            operation.id,
            operation.path
        );
        assert_ne!(
            operation.method,
            HttpMethod::Post,
            "a POST snuck into a read-only connector — `{}` should not exist here unless the \
             story deliberately added a write, in which case this assertion should change with it",
            operation.id
        );
    }
}

/// **C-121's role, claimed without any change to its definition.** The `models` service fills
/// `llm_catalogue`'s one required slot (`list`), the same way `openai` and `openrouter` do — the
/// falsification this story exists to run: Anthropic's shape differs from both and still needs no
/// reshaping to satisfy it.
#[test]
fn the_models_service_claims_llm_catalogue_without_reshaping() {
    let connector = anthropic();

    assert_eq!(
        connector.roles(),
        vec![Role::LlmCatalogue],
        "the connector's derived roles should be exactly llm_catalogue, from the models service"
    );

    let missing = connector.missing_role_members("models", Role::LlmCatalogue);
    assert!(
        missing.is_empty(),
        "the models service claims llm_catalogue but is missing required member(s): {missing:?}"
    );

    // Both `list` and `get` are present, even though the role's mechanism only requires `list`
    // today — see the story's Acceptance and `Role::required_members` in
    // `crates/connector-spec/src/ir.rs`.
    let model_members: Vec<&str> = connector
        .operations_of("models")
        .map(|operation| operation.id.as_str())
        .collect();
    assert!(model_members.iter().any(|id| id.ends_with("list")));
    assert!(model_members.iter().any(|id| id.ends_with("get")));
}

/// Auth is two `x-api-key` credentials, never a bearer — and the `verify` operation only ever
/// needs the regular one, so pressing "Test connection" never demands organization-admin access.
#[test]
fn the_connector_verifies_with_the_regular_key_over_x_api_key() {
    let connector = anthropic();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["anthropic.api_key", "anthropic.admin_key"],
        "two distinct x-api-key credentials: the regular key and the Admin API key"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("anthropic-models-list"),
        "the `verify` operation is the Test-connection button and must need only the regular key"
    );
    let verify = connector
        .operations
        .iter()
        .find(|operation| Some(operation.id.as_str()) == connector.verify.as_deref())
        .expect("`verify` names a declared operation");
    assert_eq!(
        verify.method,
        HttpMethod::Get,
        "`verify` runs unattended whenever a settings page opens, so it is a GET"
    );
}

/// The connection-level configuration surface: two token fields, and **no realistic-looking
/// example** on either — an `sk-ant-`-shaped placeholder is exactly the class that has tripped
/// GitHub push protection and blocked a release here before.
#[test]
fn the_credentials_are_configurable_and_carry_no_example_value() {
    let connector = anthropic();

    for name in ["api_key", "admin_key"] {
        let field = connector
            .config
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("`{name}` should be a configurable field"));

        assert!(field.secret, "a key is a secret");
        assert!(
            !field.label.is_empty() && !field.help.is_empty(),
            "a field must be renderable: `label` and `help` are what a settings page shows"
        );
        assert_eq!(
            field.example, None,
            "`{name}` carries an example — a key-shaped placeholder trips secret scanning and \
             teaches a reader to paste something that looks like a real value"
        );
    }
}

/// No anthropic operation declares a query parameter, so nothing this connector emits can carry an
/// unencoded value into a query string (C-30, the standing `zendesk-ticket-search` gap).
#[test]
fn no_anthropic_operation_declares_a_query_parameter() {
    let connector = anthropic();
    for operation in &connector.operations {
        assert!(
            operation.params.query.is_empty(),
            "operation `{}` declares query parameters {:?}. Nothing percent-encodes a query value \
             (C-30 is unimplemented), so a value carrying `&` or `#` corrupts the request or \
             injects a parameter",
            operation.id,
            operation
                .params
                .query
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>()
        );
    }
}
