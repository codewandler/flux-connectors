//! Contract for the deliberately narrow Vercel provider (C-170).
//!
//! **The archetype this connector exists for.** Every operation Vercel exposes here takes an
//! optional `?teamId=` query parameter — optional in the API, load-bearing in effect. Omit it and
//! the call is scoped to the caller's personal account instead of any team, silently: there is no
//! error, no prompt, and (for the two `list` operations) a complete, plausible-looking response for
//! the wrong account. A `description` that does not say so hands a model a silent footgun — this
//! file's real acceptance claim, the one C-107's `Notion-Version` test and C-171's root-folder
//! sentinel test are the standing archetypes for, is that **every** declared `teamId` parameter says
//! so in the text a model actually reads.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, Connector, HttpMethod, Idempotency, Param, Risk};

const PROVIDER: &str = "vercel";
const BASE_URL: &str = "https://api.vercel.com";
const CREDENTIAL: &str = "vercel.token";
const TOKEN_ENV: &str = "VERCEL_TOKEN";
const VERIFY: &str = "vercel-projects-list";
const OPERATIONS: &[&str] = &[
    "vercel-projects-list",
    "vercel-project-get",
    "vercel-deployments-list",
    "vercel-deployment-get",
    "vercel-deployment-cancel",
];

fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join(format!("{PROVIDER}.toml"))
}

fn vercel() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-170 ships the Vercel connector",
            path.display()
        )
    });
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// Every `teamId` query parameter across the connector, wherever it appears.
fn team_id_params(connector: &Connector) -> Vec<(&str, &Param)> {
    let mut found = Vec::new();
    for operation in &connector.operations {
        for param in &operation.params.query {
            if param.name == "teamId" {
                found.push((operation.id.as_str(), param));
            }
        }
    }
    found
}

#[test]
fn the_vercel_connector_loads() {
    let connector = vercel();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Vercel");
    assert_eq!(connector.base_url, BASE_URL);
    assert_eq!(connector.verify.as_deref(), Some(VERIFY));
    assert!(!connector.operations.is_empty());
    assert_eq!(connector.events.len(), 0);
    assert_eq!(connector.channels.len(), 0);
    assert_eq!(connector.graphs.len(), 0);

    assert_eq!(connector.auth.len(), 1);
    let auth = connector.auth_method(CREDENTIAL).expect("bearer token");
    assert_eq!(auth.env, [TOKEN_ENV]);
}

/// The curated set C-170 selected, exactly. Named rather than counted so that adding an operation
/// is a deliberate edit here.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = vercel();
    let ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(ids, OPERATIONS);
}

/// **The acceptance assertion.** Every operation declares an optional `teamId` query parameter, and
/// its description names the personal-account fallback — the fact a model must read before deciding
/// whether it is safe to omit.
#[test]
fn every_operation_declares_team_id_and_names_the_personal_account_fallback() {
    let connector = vercel();
    let params = team_id_params(&connector);

    assert_eq!(
        params.len(),
        OPERATIONS.len(),
        "expected every one of the {} curated operations to declare `teamId`; found it on {:?}",
        OPERATIONS.len(),
        params.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    );

    for (operation_id, param) in &params {
        assert!(
            !param.required,
            "`{operation_id}`'s `teamId` is declared required — the whole hazard is that the API \
             makes it optional"
        );
        assert_eq!(param.schema["type"], "string");
        let description = param.description.to_lowercase();
        assert!(
            description.contains("personal account"),
            "`{operation_id}`'s `teamId` description does not name the personal-account fallback. \
             Full text: {:?}",
            param.description
        );
        assert!(
            description.contains("omit"),
            "`{operation_id}`'s `teamId` description does not describe what omitting it does. Full \
             text: {:?}",
            param.description
        );
    }
}

/// The connector-level `description` on the two `list` operations also names the hazard — a model
/// choosing which tool to call reads this before it ever reaches a parameter description.
#[test]
fn list_operations_name_the_hazard_in_their_own_description() {
    let connector = vercel();
    for id in ["vercel-projects-list", "vercel-deployments-list"] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("missing {id}"));
        let description = operation.description.to_lowercase();
        assert!(
            description.contains("personal account") && description.contains("team"),
            "`{id}` description does not name the team/personal-account hazard. Full text: {:?}",
            operation.description
        );
    }
}

/// Risk and idempotency track the effect each operation can have: cancelling a build is a real,
/// non-repeatable write; everything else here is a read.
#[test]
fn risk_and_idempotency_track_effect() {
    let connector = vercel();
    for operation in &connector.operations {
        match operation.id.as_str() {
            "vercel-deployment-cancel" => {
                assert_eq!(operation.risk, Risk::High);
                assert_eq!(operation.idempotency, Idempotency::NonIdempotent);
            }
            _ => {
                assert_eq!(operation.risk, Risk::Low);
                assert_eq!(operation.idempotency, Idempotency::Idempotent);
            }
        }
    }
}

/// The connection-level configuration surface: the access token, and no realistic-looking example on
/// it. `teamId` is deliberately absent from `[[config]]` — nothing in `ConfigField::binds` can name a
/// per-request query parameter, so it stays a caller argument instead (see `providers/vercel.toml`'s
/// header comment).
#[test]
fn the_access_token_is_configurable_and_carries_no_example_value() {
    let connector = vercel();

    assert_eq!(connector.config.len(), 1, "teamId cannot be a config field");
    let field = &connector.config[0];
    assert_eq!(field.name, "token");
    assert!(field.secret, "a Vercel access token is a secret");
    assert_eq!(field.binds, "credential.vercel.token");
    assert!(
        !field.label.is_empty() && !field.help.is_empty(),
        "a field must be renderable: `label` and `help` are what a settings page shows"
    );
    assert_eq!(
        field.example, None,
        "a secret field carries no example — a token-shaped placeholder trips secret scanning"
    );
}

/// `verify` is the one operation guaranteed to run unattended whenever a settings page opens, so it
/// must be a read.
#[test]
fn verify_is_a_read() {
    let connector = vercel();
    let verify = connector
        .operations
        .iter()
        .find(|operation| Some(operation.id.as_str()) == connector.verify.as_deref())
        .expect("`verify` names a declared operation");
    assert_eq!(verify.method, HttpMethod::Get);
    assert_eq!(verify.risk, Risk::Low);
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op.
#[test]
fn every_vercel_operation_emits_an_analyzable_module_without_secret_material() {
    let connector = vercel();
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
            Some(emitted.as_str()),
            "the flux formatter would rewrite {}",
            operation.id
        );

        let module = flux_lang::program::Module::parse_str(&emitted)
            .unwrap_or_else(|error| panic!("{} does not load: {error}", operation.id));
        let program = module.program().expect("program");
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, operation.id);
    }
}
