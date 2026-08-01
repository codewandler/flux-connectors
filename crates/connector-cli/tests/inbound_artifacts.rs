//! The inbound surface reaches the manifest and the catalogue, and **nothing reaches the module**
//! (C-83).
//!
//! Events and channel bindings landed in the IR and in the hash domain with C-82 and reached no
//! artifact at all, so a host had no way to read what a connector declares. This file holds the two
//! halves of publishing them, and they pull in opposite directions on purpose:
//!
//! 1. **The declaration is published.** `connectors/<id>.connector.toml` carries `[[events]]` and
//!    `[[channels]]`, with the transport, the events a binding carries, the verification parameters,
//!    the discriminator and delivery id, the payload map, and the reply as a **rendered oip**.
//! 2. **The declaration is emitted nowhere.** Every shipped `.flux` module is byte-identical to one
//!    emitted from the same connector with its events and bindings deleted, and the emitter
//!    *refuses* rather than degrades when asked for a rendering of one. flux lifts `op` declarations
//!    only; `channel` and `trigger` are Program members an operator writes. The tempting wrong
//!    output is an event dressed up as a pollable op, and `AGENTS.md` forbids exactly that.
//!
//! The manifest assertions read the bytes a build **would write** rather than the committed file.
//! That is not a shortcut: `connectors/*.connector.toml` is a per-provider artifact and is committed
//! here, but reading the plan keeps every claim below true of the emitter under test rather than of
//! whatever a previous run happened to leave on disk.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use connector_cli::catalog::OperationRendering;
use connector_cli::workspace::Workspace;
use connector_cli::{catalog, pipeline, seam, site};
use connector_spec::{Connector, VerificationScheme};
use serde_json::Value;

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The published catalogue, as a build plans it. A whole-catalogue artifact, so the committed bytes
/// are the coordinator's; the assertions here are claims about the emitter and read the plan.
const CATALOG_JSON: &str = "web/public/catalog.json";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every provider this repository ships, read from the directory rather than listed here (C-54).
fn shipped() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(repo_root().join("providers"))
        .expect("the providers directory must exist")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .map(|path| {
            path.file_stem()
                .expect("a provider file has a stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no shipped providers found");
    names
}

fn load(provider: &str) -> Connector {
    let path = repo_root()
        .join("providers")
        .join(format!("{provider}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    shipped_provider::load_definition(provider, &source)
        .unwrap_or_else(|error| panic!("providers/{provider}.toml does not load: {error}"))
        .connector
}

/// Every artifact a full build would write, keyed by its repository-relative path.
fn planned() -> BTreeMap<String, String> {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("every shipped provider compiles");
    plan.artifacts
        .iter()
        .map(|artifact| {
            (
                workspace
                    .display_path(&artifact.path)
                    .display()
                    .to_string()
                    .replace('\\', "/"),
                artifact.contents.clone(),
            )
        })
        .collect()
}

/// One provider's default-service manifest, parsed.
fn manifest_of(artifacts: &BTreeMap<String, String>, provider: &str) -> toml::Value {
    let path = format!("connectors/{provider}.connector.toml");
    let source = artifacts
        .get(&path)
        .unwrap_or_else(|| panic!("a build plans no {path}"));
    toml::from_str(source).unwrap_or_else(|error| panic!("{path} is not valid TOML: {error}"))
}

fn table<'a>(value: &'a toml::Value, key: &str) -> &'a toml::Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("expected a `{key}` key in {value}"))
}

fn text<'a>(value: &'a toml::Value, key: &str) -> &'a str {
    table(value, key)
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` is not a string in {value}"))
}

/// The connector's `[[channels]]` blocks for the default service, in IR order.
fn blocks<'a>(manifest: &'a toml::Value, key: &str) -> Vec<&'a toml::Value> {
    manifest
        .get(key)
        .map(|value| {
            value
                .as_array()
                .unwrap_or_else(|| panic!("`{key}` is not an array of tables"))
                .iter()
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// 1 — the declaration is published
// ---------------------------------------------------------------------------------------------

/// **Every declared event and binding reaches the manifest**, with the fields a host needs in order
/// to receive a delivery.
///
/// Driven off the provider files rather than a fixture, so it covers whatever the repository ships:
/// a connector that declares no inbound surface contributes nothing and one that declares two
/// bindings must publish both. The whole-repository claim it rests on — that at least one shipped
/// connector *has* an inbound surface — is asserted at the end, so a future edit that dropped every
/// binding would fail here rather than pass vacuously.
#[test]
fn every_shipped_event_and_binding_reaches_its_manifest() {
    let artifacts = planned();
    let mut published = 0;

    for provider in shipped() {
        let connector = load(&provider);
        if connector.events.is_empty() && connector.channels.is_empty() {
            continue;
        }
        assert!(
            connector.is_default_only(),
            "`{provider}` declares services and an inbound surface; this check reads the \
             default-service manifest only and needs widening"
        );

        let manifest = manifest_of(&artifacts, &provider);

        let events = blocks(&manifest, "events");
        assert_eq!(
            events.len(),
            connector.events.len(),
            "`{provider}` declares {} events and publishes {}",
            connector.events.len(),
            events.len()
        );
        for (block, declared) in events.iter().zip(&connector.events) {
            assert_eq!(text(block, "name"), declared.name, "events keep IR order");
        }

        let channels = blocks(&manifest, "channels");
        assert_eq!(channels.len(), connector.channels.len());

        for (block, declared) in channels.iter().zip(&connector.channels) {
            published += 1;
            assert_eq!(text(block, "name"), declared.name);

            // The transport, and the events the binding carries.
            assert!(!text(block, "transport").is_empty());
            let carried: Vec<&str> = table(block, "events")
                .as_array()
                .expect("`events` is an array")
                .iter()
                .map(|event| event.as_str().expect("an event name"))
                .collect();
            assert_eq!(carried, declared.events);

            // Verification is *always* here — see the loudness check below.
            let verification = table(block, "verification");
            assert!(!text(verification, "kind").is_empty());
            assert!(verification.get("verified").is_some());

            for (key, present) in [
                ("discriminator", declared.discriminator.is_some()),
                ("delivery_id", declared.delivery_id.is_some()),
                ("payload", !declared.payload.is_empty()),
            ] {
                assert_eq!(
                    block.get(key).is_some(),
                    present,
                    "`{provider}` binding `{}` publishes `{key}` out of step with its declaration",
                    declared.name
                );
            }

            // **The reply as a rendered oip.** The local id is what a host resolves against this
            // same manifest's `operations`; the oip is the address that survives leaving the
            // repository, and it is the form the design writes a reply in.
            if let Some(reply) = &declared.reply {
                let published_reply = table(block, "reply");
                assert_eq!(text(published_reply, "operation"), reply.operation);
                let oip = connector
                    .oip_of_member(&declared.service, &reply.operation)
                    .map(|oip| oip.to_string());
                assert_eq!(
                    published_reply.get("oip").and_then(toml::Value::as_str),
                    oip.as_deref(),
                    "`{provider}` binding `{}` does not publish its reply as a rendered oip",
                    declared.name
                );
                assert!(
                    oip.is_some(),
                    "`{provider}` declares a reply but renders no address for it, so the check \
                     above compared two `None`s"
                );
            }
        }
    }

    assert!(
        published > 0,
        "no shipped connector declares a channel binding, so every assertion above passed \
         vacuously — this file is testing nothing"
    );
}

/// **No secret reaches a manifest either.** The verification block names a credential, and that name
/// is one the same manifest's connector declares with `scheme = \"signing\"`.
#[test]
fn a_manifest_verification_block_names_a_declared_signing_credential() {
    let artifacts = planned();

    for provider in shipped() {
        let connector = load(&provider);
        if connector.channels.is_empty() {
            continue;
        }
        let manifest = manifest_of(&artifacts, &provider);

        for block in blocks(&manifest, "channels") {
            let Some(hmac) = table(block, "verification").get("hmac") else {
                continue;
            };
            let secret = text(hmac, "secret");
            let method = connector.auth_method(secret).unwrap_or_else(|| {
                panic!("`{provider}` verifies with `{secret}`, which it does not declare")
            });
            assert_eq!(
                method.scheme,
                connector_spec::AuthScheme::Signing,
                "`{provider}` verifies with `{secret}`, which is an outbound credential — the two \
                 directions never share one"
            );
            for env in &method.env {
                assert!(
                    !env.is_empty(),
                    "the published secret must be a credential name resolving to environment \
                     variable *names*"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 1b — every field of the verification declaration survives the trip (C-151)
// ---------------------------------------------------------------------------------------------

/// Every key `HmacSpec` accepts, **as serde reports them**.
///
/// `connector_spec::provider::accepted_keys` reads the field list out of `deny_unknown_fields`' own
/// error, so this is derived from the `Deserialize` impl that parses real provider files rather than
/// from a second list that could disagree with it — the same derived answer `provider_schema.rs`
/// holds the published JSON schema to. Nothing below is hand written, which is the point: a field
/// added to `HmacSpec` joins this set without anyone editing this file.
fn hmac_fields() -> BTreeSet<String> {
    connector_spec::provider::accepted_keys()
        .into_iter()
        .find(|(object, _)| *object == "hmac")
        .map(|(_, keys)| keys.into_iter().collect::<BTreeSet<String>>())
        .filter(|keys| !keys.is_empty())
        .expect("`accepted_keys` documents the `hmac` object, or every check below is vacuous")
}

/// The keys of a JSON object.
fn keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("expected an object, got {value}"))
        .keys()
        .cloned()
        .collect()
}

/// One binding's declaration as the **artifacts** state it: the IR's own answer, with the single
/// default they resolve rather than pass on.
///
/// `HmacSpec::timestamp_format` is optional in the IR because an author who writes nothing means
/// unix seconds. A host reading an artifact must not be asked to know that — the cost of guessing
/// the spelling of a signed timestamp is a refused delivery at best — so both projections publish
/// the *effective* format, exactly as `connector-spec`'s reference verifier resolves it.
fn as_published(spec: &connector_spec::HmacSpec) -> connector_spec::HmacSpec {
    connector_spec::HmacSpec {
        timestamp_format: spec
            .timestamp
            .as_ref()
            .map(|_| spec.timestamp_format.unwrap_or_default()),
        ..spec.clone()
    }
}

/// One binding of one provider, from the planned `catalog.json`.
fn published_channel<'a>(document: &'a Value, provider: &str, channel: &str) -> &'a Value {
    document["providers"]
        .as_array()
        .expect("a providers array")
        .iter()
        .find(|entry| entry["id"] == Value::String(provider.to_string()))
        .unwrap_or_else(|| panic!("{CATALOG_JSON} carries no provider `{provider}`"))["channels"]
        .as_array()
        .expect("a channels array")
        .iter()
        .find(|entry| entry["name"] == Value::String(channel.to_string()))
        .unwrap_or_else(|| panic!("{CATALOG_JSON} carries no `{provider}` binding `{channel}`"))
}

/// One binding of one provider, from the planned manifest.
fn manifest_channel<'a>(manifest: &'a toml::Value, channel: &str) -> &'a toml::Value {
    blocks(manifest, "channels")
        .into_iter()
        .find(|block| text(block, "name") == channel)
        .unwrap_or_else(|| panic!("the manifest publishes no binding `{channel}`"))
}

/// **A shipped binding's declared verification round-trips, whole, into both artifacts.**
///
/// The failure this catches is the quiet one: both projections restate `HmacSpec`'s fields by hand,
/// so a field the IR gained reaches neither consumer while every test that names a field explicitly
/// keeps passing. So nothing here names a field. The comparison is driven off the declaration
/// itself — every key the IR serializes must appear in `connectors/<id>.connector.toml` and in
/// `catalog.json` with the same value — and the document is additionally held to the full accepted
/// key set, because `catalog.json` publishes every key always.
///
/// The manifest is not: TOML has no `null`, so an undeclared optional field is legitimately absent
/// there, which is why the probe below covers the manifest's key set with a binding that declares
/// everything.
#[test]
fn every_declared_hmac_field_reaches_the_manifest_and_the_document() {
    let artifacts = planned();
    let document: Value = serde_json::from_str(
        artifacts
            .get(CATALOG_JSON)
            .unwrap_or_else(|| panic!("a build plans no {CATALOG_JSON}")),
    )
    .expect("the planned document is valid JSON");
    let expected = hmac_fields();
    let mut checked = 0;

    for provider in shipped() {
        let connector = load(&provider);
        if connector.channels.is_empty() {
            continue;
        }
        let manifest = manifest_of(&artifacts, &provider);

        for channel in &connector.channels {
            let Some(VerificationScheme::Hmac(spec)) = &channel.verification else {
                continue;
            };
            checked += 1;
            let name = channel.name.as_str();
            let declared =
                serde_json::to_value(as_published(spec)).expect("an HmacSpec serializes");

            let published = &published_channel(&document, &provider, name)["verification"]["hmac"];
            assert_eq!(
                keys(published),
                expected,
                "`{provider}` binding `{name}`: {CATALOG_JSON} does not carry every field of its \
                 verification declaration — every key is always present there, so a field the \
                 emitter forgot is a field no consumer can read"
            );

            let block = manifest_channel(&manifest, name);
            let in_manifest: Value = serde_json::to_value(&block["verification"]["hmac"])
                .expect("a TOML table converts to JSON");

            for (key, value) in declared.as_object().expect("an object") {
                assert_eq!(
                    in_manifest.get(key),
                    Some(value),
                    "`{provider}` binding `{name}` declares `{key} = {value}`, which \
                     connectors/{provider}.connector.toml does not publish; a host reading the \
                     manifest cannot verify a delivery it cannot fully describe"
                );
                assert_eq!(
                    published.get(key),
                    Some(value),
                    "`{provider}` binding `{name}` declares `{key} = {value}`, which \
                     {CATALOG_JSON} does not publish"
                );
            }
        }
    }

    assert!(
        checked > 0,
        "no shipped connector declares an HMAC-verified binding, so every assertion above passed \
         vacuously"
    );
}

/// A connector whose one binding declares **every** field `HmacSpec` has.
///
/// Written here rather than added to `providers/`: no shipped vendor needs an RFC 3339 timestamp
/// yet, and the guarantee below must not wait for one — the whole point of C-151 is that the first
/// such vendor must not be the thing that discovers a projection drops a field.
const EVERY_HMAC_FIELD: &str = r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
api_version = "v1"
base_url = "https://api.acme.example"
description = "A hand-authored fixture connector."
default_auth = [{ credentials = ["acme.token"] }]

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]
description = "The fixture credential."

[[auth]]
name = "acme.signing_secret"
scheme = "signing"
env = ["ACME_SIGNING_SECRET"]
description = "The fixture webhook signing secret."

[[operations]]
id = "acme-thing-list"
method = "GET"
path = "/v1/things"
description = "List things."
risk = "low"
idempotency = "idempotent"

[[events]]
name = "thing.created"
description = "A thing was created."

[[channels]]
name = "hook"
transport = "webhook"
description = "The fixture webhook."
events = ["thing.created"]

# Every field, so that a projection that drops one is caught here rather than by the first vendor
# that needs it. `timestamp_format` is the field this fixture was written for.
[channels.verification.hmac]
algorithm = "sha256"
encoding = "base64"
header = "X-Acme-Signature"
prefix = "sha256="
signed = "{timestamp}{body}"
timestamp = { source = "header", name = "X-Acme-Timestamp" }
timestamp_format = "rfc3339"
secret = "acme.signing_secret"
tolerance = "5m"

[channels.setup]
docs_url = "https://docs.acme.example/webhooks"
steps = ["Open the Acme dashboard.", "Paste the callback URL."]
"#;

/// **The hand-enumeration stops being a place a field can go missing.**
///
/// `seam::ManifestHmac` and `site::HmacEntry` each restate `HmacSpec`'s fields, and neither could
/// simply serialize the IR type instead. The reasons are structural rather than stylistic, and both
/// are recorded beside the types:
///
/// - **The manifest's field order is load-bearing.** TOML places a nested table after its parent's
///   key/value pairs, so a scalar declared after `timestamp` would be parsed as a field *of* the
///   timestamp table. `HmacSpec` declares `secret` and `tolerance` after `timestamp`, so flattening
///   it would emit a manifest that reparses wrongly.
/// - **The document publishes every key always** (`docs/designs/catalog-json.md`), while `HmacSpec`
///   skips its `None` fields so that a provider TOML is not required to spell out absences.
///
/// So this is the C-125 resolution instead of a comment asking the next person to remember: the
/// authoritative field list is *derived* from `HmacSpec` — see [`hmac_fields`] — and both
/// projections are held to it, over a binding that declares all of them. A field added to `HmacSpec`
/// fails this test with no edit to this file, first at the fixture and then at whichever projection
/// forgot it.
#[test]
fn neither_projection_can_lose_a_field_hmac_spec_declares() {
    let expected = hmac_fields();
    let connector = connector_spec::provider::load("providers/acme.toml", EVERY_HMAC_FIELD)
        .expect("the fixture loads")
        .connector;

    let Some(VerificationScheme::Hmac(spec)) = &connector.channels[0].verification else {
        panic!("the fixture declares an HMAC-verified binding");
    };
    let declared = serde_json::to_value(spec).expect("an HmacSpec serializes");
    assert_eq!(
        keys(&declared),
        expected,
        "EVERY_HMAC_FIELD does not declare every field `HmacSpec` accepts, so the two checks below \
         would be made against a subset of them — declare the missing one in the fixture above"
    );

    let emitted = seam::emit(&connector).expect("the fixture emits");
    let manifest: toml::Value =
        toml::from_str(&emitted.services[0].manifest).expect("the manifest is valid TOML");
    let in_manifest: Value =
        serde_json::to_value(&manifest_channel(&manifest, "hook")["verification"]["hmac"])
            .expect("a TOML table converts to JSON");
    assert_eq!(
        keys(&in_manifest),
        expected,
        "the manifest projection publishes a different field set than `HmacSpec` declares, so a \
         host reading connectors/<id>.connector.toml cannot see everything the IR carries"
    );

    let entry =
        site::provider_entry(&connector, &emitted.operations).expect("the fixture compiles");
    let document: Value =
        serde_json::from_str(&site::document(vec![entry]).expect("it serializes"))
            .expect("the document is valid JSON");
    assert_eq!(
        keys(&published_channel(&document, "acme", "hook")["verification"]["hmac"]),
        expected,
        "the public-catalogue projection publishes a different field set than `HmacSpec` declares"
    );
}

// ---------------------------------------------------------------------------------------------
// 2 — the declaration is emitted nowhere
// ---------------------------------------------------------------------------------------------

/// **Nothing reaches the `.flux` module.** Every shipped module is byte-identical to one emitted
/// from the same connector with every event and every channel binding deleted.
///
/// Stronger than comparing against the committed file, which would only say the module did not
/// change *today*: this says the inbound half cannot influence the emitted Flux at all, for any
/// provider, because deleting it changes nothing. A future emitter that synthesised a poll loop from
/// a binding would fail here on the provider that has one.
#[test]
fn no_event_or_binding_reaches_any_shipped_module() {
    let mut compared = 0;

    for provider in shipped() {
        let connector = load(&provider);
        if connector.events.is_empty() && connector.channels.is_empty() {
            continue;
        }
        compared += 1;

        let with = seam::emit(&connector)
            .unwrap_or_else(|error| panic!("`{provider}` does not emit: {error}"));
        let stripped = Connector {
            events: Vec::new(),
            channels: Vec::new(),
            ..connector.clone()
        };
        let without = seam::emit(&stripped).unwrap_or_else(|error| {
            panic!("`{provider}` does not emit without its inbound half: {error}")
        });

        assert_eq!(
            with.operations, without.operations,
            "`{provider}`'s per-operation renderings change when its inbound surface is deleted"
        );
        for (unit, bare) in with.services.iter().zip(&without.services) {
            assert_eq!(
                unit.module, bare.module,
                "`{provider}`'s `{}` module is not byte-identical with and without its events and \
                 bindings — something inbound reached the Flux",
                unit.service
            );
        }
        assert_eq!(
            with.catalog, without.catalog,
            "`{provider}`'s generated catalogue table changes with its inbound surface; the Rust \
             catalogue embeds `op` renderings only"
        );

        // The complement, so the check above cannot pass because the manifests are identical too.
        assert_ne!(
            with.services[0].manifest, without.services[0].manifest,
            "`{provider}`'s manifest is unchanged by deleting its inbound surface, so it publishes \
             nothing and this test compares two empty claims"
        );
    }

    assert!(
        compared > 0,
        "no shipped connector declares an inbound surface, so this comparison is vacuous"
    );
}

/// **The emitter refuses rather than degrades.** A rendering whose id names an event or a channel
/// binding is not skipped, not published as an operation, and not turned into a pollable op — it
/// fails the whole call, in both catalogue backends, with an error that names the rule.
#[test]
fn a_rendering_for_an_event_or_a_binding_is_refused_by_name() {
    let connector = load("slack");
    assert!(
        !connector.events.is_empty() && !connector.channels.is_empty(),
        "this check needs a connector with both member kinds"
    );

    for member in [
        connector.events[0].name.clone(),
        connector.channels[0].name.clone(),
    ] {
        let dressed_up = vec![OperationRendering {
            id: member.clone(),
            source: format!("op {member}() -> Any\n"),
        }];

        for rendered in [
            catalog::render(&connector, &dressed_up).err(),
            site::provider_entry(&connector, &dressed_up).err(),
        ] {
            let error = rendered.unwrap_or_else(|| {
                panic!("`{member}` was published as an operation instead of being refused")
            });
            let message = format!("{error:#}");
            assert!(
                message.contains(&member),
                "the refusal does not name the member: {message}"
            );
            assert!(
                message.contains("pollable op"),
                "the refusal does not name the wrong output it is preventing: {message}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 3 — `--service` selects every member kind, not just the callable one
// ---------------------------------------------------------------------------------------------

/// A two-service provider whose **each** service declares an operation, an event and a binding.
///
/// Written here rather than added to `providers/`: this shape does not exist in the shipped
/// catalogue, and a selection that silently stayed operations-only would go unnoticed without one.
const TWO_SERVICE: &str = r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
base_url = "https://api.acme.example"

[[services]]
name = "mail"
description = "Mail."
api_version = "v1"

[[services]]
name = "chat"
description = "Chat."
api_version = "v2"

[[operations]]
id = "acme-mail-send"
service = "mail"
method = "POST"
path = "/mail"
description = "Send mail."
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "body"
required = true
schema = { type = "string" }

[[operations]]
id = "acme-chat-post"
service = "chat"
method = "POST"
path = "/chat"
description = "Post a message."
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "text"
required = true
schema = { type = "string" }

[[events]]
name = "mail.received"
service = "mail"
description = "Mail arrived."

[[events]]
name = "chat.posted"
service = "chat"
description = "Someone posted."

[[channels]]
name = "mail-socket"
service = "mail"
transport = "socket"
events = ["mail.received"]

[channels.payload]
body = "message.body"

[channels.reply]
operation = "acme-mail-send"
result = "body"

[[channels]]
name = "chat-socket"
service = "chat"
transport = "socket"
events = ["chat.posted"]

[channels.payload]
text = "message.text"

[channels.reply]
operation = "acme-chat-post"
result = "text"
"#;

/// **`--service` selects a service's events and bindings along with its operations.**
///
/// The failure this guards is a partial success, which is the worst kind here: `--service mail`
/// would emit a mail manifest that compiles, ships and passes every artifact check while announcing
/// an ingress surface belonging to a service nobody selected.
#[test]
fn selecting_a_service_selects_its_events_and_bindings_too() {
    let connector = connector_spec::provider::load("providers/acme.toml", TWO_SERVICE)
        .expect("the fixture loads")
        .connector;

    let mail = seam::select_service(&connector, "mail").expect("`mail` is a service");
    assert_eq!(
        mail.events
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>(),
        vec!["mail.received"]
    );
    assert_eq!(
        mail.channels
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["mail-socket"]
    );

    let manifest = &seam::emit(&mail).expect("the selection emits").services[0].manifest;
    assert!(
        manifest.contains("mail.received") && manifest.contains("mail-socket"),
        "the selected service's own inbound surface is missing from its manifest:\n{manifest}"
    );
    assert!(
        !manifest.contains("chat.posted") && !manifest.contains("chat-socket"),
        "a `--service mail` manifest announces the chat service's ingress surface:\n{manifest}"
    );

    // And the other direction, so the filter is not simply dropping everything.
    let chat = seam::select_service(&connector, "chat").expect("`chat` is a service");
    assert_eq!(
        chat.channels
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["chat-socket"]
    );
}
