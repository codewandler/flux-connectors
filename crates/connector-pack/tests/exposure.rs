//! **Unexposed is not uncallable** (C-413).
//!
//! `expose` separates two claims the emitter used to fuse: that an operation *exists and can be
//! called*, and that it *reaches a model as a tool*. This file is the half that proves the
//! separation is real in both directions — that withholding the tool withholds **only** the tool, and
//! that nothing else about the operation changes.
//!
//! **The two seams, and why there are two.** A `ToolRegistry` is both what a host advertises to a
//! model (`specs`) and what an execute route resolves through (`get`), so a single filtered registry
//! cannot express "not a tool, still callable" — filtering it withholds the call as a side effect of
//! withholding the tool. So `pack` is model-facing and withholds unexposed operations, `resolve` is
//! caller-facing and withholds nothing, and the tests below pin both halves over the real catalogue.
//!
//! The synthetic route is [`Rehearsal`], for the reason `rehearsal.rs` documents at length: every
//! other `connector-pack` entry point needs a `&'static catalog::Operation`, `catalog::Operation` is
//! `#[non_exhaustive]`, and no synthetic one can be built outside the `catalog` crate. Nothing
//! shipped is unexposed yet — that is the point of the default — so an unexposed operation has to
//! arrive as emitted Flux, which is exactly what `Rehearsal` takes.

use std::sync::Arc;

use connector_pack::{Configuration, Credentials, Egress, MemoryConfig, MemoryStore, Rehearsal};
use flux_runtime::ToolRegistry;
use serde_json::json;

/// The tenant every port here answers for.
const TENANT: &str = "t-exposure";

/// A stand-in for flux's `http.request`. This file asserts *which* operations are reachable and
/// through which seam, never what the transport does, so it only has to be one.
fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in".into(),
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

/// A bound credential port over an empty store: whether a value happens to be stored must not decide
/// whether an operation resolves.
fn credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant id")
}

/// A bound configuration port holding nothing. Resolution reads which variables an operation names,
/// never what they are — a missing value refuses at call time, not here.
fn catalogue_configuration() -> Configuration {
    Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant id")
}

/// One operation, as the emitter writes it, with `{EXPOSE}` left for the caller to fill.
///
/// Deliberately an operation with a parameter, a configuration variable and a real URL: the claim
/// under test is that the *whole* request path is untouched by exposure, and an operation that
/// declares nothing would not exercise any of it.
const THING_GET: &str = r#"op probe-thing-get(thing_id: Number) -> Any
  description "Read one thing by id"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose {EXPOSE}

  base = "https://{subdomain}.probe.example"
  url = fmt("{base}/api/v1/things/{thing_id}.json")
  response = http.request(method: "GET", url)
  return response
"#;

fn flux(expose: bool) -> String {
    THING_GET.replace("{EXPOSE}", if expose { "true" } else { "false" })
}

fn configuration() -> Configuration {
    let values = MemoryConfig::new().with_endpoint(TENANT, "probe", "default", "subdomain", "acme");
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

fn rehearse(expose: bool) -> Rehearsal {
    Rehearsal::of("probe-thing-get", "probe", "default", &flux(expose))
        .expect("an operation rehearses whether or not it is exposed")
}

/// **The Acceptance, stated as one comparison.** An unexposed operation composes the *same request*
/// an exposed one composes — same method, same URL, same headers, same body.
///
/// Byte equality against the exposed rendering rather than a "it did not error" assertion: an
/// exposure flag that leaked into the request at all — a dropped header, a different URL — would be
/// a connector whose calls change depending on whether a model can see them, which is a far worse
/// defect than the one this story set out to fix.
#[test]
fn an_unexposed_operation_composes_exactly_the_request_an_exposed_one_composes() {
    let configuration = configuration();
    let params = json!({ "thing_id": 42 });

    let exposed = rehearse(true)
        .request(&configuration, &params)
        .expect("an exposed operation composes a request");
    let unexposed = rehearse(false)
        .request(&configuration, &params)
        .expect("an unexposed operation is still callable — only the tool is withheld");

    assert_eq!(
        format!("{unexposed:?}"),
        format!("{exposed:?}"),
        "withholding the tool must withhold only the tool; the request an operation makes is not \
         the model's business and must not change with it"
    );
    assert!(
        format!("{unexposed:?}").contains("acme.probe.example"),
        "the request must be the real one: {unexposed:?}"
    );
}

/// The `ToolSpec` an unexposed operation *would* register by is unchanged too.
///
/// This is what makes the withholding a decision made in one place — `pack`'s registration loop —
/// rather than a second, subtly different projection that a reader would have to diff. An unexposed
/// operation has a perfectly good spec; nobody is handed it.
#[test]
fn exposure_does_not_change_the_spec_an_operation_would_register_by() {
    let exposed = rehearse(true);
    let unexposed = rehearse(false);

    assert_eq!(
        format!("{:?}", unexposed.spec()),
        format!("{:?}", exposed.spec()),
        "exposure must not reshape the spec — it decides whether the spec is registered, not what \
         it says"
    );
}

/// **Every shipped operation resolves, exposed or not** — the caller-facing half, at catalogue
/// scale.
///
/// This is the assertion that would have caught the defect this file was first written without:
/// filtering the registry withholds the *call* as a side effect of withholding the *tool*, because a
/// `ToolRegistry` is both what a host advertises (`specs`) and what an execute route resolves
/// through (`get`). `resolve` is the seam that separates them, and `connectors-api`'s only execute
/// path goes through it.
///
/// Deliberately **not** written as "every shipped operation is exposed". That was true when this
/// landed and is a curation snapshot, not an invariant: as a gate it would fail the first provider
/// to use the feature, in a crate that provider's story never touches. What is invariant is that
/// naming an operation resolves it.
#[test]
fn every_shipped_operation_resolves_whether_or_not_it_is_exposed() {
    let mut resolved = 0usize;

    for operation in catalog::operations() {
        connector_pack::resolve(operation, http(), credentials(), catalogue_configuration())
            .unwrap_or_else(|error| {
                panic!(
                    "`{}` does not resolve, so a caller naming it cannot run it: {error}",
                    operation.id
                )
            });
        resolved += 1;
    }

    assert!(resolved > 0, "an empty catalogue would prove nothing");
}

/// **`pack` registers exactly the exposed operations** — the model-facing half, at catalogue scale.
///
/// Together with the test above this is the whole separation: the registered set is the exposed set,
/// and the resolvable set is everything. Both halves are filters over the real catalogue rather than
/// claims about how much of it is exposed, so neither constrains what any provider chooses.
#[test]
fn pack_registers_exactly_the_exposed_operations() {
    let providers: Vec<&str> = catalog::providers()
        .iter()
        .map(|provider| provider.id)
        .collect();

    let mut registry = ToolRegistry::new();
    connector_pack::pack(&providers, http(), credentials(), catalogue_configuration())(
        &mut registry,
    )
    .expect("the shipped catalogue installs");

    let mut expected = 0usize;
    for operation in catalog::operations() {
        let dotted = connector_pack::dotted_name(operation.id).expect("a dotted name");
        let exposed = connector_pack::is_exposed(operation)
            .unwrap_or_else(|error| panic!("`{}` reads its exposure: {error}", operation.id));

        assert_eq!(
            registry.get(&dotted).is_some(),
            exposed,
            "`{}` declares `expose {exposed}` but the registry disagrees — the registry is exactly \
             what a host hands a model, so this is the line between callable and advertised",
            operation.id
        );
        expected += usize::from(exposed);
    }

    assert_eq!(
        registry.names().len(),
        expected,
        "the registry holds a different number of tools than the catalogue exposes"
    );
}

/// The converse of the test above, so it cannot pass by the exposure read answering `true` for
/// everything.
///
/// `connector_pack::is_exposed` takes a catalogue entry, which cannot be fabricated here; it and
/// [`Rehearsal::is_exposed`] read the same field of the same parsed declaration, so this covers the
/// direction the shipped catalogue cannot.
#[test]
fn the_exposure_read_reports_false_rather_than_answering_true_for_everything() {
    assert!(
        rehearse(true).is_exposed(),
        "an operation declaring `expose true` must read as exposed"
    );
    assert!(
        !rehearse(false).is_exposed(),
        "an operation declaring `expose false` must read as unexposed — otherwise the registration \
         filter is unreachable and the whole story is inert"
    );

    // And it is still a fully-formed, callable operation while unexposed.
    assert_eq!(rehearse(false).endpoint_variables(), ["subdomain"]);
}
