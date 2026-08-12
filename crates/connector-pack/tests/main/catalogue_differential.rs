//! **The differential gate** — the document-derived request plan against the Flux-derived one, for
//! every operation in the catalogue (C-538).
//!
//! Decision 0022's migration rule, executed: old and new derivations run side by side until they are
//! proven byte-identical, and only then does C-540 delete the emitter and the `.flux` artifacts.
//! This file is that proof, and it is the single piece of evidence that authorises every deletion in
//! C-540. `tests/document_differential.rs` (C-536) landed the *mechanism* on one provider with a
//! test-local evaluator; this is the whole catalogue against the **production** reader.
//!
//! # The three derivations, and why comparing them is not circular
//!
//! - **Flux-derived**: [`Rehearsal`] parses the operation's emitted `op` declaration with
//!   `flux_lang` and evaluates its body — the seven-node AST walk in `connector-pack`'s `request.rs`.
//! - **Document-derived**: [`DocumentRehearsal`] reads the request template out of
//!   `catalog/<provider>.catalog.json`, served from the pack `catalog-reader` embeds, and evaluates
//!   it in `connector-resolve`.
//! - **Published-derived** (C-553): [`Operation::build_request_plan`] — the seam a consumer that
//!   owns its own `Tool` projection reaches, driven through the **bound ports** rather than through
//!   hand-assembled inputs.
//!
//! Two independent lowerings of one IR, evaluated by two independent evaluators. They share the
//! *rules* — the brace grammar, `lit_text`, `json_truthy`, the structured-query wire contract — and
//! nothing else, which is exactly the sharing that makes a divergence in the **artifacts** visible.
//!
//! # Why the third derivation is not a third copy of the second (C-553)
//!
//! The document-derived arm calls `connector_resolve::resolve` with inputs this file assembles: an
//! endpoint map it built, and a `Vec<Assembled>` it composed. The published arm calls nothing of the
//! sort — it binds a [`Credentials`] over a seeded [`MemoryStore`] and a [`Configuration`] over a
//! [`MemoryConfig`], projects the operation, and asks it for its plan. Everything between those two
//! statements is the **enforcement topology**: the alternative-selection rule, the checked redactor
//! registration, the acquisition axis, the declared defaults, operator approval and origin
//! normalisation. So the comparison answers a question the second arm cannot: *does the plan a
//! consumer actually receives equal the one the wrapper-`Tool` path dispatches?*
//!
//! It is the same code by construction — [`Operation::build_authenticated_request`] returns
//! `plan.request` from this very call — and that is exactly why it is worth pinning across all 835
//! operations rather than asserting once. A second derivation added beside it would keep both
//! signatures and keep both green in isolation.
//!
//! # What "agree" means here
//!
//! Everything the story names, per operation:
//!
//! | | compared |
//! |---|---|
//! | the request | method, URL (query included), headers, body — **exactly**, as text |
//! | `permission_subjects` | the unauthenticated URL every derivation would hand a host's network policy |
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
    Rehearsal, Request, RequestPlan, Secret, SecretStore, SensitiveText, Slot,
};
use connector_resolve::auth::{acquire, place, placed_form, Assembled};
use connector_resolve::{ConfigField, ConfigPort, ConfigValue};
use flux_runtime::{Tool, ToolContext};
use flux_system::{System, Workspace};
use serde_json::{json, Map, Value};

const TENANT: &str = "t-catalogue-differential";

/// A value no vendor issued, long enough for a redactor to hold, and recognisable in a diff.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-CREDENTIAL";

/// The user half a Basic join is bound to, for the five connectors that declare one.
///
/// It is config rather than a secret (C-193), so it comes through the configuration port and never
/// through the store — and the published arm needs it bound or `Credentials::resolve` refuses with
/// `MissingCredentialConfig` before a request exists.
const USER: &str = "differential@example.test";

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
///
/// **The Basic user halves are bound here too** (C-553). The published arm resolves credentials
/// through the real port, and a Basic join reads its user half from *this* store — so a connector
/// whose username is not also an endpoint variable would refuse for a reason belonging to this
/// helper rather than to any derivation, exactly as twilio's four operations once did.
fn configured(
    entry: &'static catalog::Operation,
    values: &BTreeMap<String, String>,
    usernames: &BTreeMap<&'static str, String>,
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
    for (credential, user) in usernames {
        config = config.with_username(TENANT, entry.provider, entry.service, credential, user);
    }
    Configuration::new(Arc::new(config), TENANT).expect("a valid tenant id")
}

/// The user half every Basic credential of this operation's connector is bound to.
///
/// Twilio's account SID is both an endpoint variable and the Basic user half, spelled
/// `username.twilio.basic_auth`, so a value already bound through [`endpoint_values`] wins — binding
/// it twice with two spellings is how one derivation would read a value the other did not.
fn usernames_for(
    entry: &'static catalog::Operation,
    values: &BTreeMap<String, String>,
) -> BTreeMap<&'static str, String> {
    let Some(provider) = catalog::provider(ProviderKey::id(entry.provider)) else {
        return BTreeMap::new();
    };
    provider
        .auth
        .iter()
        .filter(|credential| matches!(credential.acquire, catalog::Acquisition::BasicJoin { .. }))
        .map(|credential| {
            let bound = values
                .get(&format!("username.{}", credential.name))
                .cloned();
            (credential.name, bound.unwrap_or_else(|| USER.to_string()))
        })
        .collect()
}

/// **The credential port the published arm resolves through**, over a store holding the sentinel at
/// every address this connector declares (C-553).
///
/// Every credential rather than the first mechanism's, because the selection rule is part of what is
/// under test: `Credentials::resolve` takes the **first mechanism whose credentials all resolve**,
/// and a store seeded with only that mechanism could not tell a correct selection from an accidental
/// one. A connector declaring no `authority` has no address to seed — its operations refuse
/// identically on both arms — so the reference is skipped rather than unwrapped.
async fn seeded_credentials(entry: &'static catalog::Operation) -> Credentials {
    Credentials::new(seeded_store(entry).await as Arc<dyn SecretStore>, TENANT)
        .expect("a valid tenant id")
}

/// **The raw store behind [`seeded_credentials`]**, for the engine-free producers arm (C-557).
///
/// The engine-free credential assembler takes a bare [`SecretStore`], not a `Credentials` port, so
/// this hands back the seeded store itself. It carries the sentinel at every address this connector
/// declares — the same seeding [`seeded_credentials`] does — so the alternative-selection rule is
/// exercised rather than a store that could only resolve one mechanism.
async fn seeded_store(entry: &'static catalog::Operation) -> Arc<MemoryStore> {
    let store = Arc::new(MemoryStore::new());
    if let Some(provider) = catalog::provider(ProviderKey::id(entry.provider)) {
        let credentials = Credentials::new(store.clone() as Arc<dyn SecretStore>, TENANT)
            .expect("a valid tenant id");
        for credential in provider.auth {
            let Ok(reference) = credentials.reference(entry.id, provider, credential) else {
                continue;
            };
            store
                .put(&reference, &Secret::new(SENTINEL))
                .await
                .expect("an in-memory put cannot fail");
        }
    }
    store
}

/// **A bare [`ConfigPort`] over the same values the other arms bind** (C-557).
///
/// The engine-free endpoint resolver and the Basic user half both read this — the endpoint variables
/// by their placeholder name, the user halves by credential name — exactly as a consumer that never
/// touches `connector-pack`'s `Configuration` would supply them. Everything is `proposed`, which is
/// enough because [`value_for`] hands an operator-approved field its reviewed default, and a declared
/// default satisfies the approval check without an approval flag.
struct MapConfigPort<'a> {
    values: &'a BTreeMap<String, String>,
    usernames: &'a BTreeMap<&'static str, String>,
}

impl ConfigPort for MapConfigPort<'_> {
    fn resolve(&self, field: ConfigField<'_>) -> Option<ConfigValue> {
        match field {
            ConfigField::Endpoint(name) => self
                .values
                .get(name)
                .map(|value| ConfigValue::proposed(value.clone())),
            ConfigField::Username(name) => self
                .usernames
                .get(name)
                .or_else(|| self.values.get(&format!("username.{name}")))
                .map(|value| ConfigValue::proposed(value.clone())),
        }
    }
}

/// **The plan the engine-free producers derive** — the seam X-156 takes (C-557).
///
/// No `connector-pack`, no `ToolContext`: `resolve_endpoints` over a bare [`ConfigPort`],
/// `assemble_credentials` over a bare [`SecretStore`], and `resolve`. It is the same plan the flux
/// path composes or the enforcement was reimplemented rather than relocated.
async fn engine_free_plan(
    entry: &'static catalog::Operation,
    config: &MapConfigPort<'_>,
    params: &Value,
) -> Result<RequestPlan, Box<dyn std::error::Error>> {
    let provider = catalog::provider(ProviderKey::id(entry.provider))
        .ok_or("the operation names a provider the catalogue carries")?;
    let document = connector_resolve::document::operation(entry.id)
        .ok_or("the pack carries the operation's document")?;
    let endpoints = connector_resolve::resolve_endpoints(document, provider, TENANT, config)?;
    let store = seeded_store(entry).await;
    let assembly = connector_resolve::assemble_credentials(
        entry,
        provider,
        TENANT,
        None,
        store.as_ref(),
        config,
    )
    .await?;
    Ok(connector_resolve::resolve(
        document,
        base_url_of(entry),
        params,
        &endpoints,
        &assembly.credentials,
    )?)
}

/// A `ToolContext` for the published arm, whose redactor every resolved credential is registered
/// with before a plan exists.
///
/// One for the whole run: `register` asks the redactor whether it already holds a value before
/// adding it, so the set stays the handful of distinct assembled forms the catalogue produces rather
/// than growing per operation.
fn context() -> ToolContext {
    let workspace = Workspace::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
    ToolContext::new(Arc::new(System::new(workspace)))
}

/// Every credential the operation's **first** declared mechanism names, assembled onto a sentinel.
///
/// The first mechanism rather than all of them because that is what `Credentials::resolve` selects
/// when every alternative is stored; what is under test is the *placement*, which is a property of
/// each credential and not of the choice between them. A credential the connector does not declare
/// is skipped — `Error::UndeclaredCredential` is the pack's answer to that and it is not this
/// gate's subject.
///
/// **The acquisition axis is applied, not skipped** (C-553). This used to hand [`SENTINEL`] straight
/// to [`Assembled::new`], which was enough while both arms were fed the same literal. The published
/// arm resolves through the real port, so for a Basic join the value that travels is
/// `base64(user<suffix>:secret)` and nothing else — and an expectation still naming the bare
/// sentinel would report every one of those five connectors as a divergence. [`acquire`] is
/// `connector-resolve`'s own function, so this is the published rule read rather than a second copy
/// of it; what this file supplies is the same *input* the port was seeded with.
fn assembled_credentials(
    entry: &'static catalog::Operation,
    usernames: &BTreeMap<&'static str, String>,
) -> Vec<Assembled> {
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
        // An inbound signing secret is refused identically by every derivation — it never leaves —
        // so placing one would compare refusals rather than requests.
        if matches!(credential.place, catalog::Placement::Inbound) {
            continue;
        }
        let user = match credential.acquire {
            catalog::Acquisition::BasicJoin { user_suffix, .. } => Some(format!(
                "{}{user_suffix}",
                usernames.get(credential.name).map_or(USER, String::as_str)
            )),
            _ => None,
        };
        assembled.push(Assembled::new(
            credential.name,
            acquire(credential, SENTINEL, user.as_deref()),
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

/// A plan's redaction set as plain text, for comparison against [`redaction_set`].
///
/// Reaching for [`SensitiveText::expose_secret`] is the deliberate speed bump the type exists to
/// put here; a test comparing redaction sets is one of the few honest reasons to take it.
fn exposed(redactions: &[SensitiveText]) -> Vec<String> {
    redactions
        .iter()
        .map(|text| text.expose_secret().to_string())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------------------------

/// **The gate.** For every operation in the catalogue, the document-derived plan, the Flux-derived
/// plan and the plan the **published seam** hands a consumer are the same plan.
#[tokio::test]
async fn the_document_and_the_flux_derivations_agree_for_every_operation() {
    let mut divergences: Vec<String> = Vec::new();
    let mut compared = 0usize;
    let mut byte_compared = 0usize;
    let mut published_compared = 0usize;
    let mut engine_free_compared = 0usize;
    let mut refused: Vec<String> = Vec::new();
    let ctx = context();

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
        let usernames = usernames_for(entry, &values);
        let configuration = configured(entry, &values, &usernames);
        let params = params_for(flux.spec());
        let credentials = assembled_credentials(entry, &usernames);

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
        divergences.extend(disagreements(id, "document", &from_document, &from_flux));

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
        divergences.extend(disagreements(id, "document", &plan.request, &from_flux));
        let expected = redaction_set(&credentials);
        if plan.permission_subjects != flux_subjects {
            divergences.push(format!(
                "`{id}`: permission subjects {:?} (document) vs {flux_subjects:?} (flux)",
                plan.permission_subjects
            ));
        }
        if exposed(&plan.redactions) != expected {
            divergences.push(format!(
                "`{id}`: redaction set {:?} (document) vs {expected:?} (flux)",
                exposed(&plan.redactions)
            ));
        }

        // ---- and the plan the published seam hands a consumer (C-553) ---------------------------
        //
        // The same three comparisons again, against a plan derived through the **bound ports**
        // rather than through the inputs this file assembled: the credential port over a seeded
        // store, the configuration port over a `MemoryConfig`, and `Operation::build_request_plan`.
        // That is the enforcement topology — alternative selection, the acquisition axis, checked
        // redactor registration, declared defaults, operator approval, origin normalisation — and it
        // is the path a consumer takes, so it is the one that has to agree.
        let projected = Operation::project(
            entry,
            http(),
            seeded_credentials(entry).await,
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
        match projected.build_request_plan(&ctx, &params).await {
            Ok(published) => {
                published_compared += 1;
                divergences.extend(published_plan_divergences(
                    id,
                    &published,
                    &from_flux,
                    &flux_subjects,
                    &expected,
                ));
            }
            Err(error) => divergences.push(format!(
                "`{id}`: the published seam refuses where the flux derivation builds `{} {}` — \
                 {error}",
                from_flux.method, from_flux.url
            )),
        }

        // ---- and the plan the engine-free producers derive (C-557) ------------------------------
        //
        // The seam X-156 actually takes: no `connector-pack`, no `ToolContext`. The endpoints come
        // from `resolve_endpoints` over a bare `ConfigPort`, the credentials from
        // `assemble_credentials` over a bare `SecretStore`, and the plan from `resolve`. It must be
        // the same plan the flux path composes — request, subjects and redaction set — which is the
        // evidence the enforcement was relocated into the engine-free crate rather than
        // reimplemented. `connector-pack`'s own producers now delegate here, so a divergence would be
        // a divergence in the relocated logic itself.
        let ef_config = MapConfigPort {
            values: &values,
            usernames: &usernames,
        };
        match engine_free_plan(entry, &ef_config, &params).await {
            Ok(engine_free) => {
                engine_free_compared += 1;
                divergences.extend(disagreements(
                    id,
                    "engine-free producers",
                    &engine_free.request,
                    &from_flux,
                ));
                if engine_free.permission_subjects != flux_subjects {
                    divergences.push(format!(
                        "`{id}`: permission subjects {:?} (engine-free producers) vs \
                         {flux_subjects:?} (flux)",
                        engine_free.permission_subjects
                    ));
                }
                if exposed(&engine_free.redactions) != expected {
                    divergences.push(format!(
                        "`{id}`: redaction set {:?} (engine-free producers) vs {expected:?} (flux)",
                        exposed(&engine_free.redactions)
                    ));
                }
            }
            Err(error) => divergences.push(format!(
                "`{id}`: the engine-free producers refuse where the flux derivation builds \
                 `{} {}` — {error}",
                from_flux.method, from_flux.url
            )),
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
    // **And what the published seam actually yielded** (C-553), on the identical argument.
    //
    // A refusal from `build_request_plan` is already pushed as a divergence above, so this cannot be
    // the only thing that notices one. It is here because it is the number the story's Acceptance is
    // read against — "for every operation" — and a count is the only form in which "every" is
    // checkable. The two arms are compared against the same Flux-derived request, so equality here
    // says the published plan was byte-compared exactly as often as the document-derived one was.
    assert_eq!(
        published_compared, byte_compared,
        "the published plan seam was compared for {published_compared} of {byte_compared} \
         operations, so the rest reached no comparison at all"
    );
    // **And what the engine-free producers actually yielded** (C-557), on the identical argument as
    // the published count above: a plan the bare `ConfigPort`/`SecretStore` producers derived was
    // byte-compared against the Flux-derived request exactly as often as the document-derived one.
    assert_eq!(
        engine_free_compared, byte_compared,
        "the engine-free producers were compared for {engine_free_compared} of {byte_compared} \
         operations, so the rest reached no comparison at all"
    );
    assert!(
        divergences.is_empty(),
        "{} of {compared} operations diverge between the canonical document, the published seam \
         and the emitted Flux:\n{}",
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

    let found = disagreements(OPERATION, "document", &seeded, &honest);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains(OPERATION), "{}", found[0]);
    assert!(found[0].contains("url"), "{}", found[0]);
    assert!(found[0].contains("/api/v2/users/1"), "{}", found[0]);
}

/// **The control for the published arm** (C-553), and it is a separate one on purpose.
///
/// [`a_seeded_divergence_is_caught`] proves the comparator can see a divergence between two
/// *documents*. It says nothing about the arm added by C-553, which compares a plan obtained through
/// the bound ports — a different value, reached by a different path, and carrying two fields the
/// request comparison does not look at. A green 835-operation run of that arm would otherwise be
/// indistinguishable from an arm that cannot tell.
///
/// So a real published plan is taken and each of the three things the gate checks is seeded with a
/// divergence in turn — the URL, the permission subjects, the redaction set — and
/// [`published_plan_divergences`], the gate's **own** comparison, must report each one. Routing
/// every seed through that function is the whole point: an earlier draft asserted seeds two and
/// three with `assert_ne!` over local vectors, which proves `!=` works and says nothing about
/// whether the gate's subject and redaction checks are live. The subject check authorises X-156's
/// adoption; it is exercised here by the code that ships.
#[tokio::test]
async fn a_seeded_divergence_in_the_published_plan_is_caught() {
    const OPERATION: &str = "zendesk-ticket-show";

    let entry = catalog::operation(OperationKey::id(OPERATION)).expect("a shipped operation");
    let values = BTreeMap::from([("subdomain".to_string(), PLAIN.to_string())]);
    let usernames = usernames_for(entry, &values);
    let configuration = configured(entry, &values, &usernames);
    let params = json!({"ticket_id": 1});

    let projected = Operation::project(
        entry,
        http(),
        seeded_credentials(entry).await,
        configuration.clone(),
    )
    .expect("the shipped operation projects");
    let published = projected
        .build_request_plan(&context(), &params)
        .await
        .expect("the published seam derives a plan");

    // The Flux derivation, with its credentials placed — exactly the three references the gate
    // compares the published plan against.
    let mut flux = Rehearsal::of(OPERATION, entry.provider, entry.service, entry.flux)
        .expect("the emitted declaration rehearses")
        .request(&configuration, &params)
        .expect("the flux derivation composes");
    let flux_subjects = vec![flux.url.clone()];
    let credentials = assembled_credentials(entry, &usernames);
    for credential in &credentials {
        place(OPERATION, credential, &mut flux).expect("the credential places");
    }
    let expected = redaction_set(&credentials);

    // The gate as it stands: no divergence, on any of the three.
    assert!(
        published_plan_divergences(OPERATION, &published, &flux, &flux_subjects, &expected)
            .is_empty(),
        "the shipped catalogue must agree with itself before a seed means anything"
    );

    // ---- seed one: the request ---------------------------------------------------------------
    let mut seeded = published.clone();
    seeded.request.url = seeded.request.url.replace("/tickets/", "/users/");
    let found = published_plan_divergences(OPERATION, &seeded, &flux, &flux_subjects, &expected);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains(OPERATION) && found[0].contains("url"),
        "{found:?}"
    );
    assert!(found[0].contains("published"), "{found:?}");

    // ---- seed two: the permission subjects (the check that authorises X-156's adoption) --------
    //
    // A subject a host's policy never saw — the request is byte-identical and the redaction set is
    // byte-identical, so this is the *only* thing that moved, and the gate's inline `!=` is the only
    // thing that can catch it.
    let mut seeded = published.clone();
    seeded
        .permission_subjects
        .push("https://elsewhere.example".to_string());
    let found = published_plan_divergences(OPERATION, &seeded, &flux, &flux_subjects, &expected);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains(OPERATION) && found[0].contains("permission subjects"),
        "{found:?}"
    );
    assert!(found[0].contains("published"), "{found:?}");

    // ---- seed three: the redaction set ----------------------------------------------------------
    //
    // A dropped redaction is the divergence with no other symptom: the request is byte-identical,
    // the subject is byte-identical, and a consumer registers one string fewer than travels. Seeding
    // it needs a plan whose redaction set is non-empty, which zendesk's bearer gives us.
    assert!(
        !published.redactions.is_empty(),
        "this control needs an operation that carries a credential"
    );
    let mut seeded = published.clone();
    seeded.redactions.pop();
    let found = published_plan_divergences(OPERATION, &seeded, &flux, &flux_subjects, &expected);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains(OPERATION) && found[0].contains("redaction set"),
        "{found:?}"
    );
    assert!(found[0].contains("published"), "{found:?}");
}

/// **The control for the engine-free producers arm** (C-557), and a separate one on purpose.
///
/// [`a_seeded_divergence_in_the_published_plan_is_caught`] proves the comparator can see a divergence
/// in a plan reached through `connector-pack`'s bound ports. This one proves it for the plan reached
/// through the **engine-free producers** — `resolve_endpoints` and `assemble_credentials` over bare
/// ports, the path X-156 takes with no `connector-pack` and no `ToolContext`. A green 835-operation
/// run of that arm would otherwise be indistinguishable from an arm that cannot tell.
///
/// The honest derivation exercises the real producers end to end; the seed then moves one URL
/// segment and requires [`disagreements`] — the gate's own comparison — to report it named.
#[tokio::test]
async fn a_seeded_divergence_in_the_engine_free_producers_is_caught() {
    const OPERATION: &str = "zendesk-ticket-show";

    let entry = catalog::operation(OperationKey::id(OPERATION)).expect("a shipped operation");
    let values = BTreeMap::from([("subdomain".to_string(), PLAIN.to_string())]);
    let usernames = usernames_for(entry, &values);
    let config = MapConfigPort {
        values: &values,
        usernames: &usernames,
    };
    let params = json!({"ticket_id": 1});

    let engine_free = engine_free_plan(entry, &config, &params)
        .await
        .expect("the engine-free producers derive a plan");

    // The Flux derivation, with its credentials placed — the reference the gate compares against.
    let configuration = configured(entry, &values, &usernames);
    let mut flux = Rehearsal::of(OPERATION, entry.provider, entry.service, entry.flux)
        .expect("the emitted declaration rehearses")
        .request(&configuration, &params)
        .expect("the flux derivation composes");
    let credentials = assembled_credentials(entry, &usernames);
    for credential in &credentials {
        place(OPERATION, credential, &mut flux).expect("the credential places");
    }

    // The gate as it stands: the engine-free producers agree with the flux derivation.
    assert!(
        disagreements(
            OPERATION,
            "engine-free producers",
            &engine_free.request,
            &flux
        )
        .is_empty(),
        "the shipped catalogue must agree with itself before a seed means anything"
    );

    // The seed: one URL segment moved. The producers' own output is what the gate compares, so a
    // divergence in it must surface, named for the operation and the field.
    let mut seeded = engine_free.clone();
    seeded.request.url = seeded.request.url.replace("/tickets/", "/users/");
    let found = disagreements(OPERATION, "engine-free producers", &seeded.request, &flux);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains(OPERATION) && found[0].contains("url"),
        "{found:?}"
    );
    assert!(found[0].contains("engine-free producers"), "{found:?}");
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

/// Every way two requests can disagree, each naming the operation, the field and **which
/// derivation** produced the left-hand side.
///
/// The `derivation` label joined the signature with C-553's third arm: `document` and `published`
/// are compared against the same Flux-derived request, so a failure that named neither would leave a
/// reader guessing which of the two had moved.
fn disagreements(
    operation: &str,
    derivation: &str,
    derived: &Request,
    flux: &Request,
) -> Vec<String> {
    let mut problems = Vec::new();
    if derived.method != flux.method {
        problems.push(format!(
            "`{operation}`: method `{}` ({derivation}) vs `{}` (flux)",
            derived.method, flux.method
        ));
    }
    if derived.url != flux.url {
        problems.push(format!(
            "`{operation}`: url `{}` ({derivation}) vs `{}` (flux)",
            derived.url, flux.url
        ));
    }
    if derived.headers != flux.headers {
        problems.push(format!(
            "`{operation}`: headers {:?} ({derivation}) vs {:?} (flux)",
            derived.headers, flux.headers
        ));
    }
    if derived.body != flux.body {
        problems.push(format!(
            "`{operation}`: body {:?} ({derivation}) vs {:?} (flux)",
            derived.body, flux.body
        ));
    }
    problems
}

/// **Every way the published plan can disagree with the Flux derivation**: the request, the
/// permission subjects, and the redaction set (C-553).
///
/// Extracted from the gate's own body so [`a_seeded_divergence_in_the_published_plan_is_caught`]
/// runs the **identical** comparison rather than a paraphrase of it — the same reasoning
/// [`disagreements`] is a shared function for. A control that reimplemented `!=` over local vectors
/// would prove that `!=` works, not that the gate's subject and redaction checks are live; the
/// subject check in particular authorises X-156's adoption, so it has to be exercised by the code
/// that ships.
///
/// `flux` carries its credentials already placed, `flux_subjects` is the URL **before** placement
/// (what a host's network policy is shown), and `expected_redactions` is [`redaction_set`] — exactly
/// the three references the gate compares against.
fn published_plan_divergences(
    operation: &str,
    published: &RequestPlan,
    flux: &Request,
    flux_subjects: &[String],
    expected_redactions: &[String],
) -> Vec<String> {
    let mut problems = disagreements(operation, "published", &published.request, flux);
    if published.permission_subjects != flux_subjects {
        problems.push(format!(
            "`{operation}`: permission subjects {:?} (published) vs {flux_subjects:?} (flux)",
            published.permission_subjects
        ));
    }
    if exposed(&published.redactions) != expected_redactions {
        problems.push(format!(
            "`{operation}`: redaction set {:?} (published) vs {expected_redactions:?} (flux)",
            exposed(&published.redactions)
        ));
    }
    problems
}
