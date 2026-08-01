//! The Sentry connector, and the one property that makes it different from every other shipped
//! provider: **every path ends in a trailing slash, and the slash is load-bearing.**
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims that are specific to Sentry, because they are the reason
//! C-77 exists and the reason a later reader must not "tidy" the file:
//!
//! - Sentry's REST API is Django REST Framework with `APPEND_SLASH` behavior, and the two halves of
//!   that behavior differ per endpoint. A `GET` without the slash is answered with a `301` to the
//!   slashed form — which an HTTP client may or may not follow, and which drops the method and the
//!   body when it does — and a `PUT` without it is answered `404`. So the connector's writes fail
//!   outright and its reads fail *silently* by turning into a second, unauthenticated request on
//!   some clients. Neither failure is visible anywhere else in this repository: a path without the
//!   slash loads, validates, emits, parses, analyzes and is canonical.
//! - Hence [`the_emitted_url_of_every_operation_is_pinned_including_its_trailing_slash`], which pins
//!   the whole `$url` line of every operation verbatim rather than asserting a property of it. A
//!   property test ("the path ends in `/`") is satisfied by a path that lost a segment; a pinned line
//!   is not, and the pin is what a "normalize the paths" commit has to argue with.
//!
//! The two negative claims — no query parameter, no optional body field — deliberately duplicate what
//! `asana_connector.rs`, `github_connector.rs` and `slack_connector.rs` assert for their own
//! providers. Both are gaps in the emitter rather than in Sentry (C-30 and C-56), so each connector
//! has to hold the line for itself: a shared test would be a line several concurrent stories edit,
//! and the derived per-provider gates cannot express "and this provider chose its operations around
//! that gap".

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider id, and therefore the file name, the module name and every op id's prefix.
const PROVIDER: &str = "sentry";

/// Sentry's SaaS host. Self-hosted Sentry lives somewhere else entirely, which is C-68's subject and
/// deliberately not modelled here — see the story's Notes and the header of `providers/sentry.toml`.
const BASE_URL: &str = "https://sentry.io";

/// Sentry's one version prefix. Asserted so a later addition cannot quietly address an unversioned
/// path or an internal `/api/0/internal/` one.
const API_PREFIX: &str = "/api/0/";

/// The credential the connector declares, and the environment variable it resolves from. Both are
/// public contract — an operator sets the variable, a manifest names the credential — so they are
/// pinned here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "sentry.auth_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "SENTRY_AUTH_TOKEN";

/// The write, named once because three tests make a claim about it: its `risk`, its `idempotency`,
/// and the single required body field C-56 leaves it with.
const ISSUE_UPDATE: &str = "sentry-issue-update";

/// **The four curated operations and the exact `$url` line each one emits.**
///
/// This is the story's trailing-slash gate, and it is a pinned string rather than a property for the
/// reason the module docs give: `"{base}/api/0/organizations/{organization_id_or_slug}/issues/"`
/// ends in a slash too, and addresses the issue *list* instead of one issue.
///
/// The order is the order `providers/sentry.toml` declares them, which is also the order the emitted
/// module and the catalogue publish.
const URLS: &[(&str, &str)] = &[
    (
        "sentry-issue-get",
        r#"url = fmt("{base}/api/0/organizations/{organization_id_or_slug}/issues/{issue_id}/")"#,
    ),
    (
        ISSUE_UPDATE,
        r#"url = fmt("{base}/api/0/organizations/{organization_id_or_slug}/issues/{issue_id}/")"#,
    ),
    (
        "sentry-project-get",
        r#"url = fmt("{base}/api/0/projects/{organization_id_or_slug}/{project_id_or_slug}/")"#,
    ),
    (
        "sentry-issue-event-latest",
        r#"url = fmt("{base}/api/0/organizations/{organization_id_or_slug}/issues/{issue_id}/events/latest/")"#,
    ),
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
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-77 ships the Sentry connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The emitted module for one operation, looked up by id so a test can name the operation it means.
fn emit(connector: &Connector, id: &str) -> String {
    let operation = connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("providers/{PROVIDER}.toml declares no operation `{id}`"));
    emit_operation(connector, operation)
        .unwrap_or_else(|error| panic!("`{id}` does not emit: {error}"))
}

/// The connector exists, loads, and is the one C-77 specifies: a bearer auth token over `sentry.io`,
/// with the curated operation set.
#[test]
fn the_sentry_connector_loads_and_authenticates_with_a_bearer_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Sentry");
    assert_eq!(
        connector.base_url, BASE_URL,
        "the host is `sentry.io` and is never widened; a self-hosted install is C-68's subject"
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "sentry authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("sentry declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "Sentry takes `Authorization: Bearer <token>` for an organization auth token, a user auth \
         token and an internal-integration token alike"
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
    let expected: Vec<&str> = URLS.iter().map(|(id, _)| *id).collect();
    assert_eq!(declared, expected);

    // Every operation resolves to the one bearer, whether it declares auth or inherits the default.
    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; sentry is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the auth token",
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

/// **The headline claim: the emitted URL of every operation, pinned character for character.**
///
/// Sentry's paths end in `/` and the slash is part of the address rather than decoration. Dropping it
/// fails differently per endpoint and neither failure is loud:
///
/// - `GET /api/0/organizations/{org}/issues/{id}` is answered `301` to the slashed form. A client that
///   does not follow redirects returns the `301` body; one that does follow it may reissue the request
///   without the `Authorization` header, and the connector then reports a 401 for a call that was
///   correct apart from a character.
/// - `PUT /api/0/organizations/{org}/issues/{id}` is answered `404`, because DRF's `APPEND_SLASH`
///   redirect applies to safe methods only. The write simply does not happen.
///
/// Nothing else in this repository would catch either: a path with the slash removed loads, validates,
/// emits, parses, analyzes, is a formatter fixed point, and produces a byte-identical second build.
///
/// Pinning the *whole line* rather than asserting `ends_with('/')` is the point. The property form
/// passes for `…/issues/`, which is the issue *list* endpoint, and for `…/issues/{issue_id}/events/`,
/// which is a paginated event list rather than the latest event — both plausible outcomes of an edit
/// that meant no harm.
#[test]
fn the_emitted_url_of_every_operation_is_pinned_including_its_trailing_slash() {
    let connector = load();

    for (id, expected) in URLS {
        let emitted = emit(&connector, id);

        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        assert_eq!(
            url_lines,
            vec![*expected],
            "`{id}` does not emit the URL C-77 pins. Sentry's trailing slash is part of the address: \
             a `GET` without it is a 301 that can lose the Authorization header and a `PUT` without \
             it is a 404. Nothing else in this repository fails when it is dropped.\n{emitted}"
        );
    }

    // The IR half of the same fact, so a reader of `providers/sentry.toml` is not left to infer the
    // rule from four pinned strings. `{base}` is trimmed of a trailing slash by the emitter, so the
    // path is the only place the slash can live.
    for operation in &connector.operations {
        assert!(
            operation.path.starts_with(API_PREFIX),
            "`{}` has path {:?}; every selected Sentry endpoint is under `{API_PREFIX}`",
            operation.id,
            operation.path
        );
        assert!(
            operation.path.ends_with('/'),
            "`{}` has path {:?}, which does not end in Sentry's trailing slash",
            operation.id,
            operation.path
        );
    }
}

/// **No query parameter at all** — the strong form, on the IR.
///
/// The story's requirement is that no operation declares a string-ish or `Any`-typed query parameter.
/// This asserts the stronger and simpler property that implies it: the query surface is empty.
/// Nothing in this pipeline percent-encodes a query value — the emitter interpolates it verbatim into
/// a `fmt` template and flux registers no encoding op (C-30) — and `zendesk-ticket-search` is the
/// standing demonstration that AGENTS.md lists under *Intentional gaps*.
///
/// "Empty" also survives editing in a way "no string-ish value" does not: a later author cannot
/// satisfy it by picking a narrower type for an injectable value. Sentry's excluded surface is the
/// issues *list* and its `query` parameter — Sentry's own search syntax, and the most injectable value
/// the API exposes — named in the story's Notes.
#[test]
fn no_sentry_operation_declares_a_query_parameter() {
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
             value (C-30), so a value carrying `&` or `#` corrupts the request or injects a \
             parameter — the `zendesk-ticket-search` failure AGENTS.md records. If C-30 has landed, \
             change this test deliberately",
            operation.id
        );
    }
}

/// The same claim over the **emitted text**, which is what flux actually loads.
///
/// **Every `url = ` line is checked, not just the first, and so is the `sep` binding.** The emitter
/// binds `$url` once for the path and the required query parameters, then re-binds it once more per
/// *optional* query parameter inside a `when` guard, with the `?` on a separate `sep` binding —
/// `connectors/zendesk.flux` shows the shape:
///
/// ```flux
/// url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
/// sep = "?"
/// when $page
///   url = fmt("{url}{sep}page={page}")
/// ```
///
/// So inspecting only the first binding would pass while an operation quietly appended optional
/// filters, and a check for `?` alone would miss it too.
#[test]
fn no_sentry_module_assembles_a_query_string() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit(&connector, &operation.id);

        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        assert_eq!(
            url_lines.len(),
            1,
            "`{}` binds $url {} times; the emitter does that once for the path and once per optional \
             query parameter, so anything but one binding means a query string:\n{emitted}",
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
        assert!(
            !emitted.contains("sep = "),
            "`{}` emits the `sep` query separator, which exists only to join query parameters:\n\
             {emitted}",
            operation.id
        );
    }
}

/// **No optional request-body field, until C-56 lands.**
///
/// An omitted optional body field is not omitted: the emitter binds every declared field into the
/// payload record, so a caller who passes nothing sends an explicit `null`. On Sentry's issue update
/// that is worse than a rejection — `assignedTo: null` *unassigns* the issue and `isSubscribed: null`
/// is a validation error, so an agent that set only the status would silently clear fields it never
/// named.
///
/// So the surface is required fields only, and what that costs is written down in the story's Notes
/// and in the header comment of `providers/sentry.toml`. This asserts it, because "we left the
/// optional fields out" is exactly the kind of decision a later author undoes as an obvious
/// improvement.
#[test]
fn no_sentry_body_field_is_optional() {
    let connector = load();

    let mut with_body = 0;
    for operation in &connector.operations {
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form `body_schema`, so which fields travel is the \
             caller's guess. Sentry's issue update clears a field that arrives as `null`; every \
             body here is an explicit field list",
            operation.id
        );
        if !operation.params.body.is_empty() {
            with_body += 1;
        }
        for param in &operation.params.body {
            assert!(
                param.required,
                "operation `{}`: body field `{}` is optional. An omitted optional field travels as \
                 an explicit `null` (C-56), which Sentry either rejects or applies — declare it \
                 required or leave it out",
                operation.id,
                param.name
            );
        }
    }

    assert_eq!(
        with_body, 1,
        "C-77 ships exactly one write, so this test is only as strong as that operation's body"
    );
}

/// **Issue update changes triage state a team relies on, and says so in `risk`.**
///
/// Sentry's unresolved queue *is* the team's inbox, and its alerting is driven by issue status: an
/// issue moved to `ignored` stops notifying anyone, so an agent that ignored a live production error
/// would have silenced it for everybody without a human seeing the decision. `high` — "writes a
/// reviewer would want to see first" — is what flux's approval gate reads, and `medium` would let
/// that happen unattended.
///
/// It is **not** idempotent. `PUT` is idempotent under RFC 9110 §9.2.2 and setting a status twice
/// plainly lands the same state, but Sentry documents no idempotency guarantee for the transition —
/// it records each change in the issue's activity feed, and `resolvedInNextRelease` resolves against
/// whichever release is next at the time the call is made, so "the same effect as making it once" is
/// not a property of the endpoint. The story's Acceptance is explicit that no write is idempotent
/// unless the vendor documents it as such: the cost is a forgone retry, the cost of the other error
/// is a replayed write.
#[test]
fn the_issue_update_declares_the_triage_risk_and_claims_no_idempotency() {
    let connector = load();

    let writes: Vec<&connector_spec::Operation> = connector
        .operations
        .iter()
        .filter(|operation| operation.method != HttpMethod::Get)
        .collect();
    let write_ids: Vec<&str> = writes.iter().map(|op| op.id.as_str()).collect();
    assert_eq!(
        write_ids,
        vec![ISSUE_UPDATE],
        "C-77's only write is `{ISSUE_UPDATE}`; a new one needs its own `risk` reviewed here"
    );

    let update = writes[0];
    assert_eq!(update.method, HttpMethod::Put);
    assert_eq!(
        update.risk,
        Risk::High,
        "`{ISSUE_UPDATE}` moves an issue in and out of the team's unresolved queue and, for \
         `ignored`, stops it alerting anyone. That is the tier a reviewer sees first"
    );
    assert_eq!(
        update.idempotency,
        Idempotency::NonIdempotent,
        "Sentry documents no idempotency guarantee for the status transition, so `idempotent` would \
         be a claim with nothing behind it"
    );
    assert!(
        update
            .description
            .to_lowercase()
            .contains("triage"),
        "`{ISSUE_UPDATE}`'s description is the tool contract a model reads before calling it; it has \
         to say that this changes triage state: {:?}",
        update.description
    );

    // Every read is a read: `low` and `idempotent`, so the write above stands out rather than being
    // one setting among four indistinguishable ones.
    for operation in &connector.operations {
        if operation.id == ISSUE_UPDATE {
            continue;
        }
        assert_eq!(operation.method, HttpMethod::Get);
        assert_eq!(operation.risk, Risk::Low);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
    }
}

/// **The C-11 gate, per operation.** Each one emits, parses with no diagnostics, is already a fixed
/// point of flux's own formatter, and reloads through flux-lang's module loader as exactly one
/// exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set; it is restated here so the Sentry
/// connector's own file fails on its own when the module stops being analyzable. A module that parsed
/// but did not load publishes no ops at all, so a consumer handing it to flux would get silence
/// rather than an error.
#[test]
fn every_sentry_operation_emits_an_analyzable_module() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit(&connector, &operation.id);

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

/// **Every request targets `sentry.io` and nothing wider, and carries no credential.**
///
/// `http_hosts` derives from `base_url` (`crates/connector-cli/src/catalog.rs`, `host_of`), so the
/// egress allow-list is exactly as narrow as the string asserted here: no template variable to bind,
/// no second host, no `*`. Checked against the emitted `$base`, because that is what the request is
/// actually built from.
///
/// The credential half is AGENTS.md's hard invariant. The connector carries the env-var *name* so a
/// host can resolve it; the emitted module carries neither that name nor a value, because auth
/// injection is C-10 and is deliberately absent rather than stubbed — which is also why this
/// connector cannot yet make a live call.
#[test]
fn every_sentry_request_targets_one_host_and_carries_no_credential() {
    let connector = load();
    assert!(
        !connector.base_url.contains('{'),
        "`base_url` must be a bound literal, not a template: {:?}",
        connector.base_url
    );
    assert!(
        !connector.base_url.contains('*'),
        "a wildcard in the base URL would widen the derived egress allow-list to every host: {:?}",
        connector.base_url
    );

    for operation in &connector.operations {
        let emitted = emit(&connector, &operation.id);
        assert!(
            emitted.contains(&format!(r#"base = "{BASE_URL}""#)),
            "`{}` does not bind the Sentry base URL:\n{emitted}",
            operation.id
        );
        assert!(
            !emitted.contains(TOKEN_ENV),
            "`{}` names {TOKEN_ENV} in generated Flux:\n{emitted}",
            operation.id
        );
        // Sentry auth tokens are hex strings, and the ones minted since 2023 carry a `sntry`
        // prefix. A literal one in a generated artifact is the failure this invariant exists to
        // prevent, so it is checked for by shape as well as by name.
        assert!(
            !emitted.contains("sntry"),
            "`{}` embeds something shaped like a Sentry auth token:\n{emitted}",
            operation.id
        );
    }
}
