//! `providers/bitbucket.toml` exists, emits analyzable Flux, and is **C-187's first real consumer** —
//! the story this connector is actually for.
//!
//! C-187 landed `binds = "path.<name>"` so an operator can pin a tenant scope at install time, and
//! `providers/cloudflare.toml` moved its `zone_id` onto it in the same wave. Cloudflare is a *retro*
//! fit: four of its five operations are scoped and one, `cloudflare-zone-list`, exists to discover
//! the value. Bitbucket is the shape the mechanism was designed for and is stricter in three ways
//! this file asserts rather than describes:
//!
//! 1. **Every operation is scoped.** Each one is under `/2.0/repositories/{workspace}/…`, so the pin
//!    is not a narrowing applied to most of a connector — it is the connector's entire address space.
//!    `the_workspace_is_pinned_and_every_operation_is_scoped_to_it` is the assertion.
//! 2. **There is no discovery operation, and none is needed.** Cloudflare's zone id is 32 opaque hex
//!    characters an operator has to fetch; a Bitbucket workspace slug is the segment in the vendor's
//!    own URL bar. `the_connector_ships_no_unscoped_discovery_operation` keeps a later edit from
//!    adding an unscoped `GET /2.0/workspaces` and quietly widening the connector past the pin.
//! 3. **The pin is what makes `verify` argument-free.** `bitbucket-repository-list` is
//!    `GET /2.0/repositories/{workspace}`: its path carries a variable and its parameter list is
//!    empty, which is only expressible because the variable is pinned.
//!    `the_verify_operation_is_an_argument_free_read_the_pin_makes_possible` is the assertion, and it
//!    is the one that would have been impossible to write before C-187.
//!
//! The fourth test worth naming is `a_pasted_workspace_url_is_refused_at_the_example_and_nowhere_else`
//! — the [C-214] finding, held open on purpose. `Position::validate_value` refuses a value that would
//! escape its path segment, the loader applies it to `example`, and **nothing applies it to the value
//! an operator actually types**. The mistake this field invites is pasting a Bitbucket URL instead of
//! the slug inside it, which is exactly the value that rule would catch and exactly the value it is
//! not run against yet. The test pins both halves so that closing C-214 makes it stronger rather than
//! stale.
//!
//! [C-214]: ../../../docs/stories/C-214-a-pinned-value-reaches-the-wire-unvalidated.md

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{Binding, Connector, HttpMethod, Idempotency, Level, Position, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// `<repo root>/providers/bitbucket.toml`, derived from this crate's manifest directory so the test
/// is independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("bitbucket.toml")
}

fn bitbucket() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-217 ships the Bitbucket connector",
            path.display()
        )
    });
    shipped_provider::load_definition("bitbucket", &source)
        .expect("providers/bitbucket.toml does not load")
        .connector
}

fn op<'a>(connector: &'a Connector, id: &str) -> &'a connector_spec::Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("no operation named {id:?} in the curated set"))
}

fn emit(connector: &Connector, id: &str) -> String {
    emit_operation(connector, op(connector, id))
        .unwrap_or_else(|error| panic!("`{id}` is not emittable: {error}"))
}

/// The connector exists, loads through the real loader, and addresses Bitbucket **Cloud**.
///
/// The host is the assertion that matters here. Bitbucket Server / Data Center is a different
/// product with a different API (`/rest/api/1.0`, on an installation's own host) and this connector
/// does not reach it; a later edit that templated the host would be shipping a second, undescribed
/// API under one id.
#[test]
fn the_bitbucket_cloud_connector_loads() {
    let connector = bitbucket();

    assert_eq!(connector.id, "bitbucket");
    assert_eq!(connector.vendor, "Bitbucket");
    assert_eq!(
        connector.base_url, "https://api.bitbucket.org/2.0",
        "Bitbucket Cloud's one API host. Server/Data Center is a different API on a customer's own \
         host and is out of scope — see the provider file's header"
    );
    assert!(
        !connector.base_url.contains('{'),
        "a templated host would mean this connector also claims to reach Bitbucket Server, which it \
         does not describe: {}",
        connector.base_url
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// The curated set, exactly. Endpoints deliberately excluded are named in the provider file's
/// header; this test is what makes "curated" a fact rather than an intention.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = bitbucket();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "bitbucket-pull-request-approve",
            "bitbucket-pull-request-comment",
            "bitbucket-pull-request-create",
            "bitbucket-pull-request-get",
            "bitbucket-pull-request-list",
            "bitbucket-repository-get",
            "bitbucket-repository-list",
        ],
        "the curated set changed from the one C-217 names"
    );
}

/// **The probe.** `workspace` is an operator's pin, and it scopes *every* operation.
///
/// Three claims, each of which a later edit could break independently:
///
/// - the `[[config]]` field binds `path.workspace`, is connection level, is required, and is not a
///   secret — the four properties C-187 derives rather than lets an author state;
/// - every operation's path carries `{workspace}`;
/// - no operation declares `workspace` as a parameter, so a model holding this connector cannot
///   address a workspace the operator did not choose.
///
/// The last is what makes it a pin rather than a default. A value an operator supplies at install
/// time *and* a caller may also pass is not pinned: the caller's wins.
#[test]
fn the_workspace_is_pinned_and_every_operation_is_scoped_to_it() {
    let connector = bitbucket();

    let workspace = connector
        .config_field("workspace")
        .expect("`[[config]]` must ask for the workspace this connection manages");
    assert_eq!(
        workspace.binding(),
        Some(Binding::Request {
            position: Position::Path,
            name: "workspace"
        }),
        "the whole point of this connector is that the workspace is a pinned path segment"
    );
    assert_eq!(
        workspace.level(),
        Some(Level::Connection),
        "a workspace is one per tenant, not one per vendor — the level is derived from `binds`, \
         never authored"
    );
    assert!(
        !workspace.secret,
        "a workspace slug is the segment in a Bitbucket URL; marking it secret would claim gating \
         this repository does not provide, and `secret` must agree with `binds`"
    );
    assert!(
        workspace.required,
        "a host refuses the whole request when a pinned value is missing, so an optional pin is a \
         connector that composes no URL"
    );

    for operation in &connector.operations {
        assert!(
            operation.path.contains("{workspace}"),
            "`{}` is not scoped to the pinned workspace; its path is {:?}. Every operation this \
             connector ships is under /2.0/repositories/{{workspace}}/… — an unscoped one would \
             reach outside the workspace the operator pinned",
            operation.id,
            operation.path
        );
        assert!(
            operation.params.path.iter().all(|p| p.name != "workspace"),
            "`{}` declares `workspace` as a path parameter. A value an operator pins at install \
             time and a caller may also pass is not pinned — the caller's wins, and the operator's \
             choice of workspace becomes a suggestion",
            operation.id
        );
    }
}

/// **No discovery operation, and none is needed** — the difference from Cloudflare worth keeping.
///
/// `cloudflare-zone-list` exists because a zone id is 32 opaque hex characters an operator cannot
/// know without asking. A Bitbucket workspace slug is the first path segment of every
/// `bitbucket.org` URL an operator already has open, so this connector needs no unscoped read to
/// find it — and adding one (`GET /2.0/workspaces`, `GET /2.0/user`) would widen the connector past
/// the pin it exists to demonstrate, for a value nobody has to look up.
#[test]
fn the_connector_ships_no_unscoped_discovery_operation() {
    let connector = bitbucket();

    let unscoped: Vec<&str> = connector
        .operations
        .iter()
        .filter(|operation| !operation.path.starts_with("/repositories/{workspace}"))
        .map(|operation| operation.id.as_str())
        .collect();
    assert!(
        unscoped.is_empty(),
        "these operations are not under the pinned workspace: {unscoped:?}. The pin is this \
         connector's entire address space; an operation outside it reaches whatever the token \
         reaches, which is the shape C-187 exists to end"
    );
}

/// **The pin reaches every emitted module as a placeholder, never as a value.**
///
/// The workspace is bound to a string literal spelling its own placeholder, exactly as a templated
/// `base_url` is, and `connector-pack` substitutes a tenant's value into literals only. Nothing in
/// this repository holds a workspace slug — a tenant identifier committed here is the same category
/// error as a credential value — and nothing about the pin reaches the declared parameter list.
#[test]
fn the_pinned_workspace_reaches_every_module_as_a_substitutable_placeholder() {
    let connector = bitbucket();

    for operation in &connector.operations {
        let flux = emit(&connector, &operation.id);
        assert!(
            flux.contains(r#"workspace = "{workspace}""#),
            "`{}` must bind the pin as a literal carrying its placeholder, which is what a host \
             substitutes into:\n{flux}",
            operation.id
        );
        // No trailing slash in the needle. `bitbucket-repository-list`'s path is exactly
        // `/repositories/{workspace}` — the pin is its *last* segment — and it is the one operation
        // that must satisfy this, because it is the argument-free `verify` the pin exists to make
        // possible. Requiring `{workspace}/` here would assert that the connector's own probe is not
        // scoped.
        assert!(
            flux.contains("/repositories/{workspace}"),
            "`{}` must interpolate the pinned symbol into its URL:\n{flux}",
            operation.id
        );
        let signature = flux.lines().next().expect("a declaration line");
        assert!(
            !signature.contains("workspace"),
            "the pinned value must not reach `{}`'s declared parameter list:\n{signature}",
            operation.id
        );
    }
}

/// **`verify` is an argument-free read, and the pin is what makes it one.**
///
/// `GET /2.0/repositories/{workspace}` has a path variable and no parameters. That combination is
/// the mechanism working: before C-187 this operation would have declared `workspace` as a required
/// path parameter, and a "Test connection" button that needs an argument is not a button.
#[test]
fn the_verify_operation_is_an_argument_free_read_the_pin_makes_possible() {
    let connector = bitbucket();

    assert_eq!(
        connector.verify.as_deref(),
        Some("bitbucket-repository-list"),
        "the `verify` operation is the Test-connection button and must be a read"
    );
    let verify = op(&connector, "bitbucket-repository-list");

    assert_eq!(verify.method, HttpMethod::Get);
    assert_eq!(verify.risk, Risk::Low);
    assert_eq!(verify.path, "/repositories/{workspace}");
    assert!(
        verify.params.path.is_empty()
            && verify.params.query.is_empty()
            && verify.params.body.is_empty(),
        "`verify` runs unattended whenever a settings page opens and must not require an argument a \
         human has not supplied yet — yet its path carries `{{workspace}}`, which is only possible \
         because the value is pinned"
    );

    let flux = emit(&connector, "bitbucket-repository-list");
    assert!(
        flux.starts_with("op bitbucket-repository-list -> Any"),
        "the verify op takes no arguments at all:\n{flux}"
    );
    assert!(
        flux.contains(r#"url = fmt("{base}/repositories/{workspace}")"#),
        "the verify URL is composed entirely from the base and the pin:\n{flux}"
    );
}

/// **[C-214], held open rather than papered over.**
///
/// `Position::Path::validate_value` is the rule that a pinned path value must stay inside its
/// segment, and it is real: the paste an operator is most likely to make into a field called
/// "Bitbucket workspace" — the URL the slug came from, or `workspace/repo` — is exactly what it
/// refuses. The loader runs it against `example` (`crates/connector-spec/src/provider.rs`), so the
/// placeholder this connector shows cannot itself be a value the rule would reject.
///
/// **What it does not run against is the value an operator types.** C-214 measured that: substitution
/// happens in `connector-pack`'s `request.rs` with no predicate, so a workspace containing a `/`
/// produces a wrong URL at the right vendor rather than a refusal. This connector therefore carries
/// the shape in its `help` text instead of relying on a check that is not wired up yet, and this test
/// asserts both halves so that closing C-214 strengthens it rather than making it stale.
#[test]
fn a_pasted_workspace_url_is_refused_at_the_example_and_nowhere_else() {
    let connector = bitbucket();
    let workspace = connector
        .config_field("workspace")
        .expect("the pinned field exists");

    let example = workspace
        .example
        .as_deref()
        .expect("the workspace field shows an example — it is not a secret and a user copies it");
    assert!(
        Position::Path.validate_value(example).is_ok(),
        "the example must be a value the path-segment rule accepts; a placeholder that fails its \
         own field is worse than none"
    );

    for paste in [
        "https://bitbucket.org/acme-engineering/",
        "acme-engineering/widget-service",
        "..",
    ] {
        assert!(
            Position::Path.validate_value(paste).is_err(),
            "{paste:?} would escape its path segment and the rule that says so must keep saying it"
        );
    }

    assert!(
        workspace.help.contains("bitbucket.org/"),
        "until C-214 validates the substituted value, the `help` text is the only thing standing \
         between an operator and a pasted URL, so it must show where the slug is read from: {:?}",
        workspace.help
    );
}

/// Approving a pull request is `high` risk, and not claimed idempotent.
///
/// An approval is a *claim that a human reviewed this change*, attributed to the token's own
/// account, and in a repository with an approval merge check it is the thing standing between a
/// branch and its default branch. That is `high`'s own definition — a write a reviewer would want to
/// see first — rather than `medium`'s limited blast radius.
#[test]
fn approving_a_pull_request_is_high_risk_and_not_claimed_idempotent() {
    let connector = bitbucket();
    let approve = op(&connector, "bitbucket-pull-request-approve");

    assert_eq!(approve.method, HttpMethod::Post);
    assert_eq!(
        approve.risk,
        Risk::High,
        "an automated approval asserts a review that did not happen and can release a merge check — \
         a write a reviewer would want to see first"
    );
    assert_eq!(
        approve.idempotency,
        Idempotency::NonIdempotent,
        "Bitbucket answers a repeat approval with 409, not a repeat of the first 200"
    );
}

/// Every operation declares `risk` and `idempotency`, no read is a write and no write is a read, and
/// the emitted module carries all three metadata facets a host reads — including the derived
/// `effects ["network"]`, which is the only effect anything here has.
#[test]
fn every_operation_declares_its_safety_metadata_and_emits_it() {
    let connector = bitbucket();

    for operation in &connector.operations {
        let flux = emit(&connector, &operation.id);
        assert!(
            flux.contains(r#"effects ["network"]"#),
            "`{}` must carry the network effect:\n{flux}",
            operation.id
        );
        assert!(
            flux.contains(&format!(r#"risk "{}""#, risk_word(operation.risk))),
            "`{}` must carry its declared risk:\n{flux}",
            operation.id
        );
        assert!(
            !operation.description.is_empty(),
            "`{}` has no description, which is the contract a model reads",
            operation.id
        );

        match operation.method {
            HttpMethod::Get => assert_eq!(
                operation.risk,
                Risk::Low,
                "`{}` is a read; a read that is not `low` is either mis-declared or is not a read",
                operation.id
            ),
            _ => assert_ne!(
                operation.risk,
                Risk::Low,
                "`{}` writes and must not be declared `low`",
                operation.id
            ),
        }
    }
}

fn risk_word(risk: Risk) -> &'static str {
    match risk {
        Risk::Low => "low",
        Risk::Medium => "medium",
        Risk::High => "high",
        Risk::Destructive => "destructive",
    }
}

/// One credential, placed as a bearer token, and **no example on it**. A token-shaped placeholder
/// has tripped GitHub's push protection in this repository's history and blocked a release.
#[test]
fn one_bearer_credential_covers_the_connector_and_carries_no_example() {
    let connector = bitbucket();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["bitbucket.access_token"],
        "one workspace-scoped access token covers the whole selection"
    );

    let field = connector
        .config_field("access_token")
        .expect("the access token is the one secret a human must supply");
    assert!(field.secret, "an access token is a secret");
    assert_eq!(field.binds, "credential.bitbucket.access_token");
    assert_eq!(
        field.example, None,
        "a secret field carries no example — a token-shaped placeholder trips secret scanning"
    );
}

/// Every configuration field is renderable, and `secret` agrees with `binds` on every one of them.
///
/// The agreement is a loader rule, so this cannot fail on a connector that loads — which is the
/// point: it states the invariant at the file a reviewer of *this* connector reads, and it fails
/// loudly if a later edit adds a field whose label or help is missing, which the loader does not
/// check beyond emptiness.
#[test]
fn every_configuration_field_is_renderable_and_agrees_with_its_binding() {
    let connector = bitbucket();

    let mut names: Vec<&str> = connector
        .config
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["access_token", "workspace"],
        "the install form is exactly two questions: what this connector may do, and what it may do \
         it to"
    );

    for field in &connector.config {
        let binding = field
            .binding()
            .expect("a loaded connector parses its binds");
        assert_eq!(
            field.secret,
            binding.is_secret(),
            "`{}` declares `secret = {}` against a binding that is {}",
            field.name,
            field.secret,
            binding.is_secret()
        );
        assert!(
            !field.label.is_empty() && !field.help.is_empty(),
            "`{}` is not renderable: `label` and `help` are what a settings page shows",
            field.name
        );
        assert!(
            field.docs_url.is_some(),
            "`{}` must say where a human goes to get the value",
            field.name
        );
    }
}

/// No Bitbucket operation declares a query parameter — the same curation choice
/// `providers/trello.toml` and `providers/cloudflare.toml` record. This emitter interpolates query
/// values verbatim and percent-encodes nothing, so a filter carrying `&` would inject a parameter of
/// its own; the filters this connector leaves out are named in its header.
#[test]
fn no_bitbucket_operation_declares_a_query_parameter() {
    let connector = bitbucket();

    for operation in &connector.operations {
        assert!(
            operation.params.query.is_empty(),
            "operation `{}` declares query parameters, which the provider file's header says this \
             connector deliberately does not",
            operation.id
        );
        let flux = emit(&connector, &operation.id);
        assert!(
            !flux.contains(r#"url = fmt("{base}"#) || !flux.contains('?'),
            "`{}` emits a query string:\n{flux}",
            operation.id
        );
    }
}

/// Every Bitbucket operation emits Flux that parses, is canonical under flux's own formatter, and
/// loads as exactly one composite op — the C-11 gate, held against bitbucket specifically.
#[test]
fn every_bitbucket_operation_emits_an_analyzable_module() {
    let connector = bitbucket();

    for operation in &connector.operations {
        let emitted = emit(&connector, &operation.id);

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
