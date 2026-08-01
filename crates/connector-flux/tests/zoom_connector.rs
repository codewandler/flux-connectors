//! The Zoom connector, and the two things that made it different from every other shipped provider:
//! **a meeting's options live in a nested `settings` object**, and **the credential expires**.
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims that are specific to Zoom, because they are the reason
//! C-78 exists and the reason a later reader must not "simplify" the file:
//!
//! - `POST /v2/users/{userId}/meetings` took a meeting's access control inside `settings`, not next
//!   to its topic. A flat `{"waiting_room": true}` is a top-level member Zoom does not define: it is
//!   *ignored*, the meeting is created with whatever the account default is, and the API answers
//!   `201`. That is zendesk's failure mode rather than asana's — a success and a wrong meeting.
//!   **C-430 withheld that operation** (its response carried `start_url`, the host's ZAK token in a
//!   URL), so [`no_body_field_escapes_the_settings_wire_path_rule`] keeps the IR-level rule and can
//!   no longer assert the emitted `$payload` text — see its own comment for what that costs.
//! - The credential is a **server-to-server OAuth access token** with a one-hour life. Minting it is
//!   effectful acquisition, which is C-21's business and the host's, never generated Flux's
//!   (AGENTS.md's authentication contract). [`no_zoom_module_performs_a_token_exchange`] is what
//!   keeps the exchange out of the emitted module, and
//!   [`the_zoom_connector_declares_one_expiring_bearer`] records that nothing declares the expiry.
//!
//! The two negative claims — no query parameter, no optional body field — deliberately duplicate what
//! `asana_connector.rs` and its siblings assert for their own providers. Both are gaps in the emitter
//! rather than in Zoom (C-30 and C-56), so each connector has to hold the line for itself: a shared
//! test would be a line several concurrent stories edit, and the derived per-provider gates cannot
//! express "and this provider chose its operations around that gap".

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider id, and therefore the file name, the module name and every op id's prefix.
const PROVIDER: &str = "zoom";

/// One tenant-independent host for every Zoom account, so `base_url` carries no `{template}` for an
/// operator to bind and the connector's whole egress surface is this single name.
const BASE_URL: &str = "https://api.zoom.us";

/// Every selected endpoint lives under Zoom's one versioned prefix. Asserted so that a later
/// addition cannot quietly address an unversioned path or a `/v1` one.
const API_PREFIX: &str = "/v2/";

/// The credential the connector declares, and the environment variable it resolves from. Both are
/// public contract — an operator sets the variable, a manifest names the credential — so they are
/// pinned here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "zoom.access_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "ZOOM_ACCESS_TOKEN";

/// The curated operations, in the order `providers/zoom.toml` declares them.
///
/// **Two of the four C-78 curated are withheld** — `zoom-meeting-get` and `zoom-meeting-create`,
/// both of which returned `start_url`, a URL embedding the host's ZAK token. The exclusion is
/// recorded and checked in `crates/connector-spec/tests/credential_response.rs`; C-136 is what
/// restores them, and this list grows again in the same commit.
const OPERATIONS: &[&str] = &["zoom-meeting-delete", "zoom-user-get"];

/// The nested object a meeting's options live in, and the one option declared inside it. One
/// constant each because they are asserted from two directions: the `wire` path of the body field,
/// and the emitted payload record.
const SETTINGS: &str = "settings";
/// See [`SETTINGS`]. The meeting's access control — whether somebody holding the join URL enters or
/// waits to be admitted.
const SETTING_FIELD: &str = "waiting_room";

/// The one write left. It is not `low` risk and it is not idempotent: a meeting deleted is something
/// people see disappear from their calendars, and `zoom-meeting-delete` cannot be undone.
/// `zoom-meeting-create` stood beside it until C-430 withheld it.
const WRITES: &[&str] = &["zoom-meeting-delete"];

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
            "cannot read {} ({error}) — C-78 ships the Zoom connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The connector exists, loads, and is the one C-78 specifies: one bearer credential over
/// `api.zoom.us`, with the curated operation set.
///
/// **The `oauth2` assertion is the interesting one.** [`connector_spec::AuthMethod::oauth2`] is the
/// field that would say "the host mints this token", and it is deliberately unset — but for a weaker
/// reason than on the other bearer providers. Their tokens are minted once in a dashboard and do not
/// expire; a Zoom server-to-server token dies in an hour. `OAuth2Spec` cannot express the grant
/// (Zoom's `account_credentials` is not an [`connector_spec::OAuthGrant`] variant, and its required
/// `account_id` has no field), and C-21 — which teaches the host to run a grant at all — is
/// unimplemented. So the token arrives already minted and goes stale, and this asserts the shape
/// that is true today rather than the one that would be nice.
#[test]
fn the_zoom_connector_declares_one_expiring_bearer() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Zoom");
    assert_eq!(
        connector.base_url, BASE_URL,
        "the host is `api.zoom.us` and is never widened"
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "zoom authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("zoom declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "a Zoom server-to-server OAuth access token reaches the wire as \
         `Authorization: Bearer <token>`"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a bearer has no user half"
    );
    assert!(
        method.oauth2.is_none(),
        "zoom declares an `oauth2` acquisition block. It cannot describe Zoom's grant — \
         `account_credentials` is not an `OAuthGrant` variant and its `account_id` has no field — \
         and C-21 is what runs a grant at all. If C-21 has landed, change this test deliberately"
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
            "operation `{}` has {} auth alternatives; zoom is single-mechanism",
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

/// **The headline claim was that a meeting's options travel inside `settings`, and C-430 withheld
/// the only operation that had a body at all.**
///
/// `zoom-meeting-create` carried `wire = "settings.waiting_room"` and was this fleet's one
/// demonstration of a payload root holding leaves *and* a branch — zendesk puts everything under
/// `ticket.` and asana everything under `data.`; babelforce's agent-status update is the nearest
/// remaining mixed root, and both of its fields are optional. It is withheld because its response
/// carried `start_url`, the host's ZAK token in a URL
/// (`crates/connector-spec/tests/credential_response.rs`), so this connector now declares **no body
/// field at all** and the emitted-payload half of the claim has nothing to run against.
///
/// **The rule is kept and the count is what moved.** The loop below still refuses a free-form
/// `body_schema` and still refuses a `wire` path that is not exactly one level inside `settings`, so
/// a body arriving here later — C-136 restoring the create, or any other write — meets the same
/// gate. What it can no longer assert is the emitted nesting, and saying so is the point: an
/// emitter change that stopped assembling the nested record would leave the IR intact and still send
/// Zoom a body whose access-control member sits at the root, where Zoom does not define one. Zoom
/// ignores an undefined top-level member and answers `201`, so the meeting would be created with the
/// *account's* default waiting-room setting and nothing anywhere would report a problem. Nothing in
/// this repository covers that today.
#[test]
fn no_body_field_escapes_the_settings_wire_path_rule() {
    let connector = load();

    let mut nested = 0;
    for operation in &connector.operations {
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form `body_schema`. The nesting is then the caller's \
             problem, and a model that put an option at the root would get a `201` and a meeting \
             with the wrong settings; every zoom body is a field list with explicit wire paths",
            operation.id
        );

        for param in &operation.params.body {
            // A field the vendor takes at the root needs no `wire`; one inside `settings` does, and
            // the only thing that distinguishes them is this declaration.
            let Some(wire) = param.wire.as_deref() else {
                continue;
            };
            let mut segments = wire.split('.');
            assert_eq!(
                segments.next(),
                Some(SETTINGS),
                "operation `{}`: body field `{}` has wire path `{wire}`. The only nested object in \
                 Zoom's meeting body that this connector declares is `{SETTINGS}`",
                operation.id,
                param.name
            );
            let leaf = segments.next();
            assert!(
                leaf.is_some(),
                "operation `{}`: body field `{}` has wire path `{wire}` — the object itself, with \
                 no option inside it",
                operation.id,
                param.name
            );
            assert!(
                segments.next().is_none(),
                "operation `{}`: body field `{}` has wire path `{wire}`; Zoom's meeting settings are \
                 one level deep",
                operation.id,
                param.name
            );
            nested += 1;
        }
    }

    assert_eq!(
        nested, 0,
        "{nested} zoom body fields declare a wire path. C-78 declared exactly one meeting setting, \
         `{SETTINGS}.{SETTING_FIELD}`, on `zoom-meeting-create` — and C-430 withheld that operation \
         because its response carried the host's ZAK token. If a body has legitimately come back, \
         raise this count *and* restore the emitted-payload assertion this test lost with it, which \
         is the half that catches a flattening the IR cannot see"
    );

    assert!(
        connector
            .operations
            .iter()
            .all(|operation| operation.params.body.is_empty()),
        "a zoom operation declares a request body; every one of them was withheld or is a read, so \
         a body arriving here is a new claim that wants the nesting assertion back with it"
    );
}

/// **No optional request-body field, until C-56 lands.**
///
/// An omitted optional body field is not omitted: the emitter binds every declared field into the
/// payload record, so a caller who passes nothing sends an explicit `null`. Zoom's request validator
/// is not documented to accept `null` in place of an absent member, and a connector that sends one
/// is asking the vendor to be lenient about a request the caller never made — which is the worst
/// available failure mode, because it fails *because* the caller did nothing.
///
/// So the surface is required fields only, and what that costs is written down in the story's Notes
/// and in the header comment of `providers/zoom.toml`. This asserts it, because "we left the optional
/// fields out" is exactly the kind of decision a later author undoes as an obvious improvement.
#[test]
fn no_zoom_body_field_is_optional() {
    let connector = load();

    for operation in &connector.operations {
        for param in &operation.params.body {
            assert!(
                param.required,
                "operation `{}`: body field `{}` is optional. An omitted optional field travels as \
                 an explicit `null` (C-56) — declare it required or leave it out",
                operation.id,
                param.name
            );
        }
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
/// satisfy it by picking a narrower type for an injectable value. Zoom's `type` and `page_size` on
/// the meetings list are exactly that trap — an enum and an integer, both of which would survive
/// verbatim interpolation today, on the one endpoint a text filter would later be added to. The
/// excluded surface is named in the story's Notes and in the provider file.
#[test]
fn no_zoom_operation_declares_a_query_parameter() {
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
/// **Every `url = ` line is checked, not just the first, and that is the substance of this test.**
/// The emitter binds `$url` once for the path and the required query parameters, then re-binds it once
/// more per *optional* query parameter inside a `when` guard, with the `?` on a separate `sep`
/// binding — `connectors/zendesk.flux` shows the shape:
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
fn no_zoom_module_assembles_a_query_string() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

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

/// **No generated module performs the token exchange.**
///
/// Zoom's access token is minted by POSTing an account id, a client id and a client secret to
/// `https://zoom.us/oauth/token` with `grant_type=account_credentials`. That is *effectful
/// acquisition*: AGENTS.md's authentication contract puts it on the host's side of the seam, and
/// C-21 is the story that implements it. Emitting it here would put a client secret and a raw
/// bearer token into model-visible Flux symbols, which is the precise failure the three-axis auth
/// model exists to prevent.
///
/// A grep rather than a structural check, because the failure this guards against is text appearing
/// in a generated artifact. It is deliberately spelled out per token so a partial reintroduction —
/// the token host alone, say — fails with the name of what it found.
#[test]
fn no_zoom_module_performs_a_token_exchange() {
    let connector = load();

    // Every spelling of the exchange, lowercased so a change of case cannot slip one through. The
    // base URL is `api.zoom.us`, which contains none of them.
    const FORBIDDEN: &[&str] = &[
        "oauth",
        "grant_type",
        "account_credentials",
        "client_secret",
        "client_id",
        "account_id",
        "refresh",
    ];

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        let haystack = emitted.to_lowercase();
        for needle in FORBIDDEN {
            assert!(
                !haystack.contains(needle),
                "`{}` names `{needle}` in generated Flux. Minting a Zoom access token is effectful \
                 acquisition and belongs to the host (C-21), never to a generated module:\n{emitted}",
                operation.id
            );
        }
    }
}

/// **Neither meeting write is a read.**
///
/// A created meeting occupies a slot on the host's schedule and hands out a join URL; a deleted one
/// is gone, and Zoom offers no undelete. flux's approval gate reads `risk` and `idempotency`, so a
/// `low`/`idempotent` write would be auto-approved and treated as safe to retry — and a retried
/// create is a second meeting.
///
/// `connector-flux`'s `check_write_metadata` already refuses `risk = "low"` on any state-changing
/// method and `idempotency = "idempotent"` on a `POST`. It does **not** refuse `idempotent` on a
/// `DELETE`, because `DELETE` is an idempotent method under RFC 9110 §9.2.2 — so the delete half of
/// this test is the only thing standing between the connector and a `retry` around a cancellation
/// whose first attempt already told the registrants.
#[test]
fn neither_meeting_write_is_low_risk_or_idempotent() {
    let connector = load();

    for id in WRITES {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == *id)
            .unwrap_or_else(|| panic!("zoom declares `{id}`"));
        assert_ne!(
            operation.risk,
            Risk::Low,
            "`{id}` is declared `low` risk; it changes what a person sees on their calendar"
        );
        assert_ne!(
            operation.idempotency,
            Idempotency::Idempotent,
            "`{id}` is declared idempotent; Zoom documents no idempotency key on it, so a retry \
             would either create a second meeting or cancel one twice"
        );
    }

    let delete = connector
        .operations
        .iter()
        .find(|operation| operation.id == "zoom-meeting-delete")
        .expect("zoom declares `zoom-meeting-delete`");
    assert_eq!(
        delete.risk,
        Risk::Destructive,
        "deleting a meeting is irreversible — Zoom offers no undelete and the registrations go with \
         it — which is exactly what `Risk::Destructive` is for"
    );

    // The reads are the other half of the claim: `risk` is set from what an operation does, not
    // uniformly upgraded to keep a gate quiet.
    for operation in &connector.operations {
        if WRITES.contains(&operation.id.as_str()) {
            continue;
        }
        assert_eq!(
            operation.risk,
            Risk::Low,
            "`{}` is a read and is declared {:?}",
            operation.id,
            operation.risk
        );
        assert_eq!(
            operation.idempotency,
            Idempotency::Idempotent,
            "`{}` is a read and is declared {:?}",
            operation.id,
            operation.idempotency
        );
    }
}

/// **The C-11 gate, per operation.** Each one emits, parses with no diagnostics, is already a fixed
/// point of flux's own formatter, and reloads through flux-lang's module loader as exactly one
/// exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set; it is restated here so the Zoom
/// connector's own file fails on its own when the module stops being analyzable. A module that parsed
/// but did not load publishes no ops at all, so a consumer handing it to flux would get silence
/// rather than an error.
#[test]
fn every_zoom_operation_emits_an_analyzable_module() {
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

/// **Every request targets `api.zoom.us` and nothing wider, and carries no credential.**
///
/// `http_hosts` derives from `base_url` (`crates/connector-cli/src/catalog.rs`, `host_of`), so the
/// egress allow-list is exactly as narrow as the string asserted here: no template variable to bind,
/// no second host, no `*`. Checked against the emitted `$base`, because that is what the request is
/// actually built from. Zoom's token endpoint is on `zoom.us` rather than `api.zoom.us`, which is one
/// of the reasons the exchange cannot live here at all — admitting it would widen this allow-list.
///
/// The credential half is AGENTS.md's hard invariant. The connector carries the env-var *name* so a
/// host can resolve it; the emitted module carries neither that name nor a value, because auth
/// injection is C-10 and is deliberately absent rather than stubbed — which is also why this
/// connector cannot yet make a live call.
#[test]
fn every_zoom_request_targets_one_host_and_carries_no_credential() {
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
            "`{}` has path {:?}; every selected Zoom endpoint is under `{API_PREFIX}`",
            operation.id,
            operation.path
        );

        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            emitted.contains(&format!(r#"base = "{BASE_URL}""#)),
            "`{}` does not bind the Zoom base URL:\n{emitted}",
            operation.id
        );
        assert!(
            !emitted.contains(TOKEN_ENV),
            "`{}` names {TOKEN_ENV} in generated Flux:\n{emitted}",
            operation.id
        );
        // A Zoom access token is an opaque `ey…`-prefixed JWT. A literal one in a generated artifact
        // is the failure this invariant exists to prevent, so it is checked for by shape as well as
        // by name.
        assert!(
            !emitted.contains("\"ey"),
            "`{}` embeds something shaped like a Zoom access token:\n{emitted}",
            operation.id
        );
    }
}
