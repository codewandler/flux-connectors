//! `providers/box.toml` exists, emits analyzable Flux, and **says what Box's root folder id
//! actually is** (C-171).
//!
//! Every Box folder and file id is an opaque numeric string, except the account's root folder,
//! whose id is not opaque at all — it is the fixed sentinel `"0"`. A model handed a `folder_id`
//! parameter with no further information has no way to guess this: `"root"`, `""` and `"/"` are all
//! more plausible completions than `"0"`, and every one of them is a `404`. So this file's real
//! acceptance claim, the one C-107's `Notion-Version` test is the archetype for, is that **every**
//! parameter naming a folder id says so twice — once in its `description`, which is prose a model
//! reads, and once as `default` on its JSON Schema, which is the one form of "the answer, right
//! here" that reaches the composed `input_schema` a model actually receives (C-125). Either half
//! missing is the connector shipping a trap on its very first call.
//!
//! This file also pins the two connector-shaping refusals the story records: no operation reaches
//! Box's download endpoint (`GET /files/{file_id}/content`, a `302` to a signed URL that following
//! is not a declared behaviour of this pipeline), and no operation declares a query parameter (C-30
//! — nothing here percent-encodes a query value).

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{Connector, HttpMethod, Param};

use crate::shipped_provider;

/// `<repo root>/providers/box.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("box.toml")
}

fn box_connector() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-171 ships the Box connector",
            path.display()
        )
    });
    shipped_provider::load_definition("box", &source)
        .expect("providers/box.toml does not load")
        .connector
}

/// Every path or body parameter across the connector whose caller-facing name marks it as naming a
/// folder id — `folder_id` and `parent_id`, the two spellings box.toml uses.
fn folder_id_params(connector: &Connector) -> Vec<(&str, &Param)> {
    let mut found = Vec::new();
    for operation in &connector.operations {
        for param in operation
            .params
            .path
            .iter()
            .chain(operation.params.body.iter())
        {
            if param.name == "folder_id" || param.name == "parent_id" {
                found.push((operation.id.as_str(), param));
            }
        }
    }
    found
}

/// The connector exists, loads through the real loader, and is the one C-171 describes.
#[test]
fn the_box_connector_loads() {
    let connector = box_connector();

    assert_eq!(connector.id, "box");
    assert_eq!(connector.vendor, "Box");
    assert_eq!(
        connector.base_url, "https://api.box.com",
        "one tenant-independent host, never widened"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// **The acceptance assertion: the root-folder sentinel is declared on every folder id parameter.**
///
/// Both halves are checked and they fail differently. A missing `default` is a model with no hint
/// at all in the schema it actually receives; a description that never says "0" is a trap for a
/// reader who skips straight to calling the tool. At least one folder-id parameter must exist at
/// all, or this test would vacuously pass on a connector that quietly dropped every one of them.
#[test]
fn the_root_folder_sentinel_is_declared_on_every_folder_id_parameter() {
    let connector = box_connector();
    let params = folder_id_params(&connector);

    assert!(
        !params.is_empty(),
        "no operation declares a `folder_id` or `parent_id` parameter — the connector this story \
         is about has nothing left to say \"0\" about"
    );

    for (operation_id, param) in &params {
        assert_eq!(
            param.schema.get("default").and_then(|v| v.as_str()),
            Some("0"),
            "`{operation_id}`'s `{}` carries no `default = \"0\"` in its JSON Schema. Box's root \
             folder id is the literal string \"0\", and that is the one hint that reaches the \
             composed `input_schema` a model actually sees (C-125) — a description alone is not \
             enough for a model that never reads prose",
            param.name
        );
        assert!(
            param.description.contains('0') && param.description.to_lowercase().contains("root"),
            "`{operation_id}`'s `{}` description does not name the root-folder sentinel. Full text: \
             {:?}",
            param.name,
            param.description
        );
    }
}

/// Box's download endpoint answers `302` to a signed URL on a separate host, and redirect-following
/// is not a declared behaviour of this pipeline's `http.request` primitive. Shipping it would be an
/// operation whose success depends on a client setting nobody declared — AGENTS.md's "plausible but
/// incorrect" output, refused rather than absorbed silently.
#[test]
fn the_download_endpoint_is_not_shipped() {
    let connector = box_connector();
    for operation in &connector.operations {
        assert!(
            !operation.path.ends_with("/content"),
            "`{}` reaches Box's file-content endpoint ({}), which answers 302 to a signed URL this \
             pipeline does not follow",
            operation.id,
            operation.path
        );
    }
}

/// The curated set C-171 selected, exactly. Named rather than counted so that adding an operation is
/// a deliberate edit here.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = box_connector();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "box-file-copy",
            "box-file-get",
            "box-folder-create",
            "box-folder-get",
            "box-folder-items-list",
            "box-user-me",
        ],
        "the curated set changed. File content (the 302 download) and every query-driven listing \
         are excluded for stated reasons — see providers/box.toml's header comment"
    );
}

/// Auth is one bearer access token, and the `verify` operation is a genuine read.
#[test]
fn the_connector_verifies_with_a_read_over_a_bearer_token() {
    let connector = box_connector();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["box.access_token"],
        "one credential covers the whole selection"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("box-user-me"),
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
        "`verify` runs unattended whenever a settings page opens, so it must be a GET"
    );
}

/// The connection-level configuration surface: the access token, and **no realistic-looking example
/// on it**.
#[test]
fn the_access_token_is_configurable_and_carries_no_example_value() {
    let connector = box_connector();

    let field = connector
        .config
        .iter()
        .find(|field| field.name == "access_token")
        .expect("the access token is the one thing a human must supply");

    assert!(field.secret, "a Box access token is a secret");
    assert_eq!(field.binds, "credential.box.access_token");
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

/// No Box operation declares a query parameter, so nothing this connector emits can carry an
/// unencoded value into a query string (C-30, the standing `zendesk-ticket-search` gap). This is
/// what keeps `box-folder-items-list` a first-page-only read rather than a paginated one.
#[test]
fn no_box_operation_declares_a_query_parameter() {
    let connector = box_connector();
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

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op — the C-11 gate, held against box specifically.
#[test]
fn every_box_operation_emits_an_analyzable_module() {
    let connector = box_connector();
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
