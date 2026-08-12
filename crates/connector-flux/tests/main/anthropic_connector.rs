//! `providers/anthropic.toml` exists, emits analyzable Flux, claims `llm_catalogue` without
//! reshaping the role, ships no inference operation, and **sends `anthropic-version` on every
//! request as a literal** (C-122).
//!
//! The last claim is the one this file exists for, mirroring
//! `crates/connector-flux/tests/notion_connector.rs`: Anthropic answers every request with no
//! `anthropic-version` header exactly as Notion answers `400 validation_error` to one missing
//! `Notion-Version` — no default, no grace period. Pinning the header with a JSON Schema `const` on
//! a caller-supplied `params.header` emits a required argument a model must guess with the
//! constraint silently dropped (C-55's own finding); `const_headers` is the honest mechanism, and
//! this file proves the *connector* uses it — on the emitted request, not just the IR — so a later
//! edit reverting to a parameter is caught here rather than shipped.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{Connector, HttpMethod, Role};

use crate::shipped_provider;

/// The version this connector is pinned to. Anthropic versions its API by date in this header, and
/// the value is a property of the *connector* — never a caller's to supply.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// `<repo root>/providers/anthropic.toml`, derived from this crate's manifest directory so the test
/// is independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("anthropic.toml")
}

fn anthropic() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-122 ships the Anthropic connector",
            path.display()
        )
    });
    shipped_provider::load_definition("anthropic", &source)
        .expect("providers/anthropic.toml does not load")
        .connector
}

/// The declaration line, which is where a caller-supplied argument would show up.
fn signature(emitted: &str) -> &str {
    emitted.lines().next().expect("a declaration line")
}

/// The connector exists, loads through the real loader, and is the one C-122 describes.
#[test]
fn the_anthropic_connector_loads() {
    let connector = anthropic();

    assert_eq!(connector.id, "anthropic");
    assert_eq!(connector.vendor, "Anthropic");
    assert_eq!(
        connector.base_url, "https://api.anthropic.com",
        "one tenant-independent host, never widened"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// **The headline acceptance assertion: the version header reaches every emitted operation.**
///
/// Both halves are checked and they fail differently. A missing literal is a broken request on the
/// first call; a surviving parameter is a required argument a model has to guess, which is the same
/// broken request one call later with a worse tool contract in between.
#[test]
fn the_version_header_reaches_every_emitted_operation() {
    let connector = anthropic();
    let expected_binding = format!(r#"anthropic_version = "{ANTHROPIC_VERSION}""#);

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).unwrap_or_else(|error| {
            panic!("operation `{}` is not emittable: {error}", operation.id)
        });

        assert!(
            emitted.contains(&expected_binding),
            "`{}` does not bind the version as a literal. Anthropic requires `anthropic-version` on \
             every request, so an operation missing it cannot work at all:\n{emitted}",
            operation.id
        );
        assert!(
            emitted.contains(r#""anthropic-version": anthropic_version"#),
            "`{}` binds the version but never sends it — the literal must reach the request under \
             Anthropic's own header spelling:\n{emitted}",
            operation.id
        );
        assert!(
            !signature(&emitted)
                .to_lowercase()
                .contains("anthropic_version"),
            "`{}` declares the version as a caller-supplied argument. It is a constant of the \
             connector, not an input: a model would have to guess it on every call and any caller \
             could set it to anything. Declare it in `const_headers` (C-55):\n{}",
            operation.id,
            signature(&emitted)
        );
    }
}

/// The same claim on the IR, so a provider file that dropped `const_headers` fails here with a
/// message naming the mechanism rather than only as a missing string in emitted text.
#[test]
fn every_operation_carries_the_version_in_const_headers() {
    let connector = anthropic();
    for operation in &connector.operations {
        assert_eq!(
            operation.params.const_headers.get("anthropic-version"),
            Some(&ANTHROPIC_VERSION.to_string()),
            "`{}` does not declare `anthropic-version` in `const_headers`. The provider-level \
             `const_headers` table is distributed onto every operation by the loader, so an \
             operation missing it means the table was removed or overridden",
            operation.id
        );
    }
}

/// The version is never a caller-supplied header parameter — the spelling C-55 refuses.
#[test]
fn no_operation_declares_the_version_as_a_header_parameter() {
    let connector = anthropic();
    for operation in &connector.operations {
        for header in &operation.params.header {
            let name = header.name.to_ascii_lowercase();
            let wire = header
                .wire
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            assert!(
                !name.contains("anthropic-version") && !wire.contains("anthropic-version"),
                "`{}` declares the version as a caller-supplied header parameter `{}`. \
                 `params.header` means caller-supplied; a vendor constant belongs in \
                 `const_headers`",
                operation.id,
                header.name
            );
        }
    }
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op — the C-11 gate, held against anthropic specifically.
#[test]
fn every_anthropic_operation_emits_an_analyzable_module() {
    let connector = anthropic();
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

/// The curated set, exactly — C-122's five, widened to eleven by C-441. Named rather than counted
/// so that adding an operation — in particular an inference one — is a deliberate edit here.
///
/// C-441 made that edit for the six Admin API reads it added: organization members (list and get),
/// one workspace by id, workspace members (list and get), and outstanding invites. The charter
/// claim this assertion protects is unchanged and is about *inference*, not about size — so growing
/// the management surface is in scope for an edit here and `POST /v1/messages` never is. The
/// per-service half of the same claim, including that every Admin operation is a read on the admin
/// credential, lives in `crates/connector-spec/tests/anthropic_admin_surface.rs`.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = anthropic();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "anthropic-api-keys-list",
            "anthropic-invites-list",
            "anthropic-model-get",
            "anthropic-models-list",
            "anthropic-organization-get",
            "anthropic-organization-member-get",
            "anthropic-organization-members-list",
            "anthropic-workspace-get",
            "anthropic-workspace-member-get",
            "anthropic-workspace-members-list",
            "anthropic-workspaces-list",
        ],
        "the curated set changed — this connector ships management surface and model catalogue \
         only, never inference"
    );
}

/// **The charter boundary: no inference operation.** `docs/vision.md`'s non-goal excludes
/// "replacing flux's native model providers"; `POST /v1/messages` belongs to flux's own `anthropic`
/// provider, never to this connector.
#[test]
fn no_operation_is_the_messages_inference_endpoint() {
    let connector = anthropic();
    for operation in &connector.operations {
        let path_lower = operation.path.to_lowercase();
        assert!(
            !path_lower.contains("/messages") && !path_lower.contains("/complete"),
            "`{}` at `{}` looks like an inference endpoint. This connector is management-plane \
             only — inference stays with flux's native anthropic provider (vision.md's non-goal, \
             C-123)",
            operation.id,
            operation.path
        );
        assert_ne!(
            operation.method,
            HttpMethod::Post,
            "a POST snuck into a read-only connector — `{}` should not exist here unless the \
             story deliberately added a write, in which case this assertion should change with it",
            operation.id
        );
    }
}

/// **C-121's role, claimed without any change to its definition.** The `models` service fills
/// `llm_catalogue`'s one required slot (`list`), the same way `openai` and `openrouter` do — the
/// falsification this story exists to run: Anthropic's shape differs from both and still needs no
/// reshaping to satisfy it.
#[test]
fn the_models_service_claims_llm_catalogue_without_reshaping() {
    let connector = anthropic();

    assert_eq!(
        connector.roles(),
        vec![Role::LlmCatalogue],
        "the connector's derived roles should be exactly llm_catalogue, from the models service"
    );

    let missing = connector.missing_role_members("models", Role::LlmCatalogue);
    assert!(
        missing.is_empty(),
        "the models service claims llm_catalogue but is missing required member(s): {missing:?}"
    );

    // Both `list` and `get` are present, even though the role's mechanism only requires `list`
    // today — see the story's Acceptance and `Role::required_members` in
    // `crates/connector-spec/src/ir.rs`.
    let model_members: Vec<&str> = connector
        .operations_of("models")
        .map(|operation| operation.id.as_str())
        .collect();
    assert!(model_members.iter().any(|id| id.ends_with("list")));
    assert!(model_members.iter().any(|id| id.ends_with("get")));
}

/// The `verify` operation only ever needs the regular key, so pressing "Test connection" never
/// demands organization-admin access.
///
/// **This test said "two `x-api-key` credentials, never a bearer" until C-555, and that premise is
/// now false by design rather than by drift.** The connector declares four credentials: the two
/// Console-minted API keys, which still travel as `x-api-key` and are still the only two that do,
/// and the two Console OAuth2 tokens, which travel as bearers because that is how Anthropic accepts
/// an OAuth token. The claim worth keeping is the *pairing* — which scheme each credential uses, and
/// that `verify` needs the unprivileged one — so that is what is asserted now. A future credential
/// added to either group without a deliberate edit here is still caught.
#[test]
fn the_connector_verifies_with_the_regular_key_over_x_api_key() {
    let connector = anthropic();

    let by_scheme = |wanted: fn(&connector_spec::AuthScheme) -> bool| -> Vec<&str> {
        connector
            .auth
            .iter()
            .filter(|entry| wanted(&entry.scheme))
            .map(|entry| entry.name.as_str())
            .collect()
    };

    assert_eq!(
        by_scheme(|scheme| matches!(
            scheme,
            connector_spec::AuthScheme::Header { name, .. } if name == "x-api-key"
        )),
        ["anthropic.api_key", "anthropic.admin_key"],
        "two distinct x-api-key credentials: the regular key and the Admin API key. Anthropic \
         authenticates a key with a custom header and never with `Authorization: Bearer`, so a key \
         that moved to a bearer would be sending the right secret the wrong way"
    );
    assert_eq!(
        by_scheme(|scheme| matches!(scheme, connector_spec::AuthScheme::Bearer)),
        [
            "anthropic.console_oauth",
            "anthropic.console_oauth_admin",
            "anthropic.subscription_oauth"
        ],
        "the three OAuth2-acquired tokens, and only those, travel as bearers: the two Console \
         credentials and the Claude Pro/Max subscription token (C-555 round 2)"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("anthropic-models-list"),
        "the `verify` operation is the Test-connection button and must need only the regular key"
    );
    let verify = connector
        .operations
        .iter()
        .find(|operation| Some(operation.id.as_str()) == connector.verify.as_deref())
        .expect("`verify` names a declared operation");
    assert_eq!(
        verify.method,
        HttpMethod::Get,
        "`verify` runs unattended whenever a settings page opens, so it is a GET"
    );
}

/// The connection-level configuration surface: two token fields, and **no realistic-looking
/// example** on either — an `sk-ant-`-shaped placeholder is exactly the class that has tripped
/// GitHub push protection and blocked a release here before.
#[test]
fn the_credentials_are_configurable_and_carry_no_example_value() {
    let connector = anthropic();

    for name in ["api_key", "admin_key"] {
        let field = connector
            .config
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("`{name}` should be a configurable field"));

        assert!(field.secret, "a key is a secret");
        assert!(
            !field.label.is_empty() && !field.help.is_empty(),
            "a field must be renderable: `label` and `help` are what a settings page shows"
        );
        assert_eq!(
            field.example, None,
            "`{name}` carries an example — a key-shaped placeholder trips secret scanning and \
             teaches a reader to paste something that looks like a real value"
        );
    }
}

/// No anthropic operation declares a query parameter, so nothing this connector emits can carry an
/// unencoded value into a query string (C-30, the standing `zendesk-ticket-search` gap).
#[test]
fn no_anthropic_operation_declares_a_query_parameter() {
    let connector = anthropic();
    for operation in &connector.operations {
        assert!(
            operation.params.query.is_empty(),
            "operation `{}` declares query parameters {:?}. Nothing percent-encodes a query value \
             (C-30 is unimplemented), so a value carrying `&` or `#` corrupts the request or \
             injects a parameter",
            operation.id,
            operation
                .params
                .query
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>()
        );
    }
}

/// **The Console OAuth2 acquisition, declared as two credentials because privilege differs** (C-555).
///
/// Anthropic runs two browser OAuth flows and this connector declares exactly one of them: the
/// Console flow that `ant auth login` performs, whose authorize and token legs share one origin and
/// which is therefore expressible by today's single-`endpoint` `OAuth2Spec`. The subscription flow
/// (authorize on `claude.ai`, token on `platform.claude.com`) is two-host and is deliberately absent
/// — see `the_two_host_subscription_flow_is_not_half_declared` below.
///
/// The split into two credentials is the substantive claim. The Admin API needs `org:admin`; the
/// model catalogue does not, and this file already refuses to make the catalogue ask for
/// organization-admin access (`verify` is pinned to the models read for exactly that reason). One
/// OAuth credential carrying `org:admin` for both surfaces would undo that one line at a time, so a
/// regression that merges them is caught here.
#[test]
fn the_console_oauth_grant_is_declared_as_two_credentials_split_by_privilege() {
    let connector = anthropic();

    let credential = |name: &str| {
        connector
            .auth
            .iter()
            .find(|method| method.name == name)
            .unwrap_or_else(|| panic!("credential `{name}` should be declared"))
    };

    for name in ["anthropic.console_oauth", "anthropic.console_oauth_admin"] {
        let method = credential(name);
        let spec = method
            .oauth2
            .as_ref()
            .unwrap_or_else(|| panic!("`{name}` should declare an `[auth.oauth2]` grant"));

        assert_eq!(
            method.subject,
            connector_spec::Subject::User,
            "`{name}` is issued to the person who completed the grant and is bounded by their own \
             permissions, not the organization's — a token that acted as the organization would be \
             the API key, not this"
        );
        assert_eq!(
            spec.endpoint, "login",
            "`{name}` must resolve its paths against the declared `login` service, so the token \
             exchange stays inside the egress allow-list `http_hosts` derives from declared base URLs"
        );
        assert_eq!(spec.authorize_path, "/oauth/authorize");
        assert_eq!(spec.token_path, "/v1/oauth/token");
        assert!(
            spec.grants
                .contains(&connector_spec::OAuthGrant::AuthorizationCode)
                && spec
                    .grants
                    .contains(&connector_spec::OAuthGrant::RefreshToken),
            "`{name}` declares grants {:?}. This flow issues refresh tokens and an unattended \
             deployment outlives the access token, so both grants are required",
            spec.grants
        );
        assert!(
            spec.client_id.is_empty(),
            "`{name}` carries a client id value. A registration value is deployment configuration \
             and never a declaration field — the `oauth_client_id` config field binding \
             `oauth.client_id` is where a deployment supplies it"
        );
    }

    assert!(
        credential("anthropic.console_oauth")
            .oauth2
            .as_ref()
            .expect("a grant")
            .scopes
            .is_empty(),
        "the workspace-scoped credential requests no named scope. Anthropic documents the default \
         as workspace-scoped and names a scope only for the Admin API, so a scope word invented \
         here is one the authorize endpoint has never seen"
    );
    assert_eq!(
        credential("anthropic.console_oauth_admin")
            .oauth2
            .as_ref()
            .expect("a grant")
            .scopes,
        vec!["org:admin".to_string()],
        "the admin credential requests exactly the one scope Anthropic names for the Admin API — \
         no comfortable superset"
    );
}

/// **Every base URL the composition needs is a literal** — X-154's `NoDeclaredDefault` contract.
///
/// Exchange composes the authorize URL from the artifact alone. A `{placeholder}` in the `login`
/// service's base URL with no declared default would leave it with nothing to resolve, and a
/// templated auth host is the one a consumer cannot fill in.
#[test]
fn the_login_service_base_url_is_resolvable_without_configuration() {
    let connector = anthropic();

    let login = connector
        .services
        .iter()
        .find(|service| service.name == "login")
        .expect("the `login` service should be declared");
    let base_url = login
        .base_url
        .as_deref()
        .expect("the `login` service should declare its own base URL, not inherit the API host");

    assert_eq!(base_url, "https://platform.claude.com");
    assert!(
        !base_url.contains('{'),
        "the login base URL {base_url:?} is templated. X-154's consumer contract is that a base URL \
         the composition needs is non-templated or carries a declared default; Anthropic serves one \
         Console origin for everyone, so there is nothing here to parameterise"
    );
    assert!(
        login.base_url.as_deref() != Some(connector.base_url.as_str()),
        "the auth host and the API host are different hosts, which is the whole reason the `login` \
         service exists"
    );
}

/// **The subscription flow is the two-host acquisition, declared honestly across two services**
/// (C-555 round 2, expressible since C-556).
///
/// Anthropic's Claude Pro/Max sign-in authorizes on `claude.ai` and exchanges on
/// `platform.claude.com` — two origins one `endpoint` cannot express. C-556's `token_endpoint` is
/// what makes it declarable: `endpoint` names the authorize host and `token_endpoint` names the
/// token host, both resolved from declared services.
///
/// Round 1 this test asserted the flow was *absent* rather than half-declared, because the model
/// could not state it truthfully. It now asserts the opposite: that the flow is present and each leg
/// resolves against the host it actually uses. The invariant it defends is unchanged — no leg is
/// silently composed against the wrong origin — but it is now checked over a real declaration rather
/// than over its absence.
///
/// **The half-declaration guard still bites.** Every OAuth2 grant on the connector must state both
/// paths; the loader requires neither, so an omitted one would compose against whatever host the
/// remaining `endpoint`/`token_endpoint` names — and the failure would be a plausible URL for the
/// wrong flow rather than a 404. That check now covers all three credentials.
#[test]
fn the_subscription_flow_is_two_host_and_each_leg_resolves_against_its_own_service() {
    let connector = anthropic();

    let base_of = |service_name: &str| -> String {
        connector
            .services
            .iter()
            .find(|service| service.name == service_name)
            .unwrap_or_else(|| panic!("service `{service_name}` should be declared"))
            .base_url
            .clone()
            .unwrap_or_else(|| panic!("service `{service_name}` should declare its own base URL"))
    };

    let subscription = connector
        .auth
        .iter()
        .find(|method| method.name == "anthropic.subscription_oauth")
        .expect("the subscription credential should be declared");
    let spec = subscription
        .oauth2
        .as_ref()
        .expect("the subscription credential declares an `[auth.oauth2]` grant");

    // The authorize leg is on claude.ai; the token leg is on platform.claude.com. Reading the hosts
    // back through the declared services is what proves the two references point where they must.
    assert_eq!(
        base_of(&spec.endpoint),
        "https://claude.ai",
        "the subscription authorize leg (`endpoint` = {:?}) must resolve to claude.ai",
        spec.endpoint
    );
    assert!(
        !spec.token_endpoint.is_empty(),
        "the subscription flow is two-host, so `token_endpoint` must name a second service — an \
         empty one would silently redeem the token against the authorize host claude.ai, which is \
         not where the exchange lives"
    );
    assert_eq!(
        base_of(&spec.token_endpoint),
        "https://platform.claude.com",
        "the subscription token leg (`token_endpoint` = {:?}) must resolve to platform.claude.com — \
         the console.anthropic.com host it used to live on now 404s",
        spec.token_endpoint
    );
    assert_ne!(
        spec.endpoint, spec.token_endpoint,
        "the whole point of a two-host declaration is that the authorize and token services differ; \
         if they were equal this would be a single-host flow wearing `token_endpoint` for show"
    );
    assert!(
        spec.public_client,
        "the subscription flow is a public PKCE client — it ships a shared public client id and no \
         secret, and a confidential marking here would demand an operator client secret that does \
         not exist"
    );
    assert!(
        spec.client_id.is_empty(),
        "the shared client id is a fact for a comment, never a declaration value"
    );

    // The half-declaration guard, now over all three OAuth2 credentials.
    for method in &connector.auth {
        let Some(spec) = &method.oauth2 else {
            continue;
        };
        assert!(
            !spec.authorize_path.is_empty() && !spec.token_path.is_empty(),
            "credential `{}` declares an OAuth2 grant with an empty leg (authorize {:?}, token \
             {:?}). The loader does not require either path, so an omitted one is silently composed \
             against whatever origin `endpoint`/`token_endpoint` names — state both legs or declare \
             no grant",
            method.name,
            spec.authorize_path,
            spec.token_path
        );
    }
}
