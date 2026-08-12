//! The provider-TOML front-end, exercised through both roles the file plays.
//!
//! The fixtures are not invented. The hand-authored one is Zendesk as
//! `docs/designs/provider-operation-inventory.md` §3 records it — including the `<email>/token`
//! user half, which is the shape that decides whether the schema is expressive enough for a real
//! provider. The spec-pointer one is babelforce as §5 records it, down to the base URL being stated
//! explicitly because the vendor document's `servers[0]` is staging.

use connector_spec::{provider, AuthScheme, HttpMethod, Idempotency, ParamPosition, Risk};

/// Zendesk, written out in full with no vendor spec anywhere — the "two front-ends, one IR"
/// requirement, and the shortest route to a generated `.flux` module today.
const HAND_AUTHORED: &str = r#"
id = "zendesk"
vendor = "Zendesk"
base_url = "https://acme.zendesk.com"
description = "Zendesk Support ticketing"
default_auth = [{ credentials = ["zendesk.api_token"] }]

[[auth]]
name = "zendesk.api_token"
scheme = "basic"
env = ["ZENDESK_API_TOKEN"]
user_env = ["ZENDESK_USER"]
user_suffix = "/token"
description = "Zendesk API token; the user half is the agent email with Zendesk's /token marker"

[[operations]]
id = "zendesk.test"
method = "GET"
direction = "read"
path = "/api/v2/users/me.json"
description = "Verify credentials by fetching the authenticated user"
risk = "low"
idempotency = "idempotent"

[[operations]]
id = "zendesk.ticket.show"
method = "GET"
direction = "read"
path = "/api/v2/tickets/{ticket_id}.json"
description = "Show one ticket"
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "ticket_id"
description = "The ticket id"
required = true
schema = { type = "integer", format = "uint64", minimum = 1 }

[[operations]]
id = "zendesk.ticket.search"
method = "GET"
direction = "read"
path = "/api/v2/search.json"
description = "Search tickets with Zendesk search syntax"
risk = "low"
idempotency = "idempotent"

[[operations.params.query]]
name = "query"
required = true
schema = { type = "string" }

[[operations.params.query]]
name = "per_page"
required = false
schema = { type = "integer", minimum = 1, maximum = 100 }

[operations.quirks.rate_limit]
requests = 700
per_seconds = 60
bucket = "zendesk.search"

[[operations]]
id = "zendesk.status"
method = "GET"
direction = "read"
path = "/status.json"
description = "Public status endpoint — no credential at all"
risk = "low"
idempotency = "idempotent"
auth = []
"#;

/// The repository-relative path [`spec_pointer`] pins, spelled as `[spec] path` spells it.
const SPEC_PATH: &str = "specs/babelforce/manager-0.7.0.openapi.json";

/// A two-operation stand-in for the document [`spec_pointer`] pins.
///
/// **This fixture arrived with C-421 and the tests below did not have one before.** They used to go
/// through plain `provider::load`, which answered a spec-backed file with a skeleton and never read
/// a document at all — so `select = "listAgents"` was accepted while matching nothing. Now that the
/// pure loader refuses that input, the pointer is compiled the way a build compiles it, and the two
/// selections have to name operations that exist. That is a strictly stronger fixture: the patch set
/// these tests assert on is now one that actually applied.
///
/// Deliberately small. What is under test here is how the *file* declares a spec and a patch set;
/// ingest itself is `openapi_ingest.rs`, and the join is `spec_backed_provider.rs`, both of which
/// read the committed Zendesk excerpt off disk.
const SPEC_DOCUMENT: &str = r#"{
  "openapi": "3.0.3",
  "info": { "title": "Babelforce Manager (fixture)", "version": "0.7.0" },
  "servers": [{ "url": "https://services.babelforce.com" }],
  "paths": {
    "/api/v2/agents": {
      "get": {
        "operationId": "listAgents",
        "summary": "List agents.",
        "responses": { "200": { "description": "The agents." } }
      }
    },
    "/api/v2/calls/{id}/hangup": {
      "post": {
        "operationId": "hangupCall",
        "summary": "Hang up a call.",
        "parameters": [
          {
            "name": "id",
            "in": "path",
            "required": true,
            "description": "The call.",
            "schema": { "type": "string" }
          }
        ],
        "responses": { "204": { "description": "Hung up." } }
      }
    }
  }
}
"#;

/// The spec cache holding exactly [`SPEC_DOCUMENT`], at the path the pointer names.
fn cache() -> Vec<connector_spec::SpecDocument<'static>> {
    vec![connector_spec::SpecDocument {
        path: SPEC_PATH,
        document: SPEC_DOCUMENT,
    }]
}

/// [`spec_pointer`], compiled against [`cache`].
fn load_pointer(definition: &str) -> connector_spec::LoadedProvider {
    provider::load_with_spec("providers/babelforce.toml", definition, &cache())
        .expect("a spec-pointer provider file must load against its cache")
}

/// Babelforce as a pointer at its vendored spec, plus the patch set C-6 will apply.
///
/// A function rather than a `const` because the declared `sha256` has to be the fixture document's
/// real hash: `load_with_spec` checks it against the bytes it ingests, which is what keeps
/// `Provenance::spec_sha256` a measurement rather than a claim the file makes about itself.
fn spec_pointer() -> String {
    format!(
        r#"
id = "babelforce"
vendor = "Babelforce"
# Stated explicitly: the vendor document's servers[0] is staging, so a positional default would
# point the connector at the dev environment.
base_url = "https://services.babelforce.com"
default_auth = [{{ credentials = ["babelforce.access_token"] }}]

[spec]
path = "{SPEC_PATH}"
source_url = "https://example.invalid/manager.openapi.json"
upstream_version = "0.7.0"
sha256 = "{}"
fetched_at = "2026-07-30T09:00:00Z"

[[auth]]
name = "babelforce.access_token"
scheme = "bearer"
env = ["BABELFORCE_ACCESS_TOKEN"]

[[patch.operations]]
select = "listAgents"
direction = "read"
rename = "babelforce.agent.list"
description = "List and filter agents"
risk = "low"
idempotency = "idempotent"

[patch.operations.quirks.pagination]
page = {{ page_param = "page", size_param = "max", page_size = 100, max_pages = 20 }}

[[patch.operations]]
select = "hangupCall"
direction = "write"
rename = "babelforce.call.hangup"
description = "Hang up a live call"
risk = "destructive"
idempotency = "non_idempotent"
# The spec's root `security` offers the deprecated X-Auth-Access-Id / X-Auth-Access-Token pair as
# an alternative. Ingest must keep seeing it; the overlay is the only place it may be removed.
auth = [{{ credentials = ["babelforce.access_token"] }}]

[[patch.operations.params]]
name = "id"
position = "path"
required = true
description = "Call id"
schema = {{ type = "string", format = "uuid" }}
"#,
        connector_spec::sha256_hex(SPEC_DOCUMENT.as_bytes())
    )
}

/// A hand-authored TOML with no vendor spec present at all produces a complete, valid `Connector`.
#[test]
fn a_hand_authored_file_produces_a_complete_connector() {
    let loaded = provider::load("providers/zendesk.toml", HAND_AUTHORED)
        .expect("a hand-authored provider file must load");

    assert!(
        loaded.is_hand_authored(),
        "no `[spec]` means nothing to ingest and nothing to overlay"
    );
    assert!(loaded.patch.is_empty());

    let connector = &loaded.connector;
    assert_eq!(connector.id, "zendesk");
    assert_eq!(connector.base_url, "https://acme.zendesk.com");
    assert_eq!(connector.operations.len(), 4);

    // The operation contract survived intact — method, path template, metadata and a real schema.
    let show = connector
        .operation("zendesk.ticket.show")
        .expect("declared operation");
    assert_eq!(show.method, HttpMethod::Get);
    assert_eq!(show.path, "/api/v2/tickets/{ticket_id}.json");
    assert_eq!(show.risk, Risk::Low);
    assert_eq!(show.idempotency, Idempotency::Idempotent);
    assert_eq!(show.params.path[0].name, "ticket_id");
    assert_eq!(
        show.params.path[0].schema["minimum"],
        serde_json::json!(1),
        "a numeric bound must not degrade into a string on the way through the loader"
    );

    // Quirks authored inline reach the IR, which is what C-12 compiles into Flux control flow.
    let search = connector
        .operation("zendesk.ticket.search")
        .expect("declared operation");
    let rate_limit = search
        .quirks
        .rate_limit
        .as_ref()
        .expect("the rate limit must survive");
    assert_eq!(rate_limit.requests, 700);

    // And the whole thing is ready for codegen: it round-trips through the canonical encoding
    // `connectors.lock` hashes.
    let encoded = connector.canonical_json().expect("serialize");
    let decoded: connector_spec::Connector = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(connector, &decoded);
}

/// Zendesk's Basic user half is an env value **plus a literal suffix**. If the schema could not say
/// that, the only way to author Zendesk would be to tell an operator to paste `me@corp.com/token`
/// into a variable named for an email address — storing a value that is not the thing it is named
/// after, which is the pre-composed-credential mistake `docs/designs/auth-seam.md` §7.5 rejects.
#[test]
fn a_basic_credential_can_state_a_literal_user_suffix() {
    let loaded = provider::load("providers/zendesk.toml", HAND_AUTHORED).expect("loads");

    let credential = loaded
        .connector
        .auth_method("zendesk.api_token")
        .expect("declared credential");

    assert_eq!(credential.scheme, AuthScheme::Basic);
    assert_eq!(credential.user_env, ["ZENDESK_USER"]);
    assert_eq!(
        credential.user_suffix.as_deref(),
        Some("/token"),
        "the `/token` marker is Zendesk's API syntax and belongs in the connector, not in the \
         operator's environment"
    );
    assert_eq!(
        credential.env,
        ["ZENDESK_API_TOKEN"],
        "the secret half stays the only secret"
    );
}

/// Three auth states, all reachable from the file, all distinct after loading. Getting this wrong
/// is invisible until a connector ships an operation carrying credentials it should not have.
#[test]
fn the_three_auth_states_survive_the_loader() {
    let loaded = provider::load("providers/zendesk.toml", HAND_AUTHORED).expect("loads");
    let connector = &loaded.connector;

    // UNSET — the key is absent, so the operation inherits the connector default.
    let show = connector.operation("zendesk.ticket.show").expect("present");
    assert_eq!(show.auth, None);
    assert_eq!(
        connector.effective_auth(show),
        connector.default_auth.as_slice()
    );

    // EXPLICITLY NONE — `auth = []`, which does *not* inherit.
    let status = connector.operation("zendesk.status").expect("present");
    assert_eq!(status.auth, Some(Vec::new()));
    assert!(
        connector.effective_auth(status).is_empty(),
        "an explicitly unauthenticated operation must not pick up the connector default"
    );

    // NAMED — the connector default, resolving to a declared credential.
    assert_eq!(connector.default_auth.len(), 1);
    assert!(connector.default_auth[0].contains("zendesk.api_token"));
}

/// The loader records the hash of the file it read, which is what lets `connectors.lock` (C-7)
/// notice an edited provider file without re-reading it.
#[test]
fn the_provider_file_hash_is_recorded_and_is_a_function_of_the_bytes() {
    let first = provider::load("providers/zendesk.toml", HAND_AUTHORED).expect("loads");
    let again = provider::load("providers/zendesk.toml", HAND_AUTHORED).expect("loads");
    let edited = provider::load(
        "providers/zendesk.toml",
        &HAND_AUTHORED.replace("Zendesk Support ticketing", "Zendesk Support"),
    )
    .expect("loads");

    let hash = |loaded: &connector_spec::LoadedProvider| {
        loaded
            .connector
            .provenance
            .toml_sha256
            .clone()
            .expect("the loader records the file hash")
    };

    assert_eq!(hash(&first).len(), 64, "lowercase hex SHA-256");
    assert_eq!(hash(&first), hash(&again));
    assert_ne!(
        hash(&first),
        hash(&edited),
        "an edited provider file must produce a different hash, or drift-check is blind"
    );

    // A hand-authored connector has no spec, so the spec half of provenance stays empty rather
    // than being filled with something invented.
    assert_eq!(first.connector.provenance.spec_sha256, None);
    assert_eq!(first.connector.provenance.source_url, None);
}

/// A file that only points at a spec plus patches parses into the patch set C-6 consumes.
#[test]
fn a_spec_pointer_file_produces_the_patch_set() {
    let loaded = load_pointer(&spec_pointer());

    assert!(!loaded.is_hand_authored());

    // A single `[spec]` table is the one-element case of `[[spec]]` — C-410.
    assert_eq!(loaded.specs.len(), 1);
    let spec = loaded.specs.first().expect("the spec pointer is present");
    assert_eq!(spec.path, "specs/babelforce/manager-0.7.0.openapi.json");
    assert_eq!(spec.upstream_version.as_deref(), Some("0.7.0"));
    assert_eq!(
        spec.service(),
        connector_spec::DEFAULT_SERVICE,
        "a document that names no service joins the reserved one, exactly as it did before the key \
         existed"
    );

    // `[spec]` folds into provenance, so drift-check (C-14) and the lockfile (C-7) read one place.
    let provenance = &loaded.connector.provenance;
    assert_eq!(provenance.spec_sha256.as_deref(), spec.sha256.as_deref());
    assert_eq!(provenance.upstream_version.as_deref(), Some("0.7.0"));
    assert_eq!(
        provenance.fetched_at.as_deref(),
        Some("2026-07-30T09:00:00Z")
    );
    assert!(provenance.toml_sha256.is_some());
    // And the per-document record carries the same document — one entry, not one per connector.
    assert_eq!(provenance.specs, loaded.specs);

    // The connector declares no operations of its own: every one it publishes arrived through the
    // document, renamed by the overlay. That the file is valid with an empty `[[operations]]` list
    // is the point of this role.
    assert_eq!(
        loaded
            .connector
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        vec!["babelforce.agent.list", "babelforce.call.hangup"]
    );

    // The patch set itself.
    assert_eq!(loaded.patch.operations.len(), 2);

    let agents = &loaded.patch.operations[0];
    assert_eq!(agents.select, "listAgents");
    assert_eq!(agents.rename.as_deref(), Some("babelforce.agent.list"));
    assert_eq!(agents.risk, Some(Risk::Low));
    assert!(
        agents
            .quirks
            .as_ref()
            .is_some_and(|q| q.pagination.is_some()),
        "pagination is declared in the patch because no spec publishes it"
    );

    let hangup = &loaded.patch.operations[1];
    assert_eq!(hangup.select, "hangupCall");
    assert_eq!(hangup.risk, Some(Risk::Destructive));
    assert_eq!(hangup.idempotency, Some(Idempotency::NonIdempotent));
    assert_eq!(
        hangup
            .auth
            .as_ref()
            .expect("the overlay states the auth explicitly")
            .len(),
        1,
        "the overlay is where the deprecated header pair is removed — ingest must keep seeing it"
    );

    let param = &hangup.params[0];
    assert_eq!(param.name, "id");
    assert_eq!(param.position, ParamPosition::Path);
    assert_eq!(param.required, Some(true));
    assert_eq!(
        param.schema.as_ref().expect("schema override")["format"],
        serde_json::json!("uuid")
    );
}

/// An override the author did not write stays `None`, so the overlay can tell "not stated" from
/// "stated as the value the spec happens to have". A spec that later changes must move an unstated
/// field and must not move a stated one.
#[test]
fn unstated_patch_overrides_stay_distinguishable_from_stated_ones() {
    let loaded = load_pointer(&spec_pointer());

    let agents = &loaded.patch.operations[0];
    assert_eq!(agents.auth, None, "auth was not stated for this operation");
    assert!(agents.params.is_empty());

    let hangup = &loaded.patch.operations[1];
    assert_eq!(hangup.quirks, None, "no quirks were stated for this one");
}

/// A file may carry both roles at once: a spec to ingest *and* an operation the vendor document
/// does not describe. Nothing in the model forces the two apart.
#[test]
fn a_file_may_point_at_a_spec_and_still_declare_operations_inline() {
    let source = format!(
        "{}
[[operations]]
id = \"babelforce.health\"
method = \"GET\"
direction = \"read\"
path = \"/health\"
risk = \"low\"
idempotency = \"idempotent\"
auth = []
",
        spec_pointer()
    );

    let loaded = load_pointer(&source);
    // The inline operation first, then the two the patch set selected — one connector, both roles.
    assert_eq!(
        loaded
            .connector
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "babelforce.health",
            "babelforce.agent.list",
            "babelforce.call.hangup"
        ]
    );
    assert_eq!(loaded.patch.operations.len(), 2);
}

/// The credential order an author happens to type inside one mechanism must not reach the encoding:
/// `connectors.lock` hashes it, and two files that mean the same thing must hash the same.
#[test]
fn authoring_order_inside_a_mechanism_does_not_reach_the_ir() {
    let template = r#"
id = "babelforce"
base_url = "https://services.babelforce.com"
default_auth = [{ credentials = [CREDENTIALS] }]

[[auth]]
name = "babelforce.access_id"
scheme = { header = { name = "X-Auth-Access-Id" } }
env = ["BABELFORCE_ACCESS_ID"]

[[auth]]
name = "babelforce.access_token"
scheme = { header = { name = "X-Auth-Access-Token" } }
env = ["BABELFORCE_ACCESS_TOKEN"]

[[operations]]
id = "babelforce.call.list"
method = "GET"
direction = "read"
path = "/api/v2/calls/reporting"
risk = "low"
idempotency = "idempotent"
"#;

    let one_way = provider::load(
        "providers/babelforce.toml",
        &template.replace(
            "CREDENTIALS",
            r#""babelforce.access_id", "babelforce.access_token""#,
        ),
    )
    .expect("loads");
    let the_other = provider::load(
        "providers/babelforce.toml",
        &template.replace(
            "CREDENTIALS",
            r#""babelforce.access_token", "babelforce.access_id""#,
        ),
    )
    .expect("loads");

    // The AND-set survives as a set of two, and encodes identically either way round.
    assert_eq!(one_way.connector.default_auth[0].len(), 2);
    assert_eq!(
        one_way.connector.default_auth,
        the_other.connector.default_auth
    );
}
