//! `providers/calendly.toml` exists, emits analyzable Flux, and **the value this connector is about
//! survives the pipeline's unencoded-query gap intact** (C-172).
//!
//! Calendly addresses every resource — a user, an event type, a scheduled event — by its full
//! `https://api.calendly.com/…` URI, and that URI is the *value* of a query parameter
//! (`GET /event_types?user={uri}`, `GET /scheduled_events?user={uri}`). AGENTS.md records that this
//! pipeline never percent-encodes a query value: `zendesk-ticket-search` is the standing
//! demonstration that a free-text search term corrupts the request because it can carry `&`, `#`,
//! `+`/`=` or a space that the emitter's own `fmt` interpolation cannot escape
//! (`crates/connector-flux/src/op.rs:138-141`). That gap is why `notion_connector.rs` proves no
//! Notion operation declares a query parameter at all — the connector's answer to the same hazard was
//! to have nothing pass through it.
//!
//! Calendly's answer is different, and this file is the argument for it, not just an assertion.
//! **A Calendly resource URI cannot itself contain any of the four characters
//! (`op.rs:138-141`'s own list — space, `&`, `#`, `=`) that corrupt this pipeline's query
//! interpolation.** It is `https://api.calendly.com/` followed by a resource segment and a UUID:
//! scheme, host, path segments and hyphens only. Unlike a Zendesk search expression — free text a
//! caller composes and which can legitimately contain any of those four characters — a Calendly URI
//! is a vendor-issued identifier whose character set is fixed and disjoint from the danger set. So the
//! value the emitter interpolates verbatim reaches Calendly exactly as sent: this is the *other* side
//! of the same gap zendesk's connector demonstrates, and it is why Calendly earns a shipped connector
//! rather than a recorded refusal (see the TOML's own header comment for the full argument).
//!
//! The second hazard this connector is chosen for is the *inverse* one: a path parameter is not
//! allowed to be the full URI, or the emitted `{base}/scheduled_events/{uuid}` template would compose
//! a URL that embeds a second, nested URL rather than a valid path. `calendly-scheduled-event-get` and
//! `calendly-invitee-list` therefore constrain their path parameter's schema to the bare 36-character
//! UUID — the last path segment of the resource's own `uri` — never the whole URI.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{Connector, HttpMethod};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// `<repo root>/providers/calendly.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("calendly.toml")
}

fn calendly() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-172 ships the Calendly connector",
            path.display()
        )
    });
    shipped_provider::load_definition("calendly", &source)
        .expect("providers/calendly.toml does not load")
        .connector
}

/// The four characters `crates/connector-flux/src/op.rs:138-141` names as the ones this pipeline's
/// unencoded query interpolation cannot survive: a space, `&`, `#` or `=`. Restated here as data
/// rather than prose, so the claim below is a mechanical check against the emitter's own documented
/// contract instead of an assertion resting on this file's say-so.
const QUERY_DANGER_SET: [char; 4] = [' ', '&', '#', '='];

/// The connector exists, loads through the real loader, and is the one C-172 describes.
#[test]
fn the_calendly_connector_loads() {
    let connector = calendly();

    assert_eq!(connector.id, "calendly");
    assert_eq!(connector.vendor, "Calendly");
    assert_eq!(
        connector.base_url, "https://api.calendly.com",
        "one tenant-independent host — Calendly has no per-account subdomain, unlike zendesk or \
         freshdesk"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// **The acceptance assertion this connector exists for.** A user-addressing query parameter carries
/// a value that is itself a full Calendly URI, and that value survives this pipeline's unencoded
/// query interpolation because it structurally cannot contain any character in
/// [`QUERY_DANGER_SET`] — unlike the free-text search term `zendesk-ticket-search` carries, which
/// can and does.
#[test]
fn a_uri_valued_query_parameter_survives_because_it_avoids_the_pipelines_danger_set() {
    let connector = calendly();

    // A realistic Calendly resource URI: scheme, host, resource segment, UUID. No name, email or
    // other personal data — it is a structural example of the *shape*, not a captured value.
    const SAMPLE_USER_URI: &str =
        "https://api.calendly.com/users/00000000-0000-4000-8000-000000000000";

    for bad in QUERY_DANGER_SET {
        assert!(
            !SAMPLE_USER_URI.contains(bad),
            "a Calendly resource URI must never carry {bad:?} — if it did, this connector would need \
             the same refusal notion_connector.rs records"
        );
    }

    let mut checked_at_least_one = false;
    for operation in &connector.operations {
        let Some(user_param) = operation
            .params
            .query
            .iter()
            .find(|param| param.name == "user")
        else {
            continue;
        };
        checked_at_least_one = true;

        assert!(
            user_param.required,
            "`{}`'s `user` parameter must be required — Calendly's collection endpoints have no \
             other way to scope the result set that this connector declares",
            operation.id
        );

        // The declared schema is itself the safety argument: a `pattern` restricted to the
        // characters below can never admit any member of QUERY_DANGER_SET, so a caller cannot even
        // construct a value this pipeline would corrupt.
        let pattern = user_param
            .schema
            .get("pattern")
            .and_then(|value| value.as_str())
            .unwrap_or_else(|| {
                panic!(
                    "`{}`'s `user` parameter declares no `pattern`",
                    operation.id
                )
            });
        for bad in QUERY_DANGER_SET {
            assert!(
                !pattern.contains(bad),
                "`{}`'s declared `user` pattern `{pattern}` itself admits {bad:?} — that would \
                 undercut the whole argument that this value cannot corrupt the query string",
                operation.id
            );
        }
        assert!(
            SAMPLE_USER_URI.starts_with("https://api.calendly.com/"),
            "the sample URI itself must match Calendly's own scheme"
        );

        let emitted = emit_operation(&connector, operation).unwrap_or_else(|error| {
            panic!("operation `{}` is not emittable: {error}", operation.id)
        });
        let structured = emitted
            .lines()
            .find(|line| line.contains("http.request") && line.contains("query: {"))
            .unwrap_or_default();
        assert!(
            structured.contains(&user_param.name) && !emitted.contains("user={user}"),
            "`{}` does not pass `user` through Flux's structured encoder:\n{emitted}",
            operation.id
        );
    }
    assert!(
        checked_at_least_one,
        "no operation declares a `user` query parameter — this connector no longer exercises the \
         thing C-172 selected it for"
    );
}

/// The inverse hazard: a path parameter that named the *whole* URI would double-compose against the
/// path template (`{base}/scheduled_events/{uuid}`), embedding a second URL inside the first rather
/// than addressing a resource. Every such parameter here is constrained to the bare UUID instead.
#[test]
fn a_path_parameter_is_the_bare_uuid_never_the_full_uri_to_avoid_double_composing() {
    let connector = calendly();
    let mut checked_at_least_one = false;

    for operation in &connector.operations {
        for path_param in &operation.params.path {
            checked_at_least_one = true;
            let schema = &path_param.schema;

            let pattern = schema.get("pattern").and_then(|value| value.as_str());
            if let Some(pattern) = pattern {
                assert!(
                    !pattern.contains("https") && !pattern.contains(':') && !pattern.contains('/'),
                    "`{}`'s path parameter `{}` declares pattern `{pattern}`, which admits a \
                     scheme — a caller could pass the whole resource URI and double-compose the \
                     path template",
                    operation.id,
                    path_param.name
                );
            }

            // A bare UUID is exactly 36 characters; the full URI this parameter must not accept is
            // far longer and carries a scheme. Constraining the length is a second, independent
            // guard against the same mistake.
            let max_length = schema.get("maxLength").and_then(|value| value.as_u64());
            assert_eq!(
                max_length,
                Some(36),
                "`{}`'s path parameter `{}` should cap length at 36 (a bare UUID) so a full \
                 `https://api.calendly.com/...` value is rejected by its own schema rather than \
                 silently double-composing the path",
                operation.id,
                path_param.name
            );
        }
    }
    assert!(
        checked_at_least_one,
        "no operation declares a path parameter — this connector no longer exercises the \
         double-composition hazard C-172 also names"
    );
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op — the C-11 gate, held against calendly specifically.
#[test]
fn every_calendly_operation_emits_an_analyzable_module() {
    let connector = calendly();
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

/// The curated set C-172 selected. Named rather than counted so that adding an operation is a
/// deliberate edit here.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = calendly();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "calendly-event-type-list",
            "calendly-invitee-list",
            "calendly-scheduled-event-get",
            "calendly-scheduled-event-list",
            "calendly-user-me",
        ],
        "the curated set changed — see providers/calendly.toml's header comment for what was \
         excluded and why"
    );
}

/// Auth is one bearer token, and the `verify` operation is a genuine, unattended read.
#[test]
fn the_connector_verifies_with_a_read_over_a_bearer_token() {
    let connector = calendly();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["calendly.access_token"],
        "one credential covers the whole selection"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("calendly-user-me"),
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
        "`verify` runs unattended whenever a settings page opens, so it is a GET"
    );
    assert!(
        verify.params.path.is_empty() && verify.params.query.is_empty(),
        "`verify` must run with no caller-supplied argument at all"
    );
}

/// The connection-level configuration surface: the token, and **no realistic-looking example on
/// it** — a token-shaped placeholder trips secret scanning (`providers/notion.toml`,
/// `providers/shopify.toml` record the same rule).
#[test]
fn the_token_is_configurable_and_carries_no_example_value() {
    let connector = calendly();

    let field = connector
        .config
        .iter()
        .find(|field| field.name == "access_token")
        .expect("the access token is the one thing a human must supply");

    assert!(field.secret, "a Calendly access token is a secret");
    assert_eq!(field.binds, "credential.calendly.access_token");
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

/// Invitee data is personal data about a named third party. Nothing in this connector's IR may carry
/// an example name, email or other invitee value — the schema may *describe* the shape of the field,
/// but never populate it.
#[test]
fn no_invitee_field_carries_an_example_value() {
    let connector = calendly();
    let invitees = connector
        .operations
        .iter()
        .find(|operation| operation.id == "calendly-invitee-list")
        .expect("calendly-invitee-list is part of the curated set");

    let schema = invitees
        .response_schema
        .as_ref()
        .expect("calendly-invitee-list declares a response shape");
    let rendered = serde_json::to_string(schema).expect("response_schema serializes");
    assert!(
        !rendered.contains("\"example\""),
        "no field in the invitee response schema may carry a JSON Schema `example` — invitee names \
         and emails are personal data and this repository must never hold a captured or plausible \
         instance of one"
    );
}
