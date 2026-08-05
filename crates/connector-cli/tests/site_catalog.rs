//! `web/public/catalog.json` is a **checked** artifact, and it is the one a website is written
//! against (C-42). It sits in VitePress's `public/` directory, which is served verbatim at the site
//! root, so the explorer reads it with no copy step (C-44).
//!
//! The site's whole reason for existing is that catalogue data must never be hand-maintained — that
//! is the action-proxy failure this repository exists to correct, re-enacted in JavaScript. So the
//! committed JSON is recomputed here from `providers/*.toml` and compared, exactly the way
//! `catalog_artifacts.rs` checks the Rust catalogue.
//!
//! Two of these assertions are not about staleness at all and are the reason the story exists:
//!
//! - **`status` is derived, not decorative.** An operation that does not work must say so, and say
//!   it from a rule applied to the IR rather than from a list someone maintains by hand.
//! - **No credential value ever reaches the document.** Env var *names* only. The check runs the
//!   real binary with a credential variable set to a sentinel and asserts the sentinel is nowhere in
//!   the output.
//!
//! The format itself is specified in `docs/designs/catalog-json.md`.

use std::path::{Path, PathBuf};

use connector_cli::pipeline;
use connector_cli::workspace::Workspace;
use serde_json::Value;

mod common;

use common::Fixture;

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The document's path, relative to the repository root. Chosen by C-42, moved into the site's own
/// `public/` tree by C-44, and named here so a change to it is a change to a test rather than a
/// silent break of whatever reads it.
const CATALOG_JSON: &str = "web/public/catalog.json";

/// The repository root, derived from this crate's manifest directory so the test is independent of
/// the working directory a runner happens to use.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// Every provider this repository ships, **read from `providers/` rather than listed here** (C-54).
///
/// The document is generated from this directory, so the check that it is complete has to iterate the
/// same directory. A constant would let a provider be published to the site — or omitted from it —
/// without this test having an opinion. Empty is a failure rather than a vacuous pass.
fn shipped() -> Vec<String> {
    let dir = repo_root().join("providers");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .map(|entry| entry.expect("readable directory entry").file_name())
        .filter_map(|name| {
            name.to_string_lossy()
                .strip_suffix(".toml")
                .map(str::to_string)
        })
        .collect();
    names.sort();

    assert!(
        !names.is_empty(),
        "{} holds no provider definitions, so the completeness gate below would pass vacuously",
        dir.display()
    );
    names
}

/// The committed document, parsed.
fn committed() -> Value {
    let path = repo_root().join(CATALOG_JSON);
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{CATALOG_JSON} is missing or unreadable ({error}) — the site has no catalogue to read. \
             Run `cargo run -p connector-cli -- build`"
        )
    });
    serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("{CATALOG_JSON} is not valid JSON: {error}"))
}

/// Every operation in the document, flattened across providers.
fn operations(document: &Value) -> Vec<&Value> {
    document["providers"]
        .as_array()
        .unwrap_or_else(|| panic!("{CATALOG_JSON} carries a `providers` array"))
        .iter()
        .flat_map(|provider| {
            provider["operations"]
                .as_array()
                .unwrap_or_else(|| panic!("every provider carries an `operations` array"))
        })
        .collect()
}

/// One operation by id.
fn operation<'a>(document: &'a Value, id: &str) -> &'a Value {
    operations(document)
        .into_iter()
        .find(|operation| operation["id"] == Value::String(id.to_string()))
        .unwrap_or_else(|| panic!("{CATALOG_JSON} carries the operation `{id}`"))
}

/// The `code` of every issue on an operation's status.
fn issue_codes(operation: &Value) -> Vec<String> {
    operation["status"]["issues"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "operation `{}` carries a `status.issues` array",
                operation["id"]
            )
        })
        .iter()
        .map(|issue| {
            issue["code"]
                .as_str()
                .expect("every issue carries a string `code`")
                .to_string()
        })
        .collect()
}

/// **The document is planned and committed like every other artifact.**
///
/// A build writes it, `diff` reports it stale, and the committed bytes are a fixed point of a
/// rebuild. Without this the site's data is generated in name only.
#[test]
fn the_build_writes_and_checks_site_catalog_json() {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("every shipped provider compiles");

    let planned: Vec<String> = plan
        .artifacts
        .iter()
        .map(|artifact| {
            workspace
                .display_path(&artifact.path)
                .display()
                .to_string()
                .replace('\\', "/")
        })
        .collect();

    assert!(
        planned.iter().any(|path| path == CATALOG_JSON),
        "a build plans no {CATALOG_JSON}; the site would have to hand-maintain the catalogue. \
         Planned artifacts:\n  {}",
        planned.join("\n  ")
    );

    let stale: Vec<String> = plan
        .changes()
        .map(|artifact| workspace.display_path(&artifact.path).display().to_string())
        .collect();
    assert!(
        stale.is_empty(),
        "a rebuild would change committed artifacts — run `cargo run -p connector-cli -- build`:\n  {}",
        stale.join("\n  ")
    );
}

/// **Every shipped operation is in the document**, with the fields the explorer is written against:
/// the metadata, the typed parameters with their JSON Schema, and the generated Flux verbatim.
#[test]
fn every_shipped_operation_carries_its_metadata_and_its_flux() {
    let document = committed();

    for provider in shipped() {
        let provider = provider.as_str();
        let path = repo_root()
            .join("providers")
            .join(format!("{provider}.toml"));
        let source = std::fs::read_to_string(&path).expect("a shipped provider definition");
        let connector = shipped_provider::load_definition(provider, &source)
            .expect("a shipped provider loads")
            .connector;

        for declared in &connector.operations {
            let entry = operation(&document, &declared.id);
            assert_eq!(entry["provider"], Value::String(provider.to_string()));
            assert_eq!(entry["path"], Value::String(declared.path.clone()));
            assert!(
                entry["risk"].is_string() && entry["idempotency"].is_string(),
                "operation `{}` is missing its risk/idempotency",
                declared.id
            );
            assert!(
                matches!(entry["direction"].as_str(), Some("read" | "write")),
                "operation `{}` publishes no closed vendor-state direction",
                declared.id
            );
            let semantic_effects = entry["semantic_effects"].as_array().unwrap_or_else(|| {
                panic!(
                    "operation `{}` carries a `semantic_effects` array, `[]` when it has none",
                    declared.id
                )
            });
            let expected_effects: Vec<Value> = declared
                .semantic_effects
                .iter()
                .map(|effect| Value::String(effect.tag().to_string()))
                .collect();
            assert_eq!(
                semantic_effects, &expected_effects,
                "operation `{}` publishes semantic effects different from its IR",
                declared.id
            );

            let flux = entry["flux"].as_str().unwrap_or_else(|| {
                panic!(
                    "operation `{}` carries its Flux source verbatim",
                    declared.id
                )
            });
            let emitted = connector_flux::emit_operation(&connector, declared)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", declared.id));
            assert_eq!(
                flux, emitted,
                "the Flux in {CATALOG_JSON} for `{}` is not what the emitter produces",
                declared.id
            );

            // Typed parameters keep their JSON Schema — the whole point of carrying the IR's types
            // through rather than a stringly-typed shadow of them.
            let parameters = entry["parameters"].as_array().unwrap_or_else(|| {
                panic!("operation `{}` carries a `parameters` array", declared.id)
            });
            assert_eq!(parameters.len(), declared.params.iter().count());
            for parameter in parameters {
                assert!(
                    parameter["schema"].is_object(),
                    "parameter `{}` of `{}` lost its JSON Schema",
                    parameter["name"],
                    declared.id
                );
                assert!(parameter["in"].is_string());
            }
        }
    }
}

/// **`status` is derived from the IR, and it is the field that carries the honesty.**
///
/// Three facts the repository already states in prose, asserted here as machine-readable data:
///
/// - C-30 closed the query-encoding issue for `zendesk-ticket-search`, so neither it nor the other
///   Zendesk operations publish the retired status token;
/// - every freshdesk operation ships with no credential at all (C-17), and zendesk's do not;
/// - `works` is true only when nothing is wrong, so a consumer can filter on one boolean.
#[test]
fn the_status_of_every_operation_is_derived_from_the_ir() {
    let document = committed();

    let search = operation(&document, "zendesk-ticket-search");
    assert!(
        !issue_codes(search).contains(&"unencodable-query-value".to_string()),
        "`zendesk-ticket-search` still publishes the query gap C-30 closed. Issues: {:?}",
        issue_codes(search)
    );

    let show = operation(&document, "zendesk-ticket-show");
    assert!(
        !issue_codes(show).contains(&"unencodable-query-value".to_string()),
        "`zendesk-ticket-show` publishes the retired C-30 issue. Issues: {:?}",
        issue_codes(show)
    );

    for entry in operations(&document) {
        let codes = issue_codes(entry);
        let provider = entry["provider"].as_str().expect("a provider id");
        let has_no_credential = codes.iter().any(|code| code == "no-credential");

        if provider == "freshdesk" {
            assert!(
                has_no_credential,
                "freshdesk operation `{}` is published without saying it ships with no credential",
                entry["id"]
            );
        } else {
            assert!(
                !has_no_credential,
                "operation `{}` is flagged as having no credential, but `{provider}` declares one",
                entry["id"]
            );
        }

        assert_eq!(
            entry["status"]["works"],
            Value::Bool(codes.is_empty()),
            "operation `{}` reports `works` out of step with its issues ({codes:?})",
            entry["id"]
        );
    }
}

/// **No credential value, anywhere — env var names only, in both call directions.**
///
/// The check is a real build of a fixture connector, run as a subprocess with **two** credential
/// variables set to sentinels: the outbound bearer token, and the inbound HMAC signing secret a
/// channel binding verifies with (C-83). If any future edit resolves a credential while generating
/// the document, the sentinel lands in the JSON and this fails. Asserting the *names* are present as
/// well is what keeps the test from passing because nothing about auth is emitted at all.
///
/// The signing half is the one worth being explicit about. A verification block is the one place in
/// the document where a secret is *near* the thing that uses it, so it is the one place a plausible
/// edit would inline a value instead of a reference. The assertion is therefore positive as well as
/// negative: the published block must name `acme.signing_secret`, which is a credential of
/// `auth.credentials`, and it must reach the reader through the binding rather than only through the
/// credential list.
#[test]
fn no_credential_value_reaches_the_document() {
    const ENV_NAME: &str = "FLUX_CONNECTORS_C42_SENTINEL";
    const ENV_VALUE: &str = "sentinel-credential-value-must-never-be-emitted";
    const SIGNING_ENV_NAME: &str = "FLUX_CONNECTORS_C83_SIGNING_SENTINEL";
    const SIGNING_ENV_VALUE: &str = "sentinel-signing-secret-must-never-be-emitted";

    let fixture = Fixture::new("c42-credential");
    fixture.write_provider("acme", &inbound_fixture(ENV_NAME, SIGNING_ENV_NAME));

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_flux-connectors"))
        .args(["build", "--root"])
        .arg(fixture.root())
        .env(ENV_NAME, ENV_VALUE)
        .env(SIGNING_ENV_NAME, SIGNING_ENV_VALUE)
        .output()
        .expect("the flux-connectors binary runs");
    assert!(
        output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        fixture.exists(CATALOG_JSON),
        "a build wrote no {CATALOG_JSON}"
    );
    let document = fixture.read(CATALOG_JSON);

    for (name, value) in [(ENV_NAME, ENV_VALUE), (SIGNING_ENV_NAME, SIGNING_ENV_VALUE)] {
        assert!(
            !document.contains(value),
            "{CATALOG_JSON} carries a resolved value for `{name}` — env var names only"
        );
        assert!(
            document.contains(name),
            "{CATALOG_JSON} does not name the environment variable `{name}`, so the check above \
             passes vacuously"
        );
    }

    // The positive half: the binding's verification block names the **credential**, and that name
    // resolves against the connector's own credential list.
    let parsed: Value = serde_json::from_str(&document)
        .unwrap_or_else(|error| panic!("{CATALOG_JSON} is not valid JSON: {error}"));
    let provider = &parsed["providers"][0];
    let config = provider["config"]
        .as_array()
        .expect("the fixture publishes its configuration form");
    assert_eq!(config.len(), 2, "both credential inputs reach the form");
    assert!(
        config.iter().all(|field| field["level"] == "connection"),
        "credential-bound fields publish their derived connection level: {config:?}"
    );
    let secret = &provider["channels"][0]["verification"]["hmac"]["secret"];
    assert_eq!(
        secret,
        &Value::String("acme.signing_secret".to_string()),
        "the published binding does not name the credential its signature is verified with; \
         verification block was {}",
        provider["channels"][0]["verification"]
    );
    let declared: Vec<&Value> = provider["auth"]["credentials"]
        .as_array()
        .expect("a credentials array")
        .iter()
        .map(|credential| &credential["name"])
        .collect();
    assert!(
        declared.contains(&secret),
        "the verification secret `{secret}` is not one of the connector's declared credentials \
         {declared:?} — a binding must reference the one namespace, never introduce a second"
    );
}

/// A fixture connector with an inbound surface: one event, one verified webhook binding, and the
/// reply operation that answers it.
///
/// Shared by the credential and the loud-verification checks below so that both are made against a
/// connector of the same shape — the shape `providers/slack.toml` ships.
fn inbound_fixture(token_env: &str, signing_env: &str) -> String {
    format!(
        r#"id = "acme"
vendor = "Acme"
authority = "com.acme.api"
api_version = "v1"
base_url = "https://api.acme.example"
description = "A hand-authored fixture connector."
default_auth = [{{ credentials = ["acme.token"] }}]

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["{token_env}"]
description = "The fixture credential."

[[auth]]
name = "acme.signing_secret"
scheme = "signing"
env = ["{signing_env}"]
description = "The fixture webhook signing secret."

[[config]]
name = "access_token"
label = "Access token"
help = "The token Acme issued for this connection."
format = "token"
secret = true
binds = "credential.acme.token"

[[config]]
name = "signing_secret"
label = "Webhook signing secret"
help = "The secret Acme uses to sign webhook deliveries."
format = "token"
secret = true
binds = "credential.acme.signing_secret"

[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/v1/things/{{thing_id}}"
description = "Fetch one thing."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "thing_id"
required = true
schema = {{ type = "integer" }}

[[operations]]
id = "acme-reply"
method = "POST"
direction = "write"
path = "/v1/reply"
description = "Answer a delivery."
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "room"
required = true
schema = {{ type = "string" }}

[[operations.params.body]]
name = "text"
required = true
schema = {{ type = "string" }}

[[events]]
name = "thing.created"
description = "A thing was created."
group = "Things"

[[channels]]
name = "hook"
transport = "webhook"
description = "The fixture webhook."
events = ["thing.created"]
discriminator = {{ source = "body", name = "event.type" }}
delivery_id = {{ source = "header", name = "X-Acme-Delivery" }}

[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "X-Acme-Signature"
prefix = "sha256="
signed = "{{timestamp}}.{{body}}"
timestamp = {{ source = "header", name = "X-Acme-Timestamp" }}
secret = "acme.signing_secret"
tolerance = "5m"

[channels.payload]
room = "event.room"
body = "event.body"

[channels.reply]
operation = "acme-reply"
result = "text"

[channels.reply.bind]
room = "room"

[channels.setup]
docs_url = "https://docs.acme.example/webhooks"
steps = ["Open the Acme dashboard.", "Paste the callback URL."]
"#
    )
}

/// **The complete configuration and OAuth declarations reach both consumer artifacts** — C-87.
///
/// This fixture is deliberately OAuth-backed because no shipped provider declares `[auth.oauth2]`
/// yet. A shipped-only assertion would therefore pass while the catalogue continued collapsing the
/// complete grant contract to the boolean `true`, which is the lossy shape this story removes.
#[test]
fn configuration_verify_and_oauth_reach_the_consumer_artifacts() {
    let fixture = Fixture::new("c87-configuration-artifacts");
    fixture.write_provider("acme", OAUTH_CONFIGURATION);

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_flux-connectors"))
        .args(["build", "--root"])
        .arg(fixture.root())
        .output()
        .expect("the flux-connectors binary runs");
    assert!(
        output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document: Value = serde_json::from_str(&fixture.read(CATALOG_JSON))
        .expect("the published document is valid JSON");
    assert_eq!(
        document["schema_version"], 3,
        "changing `auth.credentials[].oauth2` from a boolean to the grant spec is a schema break"
    );

    let provider = &document["providers"][0];
    assert_eq!(provider["verify"], "acme-ping");
    let config = provider["config"]
        .as_array()
        .expect("the provider carries its form declaration");
    assert_eq!(config.len(), 2);
    assert_eq!(config[0]["level"], "operator");
    assert_eq!(config[1]["level"], "operator");

    let oauth = &provider["auth"]["credentials"][0]["oauth2"];
    assert!(
        oauth.is_object(),
        "OAuth was flattened instead of published: {oauth}"
    );
    assert_eq!(oauth["endpoint"], "acme");
    assert_eq!(oauth["authorize_path"], "/oauth/authorize");
    assert_eq!(oauth["token_path"], "/oauth/token");
    assert_eq!(oauth["client_id"], "");
    assert_eq!(oauth["scopes"], serde_json::json!(["things:read"]));
    assert_eq!(
        oauth["grants"],
        serde_json::json!(["authorization_code", "refresh_token"])
    );
    assert_eq!(oauth["redirect"]["port"], 8765);
    assert_eq!(oauth["redirect"]["path"], "/oauth/callback");

    let manifest: toml::Value = toml::from_str(&fixture.read("connectors/acme.connector.toml"))
        .expect("the generated manifest is valid TOML");
    assert_eq!(manifest["verify"].as_str(), Some("acme-ping"));
    let manifest_config = manifest["config"]
        .as_array()
        .expect("the manifest carries its form declaration");
    assert_eq!(manifest_config.len(), 2);
    assert!(manifest_config
        .iter()
        .all(|field| field["level"].as_str() == Some("operator")));
}

/// Configuration describes setup and verification; it never changes the executable module.
#[test]
fn configuration_and_verify_reach_no_flux_module() {
    let configured = Fixture::new("c87-configured-module");
    configured.write_provider("acme", MODULE_WITH_CONFIGURATION);
    let plain = Fixture::new("c87-plain-module");
    plain.write_provider(
        "acme",
        &MODULE_WITH_CONFIGURATION
            .replace("verify = \"acme-ping\"\n", "")
            .replace(CONFIG_BLOCK, ""),
    );

    for fixture in [&configured, &plain] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_flux-connectors"))
            .args(["build", "--root"])
            .arg(fixture.root())
            .output()
            .expect("the flux-connectors binary runs");
        assert!(
            output.status.success(),
            "build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    assert_eq!(
        configured.read("connectors/acme.flux"),
        plain.read("connectors/acme.flux"),
        "configuration or verify changed executable Flux"
    );
}

const OAUTH_CONFIGURATION: &str = r#"id = "acme"
vendor = "Acme"
authority = "com.acme.api"
api_version = "v1"
base_url = "https://api.acme.example"
description = "A fixture OAuth connector."
verify = "acme-ping"
default_auth = [{ credentials = ["acme.oauth"] }]

[[auth]]
name = "acme.oauth"
scheme = "bearer"
env = ["ACME_ACCESS_TOKEN"]
description = "The access token acquired through OAuth."

[auth.oauth2]
endpoint = "acme"
authorize_path = "/oauth/authorize"
token_path = "/oauth/token"
client_id = ""
scopes = ["things:read"]
grants = ["authorization_code", "refresh_token"]
redirect = { port = 8765, path = "/oauth/callback" }

[[config]]
name = "client_id"
label = "Client ID"
help = "The public id of the Acme app registration."
binds = "oauth.client_id"

[[config]]
name = "client_secret"
label = "Client secret"
help = "The secret from the Acme app registration."
format = "token"
secret = true
binds = "oauth.client_secret"

[[operations]]
id = "acme-ping"
method = "GET"
direction = "read"
path = "/ping"
description = "Verify the Acme connection."
risk = "low"
idempotency = "idempotent"
"#;

const CONFIG_BLOCK: &str = r#"
[[config]]
name = "token"
label = "Access token"
help = "The token Acme issued for this connection."
format = "token"
secret = true
binds = "credential.acme.token"
"#;

const MODULE_WITH_CONFIGURATION: &str = r#"id = "acme"
vendor = "Acme"
authority = "com.acme.api"
api_version = "v1"
base_url = "https://api.acme.example"
description = "A fixture connector."
verify = "acme-ping"
default_auth = [{ credentials = ["acme.token"] }]

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]
description = "The fixture credential."

[[config]]
name = "token"
label = "Access token"
help = "The token Acme issued for this connection."
format = "token"
secret = true
binds = "credential.acme.token"

[[operations]]
id = "acme-ping"
method = "GET"
direction = "read"
path = "/ping"
description = "Verify the Acme connection."
risk = "low"
idempotency = "idempotent"
"#;

/// The document a full build **would** write, recomputed from `providers/*.toml`.
///
/// `catalog.json` is a whole-catalogue artifact and therefore coordinator-owned: it is written by a
/// full build only, and a story implementor does not regenerate it (`AGENTS.md`). The staleness
/// checks above are the ones that hold the committed bytes to it; the shape assertions below are
/// claims about the *emitter*, so they read what it produces rather than what is on disk.
fn recomputed() -> Value {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("every shipped provider compiles");
    let document = plan
        .artifacts
        .iter()
        .find(|artifact| {
            workspace
                .display_path(&artifact.path)
                .display()
                .to_string()
                .replace('\\', "/")
                == CATALOG_JSON
        })
        .unwrap_or_else(|| panic!("a build plans no {CATALOG_JSON}"));
    serde_json::from_str(&document.contents).expect("the planned document is valid JSON")
}

/// **The inbound surface is published too** (C-83): every declared event and every channel binding
/// reaches `catalog.json`, in IR order, with the fields the design specifies.
///
/// Driven off `providers/*.toml`, so it covers whatever the repository ships. The vacuity guard at
/// the end is the load-bearing line: a connector with no inbound surface contributes nothing, so
/// without it a future edit that dropped every binding would make this pass rather than fail.
#[test]
fn every_shipped_event_and_binding_reaches_the_document() {
    let document = recomputed();
    let mut published = 0;

    for name in shipped() {
        let path = repo_root().join("providers").join(format!("{name}.toml"));
        let source = std::fs::read_to_string(&path).expect("a shipped provider definition");
        let connector = shipped_provider::load_definition(&name, &source)
            .expect("a shipped provider loads")
            .connector;

        let entry = document["providers"]
            .as_array()
            .expect("a providers array")
            .iter()
            .find(|provider| provider["id"] == Value::String(name.clone()))
            .unwrap_or_else(|| panic!("{CATALOG_JSON} carries the provider `{name}`"));

        let events = entry["events"]
            .as_array()
            .unwrap_or_else(|| panic!("`{name}` carries an `events` array, `[]` when it has none"));
        assert_eq!(events.len(), connector.events.len());
        for (published_event, declared) in events.iter().zip(&connector.events) {
            assert_eq!(
                published_event["name"],
                Value::String(declared.name.clone())
            );
            assert_eq!(
                published_event["service"],
                Value::String(declared.service.clone())
            );
            // Every key always present: an absent schema is `null`, never a missing key.
            for key in ["oip", "description", "default", "group", "when", "schema"] {
                assert!(
                    published_event.get(key).is_some(),
                    "`{name}` event `{}` is missing the key `{key}`",
                    declared.name
                );
            }
        }

        let channels = entry["channels"]
            .as_array()
            .unwrap_or_else(|| panic!("`{name}` carries a `channels` array"));
        assert_eq!(channels.len(), connector.channels.len());
        for (published_channel, declared) in channels.iter().zip(&connector.channels) {
            published += 1;
            assert_eq!(
                published_channel["name"],
                Value::String(declared.name.clone())
            );
            for key in [
                "oip",
                "description",
                "transport",
                "events",
                "verification",
                "discriminator",
                "delivery_id",
                "payload",
                "reply",
                "cursor",
                "interval",
                "subscription",
                "setup",
            ] {
                assert!(
                    published_channel.get(key).is_some(),
                    "`{name}` binding `{}` is missing the key `{key}` — an absent value is `null`, \
                     never an absent key",
                    declared.name
                );
            }

            if let Some(reply) = &declared.reply {
                let published_reply = &published_channel["reply"];
                assert_eq!(
                    published_reply["operation"],
                    Value::String(reply.operation.clone())
                );
                let oip = connector
                    .oip_of_member(&declared.service, &reply.operation)
                    .map(|oip| oip.to_string());
                assert_eq!(
                    published_reply["oip"].as_str(),
                    oip.as_deref(),
                    "`{name}` binding `{}` does not carry its reply as a rendered oip",
                    declared.name
                );
                assert!(oip.is_some(), "the comparison above compared two nulls");
            }
        }
    }

    assert!(
        published > 0,
        "no shipped connector declares a channel binding, so every assertion above passed vacuously"
    );
}

/// **A deliberately-unverifiable binding is published loudly.**
///
/// The C-82 invariant is that silence is never a verification answer, and the way to break it in an
/// artifact is subtle: publish the HMAC parameters when there are any and nothing when there are
/// not. A consumer would then be telling "signed" from "anyone can POST here" by testing whether a
/// key exists — which is exactly how an unverified event comes to be presented as a trusted one, in
/// the one consumer that forgot to test.
///
/// So the assertion is not merely that the two differ. It is that they carry the **same key set** and
/// differ in a *value*, which is what makes the distinction impossible to miss by omission.
#[test]
fn a_deliberately_unverifiable_binding_is_published_loudly() {
    let fixture = Fixture::new("c83-loud");
    fixture.write_provider("acme", UNVERIFIABLE);

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_flux-connectors"))
        .args(["build", "--root"])
        .arg(fixture.root())
        .output()
        .expect("the flux-connectors binary runs");
    assert!(
        output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document: Value = serde_json::from_str(&fixture.read(CATALOG_JSON))
        .expect("the published document is valid JSON");
    let channels = document["providers"][0]["channels"]
        .as_array()
        .expect("the fixture publishes its bindings");
    assert_eq!(channels.len(), 2, "the fixture declares two bindings");

    let signed = &channels[0]["verification"];
    let open = &channels[1]["verification"];

    let keys = |value: &Value| -> Vec<String> {
        value
            .as_object()
            .expect("a verification object")
            .keys()
            .cloned()
            .collect()
    };
    assert_eq!(
        keys(signed),
        keys(open),
        "the two verification blocks differ in which keys they carry, so a consumer would have to \
         test for existence to tell a signed surface from an open one"
    );

    assert_eq!(signed["kind"], Value::String("hmac".to_string()));
    assert_eq!(signed["verified"], Value::Bool(true));
    assert_eq!(open["kind"], Value::String("none".to_string()));
    assert_eq!(
        open["verified"],
        Value::Bool(false),
        "a binding whose vendor publishes no signature must say so in the one boolean a consumer \
         filters on: {open}"
    );

    // The same statement reaches the manifest, so a host that reads only the installable pair is
    // told the same thing.
    let manifest = fixture.read("connectors/acme.connector.toml");
    assert!(
        manifest.contains("kind = \"none\"") && manifest.contains("verified = false"),
        "the manifest does not state the unverifiable binding loudly:\n{manifest}"
    );
}

/// Two webhook bindings over the same event: one signed, one whose vendor publishes no signature at
/// all and says so.
const UNVERIFIABLE: &str = r#"id = "acme"
vendor = "Acme"
authority = "com.acme.api"
api_version = "v1"
base_url = "https://api.acme.example"
description = "A hand-authored fixture connector."

[[auth]]
name = "acme.signing_secret"
scheme = "signing"
env = ["ACME_SIGNING_SECRET"]
description = "The fixture webhook signing secret."

[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/v1/things/{thing_id}"
description = "Fetch one thing."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "thing_id"
required = true
schema = { type = "integer" }

[[events]]
name = "thing.created"
description = "A thing was created."

[[channels]]
name = "signed"
transport = "webhook"
description = "The signed surface."
events = ["thing.created"]

[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "X-Acme-Signature"
signed = "{body}"
secret = "acme.signing_secret"

[channels.setup]
steps = ["Paste the callback URL into the Acme dashboard."]

[[channels]]
name = "open"
transport = "webhook"
description = "The surface Acme publishes no signature for."
events = ["thing.created"]
verification = "none"

[channels.setup]
steps = ["Paste the callback URL into the Acme dashboard."]
"#;

/// **Rebuilding from unchanged inputs is byte-identical.** The document travels through
/// `pipeline::plan` like every other artifact, so an unchanged input writes nothing at all.
#[test]
fn rebuilding_the_document_writes_nothing() {
    let fixture = Fixture::with_provider("c42-determinism", "acme");
    let binary = env!("CARGO_BIN_EXE_flux-connectors");

    let first = std::process::Command::new(binary)
        .args(["build", "--root"])
        .arg(fixture.root())
        .output()
        .expect("the flux-connectors binary runs");
    assert!(first.status.success());
    assert!(
        fixture.exists(CATALOG_JSON),
        "a build wrote no {CATALOG_JSON}"
    );

    let before = fixture.snapshot();
    let second = std::process::Command::new(binary)
        .args(["build", "--root"])
        .arg(fixture.root())
        .output()
        .expect("the flux-connectors binary runs");
    assert!(second.status.success());

    assert_eq!(
        before,
        fixture.snapshot(),
        "a second build over unchanged inputs changed the tree"
    );
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("up to date"),
        "a second build did not report the tree up to date: {}",
        String::from_utf8_lossy(&second.stdout)
    );
}

/// **The closed sets a connector declares reach the published document** — C-225.
///
/// A set that stayed in the IR would be a declaration a product cannot act on: a hosted form that
/// cannot see the choices renders a text box, and the wrong New Relic region is a `401` on every
/// call that reads exactly like a bad key. So this asserts the published shape, on the shipped
/// document, for the two connectors the story is about.
///
/// It names those two rather than walking the catalogue (`AGENTS.md`), so a fifty-fourth connector
/// cannot turn it red by existing — only New Relic or Intercom changing their regions can.
#[test]
fn every_declared_closed_set_reaches_the_published_document() {
    let document = committed();
    let providers = document["providers"]
        .as_array()
        .unwrap_or_else(|| panic!("{CATALOG_JSON} carries a `providers` array"));

    for (id, values) in [
        ("newrelic", &["api.newrelic.com", "api.eu.newrelic.com"][..]),
        (
            "intercom",
            &[
                "api.intercom.io",
                "api.eu.intercom.io",
                "api.au.intercom.io",
            ][..],
        ),
    ] {
        let provider = providers
            .iter()
            .find(|entry| entry["id"] == Value::String(id.to_string()))
            .unwrap_or_else(|| panic!("{CATALOG_JSON} carries the provider `{id}`"));
        let sets = provider["config_choices"]
            .as_array()
            .unwrap_or_else(|| panic!("`{id}` carries no `config_choices` array"));
        assert_eq!(sets.len(), 1, "`{id}` declares exactly one closed set");

        let host = &sets[0];
        // Addressed by `(service, kind, name)` — the same address the runtime configuration port
        // keys a stored value on, so a consumer joins on it rather than re-parsing a `binds` string
        // this document does not carry yet.
        assert_eq!(host["service"], "default", "{id}");
        assert_eq!(host["kind"], "endpoint", "{id}");
        assert_eq!(host["name"], "host", "{id}");
        assert_eq!(host["field"], "host", "{id}");
        assert!(
            host["label"]
                .as_str()
                .is_some_and(|label| !label.is_empty()),
            "`{id}`'s set carries the form label"
        );

        let choices = host["choices"]
            .as_array()
            .unwrap_or_else(|| panic!("`{id}`'s set carries a `choices` array"));
        assert_eq!(
            choices
                .iter()
                .map(|choice| choice["value"].as_str().expect("value"))
                .collect::<Vec<_>>(),
            values,
            "`{id}` publishes its regions in the vendor's own order"
        );
        assert!(
            choices
                .iter()
                .all(|choice| choice["label"].as_str().is_some_and(|l| !l.is_empty())),
            "`{id}` labels every region — a dropdown of bare hostnames is one nobody can answer"
        );
    }

    // The key is additive, so every provider carries it and the open ones carry `[]`. A consumer
    // that reads it unconditionally is what makes "additive" true rather than aspirational.
    assert!(
        providers
            .iter()
            .all(|provider| provider["config_choices"].is_array()),
        "`config_choices` is present on every provider, empty where nothing is closed"
    );
    assert_eq!(
        document["schema_version"],
        Value::from(3),
        "the closed-set key remained additive; C-87 later bumped the version because the existing \
         `auth.oauth2` field changed type — see docs/designs/catalog-json.md"
    );
}
