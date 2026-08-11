//! The GitLab connector, and the property C-166 exists to state: **every project reference is a
//! numeric project id, never GitLab's URL-encoded namespaced path.**
//!
//! GitLab addresses a project two ways — a numeric id (`278964`) or a URL-encoded
//! `namespace%2Fproject` (`gitlab-org%2Fgitlab`) — and both are accepted by the same `:id` path
//! segment on every project-scoped endpoint. The encoded form is what a human actually has (nobody
//! carries a project's numeric id around), but producing it needs a percent-encoder this pipeline
//! does not have: `crates/connector-flux/src/op.rs` interpolates every path and query value
//! *verbatim* into a `fmt` template, and `zendesk-ticket-search` is AGENTS.md's standing
//! demonstration of what an unencoded `/` or `%` does to a request. A `namespace%2Fproject` value
//! typed as a plain string would ship a parameter a caller must pre-encode by hand before calling —
//! exactly what the story forbids — or, typed as the raw `group/project` slash form, would corrupt
//! the path template's own segment boundaries the moment flux interpolated it.
//!
//! So this connector takes the other branch, the one C-106 took for Stripe: **numeric id only**.
//! Every project-scoped operation below declares its project parameter as a JSON Schema `integer`,
//! which cannot carry a `/` or a `%` in the first place — safe by construction, not merely untested,
//! the same reasoning `providers/stripe.toml` applies to a Stripe object id's `pattern`. The
//! consequence stated on every such parameter's `description` is the one the story requires: a
//! caller who has only the namespaced path cannot use this connector until a percent-encoder lands.
//!
//! [`every_project_reference_is_a_bare_numeric_id`] is the test that keeps that true — not a
//! parse-succeeds smoke test, but an assertion that would fail the moment a future edit relaxed a
//! project parameter's schema to `string` (which would silently reopen the door to the encoded
//! form) or introduced a second, differently-named project parameter that skipped the check.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{
    Approval, AuthScheme, Connector, Format, Idempotency, Level, OperationDirection, Risk,
};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider id, and therefore the file name and every op id's prefix.
const PROVIDER: &str = "gitlab";

/// The connector owns the REST path while one reviewed default or operator-approved origin fills
/// the endpoint head.
const BASE_URL: &str = "{origin}/api/v4";
const DEFAULT_ORIGIN: &str = "https://gitlab.com";

/// The credential name and the environment variable it resolves from.
const TOKEN: &str = "gitlab.token";
const TOKEN_ENV: &str = "GITLAB_TOKEN";

/// The delegated credential (C-530): obtained by an OAuth2 grant the host runs on behalf of a
/// signed-in user, and therefore bounded by that user's own permissions rather than the
/// application's.
const OAUTH_TOKEN: &str = "gitlab.oauth_token";

/// The caller-facing name every project-scoped operation gives its numeric project id. One constant
/// so the test below cannot drift from the connector without a compile-time reminder of why it
/// matters — see the module documentation.
const PROJECT_ID: &str = "project_id";

/// The curated operation set, each with the risk and idempotency C-166's Acceptance requires it to
/// carry, and whether it addresses a project at all (`gitlab-user-get` does not — it is the `verify`
/// read and takes no parameters).
const OPERATIONS: &[(&str, Risk, Idempotency, bool)] = &[
    ("gitlab-user-get", Risk::Low, Idempotency::Idempotent, false),
    // The two discovery reads (C-527), and the decision this table exists to make explicit. Neither
    // addresses a project, because they are what a caller uses to *find* one: every other operation
    // here takes a numeric `project_id`, and until now nothing in the connector could produce it.
    // `gitlab-project-list` returns that id — and `http_url_to_repo` beside it, which is the clone
    // address a git client is given, since cloning is not a connector operation.
    (
        "gitlab-group-list",
        Risk::Low,
        Idempotency::Idempotent,
        false,
    ),
    (
        "gitlab-project-list",
        Risk::Low,
        Idempotency::Idempotent,
        false,
    ),
    (
        "gitlab-issue-list",
        Risk::Low,
        Idempotency::Idempotent,
        true,
    ),
    ("gitlab-issue-get", Risk::Low, Idempotency::Idempotent, true),
    (
        "gitlab-issue-create",
        Risk::High,
        Idempotency::NonIdempotent,
        true,
    ),
    (
        "gitlab-merge-request-list",
        Risk::Low,
        Idempotency::Idempotent,
        true,
    ),
    (
        "gitlab-pipeline-get",
        Risk::Low,
        Idempotency::Idempotent,
        true,
    ),
    (
        "gitlab-branch-list",
        Risk::Low,
        Idempotency::Idempotent,
        true,
    ),
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that has drifted from what ships.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-166 ships the GitLab connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The connector exists, loads, and is the one C-166 specifies: a bearer personal access token over
/// `gitlab.com`, with the curated operation set and a read-shaped `verify`.
#[test]
fn the_gitlab_connector_loads_and_authenticates_with_a_bearer_personal_access_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "GitLab");
    assert_eq!(
        connector.base_url, BASE_URL,
        "the connector must retain ownership of `/api/v4` after the resolved origin"
    );

    let token = connector
        .auth_method(TOKEN)
        .unwrap_or_else(|| panic!("gitlab declares `{TOKEN}`"));
    assert_eq!(
        token.scheme,
        AuthScheme::Bearer,
        "the story states `Authorization: Bearer <pat>`"
    );
    assert_eq!(token.env, [TOKEN_ENV]);

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    let expected: Vec<&str> = OPERATIONS.iter().map(|(id, ..)| *id).collect();
    assert_eq!(
        declared, expected,
        "the operation set is curated and ordered; adding one is a decision, not an omission"
    );

    for (id, risk, idempotency, _) in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("gitlab declares `{id}`"));
        assert_eq!(
            operation.risk, *risk,
            "operation `{id}` has an unexpected risk"
        );
        assert_eq!(
            operation.idempotency, *idempotency,
            "operation `{id}` has an unexpected idempotency"
        );
        assert!(
            !operation.description.is_empty(),
            "operation `{id}` has no description — every description here is a model's tool contract"
        );

        // **Two alternatives, not two requirements** (C-530). Every operation authenticates with
        // either the delegated OAuth token or the static personal access token, and each alternative
        // is a single credential. That is one connector serving two genuinely different
        // deployments — an org-wide token provisioned once, or a grant each user completes for
        // themselves — and nothing here chooses between them: `subject` is what lets a host decide.
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            2,
            "operation `{id}` has {} auth alternatives; gitlab offers the delegated grant and the \
             static token",
            effective.len()
        );
        assert!(
            effective.iter().all(|alternative| alternative.len() == 1),
            "operation `{id}` requires two credentials at once; each alternative is one credential"
        );
        assert!(
            effective
                .iter()
                .any(|alternative| alternative.contains(TOKEN)),
            "operation `{id}` no longer accepts the personal access token"
        );
        assert!(
            effective
                .iter()
                .any(|alternative| alternative.contains(OAUTH_TOKEN)),
            "operation `{id}` no longer accepts the delegated OAuth token"
        );
    }

    // The "Test connection" button: a parameterless read that proves the token resolves.
    assert_eq!(
        connector.verify.as_deref(),
        Some("gitlab-user-get"),
        "`verify` names the parameterless authenticated-user read"
    );
    let verify_op = connector.operation("gitlab-user-get").unwrap();
    assert_eq!(
        verify_op.risk,
        Risk::Low,
        "a `verify` operation must be a read; the loader refuses a `high` or `destructive` one"
    );
}

/// **The headline assertion.** Every project-scoped operation addresses its project as a bare
/// `integer` — GitLab's numeric project id — and never as a `string`, which is what would let the
/// URL-encoded `namespace%2Fproject` form back in.
///
/// Checked on the IR *and* on the emitted path template, because they fail differently: a `string`
/// schema here would ship a parameter callers still must pre-encode by hand — the exact outcome the
/// story forbids — and a change to the *emitted* path (say, a `wire` alias that stopped
/// interpolating `{project_id}`) would fail this half even with a correct schema.
#[test]
fn every_project_reference_is_a_bare_numeric_id() {
    let connector = load();

    for (id, _, _, addresses_project) in OPERATIONS {
        let operation = connector.operation(id).unwrap();
        if !addresses_project {
            assert!(
                !operation.path.contains(&format!("{{{PROJECT_ID}}}")),
                "operation `{id}` is declared not to address a project but its path is {:?}",
                operation.path
            );
            continue;
        }

        assert!(
            operation.path.contains(&format!("{{{PROJECT_ID}}}")),
            "operation `{id}` addresses a project but its path {:?} does not interpolate \
             `{{{PROJECT_ID}}}`",
            operation.path
        );

        let param = operation
            .params
            .path
            .iter()
            .find(|param| param.name == PROJECT_ID)
            .unwrap_or_else(|| {
                panic!("operation `{id}` declares no `{PROJECT_ID}` path parameter")
            });

        assert_eq!(
            param.schema.get("type").and_then(|t| t.as_str()),
            Some("integer"),
            "operation `{id}` types `{PROJECT_ID}` as {:?}, not `integer`. GitLab also accepts a \
             URL-encoded `namespace%2Fproject` on this same path segment, but this pipeline has no \
             percent-encoder (AGENTS.md, `zendesk-ticket-search`), so only the numeric id may be \
             accepted here",
            param.schema.get("type")
        );
        assert!(
            param.required,
            "operation `{id}`'s `{PROJECT_ID}` must be required; there is no default project"
        );
        assert!(
            param.description.to_lowercase().contains("numeric"),
            "operation `{id}`'s `{PROJECT_ID}` description does not say it takes the numeric id \
             only — a model reading the tool contract has no other way to learn that the encoded \
             path form is refused"
        );

        // And on the emitted side: the path template flux actually builds interpolates the same
        // symbol, so a `wire` alias or a hand-edited path could not reintroduce the encoded form
        // without this test noticing.
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("operation `{id}` is not emittable: {error}"));
        assert!(
            emitted.contains(&format!("{{{PROJECT_ID}}}")),
            "operation `{id}`'s emitted Flux does not interpolate `{{{PROJECT_ID}}}`:\n{emitted}"
        );
    }
}

/// No operation declares a second, differently-named project-identifying parameter — the check
/// above only inspects `project_id`, so this closes the gap a parameter named e.g. `id_or_path` or
/// `project_path` would otherwise slip through unnoticed.
#[test]
fn no_operation_declares_a_project_path_parameter() {
    let connector = load();
    const FORBIDDEN: &[&str] = &["project_path", "namespace", "id_or_path", "full_path"];

    for operation in &connector.operations {
        for param in operation.params.iter() {
            assert!(
                !FORBIDDEN.contains(&param.name.as_str()),
                "operation `{}` declares `{}`, which reads as GitLab's URL-encoded namespaced-path \
                 form — this connector accepts only the numeric `{PROJECT_ID}`",
                operation.id,
                param.name
            );
        }
    }
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op.
#[test]
fn every_gitlab_operation_emits_an_analyzable_module() {
    let connector = load();
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
        assert_eq!(
            program.ops.len(),
            1,
            "one operation is one declaration; `{}` loaded {}",
            operation.id,
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, operation.id);
    }
}

/// The four `[[config]]` fields, and the level each one derives.
///
/// Two are **connection** level, supplied per tenant: the operator-approved origin, whose reviewed
/// default preserves GitLab.com, and the personal access token. Two are **operator** level, supplied
/// once by whoever runs the product: the OAuth application's id and secret (C-530). The split is not
/// authored — `Level` derives from `binds`, which is what stops an end user ever being asked for the
/// product's own client secret.
#[test]
fn the_config_surface_asks_for_the_origin_the_token_and_the_oauth_app() {
    let connector = load();

    assert_eq!(
        connector.config.len(),
        5,
        "gitlab needs one endpoint choice, one credential and one OAuth app registration — id, \
         secret and redirect URI — with no GitLab-only side channel"
    );
    let level_of = |binds: &str| {
        connector
            .config
            .iter()
            .find(|field| field.binds == binds)
            .unwrap_or_else(|| panic!("gitlab declares no field binding `{binds}`"))
    };
    for (binds, secret) in [
        ("oauth.client_id", false),
        ("oauth.client_secret", true),
        // The third half of the registration (C-531): issued with the other two, supplied by the
        // same person, and public — it travels in the authorize request as a query parameter.
        ("oauth.redirect_uri", false),
    ] {
        let field = level_of(binds);
        assert_eq!(
            field.level(),
            Some(Level::Operator),
            "`{binds}` must be operator level — asking a tenant for it hands them the product's own \
             application"
        );
        assert_eq!(field.secret, secret, "`{binds}` has the wrong secrecy");
    }
    assert_eq!(
        level_of("endpoint.origin").level(),
        Some(Level::Connection),
        "the origin is chosen per connection, not once per vendor"
    );
    let origin = connector
        .config_field("origin")
        .expect("gitlab declares its origin field");
    assert!(
        !origin.label.is_empty(),
        "a config field must be renderable"
    );
    assert!(!origin.help.is_empty(), "a config field must be renderable");
    assert_eq!(origin.format, Format::Origin);
    assert_eq!(origin.approval, Approval::Operator);
    assert_eq!(origin.default.as_deref(), Some(DEFAULT_ORIGIN));
    assert_eq!(origin.binds, "endpoint.origin");
    assert!(!origin.secret, "an origin is public connection metadata");

    let field = connector
        .config
        .iter()
        .find(|field| field.binds == format!("credential.{TOKEN}"))
        .expect("gitlab declares its personal access token field");
    assert!(!field.label.is_empty(), "a config field must be renderable");
    assert!(!field.help.is_empty(), "a config field must be renderable");
    assert_eq!(field.binds, format!("credential.{TOKEN}"));
    assert!(
        field.secret,
        "the token binds a credential, so `secret` must be true"
    );
    let binding = field
        .binding()
        .unwrap_or_else(|| panic!("`{}` does not parse as a binding", field.binds));
    assert_eq!(
        field.secret,
        binding.is_secret(),
        "`secret` disagrees with what `binds` resolves to"
    );
}

/// Only `gitlab-issue-create` is authored as a write, independently of transport method.
#[test]
fn only_the_create_operation_mutates() {
    let connector = load();
    for operation in &connector.operations {
        let mutates = operation.direction == OperationDirection::Write;
        let should_mutate = operation.id == "gitlab-issue-create";
        assert_eq!(
            mutates, should_mutate,
            "operation `{}` carries {:?}, which disagrees with whether it should mutate",
            operation.id, operation.direction
        );
    }
}
