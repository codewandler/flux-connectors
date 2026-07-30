//! The Asana connector, and the one property that makes it different from every other shipped
//! provider: **every request body and every response is wrapped in a `data` envelope.**
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims that are specific to an enveloped API, because they are
//! the reason C-71 exists and the reason a later reader must not "simplify" the file:
//!
//! - Asana takes `{"data": {…}}` on every write and answers `{"data": …}` on every read. C-29 added
//!   [`connector_spec::Param::wire`] for exactly this shape, and babelforce's `presence.name` was its
//!   motivating case — one field on one operation. Asana is the first shipped vendor where the nesting
//!   is *universal*, so it is the first real proof that wire paths work rather than merely exist.
//! - A connector that flattened the envelope would emit `{"name": …, "workspace": …}`, which Asana
//!   rejects with `400 Missing input: data`. That is a better failure than zendesk's — which
//!   *accepts* a flat body, ignores it and answers 200 — but it is still a connector that cannot
//!   create a task, and nothing else in the repository would notice. Hence
//!   [`every_asana_request_body_is_wrapped_in_the_data_envelope`], asserted over the IR *and* over the
//!   emitted `$payload` text.
//! - The response half is recorded rather than emitted: `http.request` returns one flat string, so no
//!   generated Flux can dig `/data` out (see `connector-flux`'s `description()` for the same reason
//!   an error envelope lands in prose). What a consumer gets is
//!   [`connector_spec::Operation::response_schema`], published verbatim in `web/public/catalog.json`,
//!   and [`every_asana_operation_records_the_response_envelope_at_data`] is what keeps it there.
//!
//! The two negative claims — no query parameter, no optional body field — deliberately duplicate what
//! `github_connector.rs` and `slack_connector.rs` assert for their own providers. Both are gaps in the
//! emitter rather than in Asana (C-30 and C-56), so each connector has to hold the line for itself:
//! a shared test would be a line four concurrent stories edit, and the derived per-provider gates
//! cannot express "and this provider chose its operations around that gap".

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector};

/// The provider id, and therefore the file name, the module name and every op id's prefix.
const PROVIDER: &str = "asana";

/// One tenant-independent host for every workspace, so `base_url` carries no `{template}` for an
/// operator to bind and the connector's whole egress surface is this single name.
const BASE_URL: &str = "https://app.asana.com";

/// Every selected endpoint lives under Asana's one versioned prefix. Asserted so that a later
/// addition cannot quietly address an unversioned or a `/api/2.0` path.
const API_PREFIX: &str = "/api/1.0/";

/// The credential the connector declares, and the environment variable it resolves from. Both are
/// public contract — an operator sets the variable, a manifest names the credential — so they are
/// pinned here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "asana.access_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "ASANA_ACCESS_TOKEN";

/// The five curated operations, in the order `providers/asana.toml` declares them.
const OPERATIONS: &[&str] = &[
    "asana-task-get",
    "asana-task-create",
    "asana-task-update",
    "asana-task-story-add",
    "asana-project-get",
];

/// The envelope key. One constant because it is one fact asserted from three directions: the wire
/// path of every body field, the emitted payload record, and the response schema.
const ENVELOPE: &str = "data";

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
            "cannot read {} ({error}) — C-71 ships the Asana connector",
            path.display()
        )
    });
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The connector exists, loads, and is the one C-71 specifies: a bearer personal access token over
/// `app.asana.com`, with the curated operation set.
#[test]
fn the_asana_connector_loads_and_authenticates_with_a_bearer_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Asana");
    assert_eq!(
        connector.base_url, BASE_URL,
        "the host is `app.asana.com` and is never widened"
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "asana authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("asana declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "Asana takes `Authorization: Bearer <token>` for a personal access token and for an \
         OAuth-issued one alike"
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
            "operation `{}` has {} auth alternatives; asana is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the access token",
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

/// **The headline claim: every body field travels inside `data`.**
///
/// Stated over the IR and over the emitted text, because they fail differently. A body field that
/// lost its `wire` path is an IR-level flattening; an emitter change that stopped assembling the
/// nested record would leave the IR intact and still send Asana a body it answers `400 Missing input:
/// data` to. Both are the same bug from a caller's point of view — the connector cannot write — so
/// both are checked.
#[test]
fn every_asana_request_body_is_wrapped_in_the_data_envelope() {
    let connector = load();

    let mut with_body = 0;
    for operation in &connector.operations {
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form `body_schema`. The envelope is then the caller's \
             problem, and a model that omitted it would produce a body Asana rejects; every Asana \
             body is a field list with `data.`-prefixed wire paths",
            operation.id
        );
        if operation.params.body.is_empty() {
            continue;
        }
        with_body += 1;

        for param in &operation.params.body {
            let wire = param.wire.as_deref().unwrap_or_else(|| {
                panic!(
                    "operation `{}`: body field `{}` declares no `wire` path, so it is emitted at \
                     the root of the body. Asana takes every write under `data` and rejects a flat \
                     body with `400 Missing input: data`",
                    operation.id, param.name
                )
            });
            let mut segments = wire.split('.');
            assert_eq!(
                segments.next(),
                Some(ENVELOPE),
                "operation `{}`: body field `{}` has wire path `{wire}`, which is outside the \
                 `{ENVELOPE}` envelope",
                operation.id,
                param.name
            );
            assert!(
                segments.next().is_some(),
                "operation `{}`: body field `{}` has wire path `{wire}` — the envelope itself, with \
                 no field inside it",
                operation.id,
                param.name
            );
        }

        // The emitted half. `$payload = { data: { … } }` is what a nested body looks like; a
        // flattened one would bind `$payload = { name: $name, … }`, which parses, analyzes and is
        // canonical, so nothing but this assertion would fail.
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            emitted.contains(&format!("$payload = {{ {ENVELOPE}: {{")),
            "`{}` does not wrap its payload in `{ENVELOPE}`:\n{emitted}",
            operation.id
        );
    }

    assert!(
        with_body >= 3,
        "only {with_body} asana operations send a body, so the envelope claim above is nearly \
         vacuous; C-71 ships three writes"
    );
}

/// **No optional request-body field, until C-56 lands.**
///
/// An omitted optional body field is not omitted: the emitter binds every declared field into the
/// payload record, so a caller who passes nothing sends an explicit `null`. Asana type-checks the
/// members of `data` and rejects `{"data": {"notes": null}}` — a connector that offered the field
/// would therefore fail *because* the caller left it alone, which is the worst available failure
/// mode.
///
/// So the surface is required fields only, and what that costs is written down in the story's Notes
/// and in the header comment of `providers/asana.toml`. This asserts it, because "we left the
/// optional fields out" is exactly the kind of decision a later author undoes as an obvious
/// improvement.
#[test]
fn no_asana_body_field_is_optional() {
    let connector = load();

    for operation in &connector.operations {
        for param in &operation.params.body {
            assert!(
                param.required,
                "operation `{}`: body field `{}` is optional. An omitted optional field travels as \
                 an explicit `null` (C-56), which Asana rejects — declare it required or leave it \
                 out",
                operation.id,
                param.name
            );
        }
    }
}

/// **The response envelope is recorded, so a consumer knows the payload is at `/data`.**
///
/// It cannot be *handled*: `http.request` returns one flat string, so no emitted Flux can extract a
/// pointer out of it (`connector-flux`'s `description()` records the same seam for error envelopes).
/// What the connector can do is publish the shape, which `web/public/catalog.json` carries verbatim
/// and the explorer renders. A consumer reading `data` off the response is then following the
/// contract rather than guessing from an example.
#[test]
fn every_asana_operation_records_the_response_envelope_at_data() {
    let connector = load();

    for operation in &connector.operations {
        let schema = operation.response_schema.as_ref().unwrap_or_else(|| {
            panic!(
                "operation `{}` records no response schema, so nothing tells a consumer the payload \
                 is at `/{ENVELOPE}`",
                operation.id
            )
        });
        assert_eq!(
            schema["type"], "object",
            "operation `{}`: the Asana response envelope is a JSON object",
            operation.id
        );
        assert!(
            schema["properties"][ENVELOPE].is_object(),
            "operation `{}`: the response schema does not describe `{ENVELOPE}`: {schema}",
            operation.id
        );
        let required = schema["required"].as_array().unwrap_or_else(|| {
            panic!("operation `{}`: the envelope key is required", operation.id)
        });
        assert!(
            required.iter().any(|key| key == ENVELOPE),
            "operation `{}`: `{ENVELOPE}` is not required, so the schema permits a response with no \
             payload: {schema}",
            operation.id
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
/// satisfy it by picking a narrower type for an injectable value. Asana's excluded surface is
/// `opt_fields`, `limit`/`offset` paging and workspace search, all named in the story's Notes.
#[test]
fn no_asana_operation_declares_a_query_parameter() {
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
/// **Every `$url = ` line is checked, not just the first, and that is the substance of this test.**
/// The emitter binds `$url` once for the path and the required query parameters, then re-binds it once
/// more per *optional* query parameter inside a `when` guard, with the `?` on a separate `$sep`
/// binding — `connectors/zendesk.flux` shows the shape:
///
/// ```flux
/// $url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
/// $sep = "?"
/// when $page
///   $url = fmt("{url}{sep}page={page}")
/// ```
///
/// So inspecting only the first binding would pass while an operation quietly appended optional
/// filters, and a check for `?` alone would miss it too.
#[test]
fn no_asana_module_assembles_a_query_string() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("$url = "))
            .collect();
        assert_eq!(
            url_lines.len(),
            1,
            "`{}` binds $url {} times; the emitter does that once for the path and once per optional \
             query parameter, so anything but one binding means a query string:\n{emitted}",
            operation.id,
            url_lines.len()
        );
        assert!(
            !url_lines[0].contains('?'),
            "`{}` emits a query string: {}",
            operation.id,
            url_lines[0]
        );
        assert!(
            !emitted.contains("$sep"),
            "`{}` emits the `$sep` query separator, which exists only to join query parameters:\n\
             {emitted}",
            operation.id
        );
    }
}

/// **The C-11 gate, per operation.** Each one emits, parses with no diagnostics, is already a fixed
/// point of flux's own formatter, and reloads through flux-lang's module loader as exactly one
/// exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set; it is restated here so the Asana
/// connector's own file fails on its own when the module stops being analyzable. A module that parsed
/// but did not load publishes no ops at all, so a consumer handing it to flux would get silence
/// rather than an error.
#[test]
fn every_asana_operation_emits_an_analyzable_module() {
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

/// **Every request targets `app.asana.com` and nothing wider, and carries no credential.**
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
fn every_asana_request_targets_one_host_and_carries_no_credential() {
    let connector = load();
    assert!(
        !connector.base_url.contains('{'),
        "`base_url` must be a bound literal, not a template: {:?}",
        connector.base_url
    );
    assert!(
        !connector.base_url.contains('*'),
        "`base_url` must name one host, never a wildcard: {:?}",
        connector.base_url
    );

    for operation in &connector.operations {
        assert!(
            operation.path.starts_with(API_PREFIX),
            "`{}` has path {:?}; every selected Asana endpoint is under `{API_PREFIX}`",
            operation.id,
            operation.path
        );

        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            emitted.contains(&format!(r#"$base = "{BASE_URL}""#)),
            "`{}` does not bind the Asana base URL:\n{emitted}",
            operation.id
        );
        assert!(
            !emitted.contains(TOKEN_ENV),
            "`{}` names {TOKEN_ENV} in generated Flux:\n{emitted}",
            operation.id
        );
        // Asana personal access tokens are `1/`-prefixed numeric strings, and its OAuth tokens are
        // `1/`-prefixed too. A literal one in a generated artifact is the failure this invariant
        // exists to prevent, so it is checked for by shape as well as by name.
        assert!(
            !emitted.contains("\"1/"),
            "`{}` embeds something shaped like an Asana access token:\n{emitted}",
            operation.id
        );
    }
}
