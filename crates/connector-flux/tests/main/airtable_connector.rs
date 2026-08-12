//! The Airtable connector, and the two claims C-75 ships it for.
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds what is specific to Airtable, because these are the reasons the
//! story exists and the reasons a later reader must not "simplify" them away.
//!
//! **1. The `fields` envelope.** Airtable wraps a record's cell values in `{"fields": {…}}` on every
//! write and answers with the same wrapper on every read. A flat body is rejected — Airtable answers
//! `422 INVALID_REQUEST_UNKNOWN` to an unknown top-level key — so the envelope is a correctness
//! contract rather than a formatting preference. It is declared with C-29's
//! [`connector_spec::Param::wire`], the same mechanism `providers/asana.toml` uses for its `data`
//! envelope, and [`every_airtable_request_body_is_wrapped_in_the_fields_envelope`] asserts it on the
//! IR *and* on the emitted `$payload`, because those two can disagree: a field that lost its `wire`
//! path is an IR-level flattening, while an emitter that stopped assembling the nested record would
//! leave the IR intact and still send a body Airtable rejects.
//!
//! Airtable's envelope differs from Asana's in one way that shapes the whole declaration: the keys
//! *inside* `fields` are the table's own column names, chosen by whoever built the base. No provider
//! file can enumerate them, so the envelope is declared as the wire path of a **single opaque
//! object** (`cell_values` → `fields`) rather than as a `fields.` prefix on a list of typed fields.
//! That is the honest shape, and it is what [`ENVELOPE_BINDING`] pins.
//!
//! **2. The path charset, argued rather than assumed.** A path value is interpolated into the URL as
//! verbatim as a query value is — C-30 is unimplemented and `status.rs` deliberately scopes its
//! refusal to query parameters — so the safety of `/v0/{baseId}/{tableIdOrName}/{recordId}` rests
//! entirely on what those values can contain. Airtable ids are a three-letter prefix followed by
//! alphanumerics (`appXXXXXXXXXXXXXX`, `tblXXXXXXXXXXXXXX`, `recXXXXXXXXXXXXXX`), and
//! percent-encoding is the identity function on that charset.
//!
//! Airtable also routes the second segment on a table **name**, which is *not* safe by that
//! argument: a table called `Tasks Backlog` would put a raw space in the request line, and one called
//! `A/B tests` would address a different resource entirely. **This connector requires the id form**,
//! and [`every_airtable_path_value_is_alphanumeric_by_declaration`] is what makes that mechanical
//! rather than prose: it takes each declared `pattern` apart and checks that the only characters it
//! admits are `[A-Za-z0-9]`. `openai-model-get` was safe because OpenAI's model ids happen to have a
//! safe charset; this is the first place in the repository where the question is answered by a
//! declaration a test can read.
//!
//! The two negative claims — no query parameter, no optional body field — deliberately duplicate what
//! `asana_connector.rs`, `github_connector.rs` and `slack_connector.rs` assert for their own
//! providers. Both are gaps in the emitter rather than in Airtable (C-30 and C-56), so each connector
//! has to hold the line for itself: a shared test would be a line five concurrent stories edit, and
//! the derived per-provider gates cannot express "and this provider chose its operations around that
//! gap".

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, Idempotency, Risk};

use crate::shipped_provider;

/// The provider id, and therefore the file name, the module name and every op id's prefix.
const PROVIDER: &str = "airtable";

/// One tenant-independent host for every base, so `base_url` carries no `{template}` for an operator
/// to bind and the connector's whole egress surface is this single name.
const BASE_URL: &str = "https://api.airtable.com";

/// Every selected endpoint lives under Airtable's one versioned prefix. Asserted so that a later
/// addition cannot quietly address an unversioned path or the metadata API's `/v0/meta` tree.
const API_PREFIX: &str = "/v0/";

/// The credential the connector declares, and the environment variable it resolves from. Both are
/// public contract — an operator sets the variable, a manifest names the credential — so they are
/// pinned here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "airtable.access_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "AIRTABLE_ACCESS_TOKEN";

/// The four curated operations, in the order `providers/airtable.toml` declares them.
const OPERATIONS: &[&str] = &[
    "airtable-record-get",
    "airtable-record-create",
    "airtable-record-update",
    "airtable-record-delete",
];

/// The envelope key: the one JSON object every Airtable record's cell values live in, on the request
/// and on the response.
const ENVELOPE: &str = "fields";

/// The exact `$payload` binding a correctly enveloped write emits.
///
/// Pinned as one string rather than probed for `fields` somewhere in the text, because the failure
/// this guards against — `payload = $cell_values`, the flattened form — also contains the word
/// `fields` in the parameter's description and in the op's doc comment.
const ENVELOPE_BINDING: &str = "payload = { fields: cell_values }";

/// The caller-facing name of the object that travels inside the envelope. Distinct from
/// [`ENVELOPE`]: Airtable calls the members of `fields` *cell values*, and keeping the two names
/// apart is what makes the `wire` path carry real information instead of restating the parameter's
/// own name.
const ENVELOPE_PARAM: &str = "cell_values";

/// Every path parameter, as `(caller-facing name, vendor placeholder, id prefix)`.
///
/// The vendor spelling is the middle column because Airtable documents its URL as
/// `/v0/{baseId}/{tableIdOrName}/{recordId}` and that is what the connector must route on, while the
/// caller-facing name follows this repository's snake_case convention — the `wire` alias Jira's
/// `issue_key` → `issueIdOrKey` established.
///
/// **`table_id`, not `table_id_or_name`, is the deliberate part.** The vendor accepts either form and
/// only one of them is encodable here; naming the parameter after the id is the connector saying so
/// in the signature a model reads, and the `pattern` behind the third column is what enforces it.
const PATH_PARAMS: &[(&str, &str, &str)] = &[
    ("base_id", "baseId", "app"),
    ("table_id", "tableIdOrName", "tbl"),
    ("record_id", "recordId", "rec"),
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
            "cannot read {} ({error}) — C-75 ships the Airtable connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The connector exists, loads, and is the one C-75 specifies: a bearer personal access token over
/// `api.airtable.com`, with the curated operation set.
#[test]
fn the_airtable_connector_loads_and_authenticates_with_a_bearer_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Airtable");
    assert_eq!(
        connector.base_url, BASE_URL,
        "the host is `api.airtable.com` and is never widened"
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "airtable authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("airtable declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "Airtable takes `Authorization: Bearer <token>` for a personal access token and for an \
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
            "operation `{}` has {} auth alternatives; airtable is single-mechanism",
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

/// **The first headline claim: every body field travels inside `fields`.**
///
/// Stated over the IR and over the emitted text, because they fail differently and both failures look
/// the same to a caller: the connector cannot write. Airtable answers a flat body
/// `422 INVALID_REQUEST_UNKNOWN` — a better failure than zendesk's silent 200, and still a connector
/// that cannot create a record.
#[test]
fn every_airtable_request_body_is_wrapped_in_the_fields_envelope() {
    let connector = load();

    let mut with_body = 0;
    for operation in &connector.operations {
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form `body_schema`. The envelope is then the caller's \
             problem, and a model that omitted it would produce a body Airtable rejects; the \
             envelope is declared with a `wire` path instead",
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
                     the root of the body. Airtable takes every record write under `{ENVELOPE}` and \
                     rejects an unknown top-level key with `422 INVALID_REQUEST_UNKNOWN`",
                    operation.id, param.name
                )
            });
            assert_eq!(
                wire.split('.').next(),
                Some(ENVELOPE),
                "operation `{}`: body field `{}` has wire path `{wire}`, which is outside the \
                 `{ENVELOPE}` envelope",
                operation.id,
                param.name
            );
        }

        // The emitted half. A flattened body would bind `payload = $cell_values`, which parses,
        // analyzes and is canonical, so nothing but this assertion would fail.
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            emitted.contains(ENVELOPE_BINDING),
            "`{}` does not bind `{ENVELOPE_BINDING}`, so its payload is not wrapped in \
             `{ENVELOPE}`:\n{emitted}",
            operation.id
        );
    }

    assert_eq!(
        with_body, 2,
        "{with_body} airtable operations send a body; C-75 ships two writes that do (create and \
         update) and two that do not (get and delete)"
    );
}

/// **The envelope carries one opaque object, and that is the shape rather than a shortcut.**
///
/// The keys inside `fields` are the table's own column names, so no provider file can enumerate them
/// and the alternative — a `fields.Name`, `fields.Status`, … list — would hard-code one base's schema
/// into a connector every base shares. What the connector can declare is that the whole object goes
/// under `fields`, which is what this asserts: exactly one body field, at wire path `fields`, typed as
/// an object.
#[test]
fn the_envelope_carries_one_opaque_cell_value_object() {
    let connector = load();

    for operation in &connector.operations {
        if operation.params.body.is_empty() {
            continue;
        }
        assert_eq!(
            operation.params.body.len(),
            1,
            "operation `{}` declares {} body fields. Airtable's writable surface is the one \
             `{ENVELOPE}` object; a second field would be a top-level key, and Airtable rejects an \
             unknown one with `422 INVALID_REQUEST_UNKNOWN`",
            operation.id,
            operation.params.body.len()
        );
        let param = &operation.params.body[0];
        assert_eq!(param.name, ENVELOPE_PARAM);
        assert_eq!(param.wire.as_deref(), Some(ENVELOPE));
        assert_eq!(
            param.schema["type"], "object",
            "operation `{}`: `{ENVELOPE_PARAM}` is a map from column name to cell value, so its \
             schema is an object: {}",
            operation.id, param.schema
        );
    }
}

/// **No optional request-body field, until C-56 lands.**
///
/// An omitted optional body field is not omitted: the emitter binds every declared field into the
/// payload record, so a caller who passes nothing sends an explicit `null`. Airtable rejects
/// `{"fields": null}` outright, and it rejects a `null` for `typecast` too, so a connector that
/// offered either would fail *because* the caller left it alone — the worst available failure mode.
///
/// What that costs is written down in the story's Notes and in the header comment of
/// `providers/airtable.toml`: `typecast` and `returnFieldsByFieldId`. This asserts the decision,
/// because "we left the optional fields out" is exactly the kind of thing a later author undoes as an
/// obvious improvement.
#[test]
fn no_airtable_body_field_is_optional() {
    let connector = load();

    for operation in &connector.operations {
        for param in &operation.params.body {
            assert!(
                param.required,
                "operation `{}`: body field `{}` is optional. An omitted optional field travels as \
                 an explicit `null` (C-56), which Airtable rejects — declare it required or leave \
                 it out",
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
/// standing demonstration AGENTS.md lists under *Intentional gaps*.
///
/// "Empty" also survives editing in a way "no string-ish value" does not: a later author cannot
/// satisfy it by picking a narrower type for an injectable value. Airtable's excluded surface is
/// `listRecords` with `filterByFormula`, `view`, `sort` and `pageSize`/`offset`, plus the
/// `cellFormat`, `timeZone`, `userLocale` and `returnFieldsByFieldId` read options — all named in the
/// story's Notes.
#[test]
fn no_airtable_operation_declares_a_query_parameter() {
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
             parameter — the `zendesk-ticket-search` failure AGENTS.md records. A `filterByFormula` \
             would be the worst case in this fleet. If C-30 has landed, change this test \
             deliberately",
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
fn no_airtable_module_assembles_a_query_string() {
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

/// **The second headline claim: every path value is alphanumeric by declaration, so interpolating it
/// unencoded is safe.**
///
/// A path value reaches the URL as verbatim as a query value does — C-30 scopes its refusal to query
/// parameters and `status.rs` deliberately does not widen it — so `/v0/{baseId}/{tableIdOrName}` is
/// safe only if those values cannot carry a character that changes the shape of the request. This
/// takes each declared `pattern` apart and checks exactly that, mechanically:
///
/// `^` + an alphanumeric literal prefix + `[A-Za-z0-9]+` + `$`
///
/// Every value such a pattern admits is alphanumeric, and percent-encoding is the identity function on
/// alphanumerics. The check is structural rather than a set of example strings because an example set
/// proves only that the examples were thought of; taking the pattern apart proves there is no
/// admissible value left over.
///
/// **The table segment is the case this story exists to settle.** Airtable routes it on an id *or* a
/// table name, and a name is arbitrary user text: `Tasks Backlog` puts a raw space in the request
/// line, and `A/B tests` addresses `/v0/appX/A/B%20tests` — a different route. The name form is
/// therefore not offered at all, and the `tbl` prefix required below is what refuses it: no table
/// name can satisfy the pattern by accident, and a caller who passes one gets a value the contract
/// says is invalid rather than a corrupted request.
#[test]
fn every_airtable_path_value_is_alphanumeric_by_declaration() {
    let connector = load();

    let mut seen: Vec<&str> = Vec::new();
    for operation in &connector.operations {
        for param in &operation.params.path {
            let (_, placeholder, prefix) = PATH_PARAMS
                .iter()
                .find(|(name, _, _)| *name == param.name)
                .unwrap_or_else(|| {
                    panic!(
                        "operation `{}` declares path parameter `{}`, which C-75 does not curate. A \
                         new path segment needs its own charset argument before it can be \
                         interpolated unencoded",
                        operation.id, param.name
                    )
                });
            seen.push(param.name.as_str());

            assert_eq!(
                param.wire.as_deref(),
                Some(*placeholder),
                "operation `{}`: path parameter `{}` must alias Airtable's own `{{{placeholder}}}` \
                 placeholder",
                operation.id,
                param.name
            );
            assert!(
                operation.path.contains(&format!("{{{placeholder}}}")),
                "operation `{}` has path {:?}, which does not route on `{{{placeholder}}}`",
                operation.id,
                operation.path
            );

            let pattern = param.schema["pattern"].as_str().unwrap_or_else(|| {
                panic!(
                    "operation `{}`: path parameter `{}` declares no `pattern`, so nothing says its \
                     value is safe to interpolate unencoded (C-30): {}",
                    operation.id, param.name, param.schema
                )
            });
            let expected = format!("^{prefix}[A-Za-z0-9]+$");
            assert_eq!(
                pattern, expected,
                "operation `{}`: path parameter `{}` declares pattern `{pattern}`. C-75's safety \
                 argument is that the only admissible characters are `[A-Za-z0-9]`, on which \
                 percent-encoding is the identity; any other pattern needs its own argument",
                operation.id, param.name
            );
            // The argument itself, spelled out rather than trusted to the constant above: the
            // literal prefix is alphanumeric, and the only character class is `[A-Za-z0-9]`.
            assert!(
                prefix.chars().all(|c| c.is_ascii_alphanumeric()),
                "the `{}` prefix `{prefix}` is not alphanumeric, so the pattern admits a value that \
                 needs encoding",
                param.name
            );
            assert_eq!(
                pattern
                    .trim_start_matches('^')
                    .trim_end_matches('$')
                    .strip_prefix(prefix),
                Some("[A-Za-z0-9]+"),
                "operation `{}`: path parameter `{}` admits more than `[A-Za-z0-9]` after its \
                 prefix",
                operation.id,
                param.name
            );
        }
    }

    assert!(
        seen.contains(&"table_id"),
        "no operation declares `table_id`, so the table-name question this story settles is not \
         actually exercised"
    );
    assert!(
        !seen.contains(&"table_id_or_name"),
        "the table segment is offered in its name form. A table name is arbitrary user text and is \
         interpolated unencoded (C-30); only the `tbl…` id form is safe"
    );
}

/// **Every write says what it changes, and none claims to be idempotent.**
///
/// flux's approval gate reads `risk`, so a delete declaring anything below `destructive` would be
/// waved through unattended — and an Airtable record delete is not recoverable through the API. The
/// `idempotency` half is the story's rule: Airtable documents no idempotency key and no idempotency
/// guarantee on any record endpoint, so `idempotent` would be a claim with nothing behind it even
/// where the call happens to be repeatable.
#[test]
fn every_airtable_write_declares_its_blast_radius() {
    let connector = load();

    for operation in &connector.operations {
        let write = !matches!(operation.method, connector_spec::HttpMethod::Get);
        if !write {
            assert_eq!(
                operation.risk,
                Risk::Low,
                "`{}` is a read; anything above `low` would make flux ask for approval to look at a \
                 record",
                operation.id
            );
            continue;
        }
        assert_ne!(
            operation.idempotency,
            Idempotency::Idempotent,
            "`{}` claims idempotency Airtable does not document. C-75 is explicit that no write is \
             marked idempotent unless the vendor documents it as such",
            operation.id
        );
    }

    let delete = connector
        .operations
        .iter()
        .find(|operation| operation.id == "airtable-record-delete")
        .expect("C-75 ships a record delete");
    assert_eq!(
        delete.risk,
        Risk::Destructive,
        "a deleted Airtable record cannot be restored through the API, and flux's approval gate \
         reads this field"
    );
}

/// **The response envelope is recorded, so a consumer knows where a record's values are.**
///
/// It cannot be *handled*: `http.request` returns one flat string, so no emitted Flux can extract a
/// pointer out of it (`connector-flux`'s `description()` records the same seam for error envelopes).
/// What the connector can do is publish the shape, which `web/public/catalog.json` carries verbatim
/// and the explorer renders.
///
/// The delete response is the one that does not carry `fields` — Airtable answers
/// `{"id": …, "deleted": true}` — so the claim is per operation rather than uniform, which is itself
/// worth pinning: a later author who copied a `fields` schema onto the delete would be publishing a
/// shape Airtable never sends.
#[test]
fn every_airtable_operation_records_its_response_shape() {
    let connector = load();

    for operation in &connector.operations {
        let schema = operation.response_schema.as_ref().unwrap_or_else(|| {
            panic!(
                "operation `{}` records no response schema, so nothing tells a consumer where a \
                 record's cell values are",
                operation.id
            )
        });
        assert_eq!(
            schema["type"], "object",
            "operation `{}`: every Airtable response is a JSON object",
            operation.id
        );
        assert!(
            schema["properties"]["id"].is_object(),
            "operation `{}`: every Airtable record response names the record's `id`: {schema}",
            operation.id
        );

        let deletes = operation.id == "airtable-record-delete";
        let key = if deletes { "deleted" } else { ENVELOPE };
        assert!(
            schema["properties"][key].is_object(),
            "operation `{}`: the response schema does not describe `{key}`: {schema}",
            operation.id
        );
        assert!(
            !deletes || !schema["properties"][ENVELOPE].is_object(),
            "`airtable-record-delete` claims a `{ENVELOPE}` object in its response; Airtable answers \
             `{{\"id\": …, \"deleted\": true}}` and sends no cell values back: {schema}"
        );
    }
}

/// **The C-11 gate, per operation.** Each one emits, parses with no diagnostics, is already a fixed
/// point of flux's own formatter, and reloads through flux-lang's module loader as exactly one
/// exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set; it is restated here so the
/// Airtable connector's own file fails on its own when the module stops being analyzable. A module
/// that parsed but did not load publishes no ops at all, so a consumer handing it to flux would get
/// silence rather than an error.
#[test]
fn every_airtable_operation_emits_an_analyzable_module() {
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

/// **Every request targets `api.airtable.com` and nothing wider, and carries no credential.**
///
/// `http_hosts` derives from `base_url` (`crates/connector-cli/src/catalog.rs`, `host_of`), so the
/// egress allow-list is exactly as narrow as the string asserted here: no template variable to bind,
/// no second host, no `*`. Checked against the emitted `$base`, because that is what the request is
/// actually built from. Airtable serves attachment downloads from its own CDN hosts; none is reachable
/// from the operations below, and adding one would widen the allow-list for nothing.
///
/// The credential half is AGENTS.md's hard invariant. The connector carries the env-var *name* so a
/// host can resolve it; the emitted module carries neither that name nor a value, because auth
/// injection is C-10 and is deliberately absent rather than stubbed — which is also why this
/// connector cannot yet make a live call.
#[test]
fn every_airtable_request_targets_one_host_and_carries_no_credential() {
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
            "`{}` has path {:?}; every selected Airtable endpoint is under `{API_PREFIX}`",
            operation.id,
            operation.path
        );

        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            emitted.contains(&format!(r#"base = "{BASE_URL}""#)),
            "`{}` does not bind the Airtable base URL:\n{emitted}",
            operation.id
        );
        assert!(
            !emitted.contains(TOKEN_ENV),
            "`{}` names {TOKEN_ENV} in generated Flux:\n{emitted}",
            operation.id
        );
        // An Airtable personal access token is `pat`-prefixed and an OAuth access token is
        // `oaa`-prefixed. A literal one in a generated artifact is the failure this invariant exists
        // to prevent, so it is checked for by shape as well as by name.
        for shape in ["\"pat", "\"oaa"] {
            assert!(
                !emitted.contains(shape),
                "`{}` embeds something shaped like an Airtable access token ({shape}…):\n{emitted}",
                operation.id
            );
        }
    }
}
