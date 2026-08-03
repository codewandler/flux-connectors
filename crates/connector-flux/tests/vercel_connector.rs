//! Contract for the deliberately narrow Vercel provider (C-170), as C-187 reshaped it.
//!
//! **The archetype this connector exists for.** Every Vercel endpoint here takes a `?teamId=` query
//! parameter that is *optional in the API and load-bearing in effect*: omit it and the call is
//! scoped to the caller's personal account instead of any team, silently — no error, no prompt, and
//! (for the two `list` operations) a complete, plausible-looking response for the wrong account.
//!
//! C-170 could only mitigate that with prose, because nothing in `ConfigField::binds` reached a
//! query parameter: `teamId` shipped as an optional caller argument whose `description` warned about
//! the fallback, which is text a model reads and may not act on. **C-187 removed the hazard instead
//! of describing it.** `teamId` is now pinned by a `[[config]]` field (`binds = "query.teamId"`),
//! mandatory, and a parameter of no operation — so the "not sent" state the archetype is about does
//! not exist, and a model cannot act as an account the operator did not name.
//!
//! `the_team_is_pinned_by_configuration_and_is_a_parameter_of_no_operation` is the acceptance test,
//! and `every_operation_sends_the_pinned_team` is what stops a later edit from scoping only some of
//! the service's calls — which would leave the rest addressing a different account, the same failure
//! by another route.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{Binding, Connector, HttpMethod, Idempotency, Level, Position, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

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
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
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

/// **The acceptance assertion** (C-187): the team is configuration, not an argument.
///
/// Three properties, and each removes one way the original hazard could come back. It is *pinned*,
/// so it is supplied once rather than chosen per call. It is *required*, so there is no state in
/// which the parameter simply is not sent — which is the entire archetype. And it is a parameter of
/// no operation, so a model cannot override the operator's choice of account.
#[test]
fn the_team_is_pinned_by_configuration_and_is_a_parameter_of_no_operation() {
    let connector = vercel();

    let team = connector
        .config_field("team_id")
        .expect("`[[config]]` must ask for the team this connection acts on behalf of");
    assert_eq!(
        team.binding(),
        Some(Binding::Request {
            position: Position::Query,
            name: "teamId"
        })
    );
    assert_eq!(
        team.level(),
        Some(Level::Connection),
        "a team is one per tenant, not one per vendor — the level is derived from `binds`"
    );
    assert!(
        !team.secret,
        "a Vercel team id is a public identifier, shown in the vendor's own dashboard URL"
    );
    assert!(
        team.required,
        "the whole hazard is a `teamId` that goes unsent; an optional pin would reproduce it"
    );

    for operation in &connector.operations {
        assert!(
            operation.params.query.iter().all(|p| p.name != "teamId"),
            "`{}` declares `teamId` as a query parameter. A value an operator pins at install time \
             and a caller may also pass is not pinned — the caller's wins, and a write can land on \
             an account the operator never named",
            operation.id
        );
    }
}

/// **Every operation sends it, not merely most of them.**
///
/// A tenant scope honoured on some of a service's calls and not others leaves the rest addressing a
/// different account, which is the original failure wearing a different hat. Asserted on the emitted
/// Flux rather than on the declaration, because the emitted module is what actually travels.
#[test]
fn every_operation_sends_the_pinned_team() {
    let connector = vercel();
    for id in OPERATIONS {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == *id)
            .unwrap_or_else(|| panic!("missing {id}"));
        let flux = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{id}` must emit: {error}"));

        assert!(
            flux.contains("teamId = \"{teamId}\""),
            "`{id}` must bind the pinned team as a literal carrying its placeholder:\n{flux}"
        );
        assert!(
            flux.lines().any(|line| {
                line.contains("http.request")
                    && line.contains("query: {")
                    && line.contains("teamId")
            }) && !flux.contains("when teamId"),
            "`{id}` must send the structured `teamId` field unconditionally — a pinned value has \
             no \"not supplied\" state:\n{flux}"
        );
        assert!(
            !flux.starts_with(&format!("op {id}(teamId")),
            "`{id}` must not declare the pinned team as its first parameter:\n{flux}"
        );
    }
}

/// The two `list` operations are the ones whose wrong-account failure is silent, so their own
/// descriptions — the text a model reads when choosing a tool — must say which account they answer
/// for. The claim changed with the shape: it is no longer "omitting `teamId` is dangerous" but "this
/// connector is installed for one team and reaches no other".
#[test]
fn list_operations_state_the_account_they_answer_for() {
    let connector = vercel();
    for id in ["vercel-projects-list", "vercel-deployments-list"] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("missing {id}"));
        let description = operation.description.to_lowercase();
        assert!(
            description.contains("team") && description.contains("pinned at install time"),
            "`{id}` description does not say which account it answers for. Full text: {:?}",
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

/// The connection-level configuration surface: the access token and the pinned team, and no
/// realistic-looking example on the token. The pair is the connector's whole install form — one
/// field says what it may do, the other what it may do it to.
#[test]
fn the_access_token_is_configurable_and_carries_no_example_value() {
    let connector = vercel();

    let names: Vec<&str> = connector.config.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["token", "team_id"]);
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
