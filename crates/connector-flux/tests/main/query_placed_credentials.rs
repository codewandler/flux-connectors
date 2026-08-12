//! **A connector whose credential travels in the query string puts nothing else there** (C-230).
//!
//! This is C-159 §2's hazard, stated as a property of the catalogue rather than as a census of it.
//!
//! The hazard itself: a query-placed credential is percent-encoded on its way onto the URL
//! (`crates/connector-pack/src/auth.rs:157-164`, `:204-215`) while the **unencoded** value is what
//! was registered with flux's redactor. C-159 §2 closed the finding as unreachable, because the
//! committed catalogue then declared 18 `Placement::Header`, 2 `Placement::Inbound` and **zero**
//! `Placement::Query`. C-165's Trello connector made it reachable.
//!
//! The second half is the emitter's, and it is what makes the combination sharp rather than merely
//! untidy: a query *value* is interpolated verbatim (`crates/connector-flux/src/op.rs:138-143`),
//! which is why `zendesk-ticket-search` is a recorded intentional gap in `AGENTS.md` instead of a
//! working operation. **A connector whose credential lives in the query string is the worst possible
//! place to also put unencoded caller text.** A value carrying `&` would not merely corrupt a
//! filter: it lands *before* the credential the host appends and can inject a parameter of its own.
//!
//! ## Why this file exists, and why it is not `trello_connector.rs`
//!
//! It was. `trello_connector.rs::trello_is_the_only_query_placement_in_the_shipped_catalogue` walked
//! every provider and asserted the query-placed set equalled a two-element literal — Trello's key and
//! token. That assertion was green **only** because no provider since Trello had placed a credential
//! in the query string, and the next one that did would have turned *Trello's* test red, from a
//! worktree that could not see Trello's test and for a reason that had nothing to do with Trello.
//! C-230 is that defect; `crates/connector-cli/tests/per_provider_test_scope.rs` is the guard that
//! now refuses the shape.
//!
//! The measurement was worth keeping, so it moved here and changed what it asserts. The literal
//! census answered "who else does this?" — a question whose answer changes every time the catalogue
//! grows, for reasons unrelated to the hazard. The property below answers the question the hazard
//! actually poses: **does anyone combine a query-placed credential with caller text in the same
//! query string?** A 54th connector declaring a query credential and no query parameter leaves this
//! green, which is correct — it has the same containment Trello has. One that declares both turns it
//! red, which is exactly when C-159 §2's account stops being true and needs re-measuring.
//!
//! That is the difference the rule in `AGENTS.md` draws: a universally quantified property over
//! whatever ships survives catalogue growth; an equality against a hand-written literal cannot.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector};

use crate::shipped_provider;

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// Every shipped provider id, **read from `providers/` rather than listed** (C-54) — this file is a
/// claim about whatever ships, so a constant would be the wrong shape twice over.
fn shipped() -> Vec<String> {
    let dir = providers_dir();
    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .filter_map(|entry| {
            let path = entry.expect("a readable directory entry").path();
            (path.extension()? == "toml").then(|| {
                path.file_stem()
                    .expect("a .toml file has a stem")
                    .to_string_lossy()
                    .into_owned()
            })
        })
        .collect();
    ids.sort();

    assert!(
        !ids.is_empty(),
        "{} holds no provider definitions, so every claim below would pass vacuously",
        dir.display()
    );
    ids
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load_provider(id: &str) -> Connector {
    let path = providers_dir().join(format!("{id}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {} ({error})", path.display()));
    shipped_provider::load_definition(id, &source)
        .unwrap_or_else(|error| panic!("providers/{id}.toml does not load: {error}"))
        .connector
}

/// The credentials `connector` places in a query string, as `<provider>:<credential>`.
fn query_placed(connector: &Connector) -> Vec<String> {
    connector
        .auth
        .iter()
        .filter(|method| matches!(method.scheme, AuthScheme::Query { .. }))
        .map(|method| format!("{}:{}", connector.id, method.name))
        .collect()
}

/// **The property: a query-placed credential is the only thing in that connector's query string.**
///
/// Both halves are asserted, because they can disagree — the declaration (`params.query` is empty)
/// and the emitted Flux (no `?` in a URL, and none of the `sep` machinery
/// `crates/connector-flux/src/op.rs` emits for an optional filter). The IR is what a reviewer reads;
/// the Flux is what runs.
#[test]
fn a_query_placed_credential_shares_its_query_string_with_nothing() {
    let mut carriers: Vec<String> = Vec::new();

    for id in shipped() {
        let connector = load_provider(&id);
        let credentials = query_placed(&connector);
        if credentials.is_empty() {
            continue;
        }
        carriers.extend(credentials);

        for operation in &connector.operations {
            assert!(
                operation.params.query.is_empty(),
                "{}: `{}` declares a query parameter while this connector's credential travels in \
                 the query string. Every query value the emitter writes is interpolated verbatim \
                 (crates/connector-flux/src/op.rs:138-143), so a value carrying `&` lands before the \
                 credential the host appends and can inject a parameter of its own. Carry the free \
                 text in a body instead, the way providers/trello.toml does, or record why this \
                 vendor leaves no choice — C-159 §2's hazard account has to be re-measured either way",
                id,
                operation.id
            );

            let flux = emit_operation(&connector, operation)
                .unwrap_or_else(|error| panic!("{}: {} does not emit: {error}", id, operation.id));
            assert!(
                !flux.contains('?'),
                "{}: `{}` emits a `?` into its URL, and this connector's credential travels in the \
                 query string:\n{flux}",
                id,
                operation.id
            );
            assert!(
                !flux.contains("sep = "),
                "{}: `{}` emits the optional-query-parameter machinery, and this connector's \
                 credential travels in the query string:\n{flux}",
                id,
                operation.id
            );
        }
    }

    // Non-vacuity, as a floor rather than as a census. The property above is a claim about the
    // connectors that carry the hazard, so it says nothing at all if none does — and the catalogue
    // declared exactly zero query placements until C-165. A floor of one survives catalogue growth
    // in the way the literal this replaced did not: another connector declaring a query credential
    // only ever *adds* to `carriers`, and is checked by the loop above rather than counted here.
    assert!(
        !carriers.is_empty(),
        "no shipped connector places a credential in a query string, so this file proves nothing. \
         C-159 §2 recorded that state as `unreachable today`; if the catalogue has genuinely \
         returned to it, retire this file rather than leaving it green and empty"
    );
}
