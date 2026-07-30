//! **Every shipped provider compiles**, through the real pipeline, against the real repository.
//!
//! `connector-spec`'s `shipped_providers.rs` proves the three committed definitions *load*. That is
//! a strictly weaker claim than this one: a definition can load and still be something the emitter
//! refuses, or — worse — something it compiles into a request the vendor accepts and ignores. Both
//! happened, and both were invisible to every other test in the tree:
//!
//! - `babelforce-agent-status-update` declared a body field named `presence.name`, which the emitter
//!   refused. One refusal aborted the whole run, so **no `.flux` file was generated at all**.
//! - `providers/zendesk.toml` carried each body field's wire path in its `description` prose, so the
//!   three `PUT /api/v2/tickets/{ticket_id}.json` operations emitted a **flat** body. Zendesk ignores
//!   a flat body and answers 200. Nothing in the IR distinguished that from a correct result, and
//!   nothing here would have caught it either — which is why the nested-body assertion below is
//!   spelled out field by field rather than left to "it compiles".
//!
//! The test reads `providers/` from the repository root rather than a fixture, because the thing
//! under test is what ships. It goes through [`pipeline::plan`], which performs no writes, so it can
//! run before `build` has ever been executed and cannot disturb the committed artifacts.

use std::path::{Path, PathBuf};

use connector_cli::pipeline::{self, PlannedArtifact};
use connector_cli::workspace::Workspace;

/// Every provider this repository ships: the three C-17 names, then one per connector story.
const SHIPPED: &[&str] = &["zendesk", "freshdesk", "babelforce", "github"];

/// The repository root, derived from this crate's manifest directory so the test is independent of
/// the working directory a runner happens to use.
fn workspace() -> Workspace {
    Workspace::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the repository root exists"),
    )
}

/// The artifacts a build would write for `provider`, or a panic naming why it could not.
fn plan_for(provider: &str) -> Vec<PlannedArtifact> {
    pipeline::plan(&workspace(), Some(provider))
        .unwrap_or_else(|error| panic!("providers/{provider}.toml does not compile: {error:#}"))
        .artifacts
}

/// The shipped provider definition, loaded.
fn load(provider: &str) -> connector_spec::Connector {
    let source =
        std::fs::read_to_string(workspace().providers_dir().join(format!("{provider}.toml")))
            .expect("the shipped provider file is readable");
    connector_spec::provider::load(&format!("providers/{provider}.toml"), &source)
        .expect("the shipped provider file loads")
        .connector
}

/// Whether the plan writes `path`, spelled relative to the repository root.
fn plans(artifacts: &[PlannedArtifact], path: &str) -> bool {
    let wanted = PathBuf::from(path);
    artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with(&wanted))
}

/// The planned contents of one artifact path.
fn planned(provider: &str, file: &str) -> String {
    let artifacts = plan_for(provider);
    let wanted = PathBuf::from(file);
    artifacts
        .into_iter()
        .find(|artifact| artifact.path.ends_with(&wanted))
        .unwrap_or_else(|| panic!("a build of {provider} must plan {file}"))
        .contents
}

/// **The gate this story exists to open.** Every shipped provider compiles to both its artifacts.
///
/// One provider the emitter refuses aborts the whole run — `plan` compiles everything before
/// anything is written — so a single unemittable operation is the difference between three
/// generated modules and none.
#[test]
fn every_shipped_provider_compiles() {
    for provider in SHIPPED {
        let artifacts = plan_for(provider);
        let operations = load(provider).operations.len();

        // The two that ship. A connector is a module *plus* a manifest, never one of them.
        assert!(
            plans(&artifacts, &format!("connectors/{provider}.flux")),
            "a build of {provider} must plan connectors/{provider}.flux"
        );
        assert!(
            plans(&artifacts, &format!("connectors/{provider}.connector.toml")),
            "a build of {provider} must plan connectors/{provider}.connector.toml"
        );

        // The catalog's half (C-38): the generated table, plus one rendering per operation.
        assert!(
            plans(
                &artifacts,
                &format!("crates/catalog/src/generated/{provider}.rs")
            ),
            "a build of {provider} must plan its catalog table"
        );
        assert_eq!(
            artifacts.len(),
            3 + operations,
            "{provider} publishes {operations} operations, so a build plans its module, its \
             manifest, its catalog table and one rendering each; it planned {} artifacts",
            artifacts.len()
        );
    }
}

/// Every generated module declares every operation its provider does — an emitter that dropped one
/// would still produce a module that parses.
#[test]
fn every_shipped_operation_reaches_its_module() {
    for provider in SHIPPED {
        let module = planned(provider, &format!("{provider}.flux"));
        let source =
            std::fs::read_to_string(workspace().providers_dir().join(format!("{provider}.toml")))
                .expect("the shipped provider file is readable");
        let loaded = connector_spec::provider::load(&format!("providers/{provider}.toml"), &source)
            .expect("the shipped provider file loads");

        for operation in &loaded.connector.operations {
            assert!(
                module.contains(&format!("op {}(", operation.id))
                    || module.contains(&format!("op {} ", operation.id)),
                "connectors/{provider}.flux does not declare `{}`:\n{module}",
                operation.id
            );
        }
    }
}

/// **The silent failure, pinned.** Zendesk's wire body is
/// `{"ticket": {"updated_stamp": …, "safe_update": true, "comment": {"body": …, "public": …}}}`
/// (`docs/designs/provider-operation-inventory.md` §3.3.1). A flat body is not an error Zendesk
/// reports — it answers 200 and applies nothing — so the shape is asserted here in full rather than
/// approximated by "the payload mentions `body`".
#[test]
fn zendesk_writes_a_nested_body() {
    let module = planned("zendesk", "zendesk.flux");

    assert!(
        module.contains(
            "$payload = { ticket: { comment: { body: $body, public: $public }, \
             safe_update: $safe_update, updated_stamp: $updated_stamp } }"
        ),
        "`zendesk-ticket-comment-add` must nest its comment under `ticket.comment`:\n{module}"
    );
    assert!(
        module.contains("$payload = { ticket: { additional_tags: $tags, safe_update: $safe_update, updated_stamp: $updated_stamp } }"),
        "`zendesk-ticket-tag-add` must write `ticket.additional_tags` — sending `tags` *replaces* \
         the ticket's tags (inventory §3.3.3):\n{module}"
    );
    assert!(
        !module.contains("$payload = { body:")
            && !module.contains(", updated_stamp: $updated_stamp }\n"),
        "no Zendesk payload may put a ticket field at the root of the body — Zendesk ignores it \
         and answers 200:\n{module}"
    );
}

/// babelforce's agent-status update writes `presence.name`. This is the refusal quoted in the story:
/// it is the operation that stopped the whole build, and it is loud rather than silent only because
/// the dotted spelling happened to be visible in the field's `name`.
#[test]
fn babelforce_nests_the_presence_label() {
    let module = planned("babelforce", "babelforce.flux");
    assert!(
        module.contains("$payload = { enabled: $enabled, presence: { name: $presence_name } }"),
        "`babelforce-agent-status-update` must nest `presence.name`:\n{module}"
    );
}

/// A free-form object body — babelforce's two session-variable writes — reaches the request. Before
/// `ParamSet::body_schema` existed these declared no body parameter at all and emitted a `PUT` with
/// no body, which is indistinguishable from a legitimately bodiless write.
#[test]
fn babelforce_sends_its_free_form_session_bodies() {
    let module = planned("babelforce", "babelforce.flux");
    assert_eq!(
        module
            .matches("$payload = parse($body, as: \"json\")")
            .count(),
        2,
        "both session-variable operations must send the caller's body:\n{module}"
    );
}

/// A parameter whose wire name differs from its caller-facing one travels under the **vendor's**
/// spelling, while the op declares the name a caller reads (inventory §4.2 op 2).
#[test]
fn freshdesk_query_aliases_travel_under_their_wire_name() {
    let module = planned("freshdesk", "freshdesk.flux");
    assert!(
        module.contains("fmt(\"{url}{sep}requester_id={req_id}\")"),
        "`req_id` must reach Freshdesk as `requester_id`:\n{module}"
    );
    assert!(
        module.contains("req_id: String"),
        "the op must declare the caller-facing name:\n{module}"
    );
}
