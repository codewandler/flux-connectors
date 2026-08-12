//! **The differential gate** — the document-derived request plan against the Flux-derived one, for
//! every operation in the catalogue (C-538).
//!
//! Decision 0022's migration rule, executed: old and new derivations run side by side until they are
//! proven byte-identical, and only then does C-540 delete the emitter and the `.flux` artifacts.
//! This file is that proof, and it is the single piece of evidence that authorises every deletion in
//! C-540. `tests/document_differential.rs` (C-536) landed the *mechanism* on one provider with a
//! test-local evaluator; this is the whole catalogue against the **production** reader.
//!
//! # The two derivations, and why comparing them is not circular
//!
//! - **Flux-derived**: [`Rehearsal`] parses the operation's emitted `op` declaration with
//!   `flux_lang` and evaluates its body — the seven-node AST walk in `connector-pack`'s `request.rs`.
//! - **Document-derived**: [`DocumentRehearsal`] reads the request template out of
//!   `catalog/<provider>.catalog.json`, served from the pack `catalog-reader` embeds, and evaluates
//!   it in `connector-resolve`.
//!
//! Two independent lowerings of one IR, evaluated by two independent evaluators. They share the
//! *rules* — the brace grammar, `lit_text`, `json_truthy`, the structured-query wire contract — and
//! nothing else, which is exactly the sharing that makes a divergence in the **artifacts** visible.
//!
//! # What "agree" means here
//!
//! Everything the story names, per operation:
//!
//! | | compared |
//! |---|---|
//! | the request | method, URL (query included), headers, body — **exactly**, as text |
//! | `permission_subjects` | the unauthenticated URL both derivations would hand a host's network policy |
//! | the redaction set | every credential-derived string that travels, in the form it travels in |
//! | the configuration surface | endpoint variables, their slots, and the caller path parameters |
//! | the contract's parameter names | the caller-facing symbols, which the document does **not** publish |
//!
//! The last row is the one worth pausing on. A caller addresses a parameter by the name the
//! *contract* advertises, and that name is a Flux symbol: `time.start` is declared `time_start`,
//! `$top` is `_top`, and a parameter called `response` becomes `response_2` because the emitter binds
//! `response` itself. The document publishes the IR name and not the symbol, so
//! `connector-resolve`'s `document` module **reproduces** `connector-flux`'s allocator — and this
//! gate is what holds the reproduction to the emitted declaration for all 835 operations. It is also
//! the gap C-540 has to close before it deletes the emitter.
//!
//! # A refusal is part of the comparison — and is counted separately
//!
//! An operation one derivation composes a request for and the other refuses is the loudest
//! divergence there is, so both the refusal *and its sentence* are compared rather than only the
//! success case.
//!
//! But two derivations that **both** refuse have agreed about nothing, and an operation that lands
//! there never reaches the byte comparison at all. So the gate counts what it rehearsed and what it
//! actually byte-compared, and asserts the two are equal: today that is **835 of 835**. Reporting
//! the first number alone would overstate the gate by exactly the level of care it demands of the
//! catalogue.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use catalog::{OperationKey, ProviderKey};
use connector_pack::{
    Configuration, Credentials, DocumentRehearsal, Egress, MemoryConfig, MemoryStore, Operation,
    Rehearsal, Request, Slot,
};
use connector_resolve::auth::{place, placed_form, Assembled};
use flux_runtime::Tool;
use serde_json::{json, Map, Value};

const TENANT: &str = "t-catalogue-differential";

/// A value no vendor issued, long enough for a redactor to hold, and recognisable in a diff.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-CREDENTIAL";

/// The value a `Slot::Host`, `Slot::Path`, `Slot::Query`, `Slot::Header` or `Slot::Unplaced`
/// variable is bound to. One spelling for all of them, because what is being compared is two
/// derivations over the *same* inputs: any value at all, including a wrong one, detects a
/// divergence.
const PLAIN: &str = "acme";

/// The value a `Slot::Origin` variable is bound to. An origin is the one slot with a grammar, and a
/// value it refuses would make both derivations refuse identically — which is agreement about
/// nothing.
const ORIGIN: &str = "https://acme.example";

fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in no comparison reaches".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        },
        |params| async move { Ok(params) },
    ))
}

/// A plausible value for every parameter, from the rehearsed contract's own input schema.
fn params_for(spec: &flux_spec::ToolSpec) -> Value {
    let mut params = Map::new();
    if let Some(properties) = spec
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    {
        for (name, schema) in properties {
            let value = match schema.get("type").and_then(Value::as_str) {
                Some("number") | Some("integer") => Value::from(1),
                Some("boolean") => Value::from(true),
                Some("array") => Value::Array(Vec::new()),
                Some("object") => Value::Object(Map::new()),
                Some(_) => Value::String(format!("a-{name}")),
                // An untyped parameter is a free-form body, which travels through
                // `parse(…, as: "json")` — a bare word is not JSON and would be refused.
                None => Value::Object(Map::new()),
            };
            params.insert(name.clone(), value);
        }
    }
    Value::Object(params)
}

/// The value one configuration variable is bound to.
///
/// The connector's **declared default** wins when it has one, and that is not a nicety: an
/// operator-approved field bound to anything other than its reviewed default is
/// `Error::UnapprovedConfig` at projection, which would make the projected tool fall back to its
/// declared hosts and this gate compare a subject against a URL. Otherwise an origin gets a value
/// its grammar accepts and everything else gets one plain word — what is compared is two
/// derivations over the *same* inputs, so any value at all, including a wrong one, detects a
/// divergence.
fn value_for(entry: &'static catalog::Operation, variable: &str, slot: Slot) -> String {
    let binding = format!("endpoint.{variable}");
    let declared = catalog::provider(ProviderKey::id(entry.provider)).and_then(|provider| {
        provider
            .config
            .iter()
            .find(|field| field.service == entry.service && field.binds == binding)
            .and_then(|field| field.default)
    });
    match (declared, slot) {
        (Some(default), _) => default.to_string(),
        (None, Slot::Origin) => ORIGIN.to_string(),
        (None, _) => PLAIN.to_string(),
    }
}

/// The endpoint values both derivations read, covering every variable **either** of them reports.
///
/// The union rather than one side's list, deliberately: binding only what the document names would
/// make a Flux-only variable surface as a `MissingConfig` refusal rather than as the surface
/// divergence it is, and the surface is compared directly a few lines below.
fn endpoint_values(
    entry: &'static catalog::Operation,
    variables: &BTreeMap<String, Slot>,
) -> BTreeMap<String, String> {
    variables
        .iter()
        .map(|(variable, slot)| (variable.clone(), value_for(entry, variable, *slot)))
        .collect()
}

/// The same values, through the port a host binds.
///
/// **`username.` is the one reserved qualifier, and it has to be honoured here** — the same rule
/// `Field::from_placeholder` applies. Twilio's account SID is an endpoint variable spelled
/// `username.twilio.basic_auth`: one value that both scopes the request path and is the Basic user
/// half. Storing it with `with_endpoint` files it under a key the snapshot never reads, so all four
/// twilio operations refuse for a reason belonging to *this helper* rather than to either
/// derivation — which is how they used to land in the both-refuse arm and never reach the byte
/// comparison at all.
fn configured(
    entry: &'static catalog::Operation,
    values: &BTreeMap<String, String>,
) -> Configuration {
    let mut config = MemoryConfig::new();
    for (variable, value) in values {
        config = match variable
            .strip_prefix("username.")
            .filter(|name| !name.is_empty())
        {
            Some(credential) => {
                config.with_username(TENANT, entry.provider, entry.service, credential, value)
            }
            None => config.with_endpoint(TENANT, entry.provider, entry.service, variable, value),
        };
    }
    Configuration::new(Arc::new(config), TENANT).expect("a valid tenant id")
}

/// Every credential the operation's **first** declared mechanism names, assembled onto a sentinel.
///
/// The first mechanism rather than all of them because that is what `Credentials::resolve` selects
/// when every alternative is stored; what is under test is the *placement*, which is a property of
/// each credential and not of the choice between them. A credential the connector does not declare
/// is skipped — `Error::UndeclaredCredential` is the pack's answer to that and it is not this
/// gate's subject.
fn assembled_credentials(entry: &'static catalog::Operation) -> Vec<Assembled> {
    let Some(provider) = catalog::provider(ProviderKey::id(entry.provider)) else {
        return Vec::new();
    };
    let Some(mechanism) = entry.credentials.first() else {
        return Vec::new();
    };
    let mut assembled = Vec::new();
    for name in *mechanism {
        let Some(credential) = provider.auth.iter().find(|entry| entry.name == *name) else {
            continue;
        };
        // An inbound signing secret is refused identically by both derivations — it never leaves —
        // so placing one would compare two refusals rather than two requests.
        if matches!(credential.place, catalog::Placement::Inbound) {
            continue;
        }
        assembled.push(Assembled::new(
            credential.name,
            SENTINEL.to_string(),
            credential.place,
        ));
    }
    assembled
}

/// The redaction set a host must be holding: each credential's assembled value, plus the form a
/// placement that **transforms** the value puts on the wire.
fn redaction_set(credentials: &[Assembled]) -> Vec<String> {
    let mut set = Vec::new();
    for credential in credentials {
        set.push(credential.expose_value().to_string());
        if let Some(travelling) = placed_form(credential.placement(), credential.expose_value()) {
            set.push(travelling);
        }
    }
    set
}

// ---------------------------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------------------------

/// **The gate.** For every operation in the catalogue, the document-derived plan and the
/// Flux-derived plan are the same plan.
#[test]
fn the_document_and_the_flux_derivations_agree_for_every_operation() {
    let mut divergences: Vec<String> = Vec::new();
    let mut compared = 0usize;
    let mut byte_compared = 0usize;
    let mut refused: Vec<String> = Vec::new();

    for entry in catalog::operations() {
        let id = entry.id;
        let flux = match Rehearsal::of(id, entry.provider, entry.service, entry.flux) {
            Ok(rehearsal) => rehearsal,
            Err(error) => {
                divergences.push(format!(
                    "`{id}`: the emitted declaration does not rehearse: {error}"
                ));
                continue;
            }
        };
        let document = match DocumentRehearsal::of(id) {
            Ok(rehearsal) => rehearsal,
            Err(error) => {
                divergences.push(format!(
                    "`{id}`: the canonical document does not rehearse: {error}"
                ));
                continue;
            }
        };
        compared += 1;

        // ---- the configuration surface -------------------------------------------------------
        if flux.endpoint_variables() != document.endpoint_variables() {
            divergences.push(format!(
                "`{id}`: endpoint variables {:?} (document) vs {:?} (flux)",
                document.endpoint_variables(),
                flux.endpoint_variables()
            ));
        }
        if flux.endpoint_slots() != document.endpoint_slots() {
            divergences.push(format!(
                "`{id}`: endpoint slots {:?} (document) vs {:?} (flux)",
                document.endpoint_slots(),
                flux.endpoint_slots()
            ));
        }
        if flux.caller_path_parameters() != document.caller_path_parameters() {
            divergences.push(format!(
                "`{id}`: caller path parameters {:?} (document) vs {:?} (flux)",
                document.caller_path_parameters(),
                flux.caller_path_parameters()
            ));
        }
        if flux.is_exposed() != document.is_exposed() {
            divergences.push(format!(
                "`{id}`: expose {} (document) vs {} (flux)",
                document.is_exposed(),
                flux.is_exposed()
            ));
        }

        // ---- the caller-facing parameter names ------------------------------------------------
        let declared: Vec<String> = flux
            .spec()
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|properties| properties.keys().cloned().collect())
            .unwrap_or_default();
        let derived: BTreeSet<String> = document
            .caller_parameters()
            .into_iter()
            .map(str::to_owned)
            .collect();
        if declared.iter().cloned().collect::<BTreeSet<_>>() != derived {
            divergences.push(format!(
                "`{id}`: caller parameters {derived:?} (document) vs {declared:?} (flux) — the \
                 document publishes the IR name and `connector-resolve` reproduces the emitter's \
                 symbol allocation, so this is that reproduction disagreeing"
            ));
        }

        // ---- the request ----------------------------------------------------------------------
        let mut variables: BTreeMap<String, Slot> = flux.endpoint_slots().clone();
        variables.extend(
            document
                .endpoint_slots()
                .iter()
                .map(|(name, slot)| (name.clone(), *slot)),
        );
        let values = endpoint_values(entry, &variables);
        let configuration = configured(entry, &values);
        let params = params_for(flux.spec());
        let credentials = assembled_credentials(entry);

        let from_flux = flux.request(&configuration, &params);
        let from_document = document.request(&configuration, &params);
        let (mut from_flux, from_document) = match (from_flux, from_document) {
            (Ok(flux), Ok(document)) => (flux, document),
            (Err(flux), Err(document)) => {
                if flux.to_string() != document.to_string() {
                    divergences.push(format!(
                        "`{id}`: both refuse and the refusals differ — flux `{flux}`, document \
                         `{document}`"
                    ));
                }
                refused.push(format!("`{id}`: {flux}"));
                continue;
            }
            (Ok(request), Err(error)) => {
                divergences.push(format!(
                    "`{id}`: the flux derivation builds `{} {}` and the document refuses — {error}",
                    request.method, request.url
                ));
                continue;
            }
            (Err(error), Ok(request)) => {
                divergences.push(format!(
                    "`{id}`: the document derivation builds `{} {}` and the flux refuses — {error}",
                    request.method, request.url
                ));
                continue;
            }
        };

        byte_compared += 1;

        // The subject a host's network policy is shown: the URL **before** any credential is
        // placed, on both sides.
        let flux_subjects = vec![from_flux.url.clone()];
        divergences.extend(disagreements(id, &from_document, &from_flux));

        // ---- auth placement, the plan, and the redaction set -----------------------------------
        let plan = match connector_resolve::resolve(
            connector_resolve::document::operation(id).expect("the document was rehearsed above"),
            base_url_of(entry),
            &params,
            &values,
            &credentials,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                divergences.push(format!("`{id}`: the plan does not resolve — {error}"));
                continue;
            }
        };
        for credential in &credentials {
            if let Err(error) = place(id, credential, &mut from_flux) {
                divergences.push(format!(
                    "`{id}`: the flux-derived request will not carry its credentials — {error}"
                ));
                continue;
            }
        }
        divergences.extend(disagreements(id, &plan.request, &from_flux));
        if plan.permission_subjects != flux_subjects {
            divergences.push(format!(
                "`{id}`: permission subjects {:?} (document) vs {flux_subjects:?} (flux)",
                plan.permission_subjects
            ));
        }
        let registered: Vec<String> = plan
            .redactions
            .iter()
            .map(|text| text.expose_secret().to_string())
            .collect();
        let expected = redaction_set(&credentials);
        if registered != expected {
            divergences.push(format!(
                "`{id}`: redaction set {registered:?} (document) vs {expected:?} (flux)"
            ));
        }

        // ---- and the subject the projected tool actually hands a host --------------------------
        let projected = Operation::project(
            entry,
            http(),
            Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant id"),
            configuration.clone(),
        )
        .unwrap_or_else(|error| panic!("`{id}` does not project: {error}"));
        if projected.permission_subjects(&params) != flux_subjects {
            divergences.push(format!(
                "`{id}`: the projected tool gates {:?} and the flux derivation would send \
                 {flux_subjects:?}",
                projected.permission_subjects(&params)
            ));
        }
    }

    assert_eq!(
        compared,
        catalog::operations().count(),
        "an operation was skipped rather than compared, which a green run would not have shown"
    );
    // **What was actually byte-compared**, which is not the same number as what was rehearsed.
    //
    // The arm above where both derivations *refuse* agrees on the refusal and then `continue`s, so
    // an operation that lands there never reaches the method, URL, headers, body,
    // `permission_subjects` or redaction-set comparisons. A gate reporting "835 compared" while
    // some of those 835 only ever agreed about a refusal would be overstating itself by exactly one
    // level — the same argument this file makes two assertions below, applied to itself.
    //
    // It is asserted as **equality** rather than tolerated with an exemption list, because today
    // the honest number is 835 of 835 and nothing legitimately refuses. A refusal both sides share
    // is agreement about nothing: it means the inputs this file binds did not let the operation
    // compose, which is this test's defect and not the catalogue's. The four twilio operations that
    // used to land here are the worked example — see [`configured`].
    assert_eq!(
        byte_compared,
        compared,
        "{} of {compared} operations agreed only by both derivations refusing, so their method, \
         URL, headers, body, permission subjects and redaction set were never compared:\n{}",
        refused.len(),
        refused.join("\n")
    );
    assert!(
        divergences.is_empty(),
        "{} of {compared} operations diverge between the canonical document and the emitted \
         Flux:\n{}",
        divergences.len(),
        divergences
            .iter()
            .take(40)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The service base URL template the operation's provider document declares.
fn base_url_of(entry: &'static catalog::Operation) -> &'static str {
    connector_resolve::document::provider(entry.provider)
        .and_then(|document| document.base_url(entry.service))
        .unwrap_or_else(|| panic!("`{}` names a service its document does not carry", entry.id))
}

/// **The control.** A gate that is green across 835 operations has said one of two things, and they
/// are not the same: *the two derivations agree*, or *this test cannot tell*.
///
/// So a document is doctored into genuinely disagreeing — one URL segment — and the comparison must
/// report it **and name the operation and the field**. The doctoring is at the level the drift would
/// actually occur at: the canonical document's own text, parsed and evaluated by the production
/// reader.
#[test]
fn a_seeded_divergence_is_caught() {
    const OPERATION: &str = "zendesk-ticket-show";

    let entry = catalog::operation(OperationKey::id(OPERATION)).expect("a shipped operation");
    let document = connector_resolve::document::provider(entry.provider).expect("its document");
    let honest = document.operation(OPERATION).expect("its record");

    let seeded = serde_json::to_string(&json!({
        "connector": "zendesk",
        "services": [{"name": "default", "base_url": "https://{subdomain}.zendesk.com"}],
        "operations": [{
            "id": OPERATION,
            "service": "default",
            "expose": true,
            "endpoint": {"subdomain": ["host"]},
            "params": [{"name": "ticket_id", "position": "path", "required": true}],
            // The seed: `/api/v2/users/` where the shipped document says `/api/v2/tickets/`.
            "request": {"method": "GET", "url": "{base}/api/v2/users/{ticket_id}"},
        }],
    }))
    .expect("the fixture serializes");
    let doctored = connector_resolve::document::Document::parse(&seeded).expect("it parses");
    let doctored = doctored
        .operation(OPERATION)
        .expect("the fixture carries it");

    let params = json!({"ticket_id": 1});
    let endpoints = BTreeMap::from([("subdomain".to_string(), PLAIN.to_string())]);
    let base = "https://{subdomain}.zendesk.com";

    let honest = connector_resolve::build_request(honest, base, &params, &endpoints)
        .expect("the shipped document composes");
    let seeded = connector_resolve::build_request(doctored, base, &params, &endpoints)
        .expect("the fixture composes");

    let found = disagreements(OPERATION, &seeded, &honest);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains(OPERATION), "{}", found[0]);
    assert!(found[0].contains("url"), "{}", found[0]);
    assert!(found[0].contains("/api/v2/users/1"), "{}", found[0]);
}

/// A divergence in the *configuration surface* is caught too, and separately: a request comparison
/// alone would not see a slot that moved, because both derivations would still substitute the same
/// value into the same place.
#[test]
fn a_seeded_surface_divergence_is_caught() {
    const OPERATION: &str = "zendesk-ticket-show";

    let seeded = serde_json::to_string(&json!({
        "connector": "zendesk",
        "services": [{"name": "default", "base_url": "https://{subdomain}.zendesk.com"}],
        "operations": [{
            "id": OPERATION,
            "service": "default",
            "expose": true,
            // The seed: the shipped document says `["host"]`, which is the strict rule.
            "endpoint": {"subdomain": ["query"]},
            "params": [{"name": "ticket_id", "position": "path", "required": true}],
            "request": {"method": "GET", "url": "{base}/api/v2/tickets/{ticket_id}"},
        }],
    }))
    .expect("the fixture serializes");
    let doctored = connector_resolve::document::Document::parse(&seeded).expect("it parses");
    let doctored = doctored
        .operation(OPERATION)
        .expect("the fixture carries it");

    let shipped = DocumentRehearsal::of(OPERATION).expect("it rehearses");
    assert_eq!(shipped.endpoint_slots()["subdomain"], Slot::Host);
    assert_eq!(doctored.endpoint_slots()["subdomain"], Slot::Query);
    assert_ne!(shipped.endpoint_slots(), doctored.endpoint_slots());
}

/// Every way the two requests can disagree, each naming the operation and the field.
fn disagreements(operation: &str, document: &Request, flux: &Request) -> Vec<String> {
    let mut problems = Vec::new();
    if document.method != flux.method {
        problems.push(format!(
            "`{operation}`: method `{}` (document) vs `{}` (flux)",
            document.method, flux.method
        ));
    }
    if document.url != flux.url {
        problems.push(format!(
            "`{operation}`: url `{}` (document) vs `{}` (flux)",
            document.url, flux.url
        ));
    }
    if document.headers != flux.headers {
        problems.push(format!(
            "`{operation}`: headers {:?} (document) vs {:?} (flux)",
            document.headers, flux.headers
        ));
    }
    if document.body != flux.body {
        problems.push(format!(
            "`{operation}`: body {:?} (document) vs {:?} (flux)",
            document.body, flux.body
        ));
    }
    problems
}
