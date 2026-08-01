//! `providers/webflow.toml` exists, emits analyzable Flux, and answers the question C-182 was chosen
//! for: **is this a third variety of "a payload this pipeline cannot type"?**
//!
//! A Webflow CMS collection item's `fieldData` is a flat object whose keys and value shapes are
//! whatever the site owner defined for that collection — a shape that is neither `providers/
//! notion.toml`'s recursive block union (excluded outright, no `$ref` in this `JsonSchema`) nor
//! `providers/miro.toml`'s shallow, bounded discriminated union (expressible as a `oneOf` over four
//! known variants). It is genuinely open: there is no enumerable set of shapes to write down at all,
//! because the set is per-tenant, per-collection, and unbounded.
//!
//! The decision this connector was chosen to make, and the one this file asserts directly rather than
//! leaving as prose: **item creation does not ship.** `webflow-collection-get` ("get a collection
//! schema") ships instead, as the honest substitute — it lets a caller discover `fieldData`'s shape at
//! runtime, which is the thing this file cannot supply at compile time. `fieldData` itself is declared
//! as an open `object` wherever it appears in a response, inside an otherwise fully typed envelope.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// `<repo root>/providers/webflow.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("webflow.toml")
}

fn webflow() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-182 ships the Webflow connector",
            path.display()
        )
    });
    shipped_provider::load_definition("webflow", &source)
        .expect("providers/webflow.toml does not load")
        .connector
}

fn op<'a>(connector: &'a Connector, id: &str) -> &'a connector_spec::Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("no operation named {id:?} in the curated set"))
}

/// The connector exists, loads through the real loader, and is the one C-182 describes.
#[test]
fn the_webflow_connector_loads() {
    let connector = webflow();

    assert_eq!(connector.id, "webflow");
    assert_eq!(connector.vendor, "Webflow");
    assert_eq!(
        connector.base_url, "https://api.webflow.com/v2",
        "one tenant-independent host; site_id is a per-call argument, not a config binding"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// The curated set: site discovery (also `verify`), collection discovery, the collection schema read
/// that answers the archetype's hazard, the two item reads, and the one write — publish.
#[test]
fn the_curated_operation_set_is_the_one_this_story_selected() {
    let connector = webflow();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "webflow-collection-get",
            "webflow-collection-item-get",
            "webflow-collection-item-list",
            "webflow-collection-list",
            "webflow-site-list",
            "webflow-site-publish",
        ],
        "the curated set changed from the one C-182 names"
    );
}

/// **The central archetype assertion: item creation is not shipped.** Neither Notion's exclusion nor
/// Miro's `oneOf` applies to a genuinely open, per-tenant object — no operation may write a `fieldData`
/// body field, because nothing static can describe the shape it would have to satisfy.
#[test]
fn item_creation_is_not_shipped_because_fielddata_is_tenant_defined() {
    let connector = webflow();
    for operation in &connector.operations {
        assert!(
            !(operation.method == HttpMethod::Post && operation.path.ends_with("/items")),
            "`{}` is a POST to the generic items-collection path {:?} — Webflow's item \
             create/bulk-create lives there and is deliberately excluded (fieldData is \
             tenant-defined; see the provider file's header comment)",
            operation.id,
            operation.path
        );
        for field in &operation.params.body {
            assert_ne!(
                field.name, "fieldData",
                "`{}` declares a `fieldData` body field — that is exactly the shape this connector \
                 refuses to type, because it is defined per-tenant, per-collection, with no bound",
                operation.id
            );
            assert_ne!(
                field.wire.as_deref(),
                Some("fieldData"),
                "`{}` wires a body field to `fieldData` — same problem, via `wire` instead of `name`",
                operation.id
            );
        }
    }
}

/// **The honest substitute: `webflow-collection-get` ships as the runtime discovery mechanism.** Its
/// response must actually carry the field definitions a caller needs to learn `fieldData`'s shape.
#[test]
fn collection_get_ships_as_the_runtime_substitute_for_typing_fielddata() {
    let connector = webflow();
    let collection_get = op(&connector, "webflow-collection-get");

    assert_eq!(collection_get.method, HttpMethod::Get);
    assert!(
        !collection_get.path.contains("/items"),
        "the schema-discovery read must be the collection itself, not an item"
    );

    let schema = collection_get.response_schema.as_ref().expect(
        "webflow-collection-get must declare a response_schema — it is the connector's \
                 answer to the archetype hazard",
    );
    let text = schema.to_string();
    assert!(
        text.contains("fields"),
        "webflow-collection-get's response_schema does not mention `fields` — it would not let a \
         caller discover a collection's own field definitions:\n{text}"
    );
}

/// **`fieldData` is declared as an open object wherever it appears**, inside an otherwise typed
/// envelope — the honest declaration for a shape that is neither recursive (Notion) nor a bounded
/// union (Miro), but genuinely unenumerable.
#[test]
fn fielddata_is_declared_open_in_every_item_read() {
    let connector = webflow();
    for id in [
        "webflow-collection-item-list",
        "webflow-collection-item-get",
    ] {
        let operation = op(&connector, id);
        let schema = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("`{id}` declares no response_schema at all"));
        let text = schema.to_string();
        assert!(
            text.contains("fieldData"),
            "`{id}`'s response_schema does not mention fieldData at all:\n{text}"
        );
        // The envelope around fieldData must still be informative — id, timestamps, flags — so the
        // operation is not gamed into a permissive placeholder by omitting everything else.
        assert!(
            text.contains("isDraft") && text.contains("isArchived"),
            "`{id}`'s response_schema should type the envelope around fieldData, not just fieldData \
             itself:\n{text}"
        );
    }
}

/// No request body anywhere declares an array — this connector never needed C-168/C-185's gap
/// (`BodyNode` builds nested objects via `wire` and never arrays). It was designed around avoiding
/// exactly that: item creation is excluded rather than expressed as an array body.
#[test]
fn no_body_field_declares_an_array_this_connector_does_not_need_c_185() {
    let connector = webflow();
    for operation in &connector.operations {
        for field in &operation.params.body {
            assert_ne!(
                field.schema.get("type").and_then(|value| value.as_str()),
                Some("array"),
                "`{}`'s body field `{}` declares an array — this connector was designed around \
                 avoiding exactly that (see C-185); if a real Webflow operation needs one, exclude it \
                 and cite C-185 rather than working around the gap",
                operation.id,
                field.name
            );
        }
    }
}

/// No operation declares a query parameter — this connector avoids the unencoded-query hazard
/// (`zendesk-ticket-search`, AGENTS.md's *Intentional gaps*) entirely rather than risking it.
#[test]
fn no_operation_declares_a_query_parameter() {
    let connector = webflow();
    for operation in &connector.operations {
        assert!(
            operation.params.query.is_empty(),
            "`{}` declares a query parameter — this connector's pagination and filtering are \
             deliberately excluded rather than risking the unencoded-query hazard",
            operation.id
        );
    }
}

/// The publish operation has immediate public effect and must be declared `high`, not an ordinary
/// write's `medium` — the same tier `providers/cloudflare.toml` gives its cache purge and
/// `providers/launchdarkly.toml` gives its flag toggle.
#[test]
fn site_publish_is_declared_high_risk_for_its_immediate_public_effect() {
    let connector = webflow();
    let publish = op(&connector, "webflow-site-publish");

    assert_eq!(publish.method, HttpMethod::Post);
    assert_eq!(
        publish.risk,
        Risk::High,
        "publishing a site has immediate, externally-visible effect for every visitor — the same \
         tier as cloudflare's cache purge and launchdarkly's flag toggle, not an ordinary write"
    );
    assert!(
        publish.params.body.is_empty(),
        "webflow-site-publish should send no body — Webflow's optional customDomains selector is \
         itself an array this connector cannot express (C-185)"
    );
}

/// `POST` is refused `idempotency = "idempotent"` by `check_write_metadata` regardless of vendor
/// truth (C-186) — publishing the same staged state twice is, in effect, idempotent, but the emitter
/// allows nothing else on a `POST`, so `non_idempotent` is declared and the refusal itself is what
/// backs that claim.
#[test]
fn site_publish_is_forced_non_idempotent_by_the_post_rule() {
    let connector = webflow();
    let publish = op(&connector, "webflow-site-publish");

    assert_eq!(publish.idempotency, Idempotency::NonIdempotent);

    let mut idempotent_publish = connector.clone();
    let index = idempotent_publish
        .operations
        .iter()
        .position(|operation| operation.id == "webflow-site-publish")
        .expect("the publish operation exists");
    idempotent_publish.operations[index].idempotency = Idempotency::Idempotent;
    let attempt = emit_operation(&idempotent_publish, &idempotent_publish.operations[index]);
    assert!(
        attempt.is_err(),
        "declaring publish idempotent should be refused by check_write_metadata — if this now \
         emits, the compiler rule changed (see C-186) and the provider file's comment should be \
         revisited rather than this test silently passing"
    );
}

/// Auth is one bearer token, and `verify` is a genuine, parameter-free read: listing the sites the
/// token can see needs no site id, the same role `miro-board-list` and `cloudflare-zone-list` play.
#[test]
fn the_connector_verifies_with_a_read_over_a_bearer_token() {
    let connector = webflow();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(credentials, ["webflow.token"]);

    assert_eq!(connector.verify.as_deref(), Some("webflow-site-list"));
    let verify = op(&connector, "webflow-site-list");
    assert_eq!(verify.method, HttpMethod::Get);
    assert_eq!(verify.risk, Risk::Low);
    assert!(
        verify.params.path.is_empty() && verify.params.query.is_empty(),
        "`verify` runs unattended whenever a settings page opens and must not require an argument a \
         human has not supplied yet — this is why site discovery, not a collection or item read, is \
         `verify`"
    );
}

/// Every scoped operation (all but `webflow-site-list`) declares `site_id` or a downstream id as a
/// required path parameter — the same schema-driven reasoning `providers/cloudflare.toml` gives
/// `zone_id` and `providers/miro.toml` gives `board_id`: this connector's `base_url` is one host with
/// no site placeholder to bind, so `[[config]]` has no vocabulary to pin one site at install time.
#[test]
fn site_id_is_a_per_call_argument_everywhere_but_site_list() {
    let connector = webflow();
    for operation in &connector.operations {
        let site_param = operation
            .params
            .path
            .iter()
            .find(|param| param.name == "site_id");
        if operation.id == "webflow-site-list" {
            assert!(
                site_param.is_none(),
                "`webflow-site-list` is the discovery call and has nothing yet to scope"
            );
            continue;
        }
        if let Some(site_param) = site_param {
            assert!(
                site_param.required,
                "`{}`'s site_id must be required",
                operation.id
            );
        }
    }
    for field in &connector.config {
        assert!(
            !field.binds.contains("site")
                && !field.binds.contains("collection")
                && !field.binds.contains("item"),
            "config field `{}` binds a site/collection/item id; `base_url` carries no such \
             placeholder for `endpoint.<var>` to bind",
            field.name
        );
    }
}

/// The connection-level configuration surface: the token, secret, and carrying no realistic-looking
/// example.
#[test]
fn the_token_is_configurable_and_carries_no_example_value() {
    let connector = webflow();
    let field = connector
        .config
        .iter()
        .find(|field| field.name == "token")
        .expect("the API token is the one thing a human must supply");

    assert!(field.secret);
    assert_eq!(field.binds, "credential.webflow.token");
    assert!(!field.label.is_empty() && !field.help.is_empty());
    assert_eq!(
        field.example, None,
        "a secret field carries no example — a token-shaped placeholder trips secret scanning"
    );
}

/// Every Webflow operation emits Flux that parses, is canonical under flux's own formatter, and loads
/// as exactly one composite op.
#[test]
fn every_webflow_operation_emits_an_analyzable_module() {
    let connector = webflow();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).unwrap_or_else(|error| {
            panic!("operation `{}` is not emittable: {error}", operation.id)
        });

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
    }
}
