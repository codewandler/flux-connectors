//! `providers/front.toml` exists, emits analyzable Flux, and **never invents a way to reach page 2**
//! (C-179).
//!
//! Front prefixes every resource id (`cnv_55c8c149`, `msg_...`, `tag_...`) and a model handed a bare
//! `conversation_id` parameter has no way to guess that shape — the same hazard `box.toml`'s root
//! folder sentinel and `notion.toml`'s UUID pattern already guard against, and the same fix: the
//! prefix has to be said twice, once in a `pattern` a validator can check and once in `description`
//! prose a model actually reads.
//!
//! The claim this file exists for is the pagination one. Front's collection endpoints hand back a
//! `_pagination.next` field carrying a full absolute URL to the following page (verified against
//! `dev.frontapp.com`'s own reference — the underlying OpenAPI document also names a `page_token`
//! query parameter, but it is exactly the opaque, server-issued cursor shape `providers/notion.toml`
//! already refuses: unconstructable by a caller, and — even set that aside — unencodable, because
//! nothing in this pipeline percent-encodes a query value (C-30). So every collection operation below
//! returns Front's first page only, and says so in its own `response_schema`, rather than declaring a
//! `page_token` parameter no caller could ever supply a valid value for. An operation that silently
//! returned only page 1 while claiming to "list" something would be exactly the plausible-but-wrong
//! output AGENTS.md refuses; this file's job is to prove the connector says the limitation out loud
//! instead.
//!
//! The two structural claims deliberately duplicate what `shipped_modules.rs` asserts across every
//! provider. That file iterates whatever `providers/` holds; this one names only front, so this
//! story's gate stays a claim about front rather than one whose subject moves when the shipped set
//! does.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, Connector, HttpMethod};

/// `<repo root>/providers/front.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("front.toml")
}

fn front() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-179 ships the Front connector",
            path.display()
        )
    });
    provider::load("providers/front.toml", &source)
        .expect("providers/front.toml does not load")
        .connector
}

/// The connector exists, loads through the real loader, and is the one C-179 describes.
#[test]
fn the_front_connector_loads() {
    let connector = front();

    assert_eq!(connector.id, "front");
    assert_eq!(connector.vendor, "Front");
    assert_eq!(
        connector.base_url, "https://api2.frontapp.com",
        "one tenant-independent host — Front has no per-account subdomain"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// **The acceptance assertion, half one: no operation declares a cursor or page-token query
/// parameter, and nothing declares a pagination quirk.**
///
/// Front's own OpenAPI document names `page_token`, but it is an opaque value only ever obtained
/// from a previous response's `_pagination.next` — a caller cannot construct one — and this pipeline
/// cannot percent-encode a query value regardless (C-30). Declaring it as a caller-supplied parameter
/// would be an argument no model could ever fill in honestly.
#[test]
fn no_front_operation_declares_a_pagination_cursor_parameter() {
    let connector = front();
    for operation in &connector.operations {
        for param in &operation.params.query {
            let name = param.name.to_ascii_lowercase();
            assert!(
                !name.contains("page_token") && !name.contains("cursor") && name != "next",
                "`{}` declares `{}` as a caller-supplied query parameter. Front's next-page token is \
                 server-issued and opaque — a model has no way to construct a valid value, and \
                 nothing here percent-encodes a query value in any case (C-30)",
                operation.id,
                param.name
            );
        }
        assert!(
            operation.quirks.pagination.is_none(),
            "`{}` declares a `Pagination` quirk. C-12 does not compile one into control flow yet, and \
             this connector's whole point is refusing to imply one exists",
            operation.id
        );
    }
}

/// **The acceptance assertion, half two: every collection operation says plainly, in its own
/// `response_schema`, that page 2 is unreachable here.**
///
/// This is the difference between an honest first-page read and a silently-truncated one. A caller
/// reading only the schema — never this file's comments — must be able to tell the list is partial.
#[test]
fn every_listing_operation_documents_that_pagination_is_unreachable() {
    let connector = front();
    let listing_ids = ["front-conversation-list", "front-conversation-message-list"];

    for id in listing_ids {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("the curated set names `{id}`"));

        let schema = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("`{id}` declares no response_schema at all"));
        let rendered = schema.to_string();
        assert!(
            rendered.contains("_pagination"),
            "`{id}`'s response_schema does not mention `_pagination`, so a caller reading only the \
             schema has no way to learn a next page exists at all:\n{rendered}"
        );
        assert!(
            rendered.to_lowercase().contains("cannot")
                || rendered.to_lowercase().contains("first page only")
                || rendered.to_lowercase().contains("unreachable"),
            "`{id}`'s response_schema mentions pagination but never says this connector cannot \
             follow it — the exact gap a silently-truncated list would hide:\n{rendered}"
        );
    }
}

/// Every path or body parameter naming a Front resource id — scalar (`conversation_id`) or an array
/// of them (`tag_ids`) — carries both halves of the "do not invent this" contract: a `pattern`
/// requiring the vendor's own type prefix (on the schema itself for a scalar, on `items` for an
/// array), and a `description` that says so in prose. Neither is enough alone, the same reasoning
/// `box_connector.rs` and `notion_connector.rs` hold their connectors to.
///
/// `message_id` (`msg_`) does not appear here: the curated set never takes one as a *caller-supplied*
/// input, only ever returns one in a response (`front-conversation-message-list`'s `_results[].id`),
/// so there is no input parameter for this test to check.
#[test]
fn every_resource_id_parameter_declares_its_prefix_twice() {
    let connector = front();
    let prefixed = [("conversation_id", "cnv_"), ("tag_ids", "tag_")];
    let mut seen = std::collections::HashSet::new();

    for operation in &connector.operations {
        for param in operation
            .params
            .path
            .iter()
            .chain(operation.params.body.iter())
        {
            let Some((_, prefix)) = prefixed.iter().find(|(name, _)| *name == param.name) else {
                continue;
            };
            seen.insert(param.name.as_str());

            // A scalar id declares `pattern` on its own schema; an array of ids declares it on
            // `items` instead — the schema shape `tag_ids` uses.
            let pattern = param
                .schema
                .get("pattern")
                .and_then(|value| value.as_str())
                .or_else(|| {
                    param
                        .schema
                        .get("items")
                        .and_then(|items| items.get("pattern"))
                        .and_then(|value| value.as_str())
                })
                .unwrap_or_else(|| {
                    panic!(
                        "`{}`'s `{}` carries no JSON Schema `pattern` (on itself or on `items`) — \
                         nothing requires the `{prefix}` type prefix a caller cannot otherwise guess",
                        operation.id, param.name
                    )
                });
            assert!(
                pattern.contains(prefix),
                "`{}`'s `{}` pattern {pattern:?} does not require the `{prefix}` prefix",
                operation.id,
                param.name
            );
            assert!(
                param.description.contains(prefix),
                "`{}`'s `{}` description never names the `{prefix}` prefix in prose, so a model that \
                 does not read the schema's `pattern` has no way to learn it. Full text: {:?}",
                operation.id,
                param.name,
                param.description
            );
        }
    }

    assert_eq!(
        seen,
        prefixed.iter().map(|(name, _)| *name).collect(),
        "not every prefixed id this connector should use actually appears — this test would \
         otherwise pass vacuously on a connector that dropped one"
    );
}

/// The curated set C-179 selected, exactly. Named rather than counted so that adding an operation is
/// a deliberate edit here.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = front();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "front-conversation-get",
            "front-conversation-list",
            "front-conversation-message-list",
            "front-conversation-reply",
            "front-conversation-tag-add",
            "front-verify",
        ],
        "the curated set changed — see providers/front.toml's header comment for what is excluded \
         and why"
    );
}

/// Auth is one bearer API token, and the `verify` operation is a genuine, bounded read.
#[test]
fn the_connector_verifies_with_a_read_over_a_bearer_token() {
    let connector = front();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["front.api_token"],
        "one credential covers the whole selection"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("front-verify"),
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

/// The reply operation declares no recipient, channel or author override. Front documents all of
/// `to`/`cc`/`bcc`/`channel_id`/`author_id`/`subject`/`text`/`options`/`attachments` as optional on
/// this endpoint, and this pipeline's body emitter binds every declared body field unconditionally —
/// an unsupplied optional one would travel as an explicit `null` rather than being left out (C-56,
/// still open). So the only body field this connector can declare honestly is the one Front requires
/// unconditionally.
#[test]
fn the_reply_operation_declares_only_the_required_body_field() {
    let connector = front();
    let reply = connector
        .operations
        .iter()
        .find(|operation| operation.id == "front-conversation-reply")
        .expect("the curated set names front-conversation-reply");

    let body_names: Vec<&str> = reply
        .params
        .body
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    assert_eq!(
        body_names,
        ["body"],
        "front-conversation-reply declares a body field beyond the one Front requires \
         unconditionally. Every other field on this endpoint is optional, and C-56 (omit an optional \
         body field) is still open, so declaring one here would send an explicit `null` Front may \
         reject or misread"
    );
}

/// No Front operation declares a query parameter at all beyond a plain bounded integer (`limit`),
/// which needs no percent-encoding because a digit string cannot corrupt a query string. Nothing
/// here percent-encodes a *string* query value (C-30, the standing `zendesk-ticket-search` gap), so a
/// filter, search or free-text query parameter would be unsafe to declare.
#[test]
fn no_front_operation_declares_an_unencodable_query_parameter() {
    let connector = front();
    for operation in &connector.operations {
        for param in &operation.params.query {
            let ty = param.schema.get("type").and_then(|v| v.as_str());
            assert_eq!(
                ty,
                Some("integer"),
                "`{}` declares query parameter `{}` of type {ty:?}. Nothing here percent-encodes a \
                 query value (C-30), so only a digit-only type is safe to interpolate verbatim",
                operation.id,
                param.name
            );
        }
    }
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op — the C-11 gate, held against front specifically.
#[test]
fn every_front_operation_emits_an_analyzable_module() {
    let connector = front();
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
    }
}
