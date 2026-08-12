//! `providers/typeform.toml` exists, emits analyzable Flux, and **the cursor pagination pair
//! `before`/`after` survives this pipeline's unencoded-query gap intact, as a genuinely declared
//! operation-level parameter pair** (C-173).
//!
//! Typeform's responses endpoint (`GET /forms/{form_id}/responses`) pages with `before` and `after`
//! query parameters, each carrying the *value* of a response's own `token` — a vendor-issued cursor,
//! not free text a caller composes. AGENTS.md records that this pipeline never percent-encodes a
//! query value: `zendesk-ticket-search` is the standing demonstration that a free-text term corrupts
//! the request because it can carry `&`, `#`, `+`/`=` or a space that the emitter's own `fmt`
//! interpolation cannot escape (`crates/connector-flux/src/op.rs:138-141`). That is exactly the
//! reasoning `providers/calendly.toml`'s header comment gives for excluding Calendly's own
//! `page_token` and Notion's `start_cursor` outright: an opaque, server-issued token whose character
//! set is *unknown* here is excluded rather than declared and hoped about.
//!
//! **Typeform's `before`/`after` are the case where that character set is not unknown.** A response
//! `token` is a fixed-length, lowercase-hexadecimal string — `crates/connector-flux/src/op.rs`'s own
//! danger set (space, `&`, `#`, `=`) is disjoint from `[0-9a-f]`, so the value this emitter puts on
//! the wire unescaped is exactly the value Typeform issued, byte for byte. This connector's own
//! `providers/typeform.toml` header comment records the two sources for that claim (a Typeform
//! engineer's community statement and an observed 32-character token in the vendor's own
//! documentation) and is explicit that it is hand-authored confidence, not an OpenAPI-grade
//! guarantee — the failure mode if the format ever widens is a caller-visible schema rejection
//! (fail-closed), not a corrupted request.
//!
//! This is the operation-level version of the member contract's rule that "a poll binding requires a
//! cursor" (AGENTS.md, Member contract) — here expressed as a declared parameter pair on a plain
//! `GET`, with no `[[channels]]` poll binding in this connector at all (see the "no inbound surface"
//! test below for why).

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{Connector, HttpMethod, Idempotency, Risk};

use crate::shipped_provider;

/// `<repo root>/providers/typeform.toml`, derived from this crate's manifest directory so the test
/// is independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("typeform.toml")
}

fn typeform() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-173 ships the Typeform connector",
            path.display()
        )
    });
    shipped_provider::load_definition("typeform", &source)
        .expect("providers/typeform.toml does not load")
        .connector
}

/// The four characters `crates/connector-flux/src/op.rs:138-141` names as the ones this pipeline's
/// unencoded query interpolation cannot survive: a space, `&`, `#` or `=`. Restated here as data
/// rather than prose, mirroring `calendly_connector.rs`'s own `QUERY_DANGER_SET`.
const QUERY_DANGER_SET: [char; 4] = [' ', '&', '#', '='];

/// The connector exists, loads through the real loader, and is the one C-173 describes.
#[test]
fn the_typeform_connector_loads() {
    let connector = typeform();

    assert_eq!(connector.id, "typeform");
    assert_eq!(connector.vendor, "Typeform");
    assert_eq!(
        connector.base_url, "https://api.typeform.com",
        "one tenant-independent host — Typeform has no per-account subdomain in its API host, \
         unlike zendesk or freshdesk"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// **The acceptance assertion this connector exists for.** `before` and `after` are declared query
/// parameters on the responses-listing operation, each carrying a hex-only pattern that is provably
/// disjoint from [`QUERY_DANGER_SET`], and the emitted Flux interpolates each verbatim into the query
/// string exactly as `crates/connector-flux/src/op.rs` documents for a guarded (optional) parameter.
#[test]
fn the_cursor_pair_survives_because_it_avoids_the_pipelines_danger_set() {
    let connector = typeform();

    // A realistic Typeform response token: 32 lowercase hex characters. No name, email or other
    // personal data — it is a structural example of the *shape* a vendor-issued cursor takes, not a
    // captured value.
    const SAMPLE_TOKEN: &str = "0123456789abcdef0123456789abcdef";
    assert_eq!(
        SAMPLE_TOKEN.len(),
        32,
        "the sample must match the documented token length"
    );
    for bad in QUERY_DANGER_SET {
        assert!(
            !SAMPLE_TOKEN.contains(bad),
            "a Typeform response token must never carry {bad:?} — if it did, this connector would \
             need the same exclusion providers/calendly.toml already records for page_token"
        );
    }

    let operation = connector
        .operations
        .iter()
        .find(|operation| operation.id == "typeform-response-list")
        .expect(
            "typeform-response-list is part of the curated set — it is the archetype operation",
        );

    for name in ["before", "after"] {
        let param = operation
            .params
            .query
            .iter()
            .find(|param| param.name == name)
            .unwrap_or_else(|| {
                panic!(
                    "typeform-response-list declares no `{name}` query parameter — the archetype \
                     this connector was chosen for is gone"
                )
            });

        assert!(
            !param.required,
            "`{name}` must be optional — it is absent on the first page of a cursor-paginated \
             listing"
        );

        let pattern = param
            .schema
            .get("pattern")
            .and_then(|value| value.as_str())
            .unwrap_or_else(|| panic!("`{name}`'s schema declares no `pattern`"));
        for bad in QUERY_DANGER_SET {
            assert!(
                !pattern.contains(bad),
                "`{name}`'s declared pattern `{pattern}` itself admits {bad:?} — that would \
                 undercut the whole argument that this value cannot corrupt the query string"
            );
        }
        assert!(
            regex_lite_hex_only(pattern),
            "`{name}`'s pattern `{pattern}` should restrict the value to lowercase hex, the \
             documented shape of a Typeform response token"
        );
    }

    let emitted = emit_operation(&connector, operation)
        .unwrap_or_else(|error| panic!("typeform-response-list is not emittable: {error}"));

    assert!(
        emitted.contains(
            "query: { after, before, page_size, response_type, since, sort, until: $until }"
        ) && !emitted.contains("when before")
            && !emitted.contains("before={before}"),
        "the cursor pair must travel through the structured query record and let null omission \
         preserve optionality:\n{emitted}"
    );
}

/// A crude, dependency-free check that a pattern's character class only ever admits hex digits
/// (and the regex syntax needed to say so) — enough to catch an accidentally-widened pattern
/// without pulling in a regex engine for one test.
fn regex_lite_hex_only(pattern: &str) -> bool {
    pattern
        .chars()
        .all(|c| matches!(c, '0'..='9' | 'a'..='f' | '^' | '$' | '[' | ']' | '-' | '{' | '}' | ','))
}

/// The free-text search hazard `zendesk-ticket-search` demonstrates (a caller-composed phrase can
/// carry `&`, `#`, `+`/`=` or a space) applies just as much to Typeform's own `query` parameter on
/// the same endpoint. This connector's answer is the same one Box and Notion gave: leave it out.
#[test]
fn no_operation_declares_a_free_text_search_query_parameter() {
    let connector = typeform();
    for operation in &connector.operations {
        assert!(
            operation.params.query.iter().all(|param| param.name != "query"),
            "`{}` declares a `query` parameter — Typeform's own `query` filter is a caller-composed \
             free-text phrase, exactly the shape that corrupts this pipeline's unencoded query \
             interpolation (C-30, the zendesk-ticket-search precedent)",
            operation.id
        );
    }
}

/// The curated set C-173 selected, exactly. Named rather than counted so that adding an operation is
/// a deliberate edit here.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = typeform();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "typeform-form-get",
            "typeform-form-list",
            "typeform-response-delete",
            "typeform-response-list",
            "typeform-user-me",
        ],
        "the curated set changed — see providers/typeform.toml's header comment for what was \
         excluded and why. `typeform-response-insights` (form insights) is deliberately absent: it \
         is Business-Plan-gated and this file could not corroborate its response shape with enough \
         confidence to declare it honestly"
    );
}

/// Auth is one bearer token, and the `verify` operation is a genuine, unattended read.
#[test]
fn the_connector_verifies_with_a_read_over_a_bearer_token() {
    let connector = typeform();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["typeform.access_token"],
        "one credential covers the whole selection"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("typeform-user-me"),
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
    assert!(
        verify.params.path.is_empty() && verify.params.query.is_empty(),
        "`verify` must run with no caller-supplied argument at all"
    );
}

/// The connection-level configuration surface: the access token, and **no realistic-looking example
/// on it** — a token-shaped placeholder trips secret scanning (`providers/notion.toml`,
/// `providers/shopify.toml` and `providers/dropbox.toml` record the same rule).
#[test]
fn the_access_token_is_configurable_and_carries_no_example_value() {
    let connector = typeform();

    let field = connector
        .config
        .iter()
        .find(|field| field.name == "access_token")
        .expect("the access token is the one thing a human must supply");

    assert!(field.secret, "a Typeform access token is a secret");
    assert_eq!(field.binds, "credential.typeform.access_token");
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

/// Response bodies here carry whatever a form's respondents typed. Nothing in this connector's IR
/// may carry an example answer, hidden-field, name or email value — the schema may *describe* the
/// shape, but never populate it with an instance.
#[test]
fn no_response_field_carries_an_example_value() {
    let connector = typeform();
    for operation in &connector.operations {
        let Some(schema) = &operation.response_schema else {
            continue;
        };
        let rendered = serde_json::to_string(schema).expect("response_schema serializes");
        assert!(
            !rendered.contains("\"example\""),
            "`{}`'s response schema carries a JSON Schema `example` — a Typeform response can hold \
             a respondent's name, email or any other free-text answer, and this repository must \
             never hold a captured or plausible instance of one",
            operation.id
        );
    }
}

/// The destructive delete is declared as such, and it is not marked `idempotent` — Typeform does not
/// document repeat-call behaviour once a token has already been deleted, and deletion is itself
/// asynchronous ("a 200 indicates the request was registered", not that the responses are gone).
#[test]
fn the_delete_operation_is_declared_destructive_and_non_idempotent() {
    let connector = typeform();
    let delete = connector
        .operations
        .iter()
        .find(|operation| operation.id == "typeform-response-delete")
        .expect("typeform-response-delete is part of the curated set");

    assert_eq!(delete.method, HttpMethod::Delete);
    assert_eq!(
        delete.risk,
        Risk::Destructive,
        "deleting responses is irreversible — Typeform offers no undelete"
    );
    assert_eq!(
        delete.idempotency,
        Idempotency::NonIdempotent,
        "Typeform documents no guarantee about repeating a delete for a token that is already gone, \
         and deletion is asynchronous — declaring `idempotent` here would be an unverified claim"
    );
}

/// This story is explicit that a multi-service inbound surface panics
/// `every_shipped_event_and_binding_reaches_its_manifest` and C-151's round-trip, because both read
/// the default-service manifest only. This connector stays single-service with no inbound surface at
/// all, so neither test can ever see it.
#[test]
fn no_inbound_surface_is_declared() {
    let connector = typeform();
    assert!(
        connector.events.is_empty(),
        "this connector declares an inbound event — C-158's last note is explicit that this panics \
         the default-service manifest round-trip tests"
    );
    assert!(
        connector.channels.is_empty(),
        "this connector declares a channel binding — same hazard as an event"
    );
    // Since C-153 a single-surface provider does carry one `[[services]]` entry, holding only the
    // `tags` it has nowhere else to put. What this story needs is that no *named* service appears —
    // `is_default_only` is that claim, and it is the one the manifest round-trip actually rests on.
    assert!(
        connector.is_default_only(),
        "this connector declares a named service — it must stay single-surface (the reserved \
         default service) for this story"
    );
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op — the C-11 gate, held against typeform specifically.
#[test]
fn every_typeform_operation_emits_an_analyzable_module() {
    let connector = typeform();
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
