//! Contract for the deliberately narrow Fly.io Machines provider (C-111).

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

const PROVIDER: &str = "fly";
const AUTHORITY: &str = "io.fly.api";
const SERVICE: &str = "machines";
const API_VERSION: &str = "v1";
const BASE_URL: &str = "https://api.machines.dev/v1";
const CREDENTIAL: &str = "fly.api_token";
const TOKEN_ENV: &str = "FLY_API_TOKEN";
const VERIFY: &str = "fly-regions-list";
const OPERATIONS: &[&str] = &[
    "fly-regions-list",
    "fly-machines-list",
    "fly-machine-get",
    "fly-machine-events-list",
    "fly-machine-create",
    "fly-machine-start",
    "fly-machine-stop",
    "fly-machine-restart",
    "fly-machine-delete",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {} ({error})", path.display()));
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

#[test]
fn fly_is_one_named_authenticated_machine_service() {
    let connector = load();
    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Fly.io");
    assert_eq!(connector.authority.as_deref(), Some(AUTHORITY));
    assert_eq!(connector.base_url, BASE_URL);
    assert_eq!(connector.services.len(), 1);
    let service = &connector.services[0];
    assert_eq!(service.name, SERVICE);
    assert_eq!(service.api_version.as_deref(), Some(API_VERSION));
    assert_eq!(
        connector.gid_of(SERVICE).unwrap().to_string(),
        "io.fly.api/machines:v1"
    );
    assert_eq!(connector.verify.as_deref(), Some(VERIFY));

    assert_eq!(connector.auth.len(), 1);
    let auth = connector.auth_method(CREDENTIAL).expect("Fly bearer token");
    assert_eq!(auth.scheme, AuthScheme::Bearer);
    assert_eq!(auth.env, [TOKEN_ENV]);
    assert!(auth.user_env.is_empty());
    assert!(auth.user_suffix.is_none());
    assert_eq!(connector.events.len(), 0);
    assert_eq!(connector.channels.len(), 0);
    assert_eq!(connector.graphs.len(), 0);
}

#[test]
fn fly_inventory_and_routes_are_exact() {
    let connector = load();
    assert_eq!(
        connector
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        OPERATIONS
    );
    let expected = [
        ("fly-regions-list", HttpMethod::Get, "/platform/regions"),
        (
            "fly-machines-list",
            HttpMethod::Get,
            "/apps/{app_name}/machines",
        ),
        (
            "fly-machine-get",
            HttpMethod::Get,
            "/apps/{app_name}/machines/{machine_id}",
        ),
        (
            "fly-machine-events-list",
            HttpMethod::Get,
            "/apps/{app_name}/machines/{machine_id}/events",
        ),
        (
            "fly-machine-create",
            HttpMethod::Post,
            "/apps/{app_name}/machines",
        ),
        (
            "fly-machine-start",
            HttpMethod::Post,
            "/apps/{app_name}/machines/{machine_id}/start",
        ),
        (
            "fly-machine-stop",
            HttpMethod::Post,
            "/apps/{app_name}/machines/{machine_id}/stop",
        ),
        (
            "fly-machine-restart",
            HttpMethod::Post,
            "/apps/{app_name}/machines/{machine_id}/restart",
        ),
        (
            "fly-machine-delete",
            HttpMethod::Delete,
            "/apps/{app_name}/machines/{machine_id}",
        ),
    ];
    for (id, method, path) in expected {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(operation.service, SERVICE);
        assert_eq!(operation.method, method);
        assert_eq!(operation.path, path);
        assert!(operation.params.query.is_empty());
        assert!(operation.params.header.is_empty());
        assert!(operation.params.body_schema.is_none());
        let auth = connector.effective_auth(operation);
        assert_eq!(auth.len(), 1);
        assert!(auth[0].contains(CREDENTIAL));
        assert_eq!(auth[0].len(), 1);
    }
}

#[test]
fn create_has_only_the_required_nested_image_and_other_writes_are_bodyless() {
    let connector = load();
    let create = connector
        .operations
        .iter()
        .find(|operation| operation.id == "fly-machine-create")
        .unwrap();
    assert_eq!(create.params.body.len(), 1);
    let image = &create.params.body[0];
    assert_eq!(image.name, "image");
    assert_eq!(image.wire.as_deref(), Some("config.image"));
    assert!(image.required);
    assert_eq!(image.schema["type"], "string");
    assert_eq!(image.schema["minLength"], 1);
    let emitted = emit_operation(&connector, create).expect("create emits");
    assert!(emitted.contains("payload = { config: { image } }"));

    for operation in connector.operations.iter().filter(|operation| {
        matches!(
            operation.id.as_str(),
            "fly-machine-start" | "fly-machine-stop" | "fly-machine-restart" | "fly-machine-delete"
        )
    }) {
        assert!(
            operation.params.body.is_empty(),
            "{} has a body",
            operation.id
        );
    }
}

#[test]
fn lifecycle_risk_refuses_automatic_retries() {
    let connector = load();
    for operation in &connector.operations {
        match operation.id.as_str() {
            "fly-regions-list"
            | "fly-machines-list"
            | "fly-machine-get"
            | "fly-machine-events-list" => {
                assert_eq!(operation.risk, Risk::Low);
                assert_eq!(operation.idempotency, Idempotency::Idempotent);
            }
            "fly-machine-delete" => {
                assert_eq!(operation.risk, Risk::Destructive);
                assert_eq!(operation.idempotency, Idempotency::NonIdempotent);
            }
            _ => {
                assert_eq!(operation.risk, Risk::High);
                assert_eq!(operation.idempotency, Idempotency::NonIdempotent);
            }
        }
    }
}

#[test]
fn every_fly_operation_emits_an_analyzable_module_without_secret_material() {
    let connector = load();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{} does not emit: {error}", operation.id));
        assert!(emitted.contains(BASE_URL));
        assert!(!emitted.contains(TOKEN_ENV));
        assert!(!emitted.contains(CREDENTIAL));

        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "{} does not parse: {:?}\n{emitted}",
            operation.id,
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str())
        );
        let module = flux_lang::program::Module::parse_str(&emitted)
            .unwrap_or_else(|error| panic!("{} does not load: {error}", operation.id));
        let program = module.program().expect("program");
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, operation.id);
    }
}
