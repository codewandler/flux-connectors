//! `providers/dropbox.toml` exists, emits analyzable Flux, and **is the connector where every
//! single operation is a POST, including the ones that read and change nothing** (C-167).
//!
//! Dropbox's API v2 is RPC wearing HTTP: `POST /2/files/list_folder` with a JSON body is how you
//! *read* a folder. There is no `GET` anywhere in Dropbox's v2 surface — not one endpoint, not even
//! the "who am I" call. That is the whole archetype this connector exists to exercise, and it forces
//! the key counterexample to HTTP-method-shaped direction:
//!
//! 1. **Can a read transported by POST be a `verify` operation?** Yes. `validate_verify`
//!    (`crates/connector-spec/src/provider.rs`) checks the named operation's declared `risk`, never
//!    its HTTP method — it refuses `high`/`destructive` and nothing else. Box and Notion both had a
//!    genuine `GET` available and used it for `verify` instead of proving this; Dropbox has no `GET`
//!    to fall back to, so this connector's `verify` is a `POST` and the loader accepts it, exactly
//!    because the check never looked at the method.
//! 2. **Can it be low-risk and idempotent?** Yes. `dropbox-user-me` is explicitly authored `read`,
//!    `low`, and `idempotent`. The other read-shaped RPCs retain conservative authored write values
//!    pending individual review; their shared method supplies no direction evidence.
//!
//! Content upload and download are deliberately absent: they live on a different host
//! (`content.dropboxapi.com`) and carry their JSON argument in a `Dropbox-API-Arg` request *header*
//! rather than the body — a second encoding problem this story was not chosen to solve.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{Connector, HttpMethod, Idempotency, OperationDirection, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// `<repo root>/providers/dropbox.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("dropbox.toml")
}

fn dropbox() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-167 ships the Dropbox connector",
            path.display()
        )
    });
    shipped_provider::load_definition("dropbox", &source)
        .expect("providers/dropbox.toml does not load")
        .connector
}

/// The connector exists, loads through the real loader, and is the one C-167 describes.
#[test]
fn the_dropbox_connector_loads() {
    let connector = dropbox();

    assert_eq!(connector.id, "dropbox");
    assert_eq!(connector.vendor, "Dropbox");
    assert_eq!(
        connector.base_url, "https://api.dropboxapi.com",
        "one tenant-independent RPC host, never widened to the content or web hosts"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// **The headline acceptance assertion: every operation, without exception, is a POST.** A `GET`
/// anywhere in this file would mean this connector stopped being the archetype it was chosen for —
/// Dropbox's v2 API has no `GET` endpoint at all, so a `GET` here is a fabricated one, not a curated
/// one.
#[test]
fn every_operation_is_a_post_including_the_reads() {
    let connector = dropbox();
    for operation in &connector.operations {
        assert_eq!(
            operation.method,
            HttpMethod::Post,
            "`{}` is not a POST. Dropbox's v2 API has no GET endpoint anywhere — every read, \
             including this one, is routed through POST with a JSON body",
            operation.id
        );
    }
}

/// **Hazard #1, answered: the `verify` operation is itself a POST, and the loader accepts it.**
/// `validate_verify` checks declared `risk`, not HTTP method (`crates/connector-spec/src/
/// provider.rs`), so a low-risk authored read remains loader-legal regardless of POST transport.
#[test]
fn the_verify_operation_is_an_authored_read_despite_post_transport() {
    let connector = dropbox();

    assert_eq!(
        connector.verify.as_deref(),
        Some("dropbox-user-me"),
        "the `verify` operation is the Test-connection button and must be a read"
    );
    let verify = connector
        .operations
        .iter()
        .find(|operation| Some(operation.id.as_str()) == connector.verify.as_deref())
        .expect("`verify` names a declared operation");

    assert_eq!(
        verify.method,
        HttpMethod::Post,
        "`verify` is a POST — Dropbox has no GET to fall back to, unlike box-user-me and \
         notion-user-me"
    );
    assert_eq!(
        verify.risk,
        Risk::Low,
        "an authored read may carry the low risk appropriate to an unattended connection test"
    );
    assert_eq!(verify.direction, OperationDirection::Read);
    assert_eq!(verify.idempotency, Idempotency::Idempotent);
}

/// The reviewed counterexample is read/idempotent; every conservatively authored write remains
/// non-idempotent. HTTP method is the same for all of them and participates in neither assertion.
#[test]
fn direction_not_method_decides_dropbox_safety_metadata() {
    let connector = dropbox();
    for operation in &connector.operations {
        let expected = match operation.direction {
            OperationDirection::Read => Idempotency::Idempotent,
            OperationDirection::Write => Idempotency::NonIdempotent,
        };
        assert_eq!(operation.idempotency, expected, "`{}`", operation.id);
    }
}

/// Content upload and download are excluded: `content.dropboxapi.com` is a different host, and the
/// `Dropbox-API-Arg` header carries the operation's JSON argument instead of the body — a second
/// encoding problem (a JSON value inside a header) this story was not chosen to solve. Neither the
/// host nor the header may appear anywhere in the shipped connector.
#[test]
fn content_upload_and_download_are_not_shipped() {
    let connector = dropbox();

    assert!(
        !connector.base_url.contains("content.dropboxapi.com"),
        "the base URL must stay the RPC host; content upload/download live on a separate host this \
         connector never reaches"
    );

    for operation in &connector.operations {
        assert!(
            !operation.path.contains("/upload")
                && !operation.path.contains("/download")
                && !operation.path.contains("/get_thumbnail"),
            "`{}` reaches a content transfer endpoint ({}), which requires a `Dropbox-API-Arg` \
             header carrying JSON — a second encoding problem this story excludes",
            operation.id,
            operation.path
        );
        assert!(
            !operation
                .params
                .const_headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("Dropbox-API-Arg")),
            "`{}` declares `Dropbox-API-Arg` as a constant header — that header carries a JSON \
             argument, which is the excluded content-transfer encoding",
            operation.id
        );
        assert!(
            !operation
                .params
                .header
                .iter()
                .any(|param| param.name.eq_ignore_ascii_case("Dropbox-API-Arg")),
            "`{}` declares `Dropbox-API-Arg` as a caller-supplied header — that header carries a \
             JSON argument, which is the excluded content-transfer encoding",
            operation.id
        );
    }
}

/// The curated set C-167 selected, exactly. Named rather than counted so that adding an operation is
/// a deliberate edit here.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = dropbox();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "dropbox-folder-create",
            "dropbox-folder-list",
            "dropbox-metadata-get",
            "dropbox-search",
            "dropbox-temporary-link-get",
            "dropbox-user-me",
        ],
        "the curated set changed. Content upload/download and pagination continuation are excluded \
         for stated reasons — see providers/dropbox.toml's header comment"
    );
}

/// Auth is one bearer access token.
#[test]
fn the_connector_authenticates_with_one_bearer_token() {
    let connector = dropbox();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["dropbox.access_token"],
        "one credential covers the whole selection"
    );
}

/// The connection-level configuration surface: the access token, and **no realistic-looking example
/// value** — the caution `providers/notion.toml` and `providers/shopify.toml` both record against a
/// token-shaped placeholder tripping secret scanning.
#[test]
fn the_access_token_is_configurable_and_carries_no_example_value() {
    let connector = dropbox();

    let field = connector
        .config
        .iter()
        .find(|field| field.name == "access_token")
        .expect("the access token is the one thing a human must supply");

    assert!(field.secret, "a Dropbox access token is a secret");
    assert_eq!(field.binds, "credential.dropbox.access_token");
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

/// No Dropbox operation declares a query parameter: everything Dropbox's RPC style needs travels in
/// the JSON body, and nothing here percent-encodes a query value either (C-30, the standing
/// `zendesk-ticket-search` gap), so there is nothing this connector should ever put in a query
/// string.
#[test]
fn no_dropbox_operation_declares_a_query_parameter() {
    let connector = dropbox();
    for operation in &connector.operations {
        assert!(
            operation.params.query.is_empty(),
            "operation `{}` declares query parameters {:?}. Dropbox's RPC style carries every \
             argument in the JSON body, and nothing here percent-encodes a query value either",
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
/// exactly one composite op — the C-11 gate, held against dropbox specifically.
#[test]
fn every_dropbox_operation_emits_an_analyzable_module() {
    let connector = dropbox();
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
