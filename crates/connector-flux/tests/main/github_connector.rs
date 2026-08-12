//! `providers/github.toml` exists, emits analyzable Flux, and lets no caller-supplied query value
//! change the structure of a request (C-52, C-469, C-30, C-527).
//!
//! **The premise of this file changed, and the assertions changed with it.** It used to open with
//! *"nothing in this pipeline percent-encodes a query value: the emitter interpolates it verbatim"*,
//! and enforced the only rule that was safe under it — every query parameter is an integer, on four
//! frozen operation ids. C-30 landed Flux 0.54's structured `http.request(query: …)` map, and that
//! sentence stopped being true: a scalar value now travels as a record field encoded with RFC 3986
//! semantics, and the URL carries path data only. Verified rather than assumed — see
//! [`no_github_query_value_reaches_the_url`], which asserts it on the emitted text.
//!
//! So "integers only" is retired, because it was a **proxy** for the property that actually matters
//! and that proxy now excludes safe parameters while proving nothing extra. The two rules below are
//! what it was standing in for, and both are strictly stronger than what it checked:
//!
//! 1. **Every query parameter is a scalar.** An array or object has no declared wire shape, C-30
//!    refuses it with `UnencodableQueryValue`, and this asserts the connector never declares one.
//! 2. **No query value reaches the URL.** This is the injection vector itself, and it is now checked
//!    on every operation rather than on four exempted ids.
//!
//! This file names only GitHub; it never walks the catalogue, so another provider cannot change the
//! premise of a GitHub-specific assertion.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::Connector;

use crate::shipped_provider;

const PAGINATED_READS: [&str; 4] = [
    "github-issue-list",
    "github-pull-files-list",
    "github-workflow-run-list",
    "github-commit-list",
];

/// The delegated credential (C-554): obtained by an OAuth2 grant the host runs on behalf of a
/// signed-in user, beside the static token this connector has always accepted.
const OAUTH_TOKEN: &str = "github.oauth_token";

/// The auth-host service. GitHub's browser leg and token exchange are on `github.com`, which is not
/// the `api.github.com` every operation goes to, so the endpoint needs a service of its own.
const LOGIN_SERVICE: &str = "login";

/// The minimum the thirteen declared operations need, each derived from GitHub's own per-endpoint
/// scope statement and reasoned in `providers/github.toml`. Notably **not** `read:user`: no endpoint
/// page names it, and `GET /user` needs no scope for the public profile this connector declares.
const EXPECTED_SCOPES: [&str; 2] = ["repo", "read:org"];

/// `<repo root>/providers/github.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("github.toml")
}

fn github() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-52 ships the GitHub connector",
            path.display()
        )
    });
    shipped_provider::load_definition("github", &source)
        .expect("providers/github.toml does not load")
        .connector
}

/// The connector exists, loads through the real loader, and is the one C-52 describes.
#[test]
fn the_github_connector_loads() {
    let connector = github();

    assert_eq!(connector.id, "github");
    assert_eq!(connector.vendor, "GitHub");
    assert_eq!(
        connector.base_url, "https://api.github.com",
        "the host is `api.github.com` and is never widened"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op — the C-11 gate, held against github specifically.
#[test]
fn every_github_operation_emits_an_analyzable_module() {
    let connector = github();
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

/// **The honesty assertion.** Every declared query parameter is a scalar.
///
/// A scalar is what C-30's structured `query` map can encode; an array or object has no declared
/// wire shape, and rather than guess a vendor's convention the emitter refuses one with
/// `UnencodableQueryValue`. Asserting it here means a widening that would only fail at emission
/// time fails at the connector's own contract instead, naming the parameter.
///
/// The four pre-C-30 reads are still checked more tightly than the rest — they are published bytes,
/// and `PAGINATED_READS` is what keeps a "while I'm here" widening of one of them visible.
#[test]
fn github_query_parameters_are_scalars() {
    const SCALARS: [&str; 4] = ["string", "integer", "number", "boolean"];
    let connector = github();
    for operation in &connector.operations {
        let names: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| {
                let declared = param.schema["type"].as_str().unwrap_or_else(|| {
                    panic!(
                        "`{}.{}` declares no query `type`, so nothing can say it is encodable",
                        operation.id, param.name
                    )
                });
                assert!(
                    SCALARS.contains(&declared),
                    "`{}.{}` is a {declared}, which has no declared query wire shape (C-30)",
                    operation.id,
                    param.name
                );
                param.name.as_str()
            })
            .collect();
        if PAGINATED_READS.contains(&operation.id.as_str()) {
            assert_eq!(names, ["per_page", "page"], "{} widened", operation.id);
        }
    }
}

/// **The injection assertion, and the one that replaced "integers only".**
///
/// No query value reaches the URL on any operation — the emitted `url` binding carries path data
/// and nothing else, so a caller's value cannot introduce a `?`, a `&` or a second parameter no
/// matter what it contains. Every declared query parameter appears instead in the structured
/// `query: { … }` record, which is where C-30's RFC 3986 encoding is applied.
///
/// This is checked on **every** GitHub operation rather than on four exempted ids, which is what
/// makes it stronger than the rule it replaced.
#[test]
fn no_github_query_value_reaches_the_url() {
    let connector = github();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).expect("a shipped operation emits");

        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        assert_eq!(
            url_lines.len(),
            1,
            "`{}` must bind exactly one $url:\n{emitted}",
            operation.id
        );
        assert!(
            !url_lines[0].contains('?') && !url_lines[0].contains('&'),
            "`{}` puts query data in the URL, which is the injection vector C-30 closed:\n{emitted}",
            operation.id
        );

        if operation.params.query.is_empty() {
            assert!(
                !emitted.contains("query: {"),
                "`{}` emits a query record it declares no parameters for:\n{emitted}",
                operation.id
            );
            continue;
        }

        // The emitter sorts the record's fields, so the expectation is built sorted too rather than
        // in declaration order — a mismatch here would otherwise read as a missing parameter.
        let mut declared: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        declared.sort_unstable();
        let expected = format!("query: {{ {} }}", declared.join(", "));
        assert!(
            emitted.contains(&expected),
            "`{}` does not carry every declared parameter structurally; expected `{expected}`:\n{emitted}",
            operation.id
        );
    }
}

/// **The OAuth2 acquisition is declared, and a host can compose the authorize URL from the artifact
/// alone** (C-554).
///
/// GitHub's auth host is not its API host — `github.com` serves the browser leg and the token
/// exchange while `api.github.com` serves every operation — so the declaration needs a second
/// service to hang the endpoint on, exactly as `providers/gitlab.toml` does. The three things a
/// consumer composes from are asserted here rather than left to the provider file's prose: the
/// endpoint names a *declared* service, that service's base URL is a literal, and the two paths are
/// the vendor's own.
///
/// **`grants` is `authorization_code` alone, and the absence of `refresh_token` is the assertion.**
/// A grant this connector's app model does not honour would be a declaration a host acts on and the
/// vendor refuses. See the provider file for the documented reasoning; the short form is that scopes
/// and refresh tokens belong to two *different* GitHub app models, and this connector declares
/// scopes.
#[test]
fn the_oauth2_acquisition_is_declared_and_composable() {
    let connector = github();

    let method = connector
        .auth_method(OAUTH_TOKEN)
        .unwrap_or_else(|| panic!("github declares the `{OAUTH_TOKEN}` credential"));
    let spec = method
        .oauth2
        .as_ref()
        .unwrap_or_else(|| panic!("`{OAUTH_TOKEN}` declares an `[auth.oauth2]` acquisition"));

    // The endpoint is a declared service, never a URL: `http_hosts` derives from declared base URLs,
    // so a URL written here would name a host nothing admitted.
    assert_eq!(
        spec.endpoint, LOGIN_SERVICE,
        "the grant resolves against the declared auth-host service"
    );
    let login = connector
        .service(LOGIN_SERVICE)
        .unwrap_or_else(|| panic!("github declares the `{LOGIN_SERVICE}` service"));
    let base_url = login
        .base_url
        .as_deref()
        .expect("the auth-host service owns its own base URL — it is not `api.github.com`");
    assert_eq!(
        base_url, "https://github.com",
        "GitHub's browser leg and token exchange are served by `github.com`, not by the API host"
    );

    // **The X-154 consumer contract** (`NoDeclaredDefault`): a startup composing the authorize URL
    // has only the artifact, so a `{placeholder}` with nothing to resolve it from is a composition
    // that cannot complete. GitHub.com is one fixed host, so the honest answer is a literal.
    assert!(
        !base_url.contains('{'),
        "`{LOGIN_SERVICE}`'s base URL is templated ({base_url:?}) with no declared default, so a \
         consumer composing the authorize URL at startup has nothing to resolve it from"
    );

    assert_eq!(
        spec.authorize_path, "/login/oauth/authorize",
        "the browser leg's path, as GitHub documents it"
    );
    assert_eq!(
        spec.token_path, "/login/oauth/access_token",
        "the token exchange's path, as GitHub documents it"
    );

    // **No registration value.** `client_id` is per-deployment and the canonical document has no
    // field for one, so a value here is refused at the document lowering (C-536). The requirement is
    // published as configuration instead, asserted below.
    assert!(
        spec.client_id.is_empty(),
        "a registration value is per-deployment; publish the requirement through `[[config]]`"
    );

    assert_eq!(
        spec.scopes, EXPECTED_SCOPES,
        "the scope set is the minimum the declared operations need, and widening it is a decision"
    );

    // The grant list is the honest one for the app model that has scopes at all. `refresh_token`
    // being absent is the point of the assertion, not an omission.
    assert_eq!(
        spec.grants,
        vec![connector_spec::OAuthGrant::AuthorizationCode],
        "a classic OAuth app's token does not refresh; declaring the grant would be a promise the \
         vendor will not honour"
    );

    // The registration triple an `authorization_code` grant owes an operator, at operator level and
    // with only the secret marked secret.
    for (binds, secret) in [
        ("oauth.client_id", false),
        ("oauth.client_secret", true),
        ("oauth.redirect_uri", false),
    ] {
        let field = connector
            .config
            .iter()
            .find(|field| field.binds == binds)
            .unwrap_or_else(|| panic!("github publishes a `[[config]]` field binding `{binds}`"));
        assert_eq!(
            field.level(),
            Some(connector_spec::Level::Operator),
            "`{binds}` is set once per deployment, never by an end user"
        );
        assert_eq!(
            field.secret, secret,
            "`{binds}` disagrees with flux's secret partition"
        );
    }

    // The delegated credential is the user's, not the application's — a host that stored it at a
    // tenant-wide address would let any member act as whoever completed the grant (C-528).
    assert_eq!(
        method.subject,
        connector_spec::Subject::User,
        "an OAuth2 access token obtained on behalf of a signed-in user carries that user's authority"
    );

    // Both credentials authenticate everything, as alternatives rather than as a pair.
    let alternatives: Vec<Vec<&str>> = connector
        .default_auth
        .iter()
        .map(|requirement| requirement.iter().map(String::as_str).collect())
        .collect();
    assert_eq!(
        alternatives,
        vec![vec![OAUTH_TOKEN], vec!["github.token"]],
        "the delegated grant and the static token are two deployments of one connector"
    );
}
