//! `providers/notion.toml` exists, emits analyzable Flux, and **sends `Notion-Version` on every
//! request as a literal** (C-107).
//!
//! The third claim is the one this file exists for, and it is the difference between a connector and
//! a connector-shaped artifact. Notion rejects a request carrying no `Notion-Version` header with
//! `400 validation_error` — every request, every endpoint, no default and no grace period. So unlike
//! `Accept` on github, where the vendor defaults the value and the pin only protects against a future
//! change, this header is the connector working at all.
//!
//! C-107's first attempt measured what happens without a mechanism for it. The only header field the
//! schema had was `params.header`, documented as *"request headers the caller supplies"*; pinning one
//! with a JSON Schema `const` emitted
//!
//! ```flux
//! op notion-page-get(page_id: String, Notion_Version: String) -> Any
//! ```
//!
//! — a required argument a model must guess on every call and any caller may set to anything, with
//! the `const` silently dropped. That connector compiled, formatted, round-tripped and would have
//! shipped; only a test on the **emitted request** catches it, which is why the assertions below read
//! the emitted module rather than the IR alone. C-55 added `const_headers` as the honest spelling and
//! `crates/connector-flux/tests/constant_headers.rs` proves the mechanism on fixtures; this file
//! proves the *connector* uses it, and is what stops a later edit from reverting to a parameter.
//!
//! The two structural claims deliberately duplicate what `shipped_modules.rs` asserts across every
//! provider. That file iterates whatever `providers/` holds; this one names only notion, so C-107's
//! gate stays a claim about notion rather than one whose subject moves when the shipped set does.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, Connector, HttpMethod};

/// The version Notion pins this connector to. Notion's API is versioned by date in this header, and
/// the value is a property of the *connector* — the request shapes below are the ones this version
/// serves — never of a caller.
const NOTION_VERSION: &str = "2022-06-28";

/// `<repo root>/providers/notion.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("notion.toml")
}

fn notion() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-107 ships the Notion connector",
            path.display()
        )
    });
    provider::load("providers/notion.toml", &source)
        .expect("providers/notion.toml does not load")
        .connector
}

/// The declaration line, which is where a caller-supplied argument would show up.
fn signature(emitted: &str) -> &str {
    emitted.lines().next().expect("a declaration line")
}

/// The connector exists, loads through the real loader, and is the one C-107 describes.
#[test]
fn the_notion_connector_loads() {
    let connector = notion();

    assert_eq!(connector.id, "notion");
    assert_eq!(connector.vendor, "Notion");
    assert_eq!(
        connector.base_url, "https://api.notion.com",
        "one tenant-independent host, never widened"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// **The acceptance assertion: the version header reaches every emitted operation.**
///
/// Both halves are checked and they fail differently. A missing literal is a `400 validation_error`
/// on the first call; a surviving parameter is a required argument a model has to guess, which is the
/// same 400 one call later with a worse tool contract in between.
#[test]
fn the_version_header_reaches_every_emitted_operation() {
    let connector = notion();
    let expected_binding = format!(r#"Notion_Version = "{NOTION_VERSION}""#);

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).unwrap_or_else(|error| {
            panic!("operation `{}` is not emittable: {error}", operation.id)
        });

        assert!(
            emitted.contains(&expected_binding),
            "`{}` does not bind the version as a literal. Notion answers 400 validation_error to \
             every request without `Notion-Version`, so an operation missing it cannot work at \
             all:\n{emitted}",
            operation.id
        );
        assert!(
            emitted.contains(r#""Notion-Version": Notion_Version"#),
            "`{}` binds the version but never sends it — the literal must reach the request under \
             Notion's own spelling:\n{emitted}",
            operation.id
        );
        assert!(
            !signature(&emitted)
                .to_lowercase()
                .contains("notion_version"),
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
    let connector = notion();
    for operation in &connector.operations {
        assert_eq!(
            operation.params.const_headers.get("Notion-Version"),
            Some(&NOTION_VERSION.to_string()),
            "`{}` does not declare `Notion-Version` in `const_headers`. The provider-level \
             `const_headers` table is distributed onto every operation by the loader, so an \
             operation missing it means the table was removed or overridden",
            operation.id
        );
    }
}

/// The version is never a caller-supplied header parameter — the spelling C-107's first attempt
/// measured and C-55 refuses. Asserted on the IR because that refusal happens at emission, so a file
/// declaring it would fail the emitting tests with a less specific message.
#[test]
fn no_operation_declares_the_version_as_a_header_parameter() {
    let connector = notion();
    for operation in &connector.operations {
        for header in &operation.params.header {
            let name = header.name.to_ascii_lowercase();
            let wire = header
                .wire
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            assert!(
                !name.contains("notion") && !wire.contains("notion-version"),
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
/// exactly one composite op — the C-11 gate, held against notion specifically.
#[test]
fn every_notion_operation_emits_an_analyzable_module() {
    let connector = notion();
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

/// The curated set C-107 selected, exactly. Named rather than counted so that adding an operation is
/// a deliberate edit here — every exclusion in `providers/notion.toml` has a recorded reason, and the
/// two large ones (the recursive block model, the tenant-keyed property object) are not expressible
/// rather than merely unwritten.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = notion();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "notion-database-query",
            "notion-page-create",
            "notion-page-get",
            "notion-search",
            "notion-user-me",
        ],
        "the curated set changed. Adding page *content* needs the block model, which is a ~30-way \
         recursive union `JsonSchema` cannot express here"
    );
}

/// Auth is one bearer integration token, and the `verify` operation is a genuine read.
#[test]
fn the_connector_verifies_with_a_read_over_a_bearer_token() {
    let connector = notion();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["notion.token"],
        "one credential covers the whole selection"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("notion-user-me"),
        "the `verify` operation is the Test-connection button and must be a read"
    );
    let verify = connector
        .operations
        .iter()
        .find(|operation| Some(operation.id.as_str()) == connector.verify.as_deref())
        .expect("`verify` names a declared operation");
    assert_eq!(
        verify.method,
        HttpMethod::Get,
        "`verify` runs unattended whenever a settings page opens, so it is a GET rather than one of \
         Notion's POST reads"
    );
}

/// The connection-level configuration surface: the token, and **no realistic-looking example on it**.
///
/// A Notion token is `ntn_` followed by 46 characters, and a placeholder of that shape matches secret
/// scanning — `providers/shopify.toml` records the release this repository actually lost that way.
/// The shape belongs in `help`, where it cannot be mistaken for a value or copied into a form.
#[test]
fn the_token_is_configurable_and_carries_no_example_value() {
    let connector = notion();

    let field = connector
        .config
        .iter()
        .find(|field| field.name == "token")
        .expect("the token is the one thing a human must supply");

    assert!(field.secret, "an integration token is a secret");
    assert_eq!(field.binds, "credential.notion.token");
    assert!(
        !field.label.is_empty() && !field.help.is_empty(),
        "a field must be renderable: `label` and `help` are what a settings page shows"
    );
    assert_eq!(
        field.example, None,
        "a secret field carries no example — a token-shaped placeholder trips secret scanning and \
         teaches a reader to paste something that looks like a real value"
    );
}

/// No notion operation declares a query parameter, so nothing this connector emits can carry an
/// unencoded value into a query string (C-30, the standing `zendesk-ticket-search` gap). This is what
/// keeps Notion's pagination — an opaque server-issued `start_cursor` — out until C-30 lands.
#[test]
fn no_notion_operation_declares_a_query_parameter() {
    let connector = notion();
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
