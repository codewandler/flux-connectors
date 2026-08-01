//! **Unexposed is not uncallable** (C-413).
//!
//! `expose` separates two claims the emitter used to fuse: that an operation *exists and can be
//! called*, and that it *reaches a model as a tool*. This file is the half that proves the
//! separation is real in both directions — that withholding the tool withholds **only** the tool, and
//! that nothing else about the operation changes.
//!
//! The route is [`Rehearsal`], for the reason `rehearsal.rs` documents at length: every other
//! `connector-pack` entry point needs a `&'static catalog::Operation`, `catalog::Operation` is
//! `#[non_exhaustive]`, and no synthetic one can be built outside the `catalog` crate. Nothing
//! shipped is unexposed yet — that is the point of the default — so an unexposed operation has to
//! arrive as emitted Flux, which is exactly what `Rehearsal` takes.

use std::sync::Arc;

use connector_pack::{Configuration, MemoryConfig, Rehearsal};
use serde_json::json;

/// The tenant every port here answers for.
const TENANT: &str = "t-exposure";

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

/// **Every operation the catalogue ships is exposed**, which is the pack-level statement of "no
/// shipped artifact moves".
///
/// `pack` skips unexposed operations, so this is also what proves that skip is a **no-op today**: if
/// it were not, landing C-413 would have silently removed tools from every host that installs this
/// catalogue, and no artifact diff would have shown it.
#[test]
fn every_shipped_operation_is_exposed_so_the_registration_filter_removes_nothing() {
    let mut unexposed = Vec::new();

    for provider in catalog::providers() {
        for operation in provider.operations {
            if !connector_pack::is_exposed(operation)
                .unwrap_or_else(|error| panic!("`{}` reads its exposure: {error}", operation.id))
            {
                unexposed.push(operation.id);
            }
        }
    }

    assert!(
        unexposed.is_empty(),
        "these shipped operations are unexposed, so installing the catalogue now registers fewer \
         tools than it did before C-413 — which is a change to what every host serves, and one no \
         artifact diff reports: {unexposed:?}"
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
