//! A published `default` service growing named siblings without moving its addresses — C-458.

use connector_spec::credential::TenantInstances;
use connector_spec::{
    provider, Gid, Layout, Oip, Role, SpecDocument, Tag, TenantLayout, DEFAULT_SERVICE,
};

const MIXED: &str = r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
api_version = "v1"
base_url = "https://api.acme.example"
description = "A published API growing a second surface."
verify = "acme-models-list"
default_auth = [{ credentials = ["acme.token"] }]

[[services]]
name = "default"
legacy = true
roles = ["llm_catalogue"]
tags = ["support"]

[[services]]
name = "chat"
description = "Chat completions."
tags = ["messaging"]

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]

[[config]]
name = "api_token"
service = "default"
label = "Acme API token"
help = "Create the token in Acme account settings"
format = "token"
secret = true
binds = "credential.acme.token"

[[operations]]
id = "acme-models-list"
service = "default"
method = "GET"
direction = "read"
path = "/v1/models"
description = "List models."
risk = "low"
idempotency = "idempotent"

[[operations]]
id = "acme-chat-completion"
service = "chat"
method = "POST"
direction = "write"
path = "/v1/chat"
description = "Create a chat completion."
risk = "medium"
idempotency = "non_idempotent"

[[events]]
name = "model.changed"
service = "default"

[[channels]]
name = "model-events"
service = "default"
transport = "socket"
events = ["model.changed"]

[[graphs]]
name = "acme-model-refresh"
service = "default"

[[graphs.nodes]]
id = "list"
kind = { operation = { operation = "acme-models-list" } }
"#;

fn load(source: &str) -> connector_spec::Connector {
    provider::load("providers/acme.toml", source)
        .unwrap_or_else(|error| panic!("the mixed connector must load:\n{error}"))
        .connector
}

fn refusal(source: &str) -> String {
    provider::load("providers/acme.toml", source)
        .expect_err("the mixed connector must be refused")
        .to_string()
}

/// Failing first: before C-458 the loader refuses the `default` entry merely because `chat` exists.
#[test]
fn an_explicit_legacy_default_can_coexist_with_a_named_service() {
    let connector = load(MIXED);

    assert_eq!(connector.service_names(), [DEFAULT_SERVICE, "chat"]);
    assert!(!connector.is_default_only());
    assert_eq!(
        connector.gid_of(DEFAULT_SERVICE).map(|gid| gid.to_string()),
        Some("com.acme.api:v1".to_owned())
    );
    assert_eq!(
        connector.gid_of("chat").map(|gid| gid.to_string()),
        Some("com.acme.api/chat:v1".to_owned())
    );
    assert_eq!(
        connector
            .oip_of(connector.operation("acme-models-list").expect("declared"))
            .map(|oip| oip.to_string()),
        Some("com.acme.api:v1#acme-models-list".to_owned())
    );
    let legacy_gid = connector.gid_of(DEFAULT_SERVICE).expect("addressed");
    assert_eq!(
        Gid::parse(&legacy_gid.to_string()).expect("legacy gid round-trips"),
        legacy_gid
    );
    let legacy_oip = connector
        .oip_of(connector.operation("acme-models-list").expect("declared"))
        .expect("addressed");
    assert_eq!(
        Oip::parse(&legacy_oip.to_string()).expect("legacy oip round-trips"),
        legacy_oip
    );

    let legacy = connector.service(DEFAULT_SERVICE).expect("declared legacy");
    assert!(legacy.legacy);
    let encoded = serde_json::to_string(&connector).expect("connector IR encodes");
    assert!(
        encoded.contains(r#"{"name":"default","legacy":true"#),
        "the accepted IR must retain the explicit migration decision: {encoded}"
    );
    assert_eq!(legacy.roles, [Role::LlmCatalogue]);
    assert_eq!(legacy.tags, [Tag::Support]);
    assert_eq!(connector.config_of(DEFAULT_SERVICE).count(), 1);
    assert_eq!(connector.events_of(DEFAULT_SERVICE).count(), 1);
    assert_eq!(connector.channels_of(DEFAULT_SERVICE).count(), 1);
    assert_eq!(connector.graphs_of(DEFAULT_SERVICE).count(), 1);
    assert_eq!(
        connector
            .operation(connector.verify.as_deref().expect("verify declared"))
            .expect("verify resolves")
            .service,
        DEFAULT_SERVICE
    );

    let credential = connector
        .credential_ref_for("tenant-1", "acme.token", TenantInstances::sole())
        .expect("valid address")
        .expect("the connector has an authority");
    assert!(credential.is_default_service());
    let credential_path = TenantLayout.render(&credential);
    assert_eq!(credential_path, "tenants/tenant-1/com.acme.api/token");
    assert_eq!(TenantLayout.parse(&credential_path), Ok(credential));
}

/// A mixed connector may preserve an old default only by saying so on the service declaration.
#[test]
fn a_default_beside_a_named_service_without_the_legacy_marker_stays_refused() {
    let error = refusal(&MIXED.replace("legacy = true\n", ""));
    assert!(error.contains("legacy"), "{error}");
    assert!(error.contains("default"), "{error}");
    assert!(error.contains("chat"), "{error}");
}

/// The marker is an address-migration capability, not shorthand for a new default-only provider.
#[test]
fn a_legacy_marker_without_a_named_sibling_is_refused() {
    let source = MIXED
        .replace(
            "\n[[services]]\nname = \"chat\"\ndescription = \"Chat completions.\"\ntags = [\"messaging\"]\n",
            "\n",
        )
        .replace(
            "\n[[operations]]\nid = \"acme-chat-completion\"\nservice = \"chat\"\nmethod = \"POST\"\npath = \"/v1/chat\"\ndescription = \"Create a chat completion.\"\nrisk = \"medium\"\nidempotency = \"non_idempotent\"\n",
            "\n",
        );
    let error = refusal(&source);
    assert!(error.contains("legacy"), "{error}");
    assert!(error.contains("named"), "{error}");
}

#[test]
fn a_named_service_cannot_claim_the_legacy_default_marker() {
    let error = refusal(&MIXED.replace(
        "name = \"chat\"\ndescription = \"Chat completions.\"",
        "name = \"chat\"\nlegacy = true\ndescription = \"Chat completions.\"",
    ));
    assert!(error.contains("chat"), "{error}");
    assert!(error.contains("legacy"), "{error}");
    assert!(error.contains("default"), "{error}");
}

/// Once the two shapes coexist, silence is not a third spelling of the legacy service.
#[test]
fn every_member_of_a_mixed_connector_must_state_its_service() {
    for (kind, name, declaration) in [
        (
            "operation",
            "acme-models-list",
            "service = \"default\"\nmethod = \"GET\"",
        ),
        (
            "event",
            "model.changed",
            "service = \"default\"\n\n[[channels]]",
        ),
        (
            "channel binding",
            "model-events",
            "service = \"default\"\ntransport = \"socket\"",
        ),
        (
            "configuration field",
            "api_token",
            "service = \"default\"\nlabel = \"Acme API token\"",
        ),
        (
            "graph",
            "acme-model-refresh",
            "service = \"default\"\n\n[[graphs.nodes]]",
        ),
    ] {
        let source = MIXED.replace(
            declaration,
            &declaration.replacen("service = \"default\"\n", "", 1),
        );
        let error = refusal(&source);
        assert!(error.contains(kind), "{kind} {name}: {error}");
        assert!(error.contains(name), "{kind} {name}: {error}");
        assert!(error.contains("service"), "{kind} {name}: {error}");
    }
}

#[test]
fn every_spec_document_of_a_mixed_connector_must_state_its_service() {
    let with_spec = MIXED.replace(
        "[[auth]]",
        "[[spec]]\npath = \"specs/acme/v1.json\"\nservice = \"default\"\n\n[[auth]]",
    );
    let cache = [SpecDocument {
        path: "specs/acme/v1.json",
        document: r#"{"openapi":"3.0.0","info":{"title":"Acme","version":"v1"},"paths":{}}"#,
    }];
    provider::load_with_spec("providers/acme.toml", &with_spec, &cache)
        .expect("an explicitly owned document is valid");

    let omitted = with_spec.replace("service = \"default\"\n\n[[auth]]", "[[auth]]");
    let error = provider::load_with_spec("providers/acme.toml", &omitted, &cache)
        .expect_err("an implicitly owned document must be refused")
        .to_string();
    assert!(error.contains("spec document"), "{error}");
    assert!(error.contains("specs/acme/v1.json"), "{error}");
    assert!(error.contains("service"), "{error}");
}
