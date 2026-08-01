//! **An unexposed operation is still catalogued** (C-413).
//!
//! `expose = false` withholds one thing — the tool a model is handed — and the story's whole claim is
//! that it withholds *nothing else*. That claim is about artifacts, so it is asserted against the
//! artifacts a build actually writes: the module, the manifest, and the catalogue module a Rust host
//! links against.
//!
//! The opposite mistake is the easy one to make and would look like a success. Filtering unexposed
//! operations out of the manifest, or out of the catalogue, would produce a green build and a
//! connector that had quietly *lost* the operations it was supposed to keep callable — which is
//! curation wearing the new field's clothes, and exactly the thing this story is not
//! (`docs/stories/C-411-selector-matches-a-set.md` owns selection, and it stays opt-in).

mod common;

use common::Fixture;

/// Two operations, identical but for exposure — so every assertion below is a comparison rather
/// than a claim about one operation in isolation.
const HAND_AUTHORED: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"
description = "The Acme support API."

[[operations]]
id = "acme-ticket-show"
method = "GET"
path = "/v2/tickets/{ticket_id}"
description = "Fetch one Acme ticket."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "ticket_id"
description = "The ticket to fetch."
required = true
schema = { type = "integer" }

[[operations]]
id = "acme-ticket-audit"
method = "GET"
path = "/v2/tickets/{ticket_id}/audits"
description = "Read one ticket's audit trail."
risk = "low"
idempotency = "idempotent"
expose = false

[[operations.params.path]]
name = "ticket_id"
description = "The ticket whose audits to read."
required = true
schema = { type = "integer" }
"#;

fn build(root: &str) -> anyhow::Result<String> {
    let invocation =
        connector_cli::cli::parse(["build", "--root", root].iter().map(|a| a.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("CLI output is UTF-8"))
}

fn built(label: &str) -> Fixture {
    let fixture = Fixture::new(label);
    fixture.write_provider("acme", HAND_AUTHORED);
    build(fixture.root().to_str().unwrap()).expect("build succeeds");
    fixture
}

/// The emitted module declares the exposure of **both** operations, positively.
///
/// Flux's formatter writes `expose true` or `expose false` and elides neither, so the module is the
/// artifact that carries the distinction — which is what lets `connector-pack` answer "is this a
/// tool" without a second field to keep in sync.
#[test]
fn the_module_declares_each_operations_exposure_positively() {
    let module = built("exposure-module").read("connectors/acme.flux");

    assert!(
        module.contains("op acme-ticket-show"),
        "the exposed operation is missing:\n{module}"
    );
    assert!(
        module.contains("op acme-ticket-audit"),
        "an unexposed operation must still be emitted — it is callable, it is simply not a tool:\n\
         {module}"
    );
    assert!(
        module.contains("expose true"),
        "the exposed operation must declare `expose true`:\n{module}"
    );
    assert!(
        module.contains("expose false"),
        "the unexposed operation must declare `expose false` rather than being omitted:\n{module}"
    );
}

/// **The manifest still lists it.** This is the assertion that would catch the tempting mistake of
/// treating `expose` as a filter.
#[test]
fn the_manifest_lists_an_unexposed_operation_beside_an_exposed_one() {
    let manifest = built("exposure-manifest").read("connectors/acme.connector.toml");

    for id in ["acme-ticket-show", "acme-ticket-audit"] {
        assert!(
            manifest.contains(id),
            "`{id}` is missing from the manifest's `operations` list; `expose` decides whether an \
             operation is a tool, never whether it exists:\n{manifest}"
        );
    }
}

/// **The embedded catalogue still carries it**, with its Flux — which is what a Rust host links
/// against, and what `connector-pack` reads the exposure out of.
#[test]
fn the_embedded_catalogue_carries_an_unexposed_operation_and_its_flux() {
    let fixture = built("exposure-catalogue");
    let generated = fixture.read("crates/catalog/src/generated/acme.rs");

    for id in ["acme-ticket-show", "acme-ticket-audit"] {
        assert!(
            generated.contains(id),
            "`{id}` is missing from the generated catalogue module:\n{generated}"
        );
    }

    let audit = fixture.read("crates/catalog/ops/acme/acme-ticket-audit.flux");
    assert!(
        audit.contains("expose false"),
        "the catalogued Flux for an unexposed operation must say so — that string is the only \
         thing telling a host not to register it:\n{audit}"
    );
}
