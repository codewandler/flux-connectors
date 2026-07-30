//! The Slack connector, and the one property that makes it different from every other shipped
//! provider: **it puts nothing in a query string.**
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims that are specific to a *method-style* API, because they
//! are the reason C-53 exists and the reason a later reader must not "tidy" the file:
//!
//! - Slack's Web API is addressed as `POST /api/<method>` with a JSON body, **including its reads**.
//!   That is not a stylistic preference. `channel` and `user` are opaque strings (`C0123ABCDEF`,
//!   `U0123ABCDEF`), and the emitter interpolates a query value into the URL with no
//!   percent-encoding whatsoever (`crates/connector-flux/src/op.rs`, C-30). A read expressed as
//!   `GET /api/conversations.history?channel=…` would therefore ship the same unencodable-value
//!   defect `zendesk-ticket-search` carries. Expressed as a POST body it cannot.
//! - So the strongest available statement of "this connector has no encoding gap" is not "its query
//!   values are numeric" but **"it declares no query parameter at all"**, and that is what is
//!   asserted here — over the IR *and* over the emitted text, since a query parameter that reached
//!   the URL would show up as the emitter's `$sep` separator machinery.
//!
//! Asserting it rather than trusting it is the point: nothing else in the repository would fail if
//! someone converted a read to a GET, and the resulting connector would look tidier and be broken.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod};

/// The provider under test. Named once so the file reads as being about Slack rather than about a
/// string.
const PROVIDER: &str = "slack";

/// The credential the connector declares, and the environment variable it resolves from. Both are
/// public contract — an operator sets the variable and a manifest names the credential — so they are
/// pinned here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "slack.bot_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "SLACK_BOT_TOKEN";

/// The four curated operations, in the order `providers/slack.toml` declares them.
const OPERATIONS: &[&str] = &[
    "slack-chat-post-message",
    "slack-conversations-history",
    "slack-users-info",
    "slack-reactions-add",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The connector exists, loads, and is the one the story specifies: a bearer bot token over
/// `slack.com`, with the curated operation set.
#[test]
fn the_slack_connector_loads_and_authenticates_with_a_bearer_bot_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Slack");
    // No tenant templating: Slack is one host for every workspace, so unlike zendesk there is no
    // `{subdomain}` for an operator to bind and the connector has a valid destination URL as it
    // stands.
    assert_eq!(connector.base_url, "https://slack.com");

    // Two credentials, and each needs its own reason — that was this assertion's point when it read
    // `1`, and it keeps it by naming them rather than by counting looser. They travel in opposite
    // directions: the bot token authenticates every outgoing call, and the signing secret verifies
    // incoming Events API requests and is never sent anywhere.
    let declared: Vec<(&str, &AuthScheme)> = connector
        .auth
        .iter()
        .map(|method| (method.name.as_str(), &method.scheme))
        .collect();
    assert_eq!(
        declared,
        vec![
            (CREDENTIAL, &AuthScheme::Bearer),
            ("slack.signing_secret", &AuthScheme::Signing),
        ],
        "slack declares one outbound credential and one inbound one; a third would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("slack declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "a JSON body is only accepted when the token travels as a bearer in the Authorization \
         header, so the scheme is what makes the POST+JSON shape legal"
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
            "operation `{}` has {} auth alternatives; slack is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the bot token",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares caller-supplied headers; the Authorization header is injected \
             by the host and must never travel through the parameter surface",
            operation.id
        );
    }
}

/// **Every operation is a POST with a JSON body, including the reads.** See the module docs for why
/// this is a correctness property and not a style: a read expressed as a GET would have to carry an
/// opaque channel or user id in a query string the emitter cannot encode.
#[test]
fn every_slack_operation_is_a_post_with_a_json_body() {
    let connector = load();

    for operation in &connector.operations {
        assert_eq!(
            operation.method,
            HttpMethod::Post,
            "operation `{}` is a {:?}; Slack's Web API is addressed as `POST /api/<method>` and a \
             GET would put an opaque id into an unencodable query string (C-30)",
            operation.id,
            operation.method
        );
        assert!(
            operation.path.starts_with("/api/"),
            "operation `{}` has path `{}`; every Slack Web API method lives under `/api/`",
            operation.id,
            operation.path
        );
        assert!(
            !operation.params.body.is_empty(),
            "operation `{}` is a POST with no body fields, so its arguments have nowhere to travel",
            operation.id
        );
        // A method-style API takes no path parameters at all: the method name *is* the path.
        assert!(
            operation.params.path.is_empty(),
            "operation `{}` declares path parameters; a Slack method name carries no placeholders",
            operation.id
        );
    }
}

/// **The load-bearing assertion: no query parameters at all.**
///
/// The story's requirement is that no operation declares a string-ish or `Any`-typed query
/// parameter. This asserts the stronger and simpler property that implies it — the query surface is
/// empty — because "empty" is a claim that stays true under editing, whereas "string-ish" invites a
/// later author to add an `integer` filter and rebuild the query-assembly path this connector exists
/// to avoid.
#[test]
fn no_slack_operation_declares_a_query_parameter() {
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
            "operation `{}` declares query parameters {declared:?}. Slack takes its arguments in a \
             JSON body; a query value would be interpolated into the URL with no percent-encoding \
             (C-30), which is exactly the defect this connector is shaped to avoid",
            operation.id
        );
    }
}

/// The same claim over the **emitted text**, which is what flux actually loads.
///
/// The IR assertion above and this one are not the same check. The emitter assembles a query string
/// from `params.query` alone, binding `$sep` to carry the `?`/`&` separator; if that machinery
/// appears in a Slack module, something put a value in the URL regardless of what the IR looked like
/// when the previous test ran.
#[test]
fn no_slack_module_assembles_a_query_string() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            !emitted.contains("$sep"),
            "`{}` emits query-string separator machinery, so a value is reaching the URL \
             unencoded:\n{emitted}",
            operation.id
        );
        assert!(
            !emitted.contains('?'),
            "`{}` emits a `?` in its request URL:\n{emitted}",
            operation.id
        );
        // The positive half: the body really is sent as JSON, so the arguments the query string is
        // not carrying are carried somewhere.
        assert!(
            emitted.contains(r#"$content_type = "application/json""#),
            "`{}` sends no JSON body:\n{emitted}",
            operation.id
        );
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical,
/// and **loads** as exactly one exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set. It is restated here so that the
/// Slack connector's own test file fails on its own when the module stops being analyzable — a
/// provider whose emitted Flux does not load publishes no ops at all, and a consumer handing it to
/// flux would get silence rather than an error.
#[test]
fn every_slack_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
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
