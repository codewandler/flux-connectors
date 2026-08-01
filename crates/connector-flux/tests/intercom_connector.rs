//! The Intercom connector (C-73), and the three properties that decide whether it is honest.
//!
//! `shipped_modules.rs` next door already asserts, for every provider in `providers/`, that each
//! operation emits Flux which parses, analyzes and is canonical under flux's own formatter. This file
//! exists for the claims that are specific to *this* connector and that nothing else in the
//! repository would notice being broken:
//!
//! 1. **No query parameter, anywhere.** The emitter interpolates a query value into the URL with
//!    `fmt` and percent-encodes nothing, because flux registers no URL-encoding op
//!    (`crates/connector-flux/src/op.rs`, C-30 unimplemented). `zendesk-ticket-search` is the
//!    standing demonstration AGENTS.md lists under *Intentional gaps*: a value carrying `&` injects a
//!    parameter rather than filtering by one. Intercom's search surface (`POST /conversations/search`
//!    is fine, but `GET /conversations?display_as=plaintext` and every `per_page`/`starting_after`
//!    page is not) is exactly the shape that trips it, so C-73 ships the path-and-body surface only.
//!    The assertion is the **strong** form — zero query parameters, not zero string-ish ones —
//!    because "empty" survives editing whereas "string-ish" invites a later author to add an
//!    `integer` page bound and rebuild the query-assembly path this connector avoids.
//! 2. **No optional request-body field.** `body_tree` inserts every declared body field
//!    unconditionally (`op.rs`), so an optional field the caller omits travels as an explicit
//!    `{"field": null}` — C-56. Intercom's contact model is full of nullable attributes, and a `null`
//!    `name` on a contact update is not the same request as an absent one. Until C-56 lands, every
//!    declared body field is required.
//! 3. **No caller-supplied header, and in particular no `Intercom-Version` parameter.** Intercom's
//!    version header is *optional* — the workspace's pinned version applies when it is absent — which
//!    is precisely why this provider could ship while Notion and Anthropic, which **require** theirs,
//!    could not. What must not happen is smuggling it in as a `params.header` entry: C-52 established
//!    that a `const`-pinned header is emitted as a required, caller-*overridable* argument with the
//!    `const` silently dropped, which is a declaration in disguise. Pinning the version waits for
//!    C-55; this test is what stops the disguise from landing in the meantime.
//!
//! Every claim is checked over the IR **and** over the emitted text where the two can disagree,
//! because the IR is what an author reads and the emitted module is what flux runs.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, HttpMethod};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider under test. Named once so the file reads as being about Intercom rather than about a
/// string.
const PROVIDER: &str = "intercom";

/// The credential the connector declares, and the environment variable it resolves from. Both are
/// public contract — an operator sets the variable, a manifest names the credential — so they are
/// pinned here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "intercom.access_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "INTERCOM_ACCESS_TOKEN";

/// The five curated operations, in the order `providers/intercom.toml` declares them.
///
/// This names one provider's operations rather than the shipped-provider set, so it is the curated
/// inventory claim C-54 deliberately kept, not the duplicated provider list it removed.
const OPERATIONS: &[&str] = &[
    "intercom-contact-get",
    "intercom-contact-create",
    "intercom-conversation-get",
    "intercom-conversation-reply",
    "intercom-contact-note-create",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that has drifted from what ships.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-73 ships the Intercom connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The connector exists, loads through the real loader, and is the one C-73 specifies: a bearer
/// access token over `api.intercom.io`, with the curated operation set.
#[test]
fn the_intercom_connector_loads_and_authenticates_with_a_bearer_access_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Intercom");
    // One tenant-independent host, so unlike zendesk there is no `{subdomain}` for an operator to
    // bind and the connector already has a valid destination URL. `http_hosts` derives from this one
    // value and is therefore exactly `api.intercom.io`, never widened.
    assert_eq!(
        connector.base_url, "https://api.intercom.io",
        "the host is `api.intercom.io` and is never widened"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "intercom authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("intercom declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "Intercom takes its access token as `Authorization: Bearer <token>`"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a bearer has no user half"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    // Every operation resolves to the one bearer, whether it declares auth or inherits the default.
    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; intercom is single-mechanism",
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

/// **The honesty assertion: no operation declares a query parameter.** See the module documentation
/// for why this is the strong form rather than a check on the parameter's type.
#[test]
fn no_intercom_operation_declares_a_query_parameter() {
    let connector = load();

    for operation in &connector.operations {
        let declared: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        assert!(
            declared.is_empty(),
            "operation `{}` declares query parameters {declared:?}. Nothing percent-encodes a query \
             value (C-30 is unimplemented), so a value carrying `&` or `#` corrupts the request or \
             injects a parameter — the `zendesk-ticket-search` failure AGENTS.md records. C-73 ships \
             the path-and-body surface only; if C-30 has landed, change this test deliberately",
            operation.id
        );
    }
}

/// The same claim over the **emitted text**, which is what flux actually loads — so an emitter that
/// synthesised a query parameter from somewhere other than `params.query` could not slip past.
///
/// **Every `url = ` line is checked, not just the first, and that is the substance of this test.**
/// The emitter binds `$url` once for the path and required query parameters, then re-binds it once
/// more per *optional* query parameter inside a `when` guard (`op.rs`, the `optional` loop);
/// `connectors/zendesk.flux` shows the shape:
///
/// ```flux
/// url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
/// sep = "?"
/// when $page
///   url = fmt("{url}{sep}page={page}")
/// ```
///
/// The `?` lives on `sep` and the parameter name lands on a *later* `$url` line than the first, so
/// inspecting only the first binding would pass while an operation quietly appended optional filters.
#[test]
fn no_intercom_module_assembles_a_query_string() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        assert!(
            !url_lines.is_empty(),
            "`{}` binds no $url:\n{emitted}",
            operation.id
        );
        // One binding and one only: a second is the emitter appending an optional query parameter.
        assert_eq!(
            url_lines.len(),
            1,
            "`{}` re-binds $url {} times; the emitter does that once per optional query parameter, \
             so this operation is appending a query string:\n{emitted}",
            operation.id,
            url_lines.len()
        );
        for line in &url_lines {
            assert!(
                !line.contains('?'),
                "`{}` emits a query string: {line}",
                operation.id
            );
        }
        // `sep` exists only to carry the `?`/`&` between query parameters, so an operation that
        // binds it is building a query string even if no single line spells the `?`.
        assert!(
            !emitted
                .lines()
                .any(|line| line.trim_start().starts_with("sep = ")),
            "`{}` binds `sep`, which the emitter emits only to separate query parameters:\n{emitted}",
            operation.id
        );
    }
}

/// **No optional request-body field, until C-56 lands.**
///
/// `body_tree` inserts every declared body field unconditionally, so an optional field the caller
/// leaves out is sent as an explicit `null`. That is not the same request as an absent field, and on
/// Intercom's contact and conversation payloads a `null` attribute reads as "clear this" rather than
/// "leave it alone". So every declared field is required, and what was dropped for this reason is
/// named in the story's Notes rather than shipped as a latent null.
#[test]
fn no_intercom_operation_declares_an_optional_body_field() {
    let connector = load();

    for operation in &connector.operations {
        let optional: Vec<&str> = operation
            .params
            .body
            .iter()
            .filter(|param| !param.required)
            .map(|param| param.name.as_str())
            .collect();
        assert!(
            optional.is_empty(),
            "operation `{}` declares optional body fields {optional:?}. An unsupplied optional body \
             field is emitted as an explicit `null` rather than omitted (C-56, \
             `crates/connector-flux/src/op.rs` `body_tree`), which Intercom reads as clearing the \
             attribute. Declare it required or leave it out until C-56 lands",
            operation.id
        );
    }
}

/// **No caller-supplied header — and `Intercom-Version` in particular is not smuggled in as one.**
///
/// The version header is genuinely optional for Intercom: an absent header means the workspace's
/// pinned version, so the connector is well-formed without it. What is *not* acceptable is declaring
/// it as a `params.header` entry with a `const`, because `op.rs` filters `constant(...)` out of the
/// declared parameter list for `body` params only — a `const`-pinned header is emitted as a required
/// argument every caller must pass and any caller may set to anything, with the constraint dropped
/// (C-52's finding, C-55's story). Pinning the version has to wait for C-55; until then the
/// limitation is recorded in prose and nothing pretends otherwise.
///
/// The Authorization header is the host's business either way — it is injected at the `$auth` seam
/// and must never travel through the parameter surface.
#[test]
fn no_intercom_operation_declares_a_header_parameter() {
    let connector = load();

    for operation in &connector.operations {
        let declared: Vec<&str> = operation
            .params
            .header
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        assert!(
            declared.is_empty(),
            "operation `{}` declares header parameters {declared:?}. A `const`-pinned header is \
             still emitted as a caller-overridable argument (C-52), so declaring `Intercom-Version` \
             this way would be a disguise rather than a version pin — that waits for C-55. The \
             Authorization header is injected by the host and never a parameter",
            operation.id
        );
    }

    // The emitted text, since a header could in principle reach the request from somewhere other
    // than `params.header`. `content-type` is the one constant the emitter hard-codes for a JSON
    // body, so it is the only header a shipped Intercom module may carry.
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        let lowered = emitted.to_lowercase();
        assert!(
            !lowered.contains("intercom-version") && !lowered.contains("intercom_version"),
            "`{}` emits an Intercom-Version header; the version cannot be pinned until C-55 and a \
             caller-supplied one is not a pin:\n{emitted}",
            operation.id
        );
    }
}

/// Every write's `risk` reflects what it changes and **who sees it**, and no write claims an
/// idempotency the vendor does not document.
///
/// The one operation the customer sees is `intercom-conversation-reply`: a `comment` reply is
/// delivered to the end user by email or in-app message and cannot be un-sent, which is the "a
/// reviewer would want to see this first" tier. Creating a contact or an internal note changes
/// workspace state that only teammates read, so both sit at `medium`. Nothing here is `idempotent`:
/// Intercom publishes no idempotency key on these endpoints, so replaying a create makes a second
/// object.
#[test]
fn every_intercom_write_declares_honest_risk_and_idempotency() {
    use connector_spec::{Idempotency, Risk};

    let connector = load();

    for operation in &connector.operations {
        let mutates = matches!(
            operation.method,
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete
        );
        if !mutates {
            continue;
        }
        assert_ne!(
            operation.risk,
            Risk::Low,
            "write `{}` declares `low` risk",
            operation.id
        );
        assert_eq!(
            operation.idempotency,
            Idempotency::NonIdempotent,
            "write `{}` claims idempotency Intercom does not document; it publishes no idempotency \
             key on these endpoints, so a replay creates a second object",
            operation.id
        );
    }

    let reply = connector
        .operations
        .iter()
        .find(|operation| operation.id == "intercom-conversation-reply")
        .expect("the connector declares `intercom-conversation-reply`");
    assert_eq!(
        reply.risk,
        Risk::High,
        "a `comment` reply is delivered to the end user and cannot be un-sent, so it is the one \
         operation here that must be `high` — flux's approval gate reads this field"
    );
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical
/// under flux's own formatter, and **loads** as exactly one exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set. It is restated here so the
/// Intercom connector's own test file fails on its own when the module stops being analyzable — a
/// provider whose emitted Flux does not load publishes no ops at all, and a consumer handing it to
/// flux would get silence rather than an error.
#[test]
fn every_intercom_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
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
