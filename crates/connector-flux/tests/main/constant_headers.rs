//! A vendor-constant request header reaches the request as a **literal**, never as an argument.
//!
//! C-55. The measurement that filed it is C-107's: Notion requires `Notion-Version: 2022-06-28` on
//! every request, and the only header mechanism the schema had was `params.header`, which the IR
//! documents — correctly — as *"request headers the caller supplies"*. Pinning such a parameter with
//! a JSON Schema `const`, the trick that works for a constant **body** field, did nothing at all:
//! `op.rs` filtered `constant(...)` out of the declared parameter list for `body` params only, so
//! the emitted module was
//!
//! ```flux
//! op notion-page-get(page_id: String, notion_version: String) -> Any
//!   $response = http.request({ headers: { "Notion-Version": notion_version }, … })
//! ```
//!
//! — a required argument every caller must pass and any caller may set to anything, with the
//! constraint dropped. A connector in that shape compiles, formats, round-trips, ships, and answers
//! 400 on every call, which is why the tests here assert on the **emitted request** rather than on
//! whether the provider builds.
//!
//! The fixtures are Notion-shaped rather than Notion: `providers/notion.toml` is C-107's to write,
//! and a mechanism story must not ship a provider to prove itself.

use connector_flux::emit_operation;
use connector_spec::provider::load;
use connector_spec::{Connector, Operation};

/// One operation, one vendor-constant header, declared at operation level.
const OPERATION_LEVEL: &str = r#"
id = "vendor"
vendor = "Vendor"
base_url = "https://api.vendor.example"
description = "A vendor that pins its API version in a request header"

[[operations]]
id = "vendor-page-get"
method = "GET"
direction = "read"
path = "/v1/pages/{page_id}"
description = "Retrieve one page."
risk = "low"
idempotency = "idempotent"

[operations.params.const_headers]
"Notion-Version" = "2022-06-28"

[[operations.params.path]]
name = "page_id"
description = "The page to read."
required = true
schema = { type = "string" }
"#;

/// The same header declared once, at provider level, for every operation.
const PROVIDER_LEVEL: &str = r#"
id = "vendor"
vendor = "Vendor"
base_url = "https://api.vendor.example"
description = "A vendor that pins its API version in a request header"

[const_headers]
"Notion-Version" = "2022-06-28"

[[operations]]
id = "vendor-page-get"
method = "GET"
direction = "read"
path = "/v1/pages/{page_id}"
description = "Retrieve one page."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "page_id"
description = "The page to read."
required = true
schema = { type = "string" }

[[operations]]
id = "vendor-page-create"
method = "POST"
direction = "write"
path = "/v1/pages"
description = "Create a page."
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "title"
description = "The page title."
required = true
schema = { type = "string" }
"#;

/// The old spelling C-52 measured: a header **parameter** pinned with a JSON Schema `const`.
const CONST_PINNED_HEADER_PARAM: &str = r#"
id = "vendor"
vendor = "Vendor"
base_url = "https://api.vendor.example"
description = "A vendor that pins its API version in a request header"

[[operations]]
id = "vendor-page-get"
method = "GET"
direction = "read"
path = "/v1/pages/{page_id}"
description = "Retrieve one page."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "page_id"
description = "The page to read."
required = true
schema = { type = "string" }

[[operations.params.header]]
name = "notion_version"
wire = "Notion-Version"
description = "The API version this request is pinned to."
required = true
schema = { type = "string", const = "2022-06-28" }
"#;

fn connector(source: &str) -> Connector {
    load("providers/fixture.toml", source)
        .unwrap_or_else(|error| panic!("the fixture must load: {error}"))
        .connector
}

fn operation<'a>(connector: &'a Connector, id: &str) -> &'a Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("the fixture declares `{id}`"))
}

fn emit(connector: &Connector, id: &str) -> String {
    emit_operation(connector, operation(connector, id))
        .unwrap_or_else(|error| panic!("`{id}` must emit: {error}"))
}

/// The declaration line, which is where a caller-supplied argument would show up.
fn signature(emitted: &str) -> &str {
    emitted.lines().next().expect("a declaration line")
}

/// **The header's value reaches the request; its name never reaches the signature.**
///
/// Both halves matter and they fail differently. A missing literal is a 400 on every call; a
/// surviving parameter is a required argument a model has to guess and may set to anything, which
/// is the same 400 one call later.
#[test]
fn a_constant_header_is_emitted_as_its_value_not_as_a_parameter() {
    let connector = connector(OPERATION_LEVEL);
    let emitted = emit(&connector, "vendor-page-get");

    assert!(
        emitted.contains(r#"Notion_Version = "2022-06-28""#),
        "the vendor's value must be bound as a literal:\n{emitted}"
    );
    assert!(
        emitted.contains(r#""Notion-Version": Notion_Version"#),
        "the literal must reach the request under the vendor's own spelling:\n{emitted}"
    );
    assert_eq!(
        signature(&emitted),
        "op vendor-page-get(page_id: String) -> Any",
        "a constant header is not an argument:\n{emitted}"
    );
}

/// **`const` on a header parameter is refused rather than silently dropped.**
///
/// This is the shape C-52 measured and C-107 hit. Whichever way it is resolved, the one outcome this
/// repository does not allow is the one it had: emitting the parameter with the constraint dropped.
/// It is refused rather than honoured because `params.header` means *caller-supplied* — reinterpreting
/// it by keyword would make one declaration mean two things — and the refusal names the mechanism
/// that does apply.
#[test]
fn a_const_pinned_header_param_never_reaches_the_module_as_an_argument() {
    let connector = connector(CONST_PINNED_HEADER_PARAM);
    let operation = operation(&connector, "vendor-page-get");

    match emit_operation(&connector, operation) {
        Err(error) => {
            let rendered = error.to_string();
            assert!(
                rendered.contains("const_headers"),
                "the refusal must name the mechanism that pins a header, got: {rendered}"
            );
        }
        Ok(emitted) => panic!(
            "a `const`-pinned header parameter is still emitted as a caller-overridable argument, \
             with the constraint dropped:\n{emitted}"
        ),
    }
}

/// A provider-level header is a property of the vendor, so it is sent by **every** operation —
/// including one that carries a body, where it has to coexist with the emitter's own `content-type`.
#[test]
fn a_provider_level_constant_header_reaches_every_operation() {
    let connector = connector(PROVIDER_LEVEL);

    for id in ["vendor-page-get", "vendor-page-create"] {
        let emitted = emit(&connector, id);
        assert!(
            emitted.contains(r#"Notion_Version = "2022-06-28""#)
                && emitted.contains(r#""Notion-Version": Notion_Version"#),
            "`{id}` must carry the provider's constant header:\n{emitted}"
        );
        assert!(
            !signature(&emitted).to_lowercase().contains("notion"),
            "`{id}` declares the constant header as an argument:\n{emitted}"
        );
    }

    let created = emit(&connector, "vendor-page-create");
    assert!(
        created.contains(r#""content-type": content_type"#),
        "a constant header must not displace the media type of the body:\n{created}"
    );
}

/// An operation may pin a version the rest of the provider does not — and the two spellings are one
/// header, so the operation's value replaces the provider's rather than joining it.
#[test]
fn an_operation_level_constant_header_overrides_the_providers() {
    let source = PROVIDER_LEVEL.replace(
        r#"[[operations.params.body]]"#,
        "[operations.params.const_headers]\n\"notion-version\" = \"2025-09-03\"\n\n[[operations.params.body]]",
    );
    let connector = connector(&source);

    let emitted = emit(&connector, "vendor-page-create");
    assert!(
        emitted.contains(r#""notion-version": notion_version"#)
            && emitted.contains(r#"notion_version = "2025-09-03""#),
        "the operation's own value must win:\n{emitted}"
    );
    assert!(
        !emitted.contains("2022-06-28"),
        "the provider's value must not travel alongside it — HTTP header names are \
         case-insensitive, so both would be one header sent twice:\n{emitted}"
    );

    // The other operation still gets the provider's.
    let untouched = emit(&connector, "vendor-page-get");
    assert!(
        untouched.contains(r#"Notion_Version = "2022-06-28""#),
        "an operation-level override must not reach the operation next door:\n{untouched}"
    );
}

/// A constant and a caller-supplied header with one name is refused: the request record has one slot
/// for it, so emitting either would drop the other without a word.
#[test]
fn a_constant_header_may_not_collide_with_a_caller_supplied_one() {
    let source = OPERATION_LEVEL.to_string()
        + r#"
[[operations.params.header]]
name = "version"
wire = "notion-version"
description = "The API version."
required = true
schema = { type = "string" }
"#;
    let connector = connector(&source);
    let operation = operation(&connector, "vendor-page-get");

    let error = emit_operation(&connector, operation)
        .expect_err("one header name cannot be both constant and caller-supplied");
    let rendered = error.to_string();
    assert!(
        rendered.contains("Notion-Version") || rendered.contains("notion-version"),
        "the refusal must name the contested header, got: {rendered}"
    );
}

/// The one constant header the emitter owns. A provider that redeclared it would be describing a
/// body encoding the emitter does not produce.
#[test]
fn the_media_type_of_the_body_is_not_redeclarable() {
    let source = PROVIDER_LEVEL.replace(
        r#""Notion-Version" = "2022-06-28""#,
        r#""Content-Type" = "application/x-www-form-urlencoded""#,
    );
    let error = load("providers/fixture.toml", &source)
        .expect_err("the media type of the request body is the emitter's");
    let rendered = error.to_string();
    assert!(
        rendered.to_lowercase().contains("content-type"),
        "the refusal must name the header, got: {rendered}"
    );
}

/// The emitted module still parses and is a fixed point of flux-lang's own formatter — the property
/// every emitted artifact rests on, asserted for the new statement shape rather than assumed.
#[test]
fn a_module_carrying_a_constant_header_is_canonical_flux() {
    let connector = connector(PROVIDER_LEVEL);

    for operation in &connector.operations {
        let emitted = emit(&connector, &operation.id);
        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "emitted Flux for `{}` does not parse: {:?}\n{emitted}",
            operation.id,
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str()),
            "the flux formatter would reformat the emitted module for `{}`",
            operation.id
        );
    }
}
