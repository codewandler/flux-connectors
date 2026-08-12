//! **The differential**: the pack's request and the shipped module's request, for every operation.
//!
//! This is the guard [the design](../../../docs/designs/connector-tool-pack.md) names last and
//! calls the risk to state now — *"two surfaces are generated from one IR and they can drift into
//! disagreeing about the same operation"* — and it is the one [C-117](../../../docs/stories/C-117-pack-codegen.md)
//! asked C-145 to host, because a dry run is what makes it checkable **offline**, for the whole
//! catalogue, without a vendor account.
//!
//! # The two artifacts, and why comparing them is not circular
//!
//! `crates/connector-pack/src/request.rs` builds a request by *evaluating* an operation's emitted
//! Flux rather than re-lowering the IR, which already removes one drift axis. But the Flux the
//! catalogue embeds is `catalog::Operation::flux` — `include_str!` over
//! **`crates/catalog/ops/<provider>/<id>.flux`** — and the human-readable contract this repository
//! ships is a different file: **`connectors/<provider>.flux`**. Two generated artifacts, one
//! emitter, and nothing until now compared them.
//!
//! # It goes through `Rehearsal` since C-538, and that is what keeps it an assertion
//!
//! `Operation::build_request` reads the **canonical document** now, so projecting two entries whose
//! only difference is their embedded Flux would produce two identical requests and this whole file
//! would pass while comparing nothing. The Flux evaluator it was written for is still reachable —
//! from [`Rehearsal`], which takes the module text and nothing else — so the comparison moved there
//! rather than being deleted or, worse, left green and vacuous.
//! `tests/catalogue_differential.rs` is the other half: the *document* against the Flux, for every
//! operation.
//!
//! `catalog::Operation::flux`'s own documentation states they are the same bytes. That claim is
//! exactly the kind that is true when written and quietly false after a partial regeneration — one
//! artifact rebuilt, the other not — so it is checked here rather than trusted, and checked through
//! the thing that matters: the **request**. A whitespace difference is not a divergence; a URL, a
//! method or a body that differs is a call the two surfaces of one operation make differently.
//!
//! # What "agree" means
//!
//! Method and URL exactly. Body by parsed JSON where both parse, so key *order* is not mistaken for
//! drift while a changed nesting — the `{"ticket": {"comment": …}}` failure `tests/request.rs`
//! exists for — still fails. And a refusal is part of the comparison: an operation one artifact can
//! build a request for and the other cannot is the loudest divergence there is.
//!
//! Every failure names the operation, because a divergence in one of 254 is a fact about one
//! connector and "the differential failed" is not a bug report.

use std::collections::BTreeMap;
use std::sync::Arc;

use connector_pack::{Configuration, MemoryConfig, Rehearsal, Request, Slot};
use serde_json::{json, Value};

const TENANT: &str = "t-differential";

/// The shipped modules, relative to this crate. `connectors/` is a repository-level artifact, so
/// the path is stated once here rather than discovered.
const MODULES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../connectors");

/// The rehearsal of one operation's emitted Flux, from whichever artifact carries it.
fn rehearse(entry: &'static catalog::Operation, flux: &str) -> Result<Rehearsal, String> {
    Rehearsal::of(entry.id, entry.provider, entry.service, flux).map_err(|error| error.to_string())
}

/// A configuration port covering every endpoint variable the catalogue declares, discovered from
/// the catalogue itself so a connector shipped later is compared rather than skipped.
///
/// # This is a discovered-input shape, and it is sound here
///
/// A value manufactured for every *discovered* variable is a value that can never be missing, which
/// would be a hole if this file asked "does it compose". It does not: it compares one artifact's
/// request against the *other* artifact's request for the same operation, and both sides read the
/// same configuration — so the values are a fixed point of the comparison rather than an input to
/// it, and any value at all, including a wrong one, still detects a divergence.
///
/// An origin slot gets a value its grammar accepts, because a value it refuses would make both
/// sides refuse identically, which is agreement about nothing.
fn configuration() -> Configuration {
    let mut values = MemoryConfig::new();
    for entry in catalog::operations() {
        let Ok(rehearsal) = rehearse(entry, entry.flux) else {
            continue;
        };
        for (variable, slot) in rehearsal.endpoint_slots() {
            let value = match slot {
                Slot::Origin => "https://acme.example",
                _ => "acme",
            };
            values = values.with_endpoint(TENANT, entry.provider, entry.service, variable, value);
        }
    }
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

/// **The comparison, over every shipped operation.**
#[test]
fn the_pack_and_the_shipped_module_agree_on_every_operations_request() {
    let configuration = configuration();
    let declarations = shipped_declarations();
    let mut divergences: Vec<String> = Vec::new();
    let mut compared = 0usize;

    for entry in catalog::operations() {
        let Some(source) = declarations.get(entry.id) else {
            divergences.push(format!(
                "`{}`: the catalogue carries it and no shipped `connectors/*.flux` declares it",
                entry.id
            ));
            continue;
        };

        // Two rehearsals of one operation, differing only in which artifact's text they were given.
        // Everything else — the id, the service, the configuration they resolve through — is the
        // same, so the only thing that can differ is the artifact under test.
        let from_catalogue =
            rehearse(entry, entry.flux).unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));
        let from_module = match rehearse(entry, source) {
            Ok(rehearsal) => rehearsal,
            Err(error) => {
                divergences.push(format!(
                    "`{}`: the module's declaration does not rehearse — {error}",
                    entry.id
                ));
                continue;
            }
        };

        // The params come from the *catalogue's* schema, so a module declaring a different
        // parameter set surfaces below as a refusal rather than being papered over by asking each
        // artifact for its own arguments.
        let params = params_from_schema(&from_catalogue);
        let catalogue = from_catalogue.request(&configuration, &params);
        let module = from_module.request(&configuration, &params);

        match (catalogue, module) {
            (Ok(catalogue), Ok(module)) => {
                divergences.extend(disagreements(entry.id, &catalogue, &module));
                compared += 1;
            }
            (Err(catalogue), Err(module)) => {
                // Both refuse. The refusal itself must match, or one surface is telling an operator
                // something the other does not.
                if catalogue.to_string() != module.to_string() {
                    divergences.push(format!(
                        "`{}`: the catalogue refuses with `{catalogue}` and the module with \
                         `{module}`",
                        entry.id
                    ));
                }
                compared += 1;
            }
            (Ok(request), Err(error)) => divergences.push(format!(
                "`{}`: the catalogue builds `{} {}` and the module refuses — {error}",
                entry.id, request.method, request.url
            )),
            (Err(error), Ok(request)) => divergences.push(format!(
                "`{}`: the module builds `{} {}` and the catalogue refuses — {error}",
                entry.id, request.method, request.url
            )),
        }
    }

    assert_eq!(
        compared,
        catalog::operations().count(),
        "an operation was skipped rather than compared, which a green run would not have shown"
    );
    assert!(
        divergences.is_empty(),
        "the Tool pack and the shipped `.flux` modules disagree about {} of {compared} \
         operations:\n{}",
        divergences.len(),
        divergences.join("\n")
    );
}

/// **The control.** A differential that is green across 254 operations has said one of two things,
/// and they are not the same: *the two artifacts agree*, or *this test cannot tell*. So one
/// declaration is doctored into genuinely disagreeing — the module's path, changed by one segment —
/// and the comparison must report it **and name the operation**, which is what makes a failure a
/// bug report rather than a red square.
///
/// The doctoring is at the level the drift would actually occur at: the module's own source text,
/// rehearsed and evaluated by the same code path as the real one.
#[test]
fn a_divergence_is_reported_and_names_the_operation() {
    const OPERATION: &str = "trello-board-get";

    let entry = catalog::operation(catalog::OperationKey::id(OPERATION))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{OPERATION}`"));
    let source = shipped_declarations()
        .get(OPERATION)
        .unwrap_or_else(|| panic!("a shipped module declares `{OPERATION}`"))
        .replace("/boards/", "/cards/");
    assert!(
        source.contains("/cards/"),
        "the doctoring did not apply, so this control proves nothing"
    );

    let configuration = configuration();
    let params = json!({"id": "b-1"});
    let catalogue = rehearse(entry, entry.flux)
        .expect("the entry rehearses")
        .request(&configuration, &params)
        .expect("the entry composes");
    let module = rehearse(entry, &source)
        .expect("the doctored declaration rehearses")
        .request(&configuration, &params)
        .expect("the doctored declaration composes");

    let found = disagreements(OPERATION, &catalogue, &module);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains(OPERATION), "{}", found[0]);
    assert!(found[0].contains("url"), "{}", found[0]);
}

/// Where two requests for one operation differ, in the terms the story names: method, URL, body
/// shape. Headers are compared too — a content type present on one surface and not the other is the
/// same class of fact, and leaving it out would make the check narrower than it costs to be.
fn disagreements(operation: &str, catalogue: &Request, module: &Request) -> Vec<String> {
    let mut found = Vec::new();
    if catalogue.method != module.method {
        found.push(format!(
            "`{operation}`: method — the catalogue says `{}` and the module says `{}`",
            catalogue.method, module.method
        ));
    }
    if catalogue.url != module.url {
        found.push(format!(
            "`{operation}`: url — the catalogue builds `{}` and the module builds `{}`",
            catalogue.url, module.url
        ));
    }
    if catalogue.headers != module.headers {
        found.push(format!(
            "`{operation}`: headers — the catalogue sets {:?} and the module sets {:?}",
            catalogue.headers.keys().collect::<Vec<_>>(),
            module.headers.keys().collect::<Vec<_>>()
        ));
    }
    if !bodies_agree(catalogue.body.as_deref(), module.body.as_deref()) {
        found.push(format!(
            "`{operation}`: body — the catalogue sends {:?} and the module sends {:?}",
            catalogue.body, module.body
        ));
    }
    found
}

/// Two bodies agree when they are both absent, or when they carry the same JSON.
///
/// Parsed rather than compared as text so that key order — which `serde_json`'s object is not even
/// asked to preserve — is not reported as drift, while a changed *nesting* still is. A body that is
/// not JSON on either side falls back to exact text, which is the honest answer for a free-form
/// payload nothing here can interpret.
fn bodies_agree(catalogue: Option<&str>, module: Option<&str>) -> bool {
    match (catalogue, module) {
        (None, None) => true,
        (Some(catalogue), Some(module)) => {
            match (
                serde_json::from_str::<Value>(catalogue),
                serde_json::from_str::<Value>(module),
            ) {
                (Ok(catalogue), Ok(module)) => catalogue == module,
                _ => catalogue == module,
            }
        }
        _ => false,
    }
}

/// **Every `op` declaration the repository ships**, by name, as source text.
///
/// The whole `connectors/` directory rather than one file per connector, because a module is named
/// per **service** — `contentful-delivery.flux`, `anthropic-admin.flux` — and a rule derived here
/// for turning `(provider, service)` into a filename would be a fourth place that has to agree with
/// the emitter. An operation id is unique across the whole catalogue (`catalog::Operation::id`), so
/// one flat index needs no such rule and cannot go stale when the naming changes.
///
/// Sliced from the files rather than re-serialized from a parse, so what is compared is the bytes
/// the repository ships. A declaration starts at column zero — the emitter indents every line of a
/// body — so the split is unambiguous without a parser of its own.
fn shipped_declarations() -> BTreeMap<String, String> {
    let mut declarations = BTreeMap::new();
    let entries = std::fs::read_dir(MODULES)
        .unwrap_or_else(|error| panic!("`{MODULES}` is a shipped directory: {error}"));

    for file in entries {
        let path = file.expect("a readable directory entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("flux") {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("`{}` is a shipped artifact: {error}", path.display()));

        let mut current: Option<(String, String)> = None;
        for line in source.lines() {
            if let Some(rest) = line.strip_prefix("op ") {
                if let Some((name, text)) = current.take() {
                    declarations.insert(name, text);
                }
                let name = rest
                    .split(['(', ' '])
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                current = Some((name, String::new()));
            }
            if let Some((_, text)) = current.as_mut() {
                text.push_str(line);
                text.push('\n');
            }
        }
        if let Some((name, text)) = current.take() {
            declarations.insert(name, text);
        }
    }

    assert!(
        !declarations.is_empty(),
        "`{MODULES}` declares no operations, so nothing would be compared"
    );
    declarations
}

/// A plausible value for every parameter an operation declares, from its own input schema.
fn params_from_schema(rehearsal: &Rehearsal) -> Value {
    let spec = rehearsal.spec();
    let mut params = serde_json::Map::new();
    if let Some(properties) = spec
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    {
        for (name, schema) in properties {
            let value = match schema.get("type").and_then(Value::as_str) {
                Some("number") | Some("integer") => json!(1),
                Some("boolean") => json!(true),
                Some("array") => json!([]),
                Some("object") => json!({}),
                Some(_) => Value::String(format!("a-{name}")),
                // An untyped schema is a free-form body (`Any`), which travels through
                // `parse(…, as: "json")` — a bare string is not JSON and would be refused.
                None => json!({}),
            };
            params.insert(name.clone(), value);
        }
    }
    Value::Object(params)
}
